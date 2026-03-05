//! ChatGPT Client for ChatGPT Plus/Pro subscription access
//!
//! Provides access to GPT Codex models via the ChatGPT backend API
//! using OAuth authentication from ChatGPT subscriptions.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::core::tool::ToolDescriptor;
use crate::model::providers::openai_compat::{
    openai_response_to_worker_decision, process_openai_stream_line, OpenAiFunctionCall,
    OpenAiResponseMessage, OpenAiToolCall, OpenAiToolCallAccumulator,
};

/// Ensure schemas conform to what the Responses API accepts:
/// - object schemas must have a `properties` field (even if empty)
/// - `items` must always be a single schema object (not a string, array, etc.)
fn normalize_schema(schema: &serde_json::Value) -> serde_json::Value {
    match schema {
        serde_json::Value::Object(obj) => {
            let mut out = obj.clone();

            // object type must have properties
            if obj.get("type").and_then(|v| v.as_str()) == Some("object")
                && !obj.contains_key("properties")
            {
                out.insert(
                    "properties".to_string(),
                    serde_json::Value::Object(serde_json::Map::new()),
                );
            }

            // Recurse into properties values
            if let Some(serde_json::Value::Object(props)) = out.get("properties").cloned() {
                let normalized_props: serde_json::Map<String, serde_json::Value> = props
                    .into_iter()
                    .map(|(k, v)| (k, normalize_schema(&v)))
                    .collect();
                out.insert(
                    "properties".to_string(),
                    serde_json::Value::Object(normalized_props),
                );
            }

            // Normalize items: must be a schema object, not a string/array/etc.
            if let Some(items) = out.get("items").cloned() {
                let normalized_items = match &items {
                    // Already an object — recurse into it
                    serde_json::Value::Object(_) => normalize_schema(&items),
                    // Array of schemas (tuple validation) — not supported; use first or {}
                    serde_json::Value::Array(arr) => {
                        if let Some(first) = arr.first() {
                            normalize_schema(first)
                        } else {
                            serde_json::json!({ "type": "string" })
                        }
                    }
                    // Primitive type shorthand like "string" — wrap into {"type": <value>}
                    serde_json::Value::String(type_str) => {
                        serde_json::json!({ "type": type_str })
                    }
                    // Anything else — fall back to generic object
                    _ => serde_json::json!({}),
                };
                out.insert("items".to_string(), normalized_items);
            }

            serde_json::Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Sanitize a tool name for the Responses API (must match `^[a-zA-Z0-9_-]+$`).
/// Dots and other forbidden chars are replaced with underscores.
fn tool_name_to_wire(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Reverse wire name → canonical name by descriptor lookup.
fn tool_name_from_wire(wire: &str, descriptors: &[ToolDescriptor]) -> String {
    descriptors
        .iter()
        .find(|d| tool_name_to_wire(&d.name) == wire)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| wire.to_string())
}
use crate::model::shared::{preferred_response_text, worker_prompt_from_request};
use crate::model::{
    AgentModelClient, ModelError, StreamDelta, WorkerActionRequest, WorkerDecision,
};
use crate::runtime::plan_mode_settings::WORKER_MAX_TOKENS;

use super::oauth;

const CODEX_API_ENDPOINT: &str = "https://chatgpt.com/backend-api/codex/responses";

/// OAuth token storage for ChatGPT
#[derive(Debug, Clone)]
pub struct ChatGPTAuth {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub account_id: Option<String>,
}

impl ChatGPTAuth {
    /// Check if token is expired (with 30 second buffer)
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.expires_at - 30 < now
    }
}

/// ChatGPT client that uses OAuth tokens to access Codex models
pub struct ChatGPTClient {
    auth: Arc<RwLock<ChatGPTAuth>>,
    model: String,
    client: reqwest::Client,
}

impl ChatGPTClient {
    /// Create a new ChatGPT client with OAuth credentials
    pub fn new(
        access_token: String,
        refresh_token: String,
        expires_at: i64,
        account_id: Option<String>,
        model: Option<String>,
    ) -> Self {
        Self {
            auth: Arc::new(RwLock::new(ChatGPTAuth {
                access_token,
                refresh_token,
                expires_at,
                account_id,
            })),
            model: model.unwrap_or_else(|| "gpt-5.2-codex".to_string()),
            client: reqwest::Client::new(),
        }
    }

    pub fn from_api_key_payload(api_key: String, model: Option<String>) -> Self {
        if let Ok(auth) = serde_json::from_str::<serde_json::Value>(&api_key) {
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
            return Self::new(access_token, refresh_token, expires_at, account_id, model);
        }

        Self::new(api_key, String::new(), i64::MAX, None, model)
    }

    pub fn model_id(&self) -> String {
        self.model.clone()
    }

    fn build_request_body(
        &self,
        system: &str,
        user: &str,
        _max_tokens: u32,
        stream: bool,
        tools: Option<Vec<ToolDescriptor>>,
    ) -> Result<serde_json::Value, ModelError> {
        let tool_values = tools
            .unwrap_or_default()
            .into_iter()
            .map(|d| {
                serde_json::json!({
                    "type": "function",
                    "name": tool_name_to_wire(&d.name),
                    "description": d.description,
                    "parameters": normalize_schema(&d.input_schema),
                })
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "model": self.model,
            "instructions": system,
            "input": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "input_text",
                            "text": user,
                        }
                    ]
                }
            ],
            "tools": tool_values,
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "store": false,
            "stream": stream,
        }))
    }

    fn parse_response_value(
        value: &serde_json::Value,
    ) -> Result<OpenAiResponseMessage, ModelError> {
        if let Some(message) = value
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
        {
            return serde_json::from_value::<OpenAiResponseMessage>(message.clone())
                .map_err(|e| ModelError::InvalidResponse(format!("ChatGPT parse failed: {}", e)));
        }

        let output = value
            .get("output")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ModelError::InvalidResponse("missing output array".to_string()))?;

        let mut content = String::new();
        let mut tool_calls = Vec::new();

        for item in output {
            match item.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                "message" => {
                    if let Some(parts) = item.get("content").and_then(|v| v.as_array()) {
                        for part in parts {
                            if part.get("type").and_then(|v| v.as_str()) == Some("output_text") {
                                if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                                    content.push_str(text);
                                }
                            }
                        }
                    }
                }
                "function_call" => {
                    let name = item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if name.trim().is_empty() {
                        continue;
                    }
                    let arguments = item
                        .get("arguments")
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}")
                        .to_string();
                    tool_calls.push(OpenAiToolCall {
                        tool_type: "function".to_string(),
                        function: OpenAiFunctionCall { name, arguments },
                    });
                }
                _ => {}
            }
        }

        Ok(OpenAiResponseMessage {
            content: if content.is_empty() {
                None
            } else {
                Some(content)
            },
            reasoning_content: None,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
        })
    }

    /// Get current auth (refreshes if needed)
    async fn get_auth(&self) -> Result<ChatGPTAuth, ModelError> {
        let auth = self.auth.read().await.clone();

        if auth.is_expired() {
            // Need to refresh
            let mut auth_guard = self.auth.write().await;

            // Double-check after acquiring write lock
            if auth_guard.is_expired() {
                tracing::info!("Refreshing ChatGPT access token");

                let new_tokens = oauth::refresh_access_token(&auth_guard.refresh_token)
                    .await
                    .map_err(|e| ModelError::Auth(format!("Token refresh failed: {}", e)))?;

                let new_account_id = oauth::extract_account_id_from_tokens(&new_tokens)
                    .or_else(|| auth_guard.account_id.clone());

                *auth_guard = ChatGPTAuth {
                    access_token: new_tokens.access_token.clone(),
                    refresh_token: new_tokens.refresh_token,
                    expires_at: oauth::calculate_expires_at(new_tokens.expires_in.unwrap_or(3600)),
                    account_id: new_account_id,
                };
            }

            Ok(auth_guard.clone())
        } else {
            Ok(auth)
        }
    }

    /// Simple text completion without tools
    #[allow(dead_code)]
    pub async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String, ModelError> {
        let response = self.run_chat(system, user, max_tokens, None).await?;
        Ok(preferred_response_text(
            response.content,
            response.reasoning_content,
        ))
    }

    /// Run chat completion with optional tools
    #[allow(dead_code)]
    async fn run_chat(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        tools: Option<Vec<ToolDescriptor>>,
    ) -> Result<OpenAiResponseMessage, ModelError> {
        let auth = self.get_auth().await?;

        let body = self.build_request_body(system, user, max_tokens, false, tools)?;

        let mut request = self
            .client
            .post(CODEX_API_ENDPOINT)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", auth.access_token))
            .header("originator", "orchestrix")
            .json(&body);

        // Add account ID header for organization subscriptions
        if let Some(account_id) = &auth.account_id {
            request = request.header("ChatGPT-Account-Id", account_id);
        }

        let response = request
            .send()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        tracing::debug!("ChatGPT API response: status={}, body={}", status, text);

        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ModelError::Auth(format!(
                "ChatGPT auth failed ({}). Your subscription may have expired or been revoked. Response: {}",
                status, text
            )));
        }

        if !status.is_success() {
            return Err(ModelError::Request(format!(
                "ChatGPT error {}: {}",
                status, text
            )));
        }

        let parsed = serde_json::from_str::<serde_json::Value>(&text)
            .map_err(|e| ModelError::InvalidResponse(format!("ChatGPT parse failed: {}", e)))?;
        Self::parse_response_value(&parsed)
    }

    /// Streaming chat completion
    async fn run_chat_streaming(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        tools: Option<Vec<ToolDescriptor>>,
        on_delta: &mut (dyn FnMut(StreamDelta) -> Result<(), String> + Send),
    ) -> Result<OpenAiResponseMessage, ModelError> {
        let auth = self.get_auth().await?;

        let body = self.build_request_body(system, user, max_tokens, true, tools)?;

        let mut request = self
            .client
            .post(CODEX_API_ENDPOINT)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", auth.access_token))
            .header("originator", "orchestrix")
            .json(&body);

        if let Some(account_id) = &auth.account_id {
            request = request.header("ChatGPT-Account-Id", account_id);
        }

        let response = request
            .send()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        let status = response.status();

        if status.as_u16() == 401 || status.as_u16() == 403 {
            let text = response.text().await.unwrap_or_default();
            return Err(ModelError::Auth(format!(
                "ChatGPT auth failed ({}). Response: {}",
                status, text
            )));
        }

        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ModelError::Request(format!(
                "ChatGPT error {}: {}",
                status, text
            )));
        }

        // Process streaming response
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut tool_call_accumulators: Vec<OpenAiToolCallAccumulator> = Vec::new();
        let mut full_tool_calls: Vec<OpenAiToolCall> = Vec::new();
        let mut saw_content_delta = false;
        let mut completed_response: Option<serde_json::Value> = None;

        use futures::StreamExt;
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut done = false;

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| ModelError::Request(e.to_string()))?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(newline_idx) = buffer.find('\n') {
                let mut line = buffer[..newline_idx].to_string();
                if line.ends_with('\r') {
                    line.pop();
                }
                buffer.drain(..=newline_idx);

                let trimmed = line.trim();
                if trimmed.starts_with("data:") {
                    let payload = trimmed.trim_start_matches("data:").trim();
                    if payload == "[DONE]" {
                        done = true;
                        break;
                    }
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) {
                        if value.get("type").and_then(|v| v.as_str())
                            == Some("response.output_text.delta")
                        {
                            if let Some(delta) = value.get("delta").and_then(|v| v.as_str()) {
                                if !delta.is_empty() {
                                    content.push_str(delta);
                                    on_delta(StreamDelta::Content(delta.to_string()))
                                        .map_err(ModelError::Request)?;
                                }
                            }
                            continue;
                        }

                        if value.get("type").and_then(|v| v.as_str()) == Some("response.completed")
                        {
                            if let Some(resp) = value.get("response") {
                                completed_response = Some(resp.clone());
                                done = true;
                                break;
                            }
                        }
                    }
                }

                if process_openai_stream_line(
                    &line,
                    &mut content,
                    &mut reasoning,
                    &mut tool_call_accumulators,
                    &mut full_tool_calls,
                    &mut saw_content_delta,
                    on_delta,
                )? {
                    done = true;
                    break;
                }
            }

            if done {
                break;
            }
        }

        if let Some(response_value) = completed_response {
            return Self::parse_response_value(&response_value);
        }

        if !done && !buffer.trim().is_empty() {
            let _ = process_openai_stream_line(
                buffer.trim_end_matches('\r'),
                &mut content,
                &mut reasoning,
                &mut tool_call_accumulators,
                &mut full_tool_calls,
                &mut saw_content_delta,
                on_delta,
            )?;
        }

        let tool_calls = if !tool_call_accumulators.is_empty() {
            Some(
                tool_call_accumulators
                    .into_iter()
                    .filter_map(|entry| {
                        if entry.function_name.trim().is_empty() {
                            return None;
                        }
                        Some(OpenAiToolCall {
                            tool_type: if entry.tool_type.trim().is_empty() {
                                "function".to_string()
                            } else {
                                entry.tool_type
                            },
                            function: crate::model::providers::openai_compat::OpenAiFunctionCall {
                                name: entry.function_name,
                                arguments: entry.arguments,
                            },
                        })
                    })
                    .collect(),
            )
        } else if !full_tool_calls.is_empty() {
            Some(full_tool_calls)
        } else {
            None
        };

        Ok(OpenAiResponseMessage {
            content: if content.is_empty() {
                None
            } else {
                Some(content)
            },
            reasoning_content: if reasoning.is_empty() {
                None
            } else {
                Some(reasoning)
            },
            tool_calls,
        })
    }

    /// Decide action with streaming
    pub async fn decide_action_streaming<F>(
        &self,
        req: WorkerActionRequest,
        mut on_delta: F,
    ) -> Result<WorkerDecision, ModelError>
    where
        F: FnMut(StreamDelta) -> Result<(), String> + Send,
    {
        let (system, user) = worker_prompt_from_request(&req)?;
        let tools_arg = if req.tool_descriptors.is_empty() {
            None
        } else {
            Some(req.tool_descriptors.clone())
        };
        let max_tokens = req.max_tokens.unwrap_or(WORKER_MAX_TOKENS);

        let response = self
            .run_chat_streaming(&system, &user, max_tokens, tools_arg, &mut on_delta)
            .await?;

        tracing::debug!(
            "ChatGPT worker response - content: {:?}, tool_calls: {:?}",
            response.content,
            response.tool_calls
        );

        Ok(openai_response_to_worker_decision(
            response,
            &req.tool_descriptors,
            &|name, descriptors| tool_name_from_wire(name, descriptors),
        ))
    }
}

impl AgentModelClient for ChatGPTClient {
    fn model_id(&self) -> String {
        self.model.clone()
    }

    async fn decide_action(&self, req: WorkerActionRequest) -> Result<WorkerDecision, ModelError> {
        let noop = |_delta: StreamDelta| Ok::<(), String>(());
        self.decide_action_streaming(req, noop).await
    }
}
