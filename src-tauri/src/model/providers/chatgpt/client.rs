//! ChatGPT Client for ChatGPT Plus/Pro subscription access
//!
//! Provides access to GPT Codex models via the ChatGPT backend API
//! using OAuth authentication from ChatGPT subscriptions.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::core::tool::ToolDescriptor;
use crate::model::shared::{
    preferred_response_text, strip_tool_call_markup, worker_system_prompt, worker_user_prompt,
};
use crate::model::{
    AgentModelClient, ModelError, StreamDelta, WorkerAction, WorkerActionRequest, WorkerDecision,
    WorkerToolCall,
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

    pub fn model_id(&self) -> String {
        self.model.clone()
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

        let body = OpenAiChatRequest {
            model: self.model.clone(),
            messages: vec![
                OpenAiRequestMessage {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                OpenAiRequestMessage {
                    role: "user".to_string(),
                    content: user.to_string(),
                },
            ],
            temperature: 0.1,
            max_tokens,
            stream: false,
            tools: tools.map(|t| t.into_iter().map(Into::into).collect()),
            tool_choice: None,
            parallel_tool_calls: None,
        };

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

        let parsed: OpenAiChatResponse = serde_json::from_str(&text)
            .map_err(|e| ModelError::InvalidResponse(format!("ChatGPT parse failed: {}", e)))?;

        parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message)
            .ok_or_else(|| {
                ModelError::InvalidResponse(
                    "missing choices[0].message from ChatGPT response".to_string(),
                )
            })
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

        let body = OpenAiChatRequest {
            model: self.model.clone(),
            messages: vec![
                OpenAiRequestMessage {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                OpenAiRequestMessage {
                    role: "user".to_string(),
                    content: user.to_string(),
                },
            ],
            temperature: 0.1,
            max_tokens,
            stream: true,
            tools: tools.map(|t| t.into_iter().map(Into::into).collect()),
            tool_choice: None,
            parallel_tool_calls: None,
        };

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

                if process_stream_line(
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

        if !done && !buffer.trim().is_empty() {
            let _ = process_stream_line(
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
                            function: OpenAiFunctionCall {
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
        let history_text = if req.prior_observations.is_empty() {
            "(none yet)".to_string()
        } else {
            serde_json::to_string(&req.prior_observations)
                .map_err(|e| ModelError::InvalidResponse(e.to_string()))?
        };

        let user = worker_user_prompt(
            &req.task_prompt,
            &req.goal_summary,
            &req.context,
            &req.available_tools,
            Some(&history_text),
        );

        let system = worker_system_prompt();
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

        // Process tool calls
        if let Some(tool_calls) = response.tool_calls.as_ref() {
            if !tool_calls.is_empty() {
                let mut calls = Vec::with_capacity(tool_calls.len());
                for call in tool_calls {
                    if call.tool_type != "function" {
                        continue;
                    }
                    let args_json =
                        serde_json::from_str::<serde_json::Value>(&call.function.arguments)
                            .unwrap_or_else(|_| serde_json::json!({}));
                    calls.push(WorkerToolCall {
                        tool_name: call.function.name.clone(),
                        tool_args: args_json,
                        rationale: None,
                    });
                }

                if !calls.is_empty() {
                    let raw_response = serde_json::to_string(&response).ok();
                    return Ok(WorkerDecision {
                        action: WorkerAction::ToolCalls { calls },
                        reasoning: response.reasoning_content.clone(),
                        raw_response,
                    });
                }
            }
        }

        let raw_response = serde_json::to_string(&response).ok();
        let raw = if response.content.as_deref().unwrap_or("").trim().is_empty() {
            response.reasoning_content.unwrap_or_default()
        } else {
            response.content.unwrap_or_default()
        };
        let summary = strip_tool_call_markup(raw.trim()).trim().to_string();

        Ok(WorkerDecision {
            action: WorkerAction::Complete {
                summary: if summary.is_empty() {
                    "Task complete.".to_string()
                } else {
                    summary
                },
            },
            reasoning: None,
            raw_response,
        })
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

// Data structures for OpenAI-compatible API

#[derive(Debug, serde::Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiRequestMessage>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parallel_tool_calls: Option<bool>,
}

#[derive(Debug, serde::Serialize)]
struct OpenAiRequestMessage {
    role: String,
    content: String,
}

#[derive(Debug, serde::Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunction,
}

#[derive(Debug, serde::Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

impl From<ToolDescriptor> for OpenAiTool {
    fn from(descriptor: ToolDescriptor) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: OpenAiFunction {
                name: descriptor.name,
                description: descriptor.description,
                parameters: descriptor.input_schema,
            },
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
struct OpenAiResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct OpenAiToolCall {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunctionCall,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Default)]
struct OpenAiToolCallAccumulator {
    tool_type: String,
    function_name: String,
    arguments: String,
}

#[derive(Debug, serde::Deserialize)]
struct OpenAiStreamChunk {
    #[serde(default)]
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Debug, serde::Deserialize)]
struct OpenAiStreamChoice {
    #[serde(default)]
    delta: Option<OpenAiStreamDelta>,
    #[serde(default)]
    message: Option<OpenAiResponseMessage>,
}

#[derive(Debug, serde::Deserialize, Default)]
struct OpenAiStreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCallDelta>>,
}

#[derive(Debug, serde::Deserialize)]
struct OpenAiToolCallDelta {
    #[serde(default)]
    index: Option<usize>,
    #[serde(rename = "type", default)]
    tool_type: Option<String>,
    #[serde(default)]
    function: Option<OpenAiFunctionCallDelta>,
}

#[derive(Debug, serde::Deserialize, Default)]
struct OpenAiFunctionCallDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

/// Process a single SSE line from streaming response
fn process_stream_line(
    line: &str,
    content: &mut String,
    reasoning: &mut String,
    tool_call_accumulators: &mut Vec<OpenAiToolCallAccumulator>,
    full_tool_calls: &mut Vec<OpenAiToolCall>,
    saw_content_delta: &mut bool,
    on_delta: &mut dyn FnMut(StreamDelta) -> Result<(), String>,
) -> Result<bool, ModelError> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with(':') || trimmed.starts_with("event:") {
        return Ok(false);
    }

    let payload = trimmed
        .strip_prefix("data:")
        .map(|s| s.trim())
        .unwrap_or(trimmed);

    if payload.is_empty() || payload == "[DONE]" {
        return Ok(payload == "[DONE]");
    }

    let chunk: OpenAiStreamChunk = match serde_json::from_str(payload) {
        Ok(v) => v,
        Err(_) => return Ok(false),
    };

    for choice in chunk.choices {
        if let Some(delta) = choice.delta {
            if let Some(delta_content) = delta.content {
                if !delta_content.is_empty() {
                    *saw_content_delta = true;
                    content.push_str(&delta_content);
                    on_delta(StreamDelta::Content(delta_content)).map_err(ModelError::Request)?;
                }
            }

            if let Some(delta_reasoning) = delta.reasoning_content {
                if !delta_reasoning.is_empty() {
                    reasoning.push_str(&delta_reasoning);
                    on_delta(StreamDelta::Reasoning(delta_reasoning))
                        .map_err(ModelError::Request)?;
                }
            }

            if let Some(tool_calls) = delta.tool_calls {
                for call in tool_calls {
                    let idx = call.index.unwrap_or(0);
                    if tool_call_accumulators.len() <= idx {
                        tool_call_accumulators
                            .resize_with(idx + 1, OpenAiToolCallAccumulator::default);
                    }
                    let entry = &mut tool_call_accumulators[idx];
                    if let Some(tool_type) = call.tool_type {
                        if !tool_type.is_empty() {
                            entry.tool_type = tool_type;
                        }
                    }
                    if let Some(function) = call.function {
                        if let Some(name) = function.name {
                            if !name.is_empty() {
                                entry.function_name.push_str(&name);
                            }
                        }
                        if let Some(arguments) = function.arguments {
                            if !arguments.is_empty() {
                                entry.arguments.push_str(&arguments);
                            }
                        }
                    }
                }
            }
        }

        if let Some(message) = choice.message {
            if !*saw_content_delta {
                if let Some(message_content) = message.content {
                    if !message_content.is_empty() {
                        content.push_str(&message_content);
                        on_delta(StreamDelta::Content(message_content))
                            .map_err(ModelError::Request)?;
                    }
                }
            }

            if reasoning.is_empty() {
                if let Some(message_reasoning) = message.reasoning_content {
                    if !message_reasoning.is_empty() {
                        reasoning.push_str(&message_reasoning);
                        on_delta(StreamDelta::Reasoning(message_reasoning))
                            .map_err(ModelError::Request)?;
                    }
                }
            }

            if let Some(tool_calls) = message.tool_calls {
                if !tool_calls.is_empty() {
                    full_tool_calls.extend(tool_calls);
                }
            }
        }
    }

    Ok(false)
}
