use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fastembed::{
    EmbeddingModel, InitOptionsUserDefined, TextEmbedding, TextInitOptions, TokenizerFiles,
    UserDefinedEmbeddingModel,
};
use tokio::sync::RwLock;
use tokio::time::timeout;

use crate::embeddings::config::{RustHfEmbeddingConfig, RustHfRuntime};
use crate::embeddings::error::EmbeddingError;
use crate::embeddings::types::{
    finalize_embeddings, EmbedOptions, EmbeddingProvider, EmbeddingProviderKind,
};

#[async_trait::async_trait]
pub trait RustHfEngine: Send + Sync {
    async fn dims(&self) -> Result<Option<usize>, EmbeddingError>;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
}

pub struct RustLocalEmbeddingProvider {
    engine: Arc<dyn RustHfEngine>,
    normalize_l2: bool,
    cached_dims: RwLock<Option<usize>>,
}

impl RustLocalEmbeddingProvider {
    pub fn new(config: RustHfEmbeddingConfig, normalize_l2: bool) -> Result<Self, EmbeddingError> {
        let engine = Arc::new(FastEmbedRustHfEngine::new(config)?);
        Ok(Self {
            engine,
            normalize_l2,
            cached_dims: RwLock::new(None),
        })
    }

    pub fn new_with_engine(engine: Arc<dyn RustHfEngine>, normalize_l2: bool) -> Self {
        Self {
            engine,
            normalize_l2,
            cached_dims: RwLock::new(None),
        }
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for RustLocalEmbeddingProvider {
    fn id(&self) -> &str {
        "rust-hf"
    }

    fn kind(&self) -> EmbeddingProviderKind {
        EmbeddingProviderKind::Local
    }

    async fn dims(&self) -> Result<Option<usize>, EmbeddingError> {
        if let Some(cached) = *self.cached_dims.read().await {
            return Ok(Some(cached));
        }

        let dims = self.engine.dims().await?;
        if let Some(dim) = dims {
            *self.cached_dims.write().await = Some(dim);
        }
        Ok(dims)
    }

    async fn embed(
        &self,
        texts: &[String],
        _opts: Option<EmbedOptions>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let vectors = self.engine.embed(texts).await?;
        let finalized = finalize_embeddings(vectors, self.normalize_l2)?;
        if let Some(dim) = finalized.first().map(|vector| vector.len()) {
            *self.cached_dims.write().await = Some(dim);
        }
        Ok(finalized)
    }
}

struct FastEmbedState {
    model: Option<TextEmbedding>,
    dims: Option<usize>,
}

pub struct FastEmbedRustHfEngine {
    model_id: String,
    model_path: Option<String>,
    cache_dir: Option<String>,
    runtime: RustHfRuntime,
    threads: Option<usize>,
    timeout: Duration,
    state: Arc<Mutex<FastEmbedState>>,
}

impl FastEmbedRustHfEngine {
    pub fn new(config: RustHfEmbeddingConfig) -> Result<Self, EmbeddingError> {
        if config.timeout_ms == 0 {
            return Err(EmbeddingError::Config(
                "rust-hf timeout must be greater than 0".to_string(),
            ));
        }

        if config.runtime == RustHfRuntime::Candle {
            return Err(EmbeddingError::Config(
                "rust-hf runtime 'candle' is not available in this build; use runtime='onnx'"
                    .to_string(),
            ));
        }

        if config.model_id.trim().is_empty() && config.model_path.is_none() {
            return Err(EmbeddingError::Config(
                "rust-hf requires modelId or modelPath".to_string(),
            ));
        }

        if let Some(threads) = config.threads {
            if threads == 0 {
                return Err(EmbeddingError::Config(
                    "rust-hf threads must be greater than 0".to_string(),
                ));
            }
        }

        if let Some(model_path) = config.model_path.as_deref() {
            if !Path::new(model_path).exists() {
                return Err(EmbeddingError::Config(format!(
                    "rust-hf modelPath does not exist: {model_path}"
                )));
            }
        }

        Ok(Self {
            model_id: config.model_id,
            model_path: config.model_path,
            cache_dir: config.cache_dir,
            runtime: config.runtime,
            threads: config.threads,
            timeout: Duration::from_millis(config.timeout_ms),
            state: Arc::new(Mutex::new(FastEmbedState {
                model: None,
                dims: None,
            })),
        })
    }

    async fn ensure_model_loaded(&self) -> Result<(), EmbeddingError> {
        {
            let state = self
                .state
                .lock()
                .map_err(|_| EmbeddingError::Runtime("rust-hf state mutex poisoned".to_string()))?;
            if state.model.is_some() {
                return Ok(());
            }
        }

        let state = self.state.clone();
        let model_id = self.model_id.clone();
        let model_path = self.model_path.clone();
        let cache_dir = self.cache_dir.clone();
        let runtime = self.runtime;
        let threads = self.threads;

        let handle = tokio::task::spawn_blocking(move || {
            let (model, dims) = load_model(model_id, model_path, cache_dir, runtime, threads)?;
            let mut state = state
                .lock()
                .map_err(|_| EmbeddingError::Runtime("rust-hf state mutex poisoned".to_string()))?;
            if state.model.is_none() {
                state.model = Some(model);
                state.dims = dims;
            }
            Ok::<(), EmbeddingError>(())
        });

        let result = timeout(self.timeout, handle)
            .await
            .map_err(|_| {
                EmbeddingError::Timeout(
                    "rust-hf model initialization timed out; increase rust-hf.timeout".to_string(),
                )
            })?
            .map_err(|error| EmbeddingError::Runtime(error.to_string()))?;
        result
    }
}

#[async_trait::async_trait]
impl RustHfEngine for FastEmbedRustHfEngine {
    async fn dims(&self) -> Result<Option<usize>, EmbeddingError> {
        self.ensure_model_loaded().await?;
        let cached_dims = {
            let state = self
                .state
                .lock()
                .map_err(|_| EmbeddingError::Runtime("rust-hf state mutex poisoned".to_string()))?;
            state.dims
        };

        if cached_dims.is_some() {
            return Ok(cached_dims);
        }

        let probe = vec!["dimensions probe".to_string()];
        let vectors = self.embed(&probe).await?;
        Ok(vectors.first().map(|vector| vector.len()))
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        self.ensure_model_loaded().await?;

        let inputs = texts.to_vec();
        let state = self.state.clone();
        let handle = tokio::task::spawn_blocking(move || {
            let mut state = state
                .lock()
                .map_err(|_| EmbeddingError::Runtime("rust-hf state mutex poisoned".to_string()))?;
            let model = state.model.as_mut().ok_or_else(|| {
                EmbeddingError::Runtime("rust-hf model is not initialized".to_string())
            })?;

            let vectors = model.embed(inputs, None).map_err(|error| {
                EmbeddingError::Runtime(format!("rust-hf embed failed: {error}"))
            })?;

            if state.dims.is_none() {
                state.dims = vectors.first().map(|vector| vector.len());
            }

            Ok::<Vec<Vec<f32>>, EmbeddingError>(vectors)
        });

        timeout(self.timeout, handle)
            .await
            .map_err(|_| {
                EmbeddingError::Timeout(
                    "rust-hf embedding call timed out; increase rust-hf.timeout".to_string(),
                )
            })?
            .map_err(|error| EmbeddingError::Runtime(error.to_string()))?
    }
}

fn load_model(
    model_id: String,
    model_path: Option<String>,
    cache_dir: Option<String>,
    runtime: RustHfRuntime,
    threads: Option<usize>,
) -> Result<(TextEmbedding, Option<usize>), EmbeddingError> {
    if runtime == RustHfRuntime::Candle {
        return Err(EmbeddingError::Config(
            "rust-hf runtime 'candle' is not available in this build; use runtime='onnx'"
                .to_string(),
        ));
    }

    if let Some(threads) = threads {
        std::env::set_var("ORT_NUM_THREADS", threads.to_string());
        std::env::set_var("OMP_NUM_THREADS", threads.to_string());
    }

    if let Some(model_path) = model_path {
        return load_user_defined_model(&model_path);
    }

    let model_name = model_id.parse::<EmbeddingModel>().map_err(|error| {
        EmbeddingError::Config(format!("invalid rust-hf modelId '{model_id}': {error}"))
    })?;

    let mut options = TextInitOptions::new(model_name.clone()).with_show_download_progress(false);
    if let Some(cache_dir) = cache_dir {
        options = options.with_cache_dir(PathBuf::from(cache_dir));
    }

    let model = TextEmbedding::try_new(options).map_err(|error| {
        EmbeddingError::Runtime(format!("failed to initialize rust-hf model: {error}"))
    })?;
    let dims = TextEmbedding::get_model_info(&model_name)
        .ok()
        .map(|model_info| model_info.dim);
    Ok((model, dims))
}

fn load_user_defined_model(
    model_path: &str,
) -> Result<(TextEmbedding, Option<usize>), EmbeddingError> {
    let provided = PathBuf::from(model_path);
    if !provided.exists() {
        return Err(EmbeddingError::Config(format!(
            "rust-hf modelPath does not exist: {}",
            provided.display()
        )));
    }

    let onnx_file_path = resolve_onnx_file(&provided)?;
    let search_root = if onnx_file_path.is_file() {
        onnx_file_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| provided.clone())
    } else {
        provided.clone()
    };

    let onnx_file = std::fs::read(&onnx_file_path).map_err(|error| {
        EmbeddingError::Runtime(format!(
            "failed to read ONNX file '{}': {error}",
            onnx_file_path.display()
        ))
    })?;

    let tokenizer_file = find_required_file_bytes(&search_root, "tokenizer.json")?;
    let config_file = find_required_file_bytes(&search_root, "config.json")?;
    let special_tokens_map_file =
        find_required_file_bytes(&search_root, "special_tokens_map.json")?;
    let tokenizer_config_file = find_required_file_bytes(&search_root, "tokenizer_config.json")?;

    let tokenizer_files = TokenizerFiles {
        tokenizer_file,
        config_file,
        special_tokens_map_file,
        tokenizer_config_file,
    };

    let model = UserDefinedEmbeddingModel::new(onnx_file, tokenizer_files);
    let options = InitOptionsUserDefined::new();
    let embedding = TextEmbedding::try_new_from_user_defined(model, options).map_err(|error| {
        EmbeddingError::Runtime(format!(
            "failed to initialize user-defined rust-hf model: {error}"
        ))
    })?;
    Ok((embedding, None))
}

fn resolve_onnx_file(path: &Path) -> Result<PathBuf, EmbeddingError> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }

    let direct = path.join("model.onnx");
    if direct.exists() {
        return Ok(direct);
    }

    let nested = path.join("onnx").join("model.onnx");
    if nested.exists() {
        return Ok(nested);
    }

    Err(EmbeddingError::Config(format!(
        "rust-hf modelPath '{}' does not contain model.onnx",
        path.display()
    )))
}

fn find_required_file_bytes(
    search_root: &Path,
    file_name: &str,
) -> Result<Vec<u8>, EmbeddingError> {
    let mut candidates = vec![search_root.join(file_name)];
    if let Some(parent) = search_root.parent() {
        candidates.push(parent.join(file_name));
    }

    for candidate in candidates {
        if candidate.exists() {
            return std::fs::read(&candidate).map_err(|error| {
                EmbeddingError::Runtime(format!(
                    "failed to read required tokenizer file '{}': {error}",
                    candidate.display()
                ))
            });
        }
    }

    Err(EmbeddingError::Config(format!(
        "rust-hf modelPath is missing required file '{file_name}'"
    )))
}
