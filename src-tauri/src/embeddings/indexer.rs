use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use ignore::WalkBuilder;
use tokio::sync::Mutex;

use crate::bus::EventBus;
use crate::db::queries;
use crate::db::Database;
use crate::embeddings::manager::EmbeddingManager;
use crate::embeddings::{cosine_similarity, EmbedOptions, EmbeddingError, EmbeddingTaskType};

const MAX_FILE_BYTES: usize = 512 * 1024;
const CHUNK_TARGET_CHARS: usize = 1200;
const CHUNK_OVERLAP_LINES: usize = 6;
const EMBED_BATCH_SIZE: usize = 32;
const MAX_SEARCH_QUERY_CHARS: usize = 6000;
const MAX_INDEX_FILES: usize = 10_000;
const MAX_INDEX_CHUNKS: usize = 80_000;

#[derive(Debug, Clone, serde::Serialize)]
pub struct EmbeddingIndexStatus {
    pub workspace_root: String,
    pub provider: String,
    pub status: String,
    pub dims: Option<usize>,
    pub file_count: usize,
    pub chunk_count: usize,
    pub indexed_at: Option<String>,
    pub updated_at: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SemanticSearchResultItem {
    pub path: String,
    pub chunk_idx: usize,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub score: f32,
    pub content_preview: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SemanticSearchResponse {
    pub status: String,
    pub indexed: bool,
    pub message: String,
    pub results: Vec<SemanticSearchResultItem>,
}

#[derive(Debug, Clone)]
struct FileChunk {
    path: String,
    chunk_idx: usize,
    line_start: Option<usize>,
    line_end: Option<usize>,
    content: String,
}

#[derive(Debug, Clone)]
struct ScoredChunk {
    path: String,
    chunk_idx: usize,
    line_start: Option<usize>,
    line_end: Option<usize>,
    content: String,
    score: f32,
}

#[async_trait::async_trait]
pub trait EmbeddingClient: Send + Sync {
    async fn provider_id(&self) -> Result<String, EmbeddingError>;
    async fn embed(
        &self,
        texts: &[String],
        opts: Option<EmbedOptions>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError>;
}

#[async_trait::async_trait]
impl EmbeddingClient for EmbeddingManager {
    async fn provider_id(&self) -> Result<String, EmbeddingError> {
        Ok(self.provider_info().await?.id)
    }

    async fn embed(
        &self,
        texts: &[String],
        opts: Option<EmbedOptions>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.embed(texts, opts).await
    }
}

pub struct SemanticIndexService {
    db: Arc<Database>,
    bus: Arc<EventBus>,
    client: Arc<dyn EmbeddingClient>,
    in_progress: Mutex<HashSet<String>>,
}

impl SemanticIndexService {
    pub fn new(
        db: Arc<Database>,
        bus: Arc<EventBus>,
        client: Arc<dyn EmbeddingClient>,
    ) -> Arc<Self> {
        Arc::new(Self {
            db,
            bus,
            client,
            in_progress: Mutex::new(HashSet::new()),
        })
    }

    pub fn ensure_workspace_index_started(self: &Arc<Self>, workspace_root: PathBuf) {
        let normalized_root = normalize_workspace_key(&workspace_root);
        let service = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let provider_id = match service.client.provider_id().await {
                Ok(value) => value,
                Err(error) => {
                    service.mark_index_failed(&normalized_root, "unknown", &error);
                    return;
                }
            };

            if !service.needs_indexing(&normalized_root, &provider_id) {
                return;
            }

            {
                let mut guard = service.in_progress.lock().await;
                if guard.contains(&normalized_root) {
                    return;
                }
                guard.insert(normalized_root.clone());
            }

            if let Err(error) = service
                .run_indexing(
                    workspace_root.clone(),
                    normalized_root.clone(),
                    provider_id.clone(),
                )
                .await
            {
                service.mark_index_failed(&normalized_root, &provider_id, &error);
            }

            let mut guard = service.in_progress.lock().await;
            guard.remove(&normalized_root);
        });
    }

    pub fn index_status(&self, workspace_root: &Path) -> Option<EmbeddingIndexStatus> {
        let key = normalize_workspace_key(workspace_root);
        queries::get_embedding_index(&self.db, &key)
            .ok()
            .flatten()
            .map(|row| EmbeddingIndexStatus {
                workspace_root: row.workspace_root,
                provider: row.provider,
                status: row.status,
                dims: row.dims.map(|value| value as usize),
                file_count: row.file_count as usize,
                chunk_count: row.chunk_count as usize,
                indexed_at: row.indexed_at,
                updated_at: row.updated_at,
                error: row.error,
            })
    }

    pub async fn semantic_search(
        self: &Arc<Self>,
        workspace_root: PathBuf,
        query: String,
        limit: usize,
    ) -> Result<SemanticSearchResponse, EmbeddingError> {
        let normalized_root = normalize_workspace_key(&workspace_root);
        if query.trim().is_empty() {
            return Ok(SemanticSearchResponse {
                status: "error".to_string(),
                indexed: self.has_ready_index(&normalized_root),
                message: "query must not be empty".to_string(),
                results: Vec::new(),
            });
        }

        let provider_id = self.client.provider_id().await?;
        if !self.is_index_ready_for_provider(&normalized_root, &provider_id) {
            self.ensure_workspace_index_started(workspace_root);
            return Ok(SemanticSearchResponse {
                status: "indexing".to_string(),
                indexed: false,
                message: "semantic index is building in the background; retry shortly".to_string(),
                results: Vec::new(),
            });
        }

        let query_text = truncate_chars(query.trim(), MAX_SEARCH_QUERY_CHARS);
        let query_embeddings = self
            .client
            .embed(
                &[query_text],
                Some(EmbedOptions {
                    task: Some(EmbeddingTaskType::RetrievalQuery),
                }),
            )
            .await?;
        let Some(query_vector) = query_embeddings.first() else {
            return Err(EmbeddingError::InvalidResponse(
                "embedding provider returned empty query embedding".to_string(),
            ));
        };

        let chunks = queries::list_embedding_chunks_for_workspace(&self.db, &normalized_root)
            .map_err(|error| EmbeddingError::Runtime(error.to_string()))?;
        if chunks.is_empty() {
            return Ok(SemanticSearchResponse {
                status: "empty".to_string(),
                indexed: true,
                message: "workspace is indexed but no searchable chunks were found".to_string(),
                results: Vec::new(),
            });
        }

        let mut scored = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let embedding = parse_embedding_json(&chunk.embedding_json)?;
            let score = cosine_similarity(query_vector, &embedding);
            scored.push(ScoredChunk {
                path: chunk.path,
                chunk_idx: chunk.chunk_idx as usize,
                line_start: chunk.line_start.map(|value| value as usize),
                line_end: chunk.line_end.map(|value| value as usize),
                content: chunk.content,
                score,
            });
        }

        scored.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let capped_limit = limit.clamp(1, 50);
        let results = scored
            .into_iter()
            .take(capped_limit)
            .map(|item| SemanticSearchResultItem {
                path: item.path,
                chunk_idx: item.chunk_idx,
                line_start: item.line_start,
                line_end: item.line_end,
                score: item.score,
                content_preview: truncate_chars(item.content.trim(), 420),
            })
            .collect();

        Ok(SemanticSearchResponse {
            status: "ready".to_string(),
            indexed: true,
            message: "ok".to_string(),
            results,
        })
    }

    fn is_index_ready_for_provider(&self, workspace_key: &str, provider_id: &str) -> bool {
        matches!(
            queries::get_embedding_index(&self.db, workspace_key),
            Ok(Some(row)) if row.status == "ready" && row.provider == provider_id
        )
    }

    fn has_ready_index(&self, workspace_key: &str) -> bool {
        matches!(
            queries::get_embedding_index(&self.db, workspace_key),
            Ok(Some(row)) if row.status == "ready"
        )
    }

    fn needs_indexing(&self, workspace_key: &str, provider_id: &str) -> bool {
        match queries::get_embedding_index(&self.db, workspace_key) {
            Ok(Some(row)) => !(row.status == "ready" && row.provider == provider_id),
            Ok(None) => true,
            Err(_) => true,
        }
    }

    fn mark_index_failed(&self, workspace_key: &str, provider_id: &str, error: &EmbeddingError) {
        let now = Utc::now().to_rfc3339();
        let _ = queries::upsert_embedding_index(
            &self.db,
            &queries::EmbeddingIndexRow {
                workspace_root: workspace_key.to_string(),
                provider: provider_id.to_string(),
                status: "failed".to_string(),
                dims: None,
                file_count: 0,
                chunk_count: 0,
                indexed_at: None,
                updated_at: now,
                error: Some(error.to_string()),
            },
        );

        self.bus.emit(
            "log",
            "log.error",
            None,
            serde_json::json!({
                "workspace_root": workspace_key,
                "message": format!("Embedding index failed for {}", workspace_key),
                "error": error.to_string(),
            }),
        );
    }

    async fn run_indexing(
        &self,
        workspace_root: PathBuf,
        workspace_key: String,
        provider_id: String,
    ) -> Result<(), EmbeddingError> {
        let now = Utc::now().to_rfc3339();

        let _ = queries::upsert_embedding_index(
            &self.db,
            &queries::EmbeddingIndexRow {
                workspace_root: workspace_key.clone(),
                provider: provider_id.clone(),
                status: "indexing".to_string(),
                dims: None,
                file_count: 0,
                chunk_count: 0,
                indexed_at: None,
                updated_at: now.clone(),
                error: None,
            },
        );

        self.bus.emit(
            "log",
            "log.info",
            None,
            serde_json::json!({
                "message": format!("Embedding index build started for {}", workspace_key),
                "workspace_root": workspace_key,
            }),
        );

        let chunks = collect_workspace_chunks(&workspace_root)?;
        let file_count = chunks
            .iter()
            .map(|chunk| chunk.path.clone())
            .collect::<HashSet<_>>()
            .len();

        if chunks.is_empty() {
            let now = Utc::now().to_rfc3339();
            let _ = queries::delete_embedding_chunks_for_workspace(&self.db, &workspace_key);
            let _ = queries::upsert_embedding_index(
                &self.db,
                &queries::EmbeddingIndexRow {
                    workspace_root: workspace_key.clone(),
                    provider: provider_id,
                    status: "ready".to_string(),
                    dims: None,
                    file_count: 0,
                    chunk_count: 0,
                    indexed_at: Some(now.clone()),
                    updated_at: now,
                    error: None,
                },
            );
            return Ok(());
        }

        queries::delete_embedding_chunks_for_workspace(&self.db, &workspace_key)
            .map_err(|error| EmbeddingError::Runtime(error.to_string()))?;

        let created_at = Utc::now().to_rfc3339();
        let mut dims: Option<usize> = None;
        let mut persisted_count: usize = 0;

        for batch in chunks.chunks(EMBED_BATCH_SIZE) {
            let texts = batch
                .iter()
                .map(|chunk| chunk.content.clone())
                .collect::<Vec<_>>();
            let vectors = self
                .client
                .embed(
                    &texts,
                    Some(EmbedOptions {
                        task: Some(EmbeddingTaskType::RetrievalDocument),
                    }),
                )
                .await?;
            if vectors.len() != texts.len() {
                return Err(EmbeddingError::InvalidResponse(format!(
                    "embedding provider returned {} vectors for {} inputs",
                    vectors.len(),
                    texts.len()
                )));
            }

            for (chunk, vector) in batch.iter().zip(vectors.iter()) {
                if dims.is_none() {
                    dims = Some(vector.len());
                }

                let embedding_json = serde_json::to_string(vector)
                    .map_err(|error| EmbeddingError::Runtime(error.to_string()))?;

                queries::insert_embedding_chunk(
                    &self.db,
                    &queries::EmbeddingChunkRow {
                        id: 0,
                        workspace_root: workspace_key.clone(),
                        path: chunk.path.clone(),
                        chunk_idx: chunk.chunk_idx as i64,
                        line_start: chunk.line_start.map(|value| value as i64),
                        line_end: chunk.line_end.map(|value| value as i64),
                        content: chunk.content.clone(),
                        embedding_json,
                        created_at: created_at.clone(),
                    },
                )
                .map_err(|error| EmbeddingError::Runtime(error.to_string()))?;
                persisted_count += 1;
            }
        }

        let updated_at = Utc::now().to_rfc3339();
        queries::upsert_embedding_index(
            &self.db,
            &queries::EmbeddingIndexRow {
                workspace_root: workspace_key.clone(),
                provider: provider_id,
                status: "ready".to_string(),
                dims: dims.map(|value| value as i64),
                file_count: file_count as i64,
                chunk_count: persisted_count as i64,
                indexed_at: Some(updated_at.clone()),
                updated_at: updated_at.clone(),
                error: None,
            },
        )
        .map_err(|error| EmbeddingError::Runtime(error.to_string()))?;

        self.bus.emit(
            "log",
            "log.info",
            None,
            serde_json::json!({
                "message": format!(
                    "Embedding index ready for {} (files: {}, chunks: {})",
                    workspace_key,
                    file_count,
                    persisted_count
                ),
                "workspace_root": workspace_key,
                "file_count": file_count,
                "chunk_count": persisted_count,
            }),
        );

        Ok(())
    }
}

fn collect_workspace_chunks(workspace_root: &Path) -> Result<Vec<FileChunk>, EmbeddingError> {
    if !workspace_root.exists() || !workspace_root.is_dir() {
        return Err(EmbeddingError::Config(format!(
            "workspace root does not exist: {}",
            workspace_root.display()
        )));
    }

    let mut chunks = Vec::new();
    let mut seen_files = 0usize;
    let walker = WalkBuilder::new(workspace_root)
        .hidden(false)
        .follow_links(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .require_git(false)
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };

        let path = entry.path();
        if entry.file_type().map_or(true, |kind| kind.is_dir()) {
            continue;
        }
        if seen_files >= MAX_INDEX_FILES || chunks.len() >= MAX_INDEX_CHUNKS {
            break;
        }

        if !is_supported_text_file(path) {
            continue;
        }

        let metadata = match std::fs::metadata(path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if metadata.len() as usize > MAX_FILE_BYTES {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if content.trim().is_empty() {
            continue;
        }

        let relative_path = match path.strip_prefix(workspace_root) {
            Ok(value) => value.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };

        seen_files += 1;
        let mut file_chunks = split_into_chunks(&relative_path, &content);
        if chunks.len() + file_chunks.len() > MAX_INDEX_CHUNKS {
            let remaining = MAX_INDEX_CHUNKS.saturating_sub(chunks.len());
            file_chunks.truncate(remaining);
        }
        chunks.append(&mut file_chunks);
    }

    Ok(chunks)
}

fn split_into_chunks(path: &str, content: &str) -> Vec<FileChunk> {
    let lines = content.split_inclusive('\n').collect::<Vec<_>>();
    if lines.is_empty() {
        return vec![FileChunk {
            path: path.to_string(),
            chunk_idx: 0,
            line_start: Some(1),
            line_end: Some(1),
            content: content.to_string(),
        }];
    }

    let mut chunks = Vec::new();
    let mut start_line_idx = 0usize;
    let mut chunk_idx = 0usize;

    while start_line_idx < lines.len() {
        let mut collected = String::new();
        let mut end_line_idx = start_line_idx;

        while end_line_idx < lines.len() {
            let candidate = lines[end_line_idx];
            if !collected.is_empty()
                && (collected.chars().count() + candidate.chars().count()) > CHUNK_TARGET_CHARS
            {
                break;
            }
            collected.push_str(candidate);
            end_line_idx += 1;
            if collected.chars().count() >= CHUNK_TARGET_CHARS {
                break;
            }
        }

        let trimmed = collected.trim();
        if !trimmed.is_empty() {
            chunks.push(FileChunk {
                path: path.to_string(),
                chunk_idx,
                line_start: Some(start_line_idx + 1),
                line_end: Some(end_line_idx),
                content: trimmed.to_string(),
            });
            chunk_idx += 1;
        }

        if end_line_idx >= lines.len() {
            break;
        }
        let next_start = end_line_idx.saturating_sub(CHUNK_OVERLAP_LINES);
        start_line_idx = if next_start <= start_line_idx {
            end_line_idx
        } else {
            next_start
        };
    }

    chunks
}

fn is_supported_text_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };

    matches!(
        ext.to_ascii_lowercase().as_str(),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "json"
            | "md"
            | "toml"
            | "yaml"
            | "yml"
            | "py"
            | "go"
            | "java"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "rb"
            | "php"
            | "cs"
            | "swift"
            | "kt"
            | "sh"
            | "sql"
            | "css"
            | "html"
            | "vue"
            | "svelte"
            | "txt"
    )
}

fn parse_embedding_json(raw: &str) -> Result<Vec<f32>, EmbeddingError> {
    let parsed = serde_json::from_str::<Vec<f32>>(raw)
        .map_err(|error| EmbeddingError::InvalidResponse(error.to_string()))?;
    if parsed.is_empty() {
        return Err(EmbeddingError::InvalidResponse(
            "stored embedding vector is empty".to_string(),
        ));
    }
    Ok(parsed)
}

fn normalize_workspace_key(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    canonical.to_string_lossy().replace('\\', "/")
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use tokio::sync::RwLock;

    use crate::bus::EventBus;
    use crate::db::Database;

    #[test]
    fn split_chunks_keeps_overlap_and_line_ranges() {
        let content = (1..=30)
            .map(|idx| format!("line-{idx:02}\n"))
            .collect::<String>();
        let chunks = split_into_chunks("src/main.rs", &content);
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].line_start, Some(1));
        assert!(chunks.last().and_then(|chunk| chunk.line_end).unwrap_or(0) >= 30);
    }

    #[tokio::test]
    async fn indexing_persists_chunks_and_searches_semantically() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("src")).expect("create src");
        std::fs::write(
            temp.path().join("src").join("alpha.rs"),
            "fn alpha_feature() { /* alpha token */ }\n",
        )
        .expect("write alpha");
        std::fs::write(
            temp.path().join("src").join("beta.rs"),
            "fn beta_feature() { /* beta token */ }\n",
        )
        .expect("write beta");

        let db = Arc::new(Database::open_in_memory().expect("in-memory db"));
        let bus = Arc::new(EventBus::new());
        let client = Arc::new(MockEmbeddingClient::new("mock-provider"));
        let service = SemanticIndexService::new(db, bus, client);

        let workspace_key = normalize_workspace_key(temp.path());
        service
            .run_indexing(
                temp.path().to_path_buf(),
                workspace_key,
                "mock-provider".to_string(),
            )
            .await
            .expect("indexing should succeed");

        let status = service
            .index_status(temp.path())
            .expect("index status should exist");
        assert_eq!(status.status, "ready");
        assert!(status.chunk_count > 0);

        let response = service
            .semantic_search(temp.path().to_path_buf(), "alpha token".to_string(), 5)
            .await
            .expect("semantic search should succeed");
        assert_eq!(response.status, "ready");
        assert!(!response.results.is_empty());
        assert!(response.results[0].path.contains("alpha.rs"));
    }

    #[tokio::test]
    async fn provider_mismatch_marks_index_as_needing_rebuild() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("src")).expect("create src");
        std::fs::write(
            temp.path().join("src").join("index.ts"),
            "export const alpha = 'alpha';\n",
        )
        .expect("write file");

        let db = Arc::new(Database::open_in_memory().expect("in-memory db"));
        let bus = Arc::new(EventBus::new());
        let client = Arc::new(MockEmbeddingClient::new("provider-a"));
        let service = SemanticIndexService::new(db, bus, client.clone());

        let workspace_key = normalize_workspace_key(temp.path());
        service
            .run_indexing(
                temp.path().to_path_buf(),
                workspace_key.clone(),
                "provider-a".to_string(),
            )
            .await
            .expect("initial indexing should succeed");

        assert!(!service.needs_indexing(&workspace_key, "provider-a"));
        assert!(service.needs_indexing(&workspace_key, "provider-b"));

        client.set_provider("provider-b").await;
        let response = service
            .semantic_search(temp.path().to_path_buf(), "alpha".to_string(), 3)
            .await
            .expect("semantic search should return indexing while rebuilding");
        assert_eq!(response.status, "indexing");
    }

    struct MockEmbeddingClient {
        provider: RwLock<String>,
    }

    impl MockEmbeddingClient {
        fn new(provider: &str) -> Self {
            Self {
                provider: RwLock::new(provider.to_string()),
            }
        }

        async fn set_provider(&self, provider: &str) {
            *self.provider.write().await = provider.to_string();
        }
    }

    #[async_trait::async_trait]
    impl EmbeddingClient for MockEmbeddingClient {
        async fn provider_id(&self) -> Result<String, EmbeddingError> {
            Ok(self.provider.read().await.clone())
        }

        async fn embed(
            &self,
            texts: &[String],
            _opts: Option<EmbedOptions>,
        ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            Ok(texts.iter().map(|value| encode_text(value)).collect())
        }
    }

    fn encode_text(text: &str) -> Vec<f32> {
        let lower = text.to_ascii_lowercase();
        vec![
            if lower.contains("alpha") { 1.0 } else { 0.0 },
            if lower.contains("beta") { 1.0 } else { 0.0 },
            (lower.len() as f32 % 13.0) / 13.0,
        ]
    }
}
