use std::sync::Arc;

use crate::core::tool::ToolDescriptor;
use crate::model::providers::openai_compat::{OpenAiCompatClient, OpenAiCompatClientConfig};
use crate::model::{
    AgentModelClient, ModelError, StreamDelta, WorkerActionRequest, WorkerDecision,
};

const DEFAULT_KIMI_BASE_URL: &str = "https://api.kimi.com/coding/v1";
const DEFAULT_KIMI_MODEL: &str = "kimi-for-coding";

pub struct KimiClient(OpenAiCompatClient);

/// Kimi rejects tool names that contain dots (must match `[a-zA-Z][a-zA-Z0-9_-]*`).
/// Convert dots to underscores before sending to the API.
#[inline]
fn tool_name_to_kimi(name: &str) -> String {
    name.replace('.', "_")
}

/// Reverse a Kimi-sanitised tool name back to its canonical form by looking it
/// up in the original descriptor list. This is the safe approach because naively
/// replacing all underscores with dots would corrupt MCP tool names that contain
/// underscores in the server or tool name components.
fn tool_name_from_kimi_with_lookup(kimi_name: &str, descriptors: &[ToolDescriptor]) -> String {
    descriptors
        .iter()
        .find(|d| tool_name_to_kimi(&d.name) == kimi_name)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| kimi_name.to_string())
}

fn normalize_kimi_base_url(base_url: String) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        return trimmed.to_string();
    }
    if trimmed.ends_with("/coding") {
        return format!("{}/v1", trimmed);
    }
    if trimmed == "https://api.kimi.com" {
        return format!("{}/v1", trimmed);
    }
    trimmed.to_string()
}

impl KimiClient {
    /// Create a new Kimi client.
    ///
    /// # Arguments
    /// * `api_key` - Your Kimi API key (format: sk-kimi-xxxxxxxx...)
    /// * `model` - Model name (e.g., "kimi-for-coding", "kimi-k2.5", "kimi-k2")
    /// * `base_url` - API base URL (default: "https://api.kimi.com/coding/v1")
    pub fn new(api_key: String, model: Option<String>, base_url: Option<String>) -> Self {
        let resolved_model = model.unwrap_or_else(|| DEFAULT_KIMI_MODEL.to_string());
        let resolved_base_url =
            normalize_kimi_base_url(base_url.unwrap_or_else(|| DEFAULT_KIMI_BASE_URL.to_string()));

        let tool_name_to_wire: Arc<dyn Fn(&str) -> String + Send + Sync> =
            Arc::new(tool_name_to_kimi);

        let tool_name_from_wire: Arc<dyn Fn(&str, &[ToolDescriptor]) -> String + Send + Sync> =
            Arc::new(tool_name_from_kimi_with_lookup);

        let config = OpenAiCompatClientConfig {
            tool_name_to_wire,
            tool_name_from_wire,
            schema_filter: Arc::new(|schema| schema.clone()), // No schema filtering needed
            extra_headers: vec![("User-Agent", "KimiCLI/0.77".to_string())],
            parallel_tool_calls: true,
            retry_on_rate_limit: false, // Kimi doesn't have the same rate limit issues
            retry_on_invalid_param_1210: false,
            fallback_model: None,
            max_tokens_cap: None,
        };

        Self(OpenAiCompatClient::new(
            api_key,
            Some(resolved_model),
            Some(resolved_base_url),
            "Kimi",
            DEFAULT_KIMI_MODEL,
            config,
        ))
    }

    pub fn model_id(&self) -> String {
        self.0.model_id()
    }

    /// Simple text completion without tools - useful for summarization and other single-turn tasks.
    #[allow(dead_code)]
    pub async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String, ModelError> {
        self.0.complete(system, user, max_tokens).await
    }

    /// Single-turn plan generation; used by integration tests. Production plan mode uses
    /// multi-turn run_multi_turn_planning in the runtime planner.
    #[allow(dead_code)]
    pub async fn generate_plan_markdown(
        &self,
        task_prompt: &str,
        prior_markdown_context: &str,
        tool_descriptors: Vec<ToolDescriptor>,
    ) -> Result<String, ModelError> {
        self.0
            .generate_plan_markdown(task_prompt, prior_markdown_context, tool_descriptors)
            .await
    }

    pub async fn decide_action_streaming<F>(
        &self,
        req: WorkerActionRequest,
        on_delta: F,
    ) -> Result<WorkerDecision, ModelError>
    where
        F: FnMut(StreamDelta) -> Result<(), String> + Send,
    {
        self.0.decide_action_streaming(req, on_delta).await
    }
}

impl AgentModelClient for KimiClient {
    fn model_id(&self) -> String {
        self.model_id()
    }

    async fn decide_action(&self, req: WorkerActionRequest) -> Result<WorkerDecision, ModelError> {
        let noop = |_delta: StreamDelta| Ok::<(), String>(());
        self.decide_action_streaming(req, noop).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name_encoding() {
        assert_eq!(
            tool_name_to_kimi("agent.create_artifact"),
            "agent_create_artifact"
        );
        assert_eq!(tool_name_to_kimi("fs.read"), "fs_read");
    }

    #[test]
    fn tool_name_decoding_with_lookup() {
        let descriptors = vec![
            ToolDescriptor {
                name: "agent.create_artifact".to_string(),
                description: "Create an artifact".to_string(),
                input_schema: serde_json::json!({}),
            },
            ToolDescriptor {
                name: "fs.read".to_string(),
                description: "Read a file".to_string(),
                input_schema: serde_json::json!({}),
            },
        ];

        // Should find the original name via lookup
        assert_eq!(
            tool_name_from_kimi_with_lookup("agent_create_artifact", &descriptors),
            "agent.create_artifact"
        );

        // Unknown names should pass through
        assert_eq!(
            tool_name_from_kimi_with_lookup("unknown_tool", &descriptors),
            "unknown_tool"
        );
    }

    #[test]
    fn base_url_normalization() {
        assert_eq!(
            normalize_kimi_base_url("https://api.kimi.com/coding".to_string()),
            "https://api.kimi.com/coding/v1"
        );
        assert_eq!(
            normalize_kimi_base_url("https://api.kimi.com/v1".to_string()),
            "https://api.kimi.com/v1"
        );
        assert_eq!(
            normalize_kimi_base_url("https://api.kimi.com".to_string()),
            "https://api.kimi.com/v1"
        );
    }
}
