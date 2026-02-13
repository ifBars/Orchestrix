use std::time::Duration;

use serde::Serialize;
use tokio::sync::RwLock;

use crate::embeddings::config::OllamaEmbeddingConfig;
use crate::embeddings::error::EmbeddingError;
use crate::embeddings::types::{
    finalize_embeddings, EmbedOptions, EmbeddingProvider, EmbeddingProviderKind,
};

pub struct OllamaEmbeddingProvider {
    base_url: String,
    model: String,
    timeout_ms: u64,
    client: reqwest::Client,
    normalize_l2: bool,
    cached_dims: RwLock<Option<usize>>,
}

impl OllamaEmbeddingProvider {
    pub fn new(config: OllamaEmbeddingConfig, normalize_l2: bool) -> Result<Self, EmbeddingError> {
        if config.model.trim().is_empty() {
            return Err(EmbeddingError::Config(
                "ollama model cannot be empty".to_string(),
            ));
        }
        if config.base_url.trim().is_empty() {
            return Err(EmbeddingError::Config(
                "ollama baseUrl cannot be empty".to_string(),
            ));
        }
        if config.timeout_ms == 0 {
            return Err(EmbeddingError::Config(
                "ollama timeout must be greater than 0".to_string(),
            ));
        }

        Ok(Self {
            base_url: config.base_url.trim().to_string(),
            model: config.model.trim().to_string(),
            timeout_ms: config.timeout_ms,
            client: reqwest::Client::builder()
                .timeout(Duration::from_millis(config.timeout_ms))
                .build()
                .map_err(|error| EmbeddingError::Runtime(error.to_string()))?,
            normalize_l2,
            cached_dims: RwLock::new(None),
        })
    }

    async fn embed_internal(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        match self.embed_batch_endpoint(texts).await {
            Ok(vectors) => {
                let finalized = finalize_embeddings(vectors, self.normalize_l2)?;
                if let Some(dim) = finalized.first().map(|vector| vector.len()) {
                    *self.cached_dims.write().await = Some(dim);
                }
                Ok(finalized)
            }
            Err(EmbeddingError::Request(error)) if error.contains("status 404") => {
                let vectors = self.embed_legacy_endpoint(texts).await?;
                let finalized = finalize_embeddings(vectors, self.normalize_l2)?;
                if let Some(dim) = finalized.first().map(|vector| vector.len()) {
                    *self.cached_dims.write().await = Some(dim);
                }
                Ok(finalized)
            }
            Err(error) => Err(error),
        }
    }

    async fn embed_batch_endpoint(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let endpoint = format!("{}/api/embed", self.base_url.trim_end_matches('/'));
        let response = self
            .client
            .post(&endpoint)
            .json(&OllamaBatchEmbedRequest {
                model: &self.model,
                input: texts,
            })
            .send()
            .await
            .map_err(|error| self.map_connectivity_error(error))?;

        let status = response.status();
        let payload: serde_json::Value = response.json().await.map_err(|error| {
            EmbeddingError::InvalidResponse(format!(
                "failed to parse Ollama /api/embed JSON: {error}"
            ))
        })?;

        if !status.is_success() {
            return Err(EmbeddingError::Request(format!(
                "ollama /api/embed returned status {}: {}",
                status.as_u16(),
                payload
            )));
        }

        let embeddings = payload
            .get("embeddings")
            .and_then(|value| value.as_array())
            .ok_or_else(|| {
                EmbeddingError::InvalidResponse(
                    "ollama /api/embed response missing 'embeddings' array".to_string(),
                )
            })?;

        if embeddings.len() != texts.len() {
            return Err(EmbeddingError::InvalidResponse(format!(
                "ollama /api/embed returned {} embeddings for {} inputs",
                embeddings.len(),
                texts.len()
            )));
        }

        let mut vectors = Vec::with_capacity(embeddings.len());
        for (index, embedding) in embeddings.iter().enumerate() {
            vectors.push(parse_vector(embedding, index)?);
        }
        Ok(vectors)
    }

    async fn embed_legacy_endpoint(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let endpoint = format!("{}/api/embeddings", self.base_url.trim_end_matches('/'));
        let mut vectors = Vec::with_capacity(texts.len());

        for (index, text) in texts.iter().enumerate() {
            let response = self
                .client
                .post(&endpoint)
                .json(&OllamaLegacyEmbedRequest {
                    model: &self.model,
                    prompt: text,
                })
                .send()
                .await
                .map_err(|error| self.map_connectivity_error(error))?;

            let status = response.status();
            let payload: serde_json::Value = response.json().await.map_err(|error| {
                EmbeddingError::InvalidResponse(format!(
                    "failed to parse Ollama /api/embeddings JSON: {error}"
                ))
            })?;

            if !status.is_success() {
                return Err(EmbeddingError::Request(format!(
                    "ollama /api/embeddings returned status {}: {}",
                    status.as_u16(),
                    payload
                )));
            }

            let embedding = payload.get("embedding").ok_or_else(|| {
                EmbeddingError::InvalidResponse(
                    "ollama /api/embeddings response missing 'embedding' field".to_string(),
                )
            })?;
            vectors.push(parse_vector(embedding, index)?);
        }

        Ok(vectors)
    }

    fn map_connectivity_error(&self, error: reqwest::Error) -> EmbeddingError {
        if error.is_connect() {
            return EmbeddingError::Request(format!(
                "could not reach Ollama at {}. Ensure 'ollama serve' is running: {error}",
                self.base_url
            ));
        }
        if error.is_timeout() {
            return EmbeddingError::Timeout(format!(
                "Ollama request timed out after {} ms",
                self.timeout_ms
            ));
        }
        EmbeddingError::Request(error.to_string())
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for OllamaEmbeddingProvider {
    fn id(&self) -> &str {
        "ollama"
    }

    fn kind(&self) -> EmbeddingProviderKind {
        EmbeddingProviderKind::Local
    }

    async fn dims(&self) -> Result<Option<usize>, EmbeddingError> {
        if let Some(cached) = *self.cached_dims.read().await {
            return Ok(Some(cached));
        }

        let probe = vec!["dimensions probe".to_string()];
        let vectors = self.embed_internal(&probe).await?;
        Ok(vectors.first().map(|vector| vector.len()))
    }

    async fn embed(
        &self,
        texts: &[String],
        _opts: Option<EmbedOptions>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.embed_internal(texts).await
    }
}

fn parse_vector(value: &serde_json::Value, index: usize) -> Result<Vec<f32>, EmbeddingError> {
    let numbers = value.as_array().ok_or_else(|| {
        EmbeddingError::InvalidResponse(format!(
            "ollama embedding at index {index} is not an array"
        ))
    })?;

    let mut vector = Vec::with_capacity(numbers.len());
    for number in numbers {
        vector.push(number.as_f64().ok_or_else(|| {
            EmbeddingError::InvalidResponse(format!(
                "ollama embedding at index {index} contains a non-numeric value"
            ))
        })? as f32);
    }
    Ok(vector)
}

#[derive(Debug, Serialize)]
struct OllamaBatchEmbedRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Debug, Serialize)]
struct OllamaLegacyEmbedRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}
