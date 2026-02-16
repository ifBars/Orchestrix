use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::core::tool::ToolDescriptor;
use crate::model::shared::{
    plan_markdown_system_prompt, preferred_response_text, strip_tool_call_markup,
    worker_system_prompt,
};
use crate::model::{
    AgentModelClient, ModelError, StreamDelta, WorkerAction, WorkerActionRequest, WorkerDecision,
    WorkerToolCall,
};
use crate::runtime::plan_mode_settings::{DEFAULT_PLAN_MODE_MAX_TOKENS, WORKER_MAX_TOKENS};

// Kimi Code API endpoint
// Uses OpenAI-compatible API format
// Full endpoint will be: {base_url}/v1/chat/completions
const DEFAULT_KIMI_BASE_URL: &str = "https://api.kimi.com/coding";

#[derive(Debug, Clone)]
pub struct KimiClient {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl KimiClient {
    /// Create a new Kimi planner.
    ///
    /// # Arguments
    /// * `api_key` - Your Kimi API key (format: sk-kimi-xxxxxxxx...)
    /// * `model` - Model name (e.g., "kimi-for-coding", "kimi-k2.5", "kimi-k2")
    /// * `base_url` - API base URL (default: "https://api.kimi.com/coding")
    ///
    /// # Environment Variables
    /// Set these to configure Kimi without hardcoding:
    /// * `KIMI_API_KEY` - Your API key
    /// * `KIMI_MODEL` - Model to use (default: "kimi-for-coding")
    /// * `KIMI_BASE_URL` - Custom base URL if needed
    pub fn new(api_key: String, model: Option<String>, base_url: Option<String>) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| "kimi-for-coding".to_string()),
            base_url: base_url.unwrap_or_else(|| DEFAULT_KIMI_BASE_URL.to_string()),
            client: reqwest::Client::new(),
        }
    }

    pub fn model_id(&self) -> String {
        self.model.clone()
    }

    /// Simple text completion without tools - useful for summarization and other single-turn tasks.
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

    /// Single chat completion path with optional tools (OpenAI-compatible). All agents use this.
    async fn run_chat(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        tools: Option<Vec<ToolDescriptor>>,
    ) -> Result<OpenAiResponseMessage, ModelError> {
        let endpoint = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let openai_tools = tools.and_then(|t| {
            if t.is_empty() {
                return None;
            }
            Some(
                t.iter()
                    .map(|d| OpenAiTool {
                        type_: "function".to_string(),
                        function: OpenAiFunction {
                            name: d.name.clone(),
                            description: d.description.clone(),
                            parameters: d.input_schema.clone(),
                        },
                    })
                    .collect::<Vec<_>>(),
            )
        });
        let has_tools = openai_tools.is_some();
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
            tools: openai_tools,
            tool_choice: if has_tools {
                Some("auto".to_string())
            } else {
                None
            },
            parallel_tool_calls: if has_tools { Some(true) } else { None },
        };

        let response = self
            .client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        // Log the response for debugging
        tracing::debug!("Kimi API response: status={}, body={}", status, text);

        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ModelError::Auth(format!(
                "Kimi auth failed ({status}). Please check:\n\
                1. Your KIMI_API_KEY environment variable is set correctly\n\
                2. The API key is valid and has not expired\n\
                3. Your account has access to the Kimi API\n\
                Response: {}",
                text
            )));
        }
        if !status.is_success() {
            return Err(ModelError::Request(format!("Kimi error {status}: {text}")));
        }

        let parsed: OpenAiChatResponse = serde_json::from_str(&text)
            .map_err(|e| ModelError::InvalidResponse(format!("Kimi parse failed: {e}")))?;

        parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message)
            .ok_or_else(|| {
                ModelError::InvalidResponse(
                    "missing choices[0].message from Kimi response".to_string(),
                )
            })
    }

    async fn run_chat_streaming(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        tools: Option<Vec<ToolDescriptor>>,
        on_delta: &mut (dyn FnMut(StreamDelta) -> Result<(), String> + Send),
    ) -> Result<OpenAiResponseMessage, ModelError> {
        let endpoint = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let openai_tools = tools.and_then(|t| {
            if t.is_empty() {
                return None;
            }
            Some(
                t.iter()
                    .map(|d| OpenAiTool {
                        type_: "function".to_string(),
                        function: OpenAiFunction {
                            name: d.name.clone(),
                            description: d.description.clone(),
                            parameters: d.input_schema.clone(),
                        },
                    })
                    .collect::<Vec<_>>(),
            )
        });
        let has_tools = openai_tools.is_some();
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
            tools: openai_tools,
            tool_choice: if has_tools {
                Some("auto".to_string())
            } else {
                None
            },
            parallel_tool_calls: if has_tools { Some(true) } else { None },
        };

        let response = self
            .client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        let status = response.status();

        if status.as_u16() == 401 || status.as_u16() == 403 {
            let text = response.text().await.unwrap_or_default();
            return Err(ModelError::Auth(format!(
                "Kimi auth failed ({status}). Response: {}",
                text
            )));
        }
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ModelError::Request(format!("Kimi error {status}: {text}")));
        }

        let mut content = String::new();
        let mut reasoning = String::new();
        let mut tool_call_accumulators: Vec<OpenAiToolCallAccumulator> = Vec::new();
        let mut full_tool_calls: Vec<OpenAiToolCall> = Vec::new();
        let mut saw_content_delta = false;

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
                            function: OpenAiFunctionCall {
                                name: entry.function_name,
                                arguments: entry.arguments,
                            },
                        })
                    })
                    .collect::<Vec<_>>(),
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

    /// Single-turn plan generation; used by integration tests. Production plan mode uses
    /// multi-turn run_multi_turn_planning in the runtime planner.
    #[allow(dead_code)]
    pub async fn generate_plan_markdown(
        &self,
        task_prompt: &str,
        prior_markdown_context: &str,
        _tool_descriptors: Vec<ToolDescriptor>,
    ) -> Result<String, ModelError> {
        let user = format!(
            "Task prompt:\n{}\n\nExisting markdown context (if any):\n{}\n\nWrite a revised or fresh implementation plan as a markdown artifact. Output only the markdown plan, no tool calls or tags.",
            task_prompt,
            if prior_markdown_context.trim().is_empty() {
                "(none)"
            } else {
                prior_markdown_context
            }
        );

        let tools_arg = if _tool_descriptors.is_empty() {
            None
        } else {
            Some(_tool_descriptors)
        };
        let response = self
            .run_chat(
                &plan_markdown_system_prompt(),
                &user,
                DEFAULT_PLAN_MODE_MAX_TOKENS,
                tools_arg,
            )
            .await?;

        let markdown = if let Some(ref tool_calls) = response.tool_calls {
            let mut content_from_tool: Option<String> = None;
            for call in tool_calls {
                if call.tool_type != "function" {
                    continue;
                }
                if call.function.name == "agent.create_artifact" {
                    let args: serde_json::Value = serde_json::from_str(&call.function.arguments)
                        .unwrap_or(serde_json::json!({}));
                    if let Some(c) = args.get("content").and_then(|v| v.as_str()) {
                        content_from_tool = Some(c.to_string());
                        break;
                    }
                }
            }
            if let Some(c) = content_from_tool {
                strip_tool_call_markup(c.trim()).trim().to_string()
            } else {
                let raw = response.content.unwrap_or_default();
                strip_tool_call_markup(raw.trim()).trim().to_string()
            }
        } else {
            let raw = response.content.unwrap_or_default();
            strip_tool_call_markup(raw.trim()).trim().to_string()
        };

        if markdown.trim().is_empty() {
            return Err(ModelError::InvalidResponse(
                "planner returned empty markdown".to_string(),
            ));
        }

        Ok(markdown)
    }

    pub async fn decide_action_streaming<F>(
        &self,
        req: WorkerActionRequest,
        mut on_delta: F,
    ) -> Result<WorkerDecision, ModelError>
    where
        F: FnMut(StreamDelta) -> Result<(), String> + Send,
    {
        let tools_text = if !req.tool_descriptions.is_empty() {
            req.tool_descriptions.clone()
        } else if req.available_tools.is_empty() {
            "(none)".to_string()
        } else {
            req.available_tools.join(", ")
        };
        let history_text = if req.prior_observations.is_empty() {
            "(none yet)".to_string()
        } else {
            serde_json::to_string(&req.prior_observations)
                .map_err(|e| ModelError::InvalidResponse(e.to_string()))?
        };

        let user = format!(
            "Task:\n{}\n\nGoal:\n{}\n\nContext:\n{}\n\nAvailable Tools:\n{}\n\nPrior Observations:\n{}\n\nUse native function calling for tools whenever needed. If the work is complete and no tool is needed, respond with a short plain-text completion summary.",
            req.task_prompt,
            req.goal_summary,
            req.context,
            tools_text,
            history_text,
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
            "Kimi worker response - content: {:?}, reasoning_content: {:?}, tool_calls: {:?}",
            response.content,
            response.reasoning_content,
            response.tool_calls
        );

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

#[derive(Debug, Default)]
struct OpenAiToolCallAccumulator {
    tool_type: String,
    function_name: String,
    arguments: String,
}

fn process_openai_stream_line(
    line: &str,
    content: &mut String,
    reasoning: &mut String,
    tool_call_accumulators: &mut Vec<OpenAiToolCallAccumulator>,
    full_tool_calls: &mut Vec<OpenAiToolCall>,
    saw_content_delta: &mut bool,
    on_delta: &mut (dyn FnMut(StreamDelta) -> Result<(), String> + Send),
) -> Result<bool, ModelError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(false);
    }
    if trimmed.starts_with(':') || trimmed.starts_with("event:") {
        return Ok(false);
    }

    let payload = if let Some(raw) = trimmed.strip_prefix("data:") {
        raw.trim()
    } else {
        trimmed
    };

    if payload.is_empty() {
        return Ok(false);
    }
    if payload == "[DONE]" {
        return Ok(true);
    }

    let chunk: OpenAiStreamChunk = match serde_json::from_str(payload) {
        Ok(value) => value,
        Err(err) => {
            tracing::debug!("Kimi stream chunk parse failed: {err}; payload={payload}");
            return Ok(false);
        }
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

impl AgentModelClient for KimiClient {
    fn model_id(&self) -> String {
        self.model.clone()
    }

    async fn decide_action(&self, req: WorkerActionRequest) -> Result<WorkerDecision, ModelError> {
        let noop = |_delta: StreamDelta| Ok::<(), String>(());
        self.decide_action_streaming(req, noop).await
    }
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    type_: String,
    function: OpenAiFunction,
}

#[derive(Debug, Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiRequestMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    #[serde(default)]
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    #[serde(default)]
    delta: Option<OpenAiStreamDelta>,
    #[serde(default)]
    message: Option<OpenAiResponseMessage>,
}

#[derive(Debug, Deserialize, Default)]
struct OpenAiStreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCallDelta {
    #[serde(default)]
    index: Option<usize>,
    #[serde(rename = "type", default)]
    tool_type: Option<String>,
    #[serde(default)]
    function: Option<OpenAiFunctionCallDelta>,
}

#[derive(Debug, Deserialize, Default)]
struct OpenAiFunctionCallDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiToolCall {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}
