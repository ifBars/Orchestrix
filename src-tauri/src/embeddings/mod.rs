pub mod config;
pub mod error;
pub mod factory;
pub mod indexer;
pub mod manager;
pub mod providers;
pub mod types;

pub use config::{
    load_embedding_config, EmbeddingConfig, EmbeddingConfigView, EmbeddingProviderId,
    GeminiEmbeddingConfig, OllamaEmbeddingConfig, RustHfEmbeddingConfig, RustHfRuntime,
    TransformersJsEmbeddingConfig,
};
pub use error::EmbeddingError;
pub use indexer::{EmbeddingIndexStatus, SemanticIndexService, SemanticSearchResponse};
pub use manager::{EmbeddingManager, EmbeddingProviderInfo};
pub use types::{
    cosine_similarity, EmbedOptions, EmbeddingProvider, EmbeddingProviderKind, EmbeddingTaskType,
};

pub fn is_semantic_search_configured(db: &crate::db::Database) -> bool {
    match load_embedding_config(db) {
        Ok(config) => config.is_configured(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests;
