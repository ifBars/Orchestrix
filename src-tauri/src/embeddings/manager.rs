use std::sync::Arc;

use serde::Serialize;
use tokio::sync::RwLock;

use crate::db::Database;
use crate::embeddings::config::{
    load_embedding_config, save_embedding_config, EmbeddingConfig, EmbeddingConfigView,
};
use crate::embeddings::error::EmbeddingError;
use crate::embeddings::factory::create_provider;
use crate::embeddings::types::{EmbedOptions, EmbeddingProvider, EmbeddingProviderKind};

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingProviderInfo {
    pub id: String,
    pub kind: EmbeddingProviderKind,
}

struct EmbeddingManagerState {
    config_key: Option<String>,
    provider: Option<Arc<dyn EmbeddingProvider>>,
}

pub struct EmbeddingManager {
    db: Arc<Database>,
    state: RwLock<EmbeddingManagerState>,
}

impl EmbeddingManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            state: RwLock::new(EmbeddingManagerState {
                config_key: None,
                provider: None,
            }),
        }
    }

    pub fn get_config_view(&self) -> Result<EmbeddingConfigView, EmbeddingError> {
        let config = load_embedding_config(&self.db)?;
        Ok(config.to_view())
    }

    pub async fn set_config(
        &self,
        config: EmbeddingConfig,
    ) -> Result<EmbeddingConfigView, EmbeddingError> {
        config.validate_selected_provider()?;

        save_embedding_config(&self.db, &config)?;
        {
            let mut state = self.state.write().await;
            state.config_key = None;
            state.provider = None;
        }
        Ok(config.to_view())
    }

    pub async fn provider_info(&self) -> Result<EmbeddingProviderInfo, EmbeddingError> {
        let provider = self.get_or_create_provider().await?;
        Ok(EmbeddingProviderInfo {
            id: provider.id().to_string(),
            kind: provider.kind(),
        })
    }

    pub async fn dims(&self) -> Result<Option<usize>, EmbeddingError> {
        let provider = self.get_or_create_provider().await?;
        provider.dims().await
    }

    pub async fn embed(
        &self,
        texts: &[String],
        opts: Option<EmbedOptions>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let provider = self.get_or_create_provider().await?;
        provider.embed(texts, opts).await
    }

    async fn get_or_create_provider(&self) -> Result<Arc<dyn EmbeddingProvider>, EmbeddingError> {
        let mut config = load_embedding_config(&self.db)?;
        config.apply_env_overrides();
        config.validate_selected_provider()?;

        let config_key = serde_json::to_string(&config)
            .map_err(|error| EmbeddingError::Runtime(error.to_string()))?;

        {
            let state = self.state.read().await;
            if state.config_key.as_ref() == Some(&config_key) {
                if let Some(provider) = state.provider.as_ref() {
                    return Ok(provider.clone());
                }
            }
        }

        let provider = create_provider(&config)?;

        let mut state = self.state.write().await;
        if state.config_key.as_ref() == Some(&config_key) {
            if let Some(cached_provider) = state.provider.as_ref() {
                return Ok(cached_provider.clone());
            }
        }

        state.config_key = Some(config_key);
        state.provider = Some(provider.clone());
        Ok(provider)
    }
}
