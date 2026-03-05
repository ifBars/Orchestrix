//! Worker model client abstraction.
//!
//! Provides a unified interface for different LLM providers (MiniMax, Kimi, GLM)
//! used by the worker during step execution.

pub use crate::model::StreamDelta;
use crate::model::{
    ChatGPTClient, GeminiClient, GlmClient, KimiClient, MiniMaxClient, ModalClient,
    WorkerActionRequest, WorkerDecision,
};

/// Runtime model configuration for worker execution.
#[derive(Debug, Clone)]
pub struct RuntimeModelConfig {
    pub provider: String,
    pub api_key: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

/// Unified model client for worker execution.
pub enum WorkerModelClient {
    MiniMax(MiniMaxClient),
    Kimi(KimiClient),
    Glm(GlmClient),
    Modal(ModalClient),
    Gemini(GeminiClient),
    ChatGPT(ChatGPTClient),
}

impl WorkerModelClient {
    /// Create a new model client from runtime configuration.
    pub fn from_config(config: &RuntimeModelConfig) -> Self {
        match config.provider.as_str() {
            "kimi" => Self::Kimi(KimiClient::new(
                config.api_key.clone(),
                config.model.clone(),
                config.base_url.clone(),
            )),
            "zhipu" | "glm" => Self::Glm(GlmClient::new(
                config.api_key.clone(),
                config.model.clone(),
                config.base_url.clone(),
            )),
            "modal" => Self::Modal(ModalClient::new(
                config.api_key.clone(),
                config.model.clone(),
                config.base_url.clone(),
            )),
            "gemini" => Self::Gemini(GeminiClient::new(
                config.api_key.clone(),
                config.model.clone(),
                config.base_url.clone(),
            )),
            "openai-chatgpt" | "chatgpt" => Self::ChatGPT(ChatGPTClient::from_api_key_payload(
                config.api_key.clone(),
                config.model.clone(),
            )),
            _ => Self::MiniMax(MiniMaxClient::new_with_base_url(
                config.api_key.clone(),
                config.model.clone(),
                config.base_url.clone(),
            )),
        }
    }

    /// Request a decision and stream text deltas as the provider responds.
    pub async fn decide_streaming<F>(
        &self,
        req: WorkerActionRequest,
        mut on_delta: F,
    ) -> Result<WorkerDecision, String>
    where
        F: FnMut(StreamDelta) -> Result<(), String> + Send,
    {
        match self {
            Self::MiniMax(model) => model
                .decide_action_streaming(req, |delta| on_delta(delta))
                .await
                .map_err(|e| e.to_string()),
            Self::Kimi(model) => model
                .decide_action_streaming(req, |delta| on_delta(delta))
                .await
                .map_err(|e| e.to_string()),
            Self::Glm(model) => model
                .decide_action_streaming(req, |delta| on_delta(delta))
                .await
                .map_err(|e| e.to_string()),
            Self::Modal(model) => model
                .decide_action_streaming(req, |delta| on_delta(delta))
                .await
                .map_err(|e| e.to_string()),
            Self::Gemini(model) => model
                .decide_action_streaming(req, |delta| on_delta(delta))
                .await
                .map_err(|e| e.to_string()),
            Self::ChatGPT(model) => model
                .decide_action_streaming(req, |delta| on_delta(delta))
                .await
                .map_err(|e| e.to_string()),
        }
    }
}
