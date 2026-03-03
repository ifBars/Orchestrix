use std::time::Duration;

use serde::Serialize;
use tokio::sync::RwLock;

use crate::embeddings::config::GeminiEmbeddingConfig;
use crate::embeddings::error::EmbeddingError;
use crate::embeddings::types::{
    finalize_embeddings, EmbedOptions, EmbeddingProvider, EmbeddingProviderKind, EmbeddingTaskType,
};

/// Maximum number of attempts for a single batch request (1 initial + N-1 retries).
const MAX_ATTEMPTS: u32 = 5;
/// Base delay for exponential backoff on 429 responses (ms).
/// Collapsed to 1 ms in tests so the retry loop runs without real sleeps.
#[cfg(not(test))]
const BACKOFF_BASE_MS: u64 = 1_000;
#[cfg(test)]
const BACKOFF_BASE_MS: u64 = 1;
/// Maximum delay cap for backoff (ms).
#[cfg(not(test))]
const BACKOFF_MAX_MS: u64 = 32_000;
#[cfg(test)]
const BACKOFF_MAX_MS: u64 = 10;

const DEFAULT_GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub struct GeminiEmbeddingProvider {
    model: String,
    api_key: String,
    base_url: String,
    client: reqwest::Client,
    timeout_ms: u64,
    normalize_l2: bool,
    output_dimensionality: Option<u32>,
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

        // Validate output_dimensionality range per Gemini API spec (128–3072).
        if let Some(dims) = config.output_dimensionality {
            if !(128..=3072).contains(&dims) {
                return Err(EmbeddingError::Config(format!(
                    "gemini output_dimensionality must be between 128 and 3072, got {dims}"
                )));
            }
        }

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
            output_dimensionality: config.output_dimensionality,
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
                    output_dimensionality: self.output_dimensionality,
                })
                .collect(),
        };

        let mut attempt: u32 = 0;
        loop {
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

            // Handle 429 rate-limit with exponential backoff before reading the body.
            if status.as_u16() == 429 {
                attempt += 1;
                if attempt >= MAX_ATTEMPTS {
                    // Drain the body for a better error message on final attempt.
                    let msg = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "rate limited".to_string());
                    return Err(EmbeddingError::RateLimit(format!(
                        "Gemini embeddings API returned 429 after {attempt} attempts: {msg}"
                    )));
                }
                // Respect Retry-After header if present, otherwise use exponential backoff.
                // Floor at 1s to avoid tight loops if the server returns Retry-After: 0.
                let delay_ms = response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(|secs| secs.max(1) * 1_000)
                    .unwrap_or_else(|| {
                        let backoff = BACKOFF_BASE_MS * (1u64 << (attempt - 1));
                        backoff.min(BACKOFF_MAX_MS)
                    });
                // Drain the body so reqwest can reuse the connection.
                drop(response);
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }

            // Check auth errors before consuming the body — if the body is not valid JSON
            // (common with 401/403 HTML error pages) this gives a clear error message.
            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(EmbeddingError::Auth(
                    "Gemini authentication failed. Check embedding.gemini.apiKey or GEMINI_API_KEY"
                        .to_string(),
                ));
            }

            let payload: serde_json::Value = response.json().await.map_err(|error| {
                EmbeddingError::InvalidResponse(format!(
                    "failed to parse Gemini response JSON: {error}"
                ))
            })?;

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
            return Ok(finalized);
        }
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
    /// Matryoshka output dimensionality (128–3072). Omitted when None (API defaults to 3072).
    /// NOTE: any value < 3072 requires L2 normalization before similarity computation because
    /// only 3072-dim embeddings are pre-normalized by the Gemini API.
    #[serde(
        rename = "outputDimensionality",
        skip_serializing_if = "Option::is_none"
    )]
    output_dimensionality: Option<u32>,
}

#[derive(Debug, Serialize)]
struct GeminiContent<'a> {
    parts: Vec<GeminiTextPart<'a>>,
}

#[derive(Debug, Serialize)]
struct GeminiTextPart<'a> {
    text: &'a str,
}
