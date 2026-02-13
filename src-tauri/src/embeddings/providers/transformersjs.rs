use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;

use crate::embeddings::config::TransformersJsEmbeddingConfig;
use crate::embeddings::error::EmbeddingError;
use crate::embeddings::types::{
    finalize_embeddings, EmbedOptions, EmbeddingProvider, EmbeddingProviderKind,
};

#[derive(Debug, Clone)]
pub struct TransformersBridgeRequest {
    pub model: String,
    pub device: String,
    pub backend: Option<String>,
    pub cache_dir: Option<String>,
}

#[async_trait::async_trait]
pub trait TransformersBridgeTransport: Send + Sync {
    async fn dims(
        &self,
        request: &TransformersBridgeRequest,
    ) -> Result<Option<usize>, EmbeddingError>;
    async fn embed(
        &self,
        request: &TransformersBridgeRequest,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingError>;
}

pub struct TransformersJsEmbeddingProvider {
    request: TransformersBridgeRequest,
    transport: Arc<dyn TransformersBridgeTransport>,
    normalize_l2: bool,
    cached_dims: RwLock<Option<usize>>,
}

impl TransformersJsEmbeddingProvider {
    pub fn new(
        config: TransformersJsEmbeddingConfig,
        normalize_l2: bool,
    ) -> Result<Self, EmbeddingError> {
        let script_path = resolve_bridge_script_path(&config)?;
        let request = TransformersBridgeRequest {
            model: config.model.trim().to_string(),
            device: config.device.trim().to_string(),
            backend: config.backend.clone(),
            cache_dir: config.cache_dir.clone(),
        };

        if request.model.is_empty() {
            return Err(EmbeddingError::Config(
                "transformersjs model cannot be empty".to_string(),
            ));
        }
        if request.device.is_empty() {
            return Err(EmbeddingError::Config(
                "transformersjs device cannot be empty".to_string(),
            ));
        }

        let transport = Arc::new(SubprocessTransformersBridgeTransport::new(
            config.bridge_command,
            script_path,
            Duration::from_millis(config.timeout_ms),
        ));

        Ok(Self {
            request,
            transport,
            normalize_l2,
            cached_dims: RwLock::new(None),
        })
    }

    pub fn new_with_transport(
        config: TransformersJsEmbeddingConfig,
        normalize_l2: bool,
        transport: Arc<dyn TransformersBridgeTransport>,
    ) -> Result<Self, EmbeddingError> {
        let request = TransformersBridgeRequest {
            model: config.model.trim().to_string(),
            device: config.device.trim().to_string(),
            backend: config.backend.clone(),
            cache_dir: config.cache_dir.clone(),
        };

        if request.model.is_empty() {
            return Err(EmbeddingError::Config(
                "transformersjs model cannot be empty".to_string(),
            ));
        }
        if request.device.is_empty() {
            return Err(EmbeddingError::Config(
                "transformersjs device cannot be empty".to_string(),
            ));
        }

        Ok(Self {
            request,
            transport,
            normalize_l2,
            cached_dims: RwLock::new(None),
        })
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for TransformersJsEmbeddingProvider {
    fn id(&self) -> &str {
        "transformersjs"
    }

    fn kind(&self) -> EmbeddingProviderKind {
        EmbeddingProviderKind::Local
    }

    async fn dims(&self) -> Result<Option<usize>, EmbeddingError> {
        if let Some(cached) = *self.cached_dims.read().await {
            return Ok(Some(cached));
        }

        let dims = self.transport.dims(&self.request).await?;
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

        let vectors = self.transport.embed(&self.request, texts).await?;
        let finalized = finalize_embeddings(vectors, self.normalize_l2)?;
        if let Some(dim) = finalized.first().map(|vector| vector.len()) {
            *self.cached_dims.write().await = Some(dim);
        }
        Ok(finalized)
    }
}

pub struct SubprocessTransformersBridgeTransport {
    command: String,
    script_path: PathBuf,
    timeout: Duration,
    next_request_id: AtomicU64,
    process: Mutex<Option<BridgeProcess>>,
}

impl SubprocessTransformersBridgeTransport {
    pub fn new(command: String, script_path: PathBuf, timeout: Duration) -> Self {
        Self {
            command,
            script_path,
            timeout,
            next_request_id: AtomicU64::new(1),
            process: Mutex::new(None),
        }
    }

    async fn spawn_process(&self) -> Result<BridgeProcess, EmbeddingError> {
        let mut command = tokio::process::Command::new(&self.command);
        command
            .arg(&self.script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = command.spawn().map_err(|error| {
            EmbeddingError::Bridge(format!(
                "failed to start transformers bridge with '{} {}': {error}",
                self.command,
                self.script_path.display()
            ))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            EmbeddingError::Bridge("failed to acquire transformers bridge stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            EmbeddingError::Bridge("failed to acquire transformers bridge stdout".to_string())
        })?;

        Ok(BridgeProcess {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    async fn call(
        &self,
        action: BridgeAction,
        request: &TransformersBridgeRequest,
        texts: Option<&[String]>,
    ) -> Result<serde_json::Value, EmbeddingError> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let payload = BridgeMessage {
            id: request_id,
            action,
            model: &request.model,
            device: &request.device,
            backend: request.backend.as_deref(),
            cache_dir: request.cache_dir.as_deref(),
            texts,
        };
        let line = serde_json::to_string(&payload)
            .map_err(|error| EmbeddingError::Bridge(error.to_string()))?;

        let mut process_guard = self.process.lock().await;
        for attempt in 0..2 {
            if process_guard.is_none() {
                *process_guard = Some(self.spawn_process().await?);
            }

            let process = process_guard
                .as_mut()
                .expect("process should be initialized");
            if let Err(error) = process.stdin.write_all(line.as_bytes()).await {
                *process_guard = None;
                if attempt == 0 {
                    continue;
                }
                return Err(EmbeddingError::Bridge(format!(
                    "failed to write request to transformers bridge: {error}"
                )));
            }
            if let Err(error) = process.stdin.write_all(b"\n").await {
                *process_guard = None;
                if attempt == 0 {
                    continue;
                }
                return Err(EmbeddingError::Bridge(format!(
                    "failed to finalize transformers bridge request: {error}"
                )));
            }
            if let Err(error) = process.stdin.flush().await {
                *process_guard = None;
                if attempt == 0 {
                    continue;
                }
                return Err(EmbeddingError::Bridge(format!(
                    "failed to flush transformers bridge request: {error}"
                )));
            }

            let mut response_line = String::new();
            let read_result = timeout(self.timeout, process.stdout.read_line(&mut response_line))
                .await
                .map_err(|_| {
                    EmbeddingError::Timeout(
                        "transformers bridge did not respond before timeout".to_string(),
                    )
                })?;

            match read_result {
                Ok(0) => {
                    let _ = process.child.start_kill();
                    *process_guard = None;
                    if attempt == 0 {
                        continue;
                    }
                    return Err(EmbeddingError::Bridge(
                        "transformers bridge exited before sending a response".to_string(),
                    ));
                }
                Ok(_) => {
                    let parsed: BridgeResponse = serde_json::from_str(response_line.trim())
                        .map_err(|error| {
                            EmbeddingError::InvalidResponse(format!(
                                "invalid transformers bridge response: {error}"
                            ))
                        })?;

                    if parsed.id != request_id {
                        return Err(EmbeddingError::InvalidResponse(format!(
                            "transformers bridge response id mismatch: expected {request_id}, got {}",
                            parsed.id
                        )));
                    }
                    if !parsed.ok {
                        return Err(EmbeddingError::Bridge(parsed.error.unwrap_or_else(|| {
                            "transformers bridge returned an unknown error".to_string()
                        })));
                    }
                    return parsed.result.ok_or_else(|| {
                        EmbeddingError::InvalidResponse(
                            "transformers bridge response missing result payload".to_string(),
                        )
                    });
                }
                Err(error) => {
                    *process_guard = None;
                    if attempt == 0 {
                        continue;
                    }
                    return Err(EmbeddingError::Bridge(format!(
                        "failed to read transformers bridge response: {error}"
                    )));
                }
            }
        }

        Err(EmbeddingError::Bridge(
            "failed to communicate with transformers bridge".to_string(),
        ))
    }
}

#[async_trait::async_trait]
impl TransformersBridgeTransport for SubprocessTransformersBridgeTransport {
    async fn dims(
        &self,
        request: &TransformersBridgeRequest,
    ) -> Result<Option<usize>, EmbeddingError> {
        let payload = self.call(BridgeAction::Dims, request, None).await?;
        Ok(payload
            .get("dims")
            .and_then(|value| value.as_u64())
            .map(|value| value as usize))
    }

    async fn embed(
        &self,
        request: &TransformersBridgeRequest,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let payload = self.call(BridgeAction::Embed, request, Some(texts)).await?;

        let vectors = payload
            .get("vectors")
            .and_then(|value| value.as_array())
            .ok_or_else(|| {
                EmbeddingError::InvalidResponse(
                    "transformers bridge response missing vectors array".to_string(),
                )
            })?;

        let mut parsed_vectors = Vec::with_capacity(vectors.len());
        for (index, vector) in vectors.iter().enumerate() {
            let values = vector.as_array().ok_or_else(|| {
                EmbeddingError::InvalidResponse(format!(
                    "transformers bridge vector at index {index} is not an array"
                ))
            })?;

            let mut parsed = Vec::with_capacity(values.len());
            for value in values {
                parsed.push(value.as_f64().ok_or_else(|| {
                    EmbeddingError::InvalidResponse(format!(
                        "transformers bridge vector at index {index} contains a non-number"
                    ))
                })? as f32);
            }
            parsed_vectors.push(parsed);
        }

        Ok(parsed_vectors)
    }
}

struct BridgeProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[derive(Debug, Serialize)]
struct BridgeMessage<'a> {
    id: u64,
    action: BridgeAction,
    model: &'a str,
    device: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    backend: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "cacheDir")]
    cache_dir: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    texts: Option<&'a [String]>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum BridgeAction {
    Dims,
    Embed,
}

#[derive(Debug, Deserialize)]
struct BridgeResponse {
    id: u64,
    ok: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

fn resolve_bridge_script_path(
    config: &TransformersJsEmbeddingConfig,
) -> Result<PathBuf, EmbeddingError> {
    if let Some(path) = config.bridge_script.as_deref() {
        let resolved = PathBuf::from(path);
        if resolved.exists() {
            return Ok(resolved);
        }
        return Err(EmbeddingError::Config(format!(
            "transformersjs bridge script not found at {}",
            resolved.display()
        )));
    }

    if let Ok(path) = std::env::var("ORCHESTRIX_TRANSFORMERS_BRIDGE") {
        let resolved = PathBuf::from(path);
        if resolved.exists() {
            return Ok(resolved);
        }
    }

    let cwd =
        std::env::current_dir().map_err(|error| EmbeddingError::Runtime(error.to_string()))?;
    let candidates = [
        cwd.join("src/embeddings/transformers_bridge.mjs"),
        cwd.join("src-tauri/src/embeddings/transformers_bridge.mjs"),
        cwd.join("../src/embeddings/transformers_bridge.mjs"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(EmbeddingError::Config(
        "could not locate transformers bridge script. Set embedding.transformersjs.bridge_script or ORCHESTRIX_TRANSFORMERS_BRIDGE"
            .to_string(),
    ))
}
