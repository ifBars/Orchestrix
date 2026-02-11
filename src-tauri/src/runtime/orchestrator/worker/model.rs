//! Worker model client abstraction.
//!
//! Provides a unified interface for different LLM providers (MiniMax, Kimi)
//! used by the worker during step execution.

use crate::model::{
    kimi::KimiPlanner,
    minimax::MiniMaxPlanner,
    PlannerModel,
    WorkerActionRequest,
    WorkerDecision,
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
    MiniMax(MiniMaxPlanner),
    Kimi(KimiPlanner),
}

impl WorkerModelClient {
    /// Create a new model client from runtime configuration.
    pub fn from_config(config: &RuntimeModelConfig) -> Self {
        if config.provider == "kimi" {
            Self::Kimi(KimiPlanner::new(
                config.api_key.clone(),
                config.model.clone(),
                config.base_url.clone(),
            ))
        } else {
            Self::MiniMax(MiniMaxPlanner::new_with_base_url(
                config.api_key.clone(),
                config.model.clone(),
                config.base_url.clone(),
            ))
        }
    }

    /// Request a decision from the model.
    pub async fn decide(&self, req: WorkerActionRequest) -> Result<WorkerDecision, String> {
        match self {
            Self::MiniMax(model) => model.decide_worker_action(req).await.map_err(|e| e.to_string()),
            Self::Kimi(model) => model.decide_worker_action(req).await.map_err(|e| e.to_string()),
        }
    }
}
