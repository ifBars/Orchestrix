//! Traits for model clients.

use crate::model::types::{ModelError, WorkerActionRequest, WorkerDecision};

/// Core trait for agent model clients.
/// Implemented by provider-specific clients (MiniMaxClient, KimiClient, etc.).
#[allow(async_fn_in_trait)]
pub trait AgentModelClient: Send + Sync {
    fn model_id(&self) -> String;
    async fn decide_action(&self, req: WorkerActionRequest) -> Result<WorkerDecision, ModelError>;
}
