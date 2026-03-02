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
                models: vec![
                    ModelInfo {
                        name: "kimi-k2-thinking".to_string(),
                        context_window: 256_000,
                        description: "Kimi K2 Thinking - Extended reasoning model".to_string(),
                    },
                    ModelInfo {
                        name: "kimi-k2.5".to_string(),
                        context_window: 256_000,
                        description: "Kimi general purpose model".to_string(),
                    },
                ],
            },
            ProviderEntry {
                provider: ProviderId::Zhipu.as_str().to_string(),
                models: vec![
                    ModelInfo {
                        name: "glm-5".to_string(),
                        context_window: 204_800,
                        description: "GLM-5 latest flagship model".to_string(),
                    },
                    ModelInfo {
                        name: "glm-4.7".to_string(),
                        context_window: 204_800,
                        description: "GLM-4.7 advanced model".to_string(),
                    },
                    ModelInfo {
                        name: "glm-4.7-flash".to_string(),
                        context_window: 200_000,
                        description: "GLM-4.7 Flash fast model".to_string(),
                    },
                    ModelInfo {
                        name: "glm-4.6".to_string(),
                        context_window: 204_800,
                        description: "GLM-4.6 model".to_string(),
                    },
                    ModelInfo {
                        name: "glm-4.6v".to_string(),
                        context_window: 128_000,
                        description: "GLM-4.6V vision model".to_string(),
                    },
                    ModelInfo {
                        name: "glm-4.5".to_string(),
                        context_window: 131_072,
                        description: "GLM-4.5 model".to_string(),
                    },
                    ModelInfo {
                        name: "glm-4.5-air".to_string(),
                        context_window: 131_072,
                        description: "GLM-4.5 Air lightweight model".to_string(),
                    },
                    ModelInfo {
                        name: "glm-4.5-flash".to_string(),
                        context_window: 131_072,
                        description: "GLM-4.5 Flash fast model".to_string(),
                    },
                ],
            },
            ProviderEntry {
                provider: ProviderId::Modal.as_str().to_string(),
                models: vec![ModelInfo {
                    name: "zai-org/GLM-5-FP8".to_string(),
                    context_window: 204_800,
                    description: "GLM-5 on Modal (FP8 quantized)".to_string(),
                }],
            },
            ProviderEntry {
                provider: ProviderId::OpenAIChatGPT.as_str().to_string(),
                models: vec![
                    ModelInfo {
                        name: "gpt-5.3-codex".to_string(),
                        context_window: 400_000,
                        description: "GPT-5.3 Codex - Latest coding model (ChatGPT Plus/Pro)"
                            .to_string(),
                    },
                    ModelInfo {
                        name: "gpt-5.2-codex".to_string(),
                        context_window: 400_000,
                        description: "GPT-5.2 Codex - Coding model (ChatGPT Plus/Pro)".to_string(),
                    },
                    ModelInfo {
                        name: "gpt-5.1-codex-max".to_string(),
                        context_window: 400_000,
                        description: "GPT-5.1 Codex Max - Full capability coding model".to_string(),
                    },
                    ModelInfo {
                        name: "gpt-5.1-codex-mini".to_string(),
                        context_window: 400_000,
                        description: "GPT-5.1 Codex Mini - Fast coding model".to_string(),
                    },
                    ModelInfo {
                        name: "gpt-5.2".to_string(),
                        context_window: 400_000,
                        description: "GPT-5.2 - Latest general model (ChatGPT Plus/Pro)"
                            .to_string(),
                    },
                    ModelInfo {
                        name: "gpt-5.1".to_string(),
                        context_window: 400_000,
                        description: "GPT-5.1 - General purpose model".to_string(),
                    },
                    ModelInfo {
                        name: "gpt-5".to_string(),
                        context_window: 200_000,
                        description: "GPT-5 - Standard model".to_string(),
                    },
                    ModelInfo {
                        name: "gpt-5-nano".to_string(),
                        context_window: 128_000,
                        description: "GPT-5 Nano - Fast, lightweight model".to_string(),
                    },
                ],
            },
            ProviderEntry {
                provider: ProviderId::Gemini.as_str().to_string(),
                models: vec![
                    ModelInfo {
                        name: "gemini-3-pro-preview".to_string(),
                        context_window: 1_000_000,
                        description:
                            "Gemini 3 Pro - 1M tokens, complex reasoning, coding, research"
                                .to_string(),
                    },
                    ModelInfo {
                        name: "gemini-3-flash-preview".to_string(),
                        context_window: 1_000_000,
                        description:
                            "Gemini 3 Flash - 1M tokens, fast, balanced performance, multimodal"
                                .to_string(),
                    },
                    ModelInfo {
                        name: "gemini-3-pro-image-preview".to_string(),
                        context_window: 65_536,
                        description: "Gemini 3 Pro Image - Image generation and editing"
                            .to_string(),
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
            ProviderId::Modal => "zai-org/GLM-5-FP8".to_string(),
            ProviderId::OpenAIChatGPT => "gpt-5.3-codex".to_string(),
            ProviderId::Gemini => "gemini-3-flash-preview".to_string(),
        }
    }
}
