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

impl From<&super::super::RuntimeModelConfig> for RuntimeModelConfig {
    fn from(config: &super::super::RuntimeModelConfig) -> Self {
        Self {
            provider: config.provider.clone(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            base_url: config.base_url.clone(),
        }
    }
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
            "openai-chatgpt" | "chatgpt" => {
                // api_key field carries JSON-encoded OAuth data: {access_token, refresh_token, expires_at, account_id}
                if let Ok(auth) = serde_json::from_str::<serde_json::Value>(&config.api_key) {
                    let access_token = auth
                        .get("access_token")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let refresh_token = auth
                        .get("refresh_token")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let expires_at = auth.get("expires_at").and_then(|v| v.as_i64()).unwrap_or(0);
                    let account_id = auth
                        .get("account_id")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    Self::ChatGPT(ChatGPTClient::new(
                        access_token,
                        refresh_token,
                        expires_at,
                        account_id,
                        config.model.clone(),
                    ))
                } else {
                    // Fallback: treat api_key as a direct Bearer token (no refresh)
                    Self::ChatGPT(ChatGPTClient::new(
                        config.api_key.clone(),
                        String::new(),
                        i64::MAX,
                        None,
                        config.model.clone(),
                    ))
                }
            }
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
