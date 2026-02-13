use std::time::Duration;

use serde::Serialize;
use tokio::sync::RwLock;

use crate::embeddings::config::GeminiEmbeddingConfig;
use crate::embeddings::error::EmbeddingError;
use crate::embeddings::types::{
    finalize_embeddings, EmbedOptions, EmbeddingProvider, EmbeddingProviderKind, EmbeddingTaskType,
};

const DEFAULT_GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub struct GeminiEmbeddingProvider {
    model: String,
    api_key: String,
    base_url: String,
    client: reqwest::Client,
    timeout_ms: u64,
    normalize_l2: bool,
    cached_dims: RwLock<Option<usize>>,
}

impl GeminiEmbeddingProvider {
    pub fn new(
        config: GeminiEmbeddingConfig,
        effective_api_key: Option<String>,
        normalize_l2: bool,
    ) -> Result<Self, EmbeddingError> {
        let api_key = effective_api_key
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                EmbeddingError::Config(
                    "gemini embedding provider requires apiKey (set embedding.gemini.apiKey or GEMINI_API_KEY)"
                        .to_string(),
                )
            })?;

        let model = config.model.trim();
        if model.is_empty() {
            return Err(EmbeddingError::Config(
                "gemini embedding model cannot be empty".to_string(),
            ));
        }

        if config.timeout_ms == 0 {
            return Err(EmbeddingError::Config(
                "gemini embedding timeout must be greater than 0".to_string(),
            ));
        }

        let base_url = config
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_GEMINI_BASE_URL)
            .to_string();

        Ok(Self {
            model: model.to_string(),
            api_key,
            base_url,
            client: reqwest::Client::builder()
                .timeout(Duration::from_millis(config.timeout_ms))
                .build()
                .map_err(|error| EmbeddingError::Runtime(error.to_string()))?,
            timeout_ms: config.timeout_ms,
            normalize_l2,
            cached_dims: RwLock::new(None),
        })
    }

    async fn embed_internal(
        &self,
        texts: &[String],
        task: EmbeddingTaskType,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let model_resource = if self.model.starts_with("models/") {
            self.model.clone()
        } else {
            format!("models/{}", self.model)
        };
        let endpoint = format!(
            "{}/{}:batchEmbedContents",
            self.base_url.trim_end_matches('/'),
            model_resource
        );

        let body = GeminiBatchEmbedRequest {
            requests: texts
                .iter()
                .map(|text| GeminiBatchEmbedRequestItem {
                    model: model_resource.clone(),
                    content: GeminiContent {
                        parts: vec![GeminiTextPart { text }],
                    },
                    task_type: Some(task),
                })
                .collect(),
        };

        let response = self
            .client
            .post(&endpoint)
            .query(&[("key", self.api_key.as_str())])
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                if error.is_connect() {
                    EmbeddingError::Request(format!(
                        "could not reach Gemini embeddings endpoint at {}: {error}",
                        self.base_url
                    ))
                } else if error.is_timeout() {
                    EmbeddingError::Timeout(format!(
                        "Gemini embeddings request timed out after {} ms",
                        self.timeout_ms
                    ))
                } else {
                    EmbeddingError::Request(error.to_string())
                }
            })?;

        let status = response.status();
        let payload: serde_json::Value = response.json().await.map_err(|error| {
            EmbeddingError::InvalidResponse(format!(
                "failed to parse Gemini response JSON: {error}"
            ))
        })?;

        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(EmbeddingError::Auth(
                "Gemini authentication failed. Check embedding.gemini.apiKey or GEMINI_API_KEY"
                    .to_string(),
            ));
        }

        if !status.is_success() {
            return Err(EmbeddingError::Request(format!(
                "Gemini embeddings API returned {status}: {}",
                payload
            )));
        }

        let embeddings = payload
            .get("embeddings")
            .and_then(|value| value.as_array())
            .ok_or_else(|| {
                EmbeddingError::InvalidResponse(
                    "Gemini embeddings response missing 'embeddings' array".to_string(),
                )
            })?;

        if embeddings.len() != texts.len() {
            return Err(EmbeddingError::InvalidResponse(format!(
                "Gemini returned {} embeddings for {} inputs",
                embeddings.len(),
                texts.len()
            )));
        }

        let mut vectors = Vec::with_capacity(embeddings.len());
        for (index, item) in embeddings.iter().enumerate() {
            let values = item
                .get("values")
                .or_else(|| item.get("embedding").and_then(|value| value.get("values")))
                .and_then(|value| value.as_array())
                .ok_or_else(|| {
                    EmbeddingError::InvalidResponse(format!(
                        "Gemini embedding at index {index} is missing 'values'"
                    ))
                })?;

            let mut vector = Vec::with_capacity(values.len());
            for value in values {
                let number = value.as_f64().ok_or_else(|| {
                    EmbeddingError::InvalidResponse(format!(
                        "Gemini embedding at index {index} contains a non-numeric value"
                    ))
                })?;
                vector.push(number as f32);
            }
            vectors.push(vector);
        }

        let finalized = finalize_embeddings(vectors, self.normalize_l2)?;
        if let Some(dim) = finalized.first().map(|vector| vector.len()) {
            *self.cached_dims.write().await = Some(dim);
        }
        Ok(finalized)
    }

    fn task_from_options(opts: Option<&EmbedOptions>) -> EmbeddingTaskType {
        opts.and_then(|options| options.task)
            .unwrap_or(EmbeddingTaskType::RetrievalDocument)
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for GeminiEmbeddingProvider {
    fn id(&self) -> &str {
        "gemini"
    }

    fn kind(&self) -> EmbeddingProviderKind {
        EmbeddingProviderKind::Remote
    }

    async fn dims(&self) -> Result<Option<usize>, EmbeddingError> {
        if let Some(cached) = *self.cached_dims.read().await {
            return Ok(Some(cached));
        }

        let probe = vec!["dimensions probe".to_string()];
        let vectors = self
            .embed_internal(&probe, EmbeddingTaskType::RetrievalDocument)
            .await?;
        Ok(vectors.first().map(|vector| vector.len()))
    }

    async fn embed(
        &self,
        texts: &[String],
        opts: Option<EmbedOptions>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let task = Self::task_from_options(opts.as_ref());
        self.embed_internal(texts, task).await
    }
}

#[derive(Debug, Serialize)]
struct GeminiBatchEmbedRequest<'a> {
    requests: Vec<GeminiBatchEmbedRequestItem<'a>>,
}

#[derive(Debug, Serialize)]
struct GeminiBatchEmbedRequestItem<'a> {
    model: String,
    content: GeminiContent<'a>,
    #[serde(rename = "taskType", skip_serializing_if = "Option::is_none")]
    task_type: Option<EmbeddingTaskType>,
}

#[derive(Debug, Serialize)]
struct GeminiContent<'a> {
    parts: Vec<GeminiTextPart<'a>>,
}

#[derive(Debug, Serialize)]
struct GeminiTextPart<'a> {
    text: &'a str,
}
