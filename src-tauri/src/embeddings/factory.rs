use std::sync::Arc;

use crate::embeddings::config::{EmbeddingConfig, EmbeddingProviderId};
use crate::embeddings::error::EmbeddingError;
use crate::embeddings::providers::{
    GeminiEmbeddingProvider, OllamaEmbeddingProvider, RustLocalEmbeddingProvider,
    TransformersJsEmbeddingProvider,
};
use crate::embeddings::types::EmbeddingProvider;

pub fn create_provider(
    config: &EmbeddingConfig,
) -> Result<Arc<dyn EmbeddingProvider>, EmbeddingError> {
    config.validate_selected_provider()?;

    let provider: Arc<dyn EmbeddingProvider> = match config.provider {
        EmbeddingProviderId::Gemini => Arc::new(GeminiEmbeddingProvider::new(
            config.gemini.clone(),
            config.effective_gemini_api_key(),
            config.normalize_l2,
        )?),
        EmbeddingProviderId::Ollama => Arc::new(OllamaEmbeddingProvider::new(
            config.ollama.clone(),
            config.normalize_l2,
        )?),
        EmbeddingProviderId::Transformersjs => Arc::new(TransformersJsEmbeddingProvider::new(
            config.transformersjs.clone(),
            config.normalize_l2,
        )?),
        EmbeddingProviderId::RustHf => Arc::new(RustLocalEmbeddingProvider::new(
            config.rust_hf.clone(),
            config.normalize_l2,
        )?),
    };

    Ok(provider)
}
