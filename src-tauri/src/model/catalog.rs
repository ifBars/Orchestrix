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
    pub output_limit: u32,
    pub description: String,
    pub deprecated: bool,
    pub deprecation_reason: Option<String>,
    pub suggested_alternative: Option<String>,
    pub capabilities: Vec<String>,
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
                        output_limit: 65_536,
                        description: "MiniMax M2.5 reasoning model (latest)".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "MiniMax-M2.1".to_string(),
                        context_window: 204_800,
                        output_limit: 65_536,
                        description: "MiniMax general purpose model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "MiniMax-M2".to_string(),
                        context_window: 196_608,
                        output_limit: 65_536,
                        description: "MiniMax M2 reasoning model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                ],
            },
            ProviderEntry {
                provider: ProviderId::Kimi.as_str().to_string(),
                models: vec![
                    ModelInfo {
                        name: "kimi-k2-thinking".to_string(),
                        context_window: 256_000,
                        output_limit: 65_536,
                        description: "Kimi K2 Thinking - Extended reasoning model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "kimi-k2.5".to_string(),
                        context_window: 256_000,
                        output_limit: 65_536,
                        description: "Kimi general purpose model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                ],
            },
            ProviderEntry {
                provider: ProviderId::Zhipu.as_str().to_string(),
                models: vec![
                    ModelInfo {
                        name: "glm-5".to_string(),
                        context_window: 204_800,
                        output_limit: 65_536,
                        description: "GLM-5 latest flagship model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "glm-4.7".to_string(),
                        context_window: 204_800,
                        output_limit: 65_536,
                        description: "GLM-4.7 advanced model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "glm-4.7-flash".to_string(),
                        context_window: 200_000,
                        output_limit: 65_536,
                        description: "GLM-4.7 Flash fast model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "glm-4.6".to_string(),
                        context_window: 204_800,
                        output_limit: 65_536,
                        description: "GLM-4.6 model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "glm-4.6v".to_string(),
                        context_window: 128_000,
                        output_limit: 65_536,
                        description: "GLM-4.6V vision model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "image".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "glm-4.5".to_string(),
                        context_window: 131_072,
                        output_limit: 65_536,
                        description: "GLM-4.5 model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "glm-4.5-air".to_string(),
                        context_window: 131_072,
                        output_limit: 65_536,
                        description: "GLM-4.5 Air lightweight model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "glm-4.5-flash".to_string(),
                        context_window: 131_072,
                        output_limit: 65_536,
                        description: "GLM-4.5 Flash fast model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                ],
            },
            ProviderEntry {
                provider: ProviderId::Modal.as_str().to_string(),
                models: vec![ModelInfo {
                    name: "zai-org/GLM-5-FP8".to_string(),
                    context_window: 204_800,
                    output_limit: 65_536,
                    description: "GLM-5 on Modal (FP8 quantized)".to_string(),
                    deprecated: false,
                    deprecation_reason: None,
                    suggested_alternative: None,
                    capabilities: vec!["text".to_string(), "function_calling".to_string()],
                }],
            },
            ProviderEntry {
                provider: ProviderId::OpenAIChatGPT.as_str().to_string(),
                models: vec![
                    ModelInfo {
                        name: "gpt-5.3-codex".to_string(),
                        context_window: 400_000,
                        output_limit: 32_768,
                        description: "GPT-5.3 Codex - Latest coding model (ChatGPT Plus/Pro)".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "gpt-5.2-codex".to_string(),
                        context_window: 400_000,
                        output_limit: 32_768,
                        description: "GPT-5.2 Codex - Coding model (ChatGPT Plus/Pro)".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "gpt-5.1-codex-max".to_string(),
                        context_window: 400_000,
                        output_limit: 32_768,
                        description: "GPT-5.1 Codex Max - Full capability coding model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "gpt-5.1-codex-mini".to_string(),
                        context_window: 400_000,
                        output_limit: 32_768,
                        description: "GPT-5.1 Codex Mini - Fast coding model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "gpt-5.2".to_string(),
                        context_window: 400_000,
                        output_limit: 32_768,
                        description: "GPT-5.2 - Latest general model (ChatGPT Plus/Pro)".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "gpt-5.1".to_string(),
                        context_window: 400_000,
                        output_limit: 32_768,
                        description: "GPT-5.1 - General purpose model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "gpt-5".to_string(),
                        context_window: 200_000,
                        output_limit: 16_384,
                        description: "GPT-5 - Standard model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                    ModelInfo {
                        name: "gpt-5-nano".to_string(),
                        context_window: 128_000,
                        output_limit: 8_192,
                        description: "GPT-5 Nano - Fast, lightweight model".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "function_calling".to_string()],
                    },
                ],
            },
            ProviderEntry {
                provider: ProviderId::Gemini.as_str().to_string(),
                models: vec![
                    // Gemini 3 Series (Current Generation)
                    ModelInfo {
                        name: "gemini-3.1-pro-preview".to_string(),
                        context_window: 1_048_576,
                        output_limit: 65_536,
                        description: "Gemini 3.1 Pro Preview - 1M context, best for agentic workflows, coding, and reasoning".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "image".to_string(), "video".to_string(), "audio".to_string(), "pdf".to_string(), "function_calling".to_string(), "code_execution".to_string(), "thinking".to_string()],
                    },
                    ModelInfo {
                        name: "gemini-3-flash-preview".to_string(),
                        context_window: 1_048_576,
                        output_limit: 65_536,
                        description: "Gemini 3 Flash Preview - 1M context, fast and balanced for multimodal tasks".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "image".to_string(), "video".to_string(), "audio".to_string(), "pdf".to_string(), "function_calling".to_string(), "code_execution".to_string(), "thinking".to_string()],
                    },
                    // Deprecated Gemini 3 Models
                    ModelInfo {
                        name: "gemini-3-pro-preview".to_string(),
                        context_window: 1_048_576,
                        output_limit: 65_536,
                        description: "Gemini 3 Pro Preview - 1M context, complex reasoning".to_string(),
                        deprecated: true,
                        deprecation_reason: Some("Shut down March 9, 2026".to_string()),
                        suggested_alternative: Some("gemini-3.1-pro-preview".to_string()),
                        capabilities: vec!["text".to_string(), "image".to_string(), "video".to_string(), "audio".to_string(), "pdf".to_string(), "function_calling".to_string(), "code_execution".to_string(), "thinking".to_string()],
                    },
                    // Gemini 2.5 Series (Stable)
                    ModelInfo {
                        name: "gemini-2.5-pro".to_string(),
                        context_window: 1_048_576,
                        output_limit: 65_536,
                        description: "Gemini 2.5 Pro - 1M context, multimodal reasoning with thinking".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "image".to_string(), "video".to_string(), "audio".to_string(), "function_calling".to_string(), "code_execution".to_string(), "thinking".to_string()],
                    },
                    ModelInfo {
                        name: "gemini-2.5-flash".to_string(),
                        context_window: 1_048_576,
                        output_limit: 65_536,
                        description: "Gemini 2.5 Flash - 1M context, best price-performance for large scale processing".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "image".to_string(), "video".to_string(), "audio".to_string(), "function_calling".to_string(), "code_execution".to_string(), "thinking".to_string()],
                    },
                    ModelInfo {
                        name: "gemini-2.5-flash-lite".to_string(),
                        context_window: 1_048_576,
                        output_limit: 65_536,
                        description: "Gemini 2.5 Flash Lite - 1M context, cost-efficient for high-volume tasks".to_string(),
                        deprecated: false,
                        deprecation_reason: None,
                        suggested_alternative: None,
                        capabilities: vec!["text".to_string(), "image".to_string(), "video".to_string(), "audio".to_string(), "pdf".to_string(), "function_calling".to_string(), "code_execution".to_string(), "thinking".to_string()],
                    },
                    // Deprecated Gemini 2.0 Series
                    ModelInfo {
                        name: "gemini-2.0-flash".to_string(),
                        context_window: 1_048_576,
                        output_limit: 8_192,
                        description: "Gemini 2.0 Flash - Legacy model, 8K output limit".to_string(),
                        deprecated: true,
                        deprecation_reason: Some("Migrate to Gemini 2.5 Flash".to_string()),
                        suggested_alternative: Some("gemini-2.5-flash".to_string()),
                        capabilities: vec!["text".to_string(), "image".to_string(), "video".to_string(), "audio".to_string()],
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
