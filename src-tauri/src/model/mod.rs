pub mod kimi;
pub mod minimax;
mod shared;

pub(crate) use shared::strip_tool_call_markup;

use crate::core::tool::ToolDescriptor;

#[derive(Debug, Clone)]
pub struct WorkerActionRequest {
    pub task_prompt: String,
    pub goal_summary: String,
    pub context: String,
    pub available_tools: Vec<String>,
    /// Detailed tool reference with schemas, for inclusion in the prompt.
    /// If empty, falls back to the tool name list.
    pub tool_descriptions: String,
    /// Structured tool descriptors for providers with native function calling.
    pub tool_descriptors: Vec<ToolDescriptor>,
    pub prior_observations: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerToolCall {
    pub tool_name: String,
    pub tool_args: serde_json::Value,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum WorkerAction {
    ToolCall {
        tool_name: String,
        tool_args: serde_json::Value,
        rationale: Option<String>,
    },
    ToolCalls {
        calls: Vec<WorkerToolCall>,
    },
    Delegate {
        objective: String,
    },
    Complete {
        summary: String,
    },
}

/// Wraps a `WorkerAction` together with optional model reasoning/thinking content.
/// The reasoning comes from the model's chain-of-thought (e.g. MiniMax `reasoning_content`)
/// and should be forwarded to the UI separately from the action itself.
#[derive(Debug, Clone)]
pub struct WorkerDecision {
    pub action: WorkerAction,
    /// Model's chain-of-thought reasoning, if the provider returned it.
    pub reasoning: Option<String>,
    /// Raw response from the provider for debugging purposes.
    pub raw_response: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("auth error: {0}")]
    Auth(String),
}

#[allow(async_fn_in_trait)]
pub trait PlannerModel: Send + Sync {
    fn model_id(&self) -> &'static str;
    async fn decide_worker_action(
        &self,
        req: WorkerActionRequest,
    ) -> Result<WorkerDecision, ModelError>;
}
