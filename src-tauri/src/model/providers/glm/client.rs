use std::sync::Arc;

use crate::core::tool::ToolDescriptor;
use crate::model::providers::openai_compat::{OpenAiCompatClient, OpenAiCompatClientConfig};
use crate::model::{
    AgentModelClient, ModelError, StreamDelta, WorkerActionRequest, WorkerDecision,
};

const DEFAULT_GLM_BASE_URL: &str = "https://api.z.ai/api/coding/paas/v4";
const DEFAULT_GLM_MODEL: &str = "glm-4.7";
const GLM_MAX_OUTPUT_TOKENS: u32 = 131_072;

pub struct GlmClient(OpenAiCompatClient);

/// Filter JSON schema to only include properties that GLM accepts.
/// GLM's API has strict validation (additionalProperties: false) and rejects
/// extra fields like `$schema` or `additionalProperties`.
fn filter_schema_for_glm(schema: &serde_json::Value) -> serde_json::Value {
    match schema {
        serde_json::Value::Object(obj) => {
            let mut filtered = serde_json::Map::new();
            // Only include standard JSON Schema fields that GLM accepts
            let allowed_keys = [
                "type",
                "properties",
                "required",
                "description",
                "enum",
                "items",
                "anyOf",
                "oneOf",
                "allOf",
                "minimum",
                "maximum",
                "minLength",
                "maxLength",
                "pattern",
                "default",
                "title",
            ];
            for key in allowed_keys.iter() {
                if let Some(val) = obj.get(*key) {
                    if key == &"properties" {
                        // Recursively filter nested property schemas
                        if let serde_json::Value::Object(props) = val {
                            let mut filtered_props = serde_json::Map::new();
                            for (k, v) in props.iter() {
                                filtered_props.insert(k.clone(), filter_schema_for_glm(v));
                            }
                            filtered
                                .insert(key.to_string(), serde_json::Value::Object(filtered_props));
                        } else {
                            filtered.insert(key.to_string(), val.clone());
                        }
                    } else if key == &"items" {
                        // Recursively filter items schema
                        filtered.insert(key.to_string(), filter_schema_for_glm(val));
                    } else {
                        filtered.insert(key.to_string(), val.clone());
                    }
                }
            }
            serde_json::Value::Object(filtered)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(|v| filter_schema_for_glm(v)).collect())
        }
        other => other.clone(),
    }
}

fn encode_tool_name_for_glm(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push_str(&format!("_x{:X}_", ch as u32));
        }
    }
    out
}

fn decode_tool_name_from_glm(name: &str) -> String {
    let bytes = name.as_bytes();
    let mut i = 0usize;
    let mut out = String::with_capacity(name.len());

    while i < bytes.len() {
        if bytes[i] == b'_' && i + 3 < bytes.len() && bytes[i + 1] == b'x' {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j] != b'_' {
                j += 1;
            }
            if j < bytes.len() {
                let hex = &name[i + 2..j];
                if let Ok(codepoint) = u32::from_str_radix(hex, 16) {
                    if let Some(ch) = char::from_u32(codepoint) {
                        out.push(ch);
                        i = j + 1;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }

    out
}

impl GlmClient {
    pub fn new(api_key: String, model: Option<String>, base_url: Option<String>) -> Self {
        let resolved_base_url = base_url
            .map(|u| u.trim().trim_end_matches('/').to_string())
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| DEFAULT_GLM_BASE_URL.to_string());

        let is_coding_endpoint = resolved_base_url
            .to_ascii_lowercase()
            .contains("/api/coding/paas/v4");

        let resolved_model = model
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            // Z.AI API expects lowercase model names (glm-4.7, not GLM-4.7)
            .map(|m| m.to_ascii_lowercase())
            .unwrap_or_else(|| {
                if is_coding_endpoint {
                    "glm-4.7".to_string()
                } else {
                    "glm-5".to_string()
                }
            });

        let fallback_model = if !resolved_model.eq_ignore_ascii_case("glm-4.7") {
            Some("glm-4.7".to_string())
        } else {
            None
        };

        let schema_filter: Arc<dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync> =
            Arc::new(filter_schema_for_glm);

        let tool_name_to_wire: Arc<dyn Fn(&str) -> String + Send + Sync> =
            Arc::new(encode_tool_name_for_glm);

        let tool_name_from_wire: Arc<dyn Fn(&str, &[ToolDescriptor]) -> String + Send + Sync> =
            Arc::new(|name, _| decode_tool_name_from_glm(name));

        let config = OpenAiCompatClientConfig {
            tool_name_to_wire,
            tool_name_from_wire,
            schema_filter,
            extra_headers: Vec::new(),
            parallel_tool_calls: false, // GLM doesn't support parallel_tool_calls
            retry_on_rate_limit: true,
            retry_on_invalid_param_1210: true,
            fallback_model,
            max_tokens_cap: Some(GLM_MAX_OUTPUT_TOKENS),
        };

        Self(OpenAiCompatClient::new(
            api_key,
            Some(resolved_model),
            Some(resolved_base_url),
            "GLM",
            DEFAULT_GLM_MODEL,
            config,
        ))
    }

    pub fn model_id(&self) -> String {
        self.0.model_id()
    }

    #[allow(dead_code)]
    pub async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String, ModelError> {
        self.0.complete(system, user, max_tokens).await
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
}

impl AgentModelClient for GlmClient {
    fn model_id(&self) -> String {
        self.model_id()
    }

    async fn decide_action(&self, req: WorkerActionRequest) -> Result<WorkerDecision, ModelError> {
        let noop = |_delta: StreamDelta| Ok::<(), String>(());
        self.decide_action_streaming(req, noop).await
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn normalize_max_tokens_caps_at_glm_limit() {
        let client = GlmClient::new(
            "test-key".to_string(),
            Some("glm-4.7".to_string()),
            Some("https://api.z.ai/api/coding/paas/v4".to_string()),
        );

        // Values within limit pass through unchanged
        assert_eq!(client.0.normalize_max_tokens(8_192), 8_192);
        assert_eq!(client.0.normalize_max_tokens(25_000), 25_000);
        assert_eq!(client.0.normalize_max_tokens(131_072), 131_072);
        // Values above GLM_MAX_OUTPUT_TOKENS are capped
        assert_eq!(
            client.0.normalize_max_tokens(180_000),
            GLM_MAX_OUTPUT_TOKENS
        );
        assert_eq!(
            client.0.normalize_max_tokens(200_000),
            GLM_MAX_OUTPUT_TOKENS
        );
    }

    #[test]
    fn model_name_normalized_to_lowercase() {
        let client = GlmClient::new("test-key".to_string(), Some("GLM-4.7".to_string()), None);
        assert_eq!(client.model_id(), "glm-4.7");

        let client2 = GlmClient::new("test-key".to_string(), Some("GLM-5".to_string()), None);
        assert_eq!(client2.model_id(), "glm-5");
    }

    #[test]
    fn base_url_trailing_slash_stripped() {
        let client = GlmClient::new(
            "test-key".to_string(),
            None,
            Some("https://api.z.ai/api/coding/paas/v4/".to_string()),
        );
        assert_eq!(client.0.base_url, "https://api.z.ai/api/coding/paas/v4");
    }

    #[test]
    fn tool_name_encoding_roundtrip() {
        let original = "agent.create_artifact";
        let encoded = encode_tool_name_for_glm(original);
        let decoded = decode_tool_name_from_glm(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn schema_filter_removes_disallowed_fields() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "$schema": "http://json-schema.org/draft-07/schema#",
            "additionalProperties": false
        });

        let filtered = filter_schema_for_glm(&schema);
        let obj = filtered.as_object().unwrap();

        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("properties"));
        assert!(!obj.contains_key("$schema"));
        assert!(!obj.contains_key("additionalProperties"));
    }
}
