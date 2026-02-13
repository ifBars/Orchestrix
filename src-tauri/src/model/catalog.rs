//! Model catalog - centralized provider and model metadata.
//!
//! This is the single source of truth for:
//! - Available models per provider
//! - Context window sizes
//! - Default models

use crate::model::provider::ProviderId;

/// Model metadata entry.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub context_window: u32,
    pub description: String,
}

/// Provider entry with its models.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderEntry {
    pub provider: String,
    pub models: Vec<ModelInfo>,
}

/// Full model catalog.
pub struct ModelCatalog;

impl ModelCatalog {
    /// Get all available models for all providers.
    pub fn all_models() -> Vec<ProviderEntry> {
        vec![
            ProviderEntry {
                provider: ProviderId::MiniMax.as_str().to_string(),
                models: vec![
                    ModelInfo {
                        name: "MiniMax-M2.5".to_string(),
                        context_window: 204_800,
                        description: "MiniMax M2.5 reasoning model (latest)".to_string(),
                    },
                    ModelInfo {
                        name: "MiniMax-M2.1".to_string(),
                        context_window: 204_800,
                        description: "MiniMax general purpose model".to_string(),
                    },
                    ModelInfo {
                        name: "MiniMax-M2".to_string(),
                        context_window: 196_608,
                        description: "MiniMax M2 reasoning model".to_string(),
                    },
                ],
            },
            ProviderEntry {
                provider: ProviderId::Kimi.as_str().to_string(),
                models: vec![ModelInfo {
                    name: "kimi-k2.5".to_string(),
                    context_window: 256_000,
                    description: "Kimi general purpose model".to_string(),
                }],
            },
            ProviderEntry {
                provider: ProviderId::Zhipu.as_str().to_string(),
                models: vec![
                    ModelInfo {
                        name: "glm-5".to_string(),
                        context_window: 1_000_000,
                        description: "GLM-5 latest flagship model".to_string(),
                    },
                    ModelInfo {
                        name: "glm-4.7".to_string(),
                        context_window: 1_000_000,
                        description: "GLM-4.7 advanced model".to_string(),
                    },
                    ModelInfo {
                        name: "glm-4.6".to_string(),
                        context_window: 1_000_000,
                        description: "GLM-4.6 model".to_string(),
                    },
                    ModelInfo {
                        name: "glm-4.5".to_string(),
                        context_window: 1_000_000,
                        description: "GLM-4.5 model".to_string(),
                    },
                    ModelInfo {
                        name: "glm-4.5-air".to_string(),
                        context_window: 1_000_000,
                        description: "GLM-4.5 Air lightweight model".to_string(),
                    },
                ],
            },
        ]
    }

    /// Get the default model for a provider.
    pub fn default_model_for_provider(provider: ProviderId) -> String {
        match provider {
            ProviderId::MiniMax => "MiniMax-M2.5".to_string(),
            ProviderId::Kimi => "kimi-k2.5".to_string(),
            ProviderId::Zhipu => "glm-5".to_string(),
        }
    }
}
