use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::core::tool::ToolDescriptor;
use crate::model::shared::{
    plan_markdown_system_prompt, preferred_response_text, strip_tool_call_markup,
};
use crate::model::{
    AgentModelClient, ModelError, StreamDelta, WorkerAction, WorkerActionRequest, WorkerDecision,
    WorkerToolCall,
};
use crate::runtime::plan_mode_settings::{DEFAULT_PLAN_MODE_MAX_TOKENS, WORKER_MAX_TOKENS};

const DEFAULT_MINIMAX_BASE_URL: &str = "https://api.minimaxi.chat";
const MINIMAX_CHAT_PATH: &str = "/v1/text/chatcompletion_v2";

#[derive(Debug, Clone)]
pub struct MiniMaxClient {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl MiniMaxClient {
    #[allow(dead_code)]
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self::new_with_base_url(api_key, model, None)
    }

    pub fn new_with_base_url(
        api_key: String,
        model: Option<String>,
        base_url: Option<String>,
    ) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| "MiniMax-M2.1".to_string()),
            base_url: base_url.unwrap_or_else(|| DEFAULT_MINIMAX_BASE_URL.to_string()),
            client: reqwest::Client::new(),
        }
    }

    pub fn model_id(&self) -> String {
        self.model.clone()
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
        let user = format!(
            "Task prompt:\n{}\n\nExisting markdown context (if any):\n{}\n\nWrite a revised or fresh implementation plan and submit it using the agent.create_artifact tool (filename e.g. plan.md, kind plan, content = the markdown). If you need to read files first, use the available tools, then call agent.create_artifact with your plan.",
            task_prompt,
            if prior_markdown_context.trim().is_empty() {
                "(none)"
            } else {
                prior_markdown_context
            }
        );

        let tools: Option<Vec<MiniMaxToolDefinition>> = if tool_descriptors.is_empty() {
            None
        } else {
            Some(
                tool_descriptors
                    .iter()
                    .map(|t| MiniMaxToolDefinition {
                        tool_type: "function".to_string(),
                        function: MiniMaxFunctionDefinition {
                            name: t.name.clone(),
                            description: t.description.clone(),
                            parameters: t.input_schema.clone(),
                        },
                    })
                    .collect(),
            )
        };
        let messages = vec![
            MiniMaxMessage {
                role: "system".to_string(),
                content: Some(plan_markdown_system_prompt()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            MiniMaxMessage {
                role: "user".to_string(),
                content: Some(user),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];

        let response = self
            .run_chat_json_native(
                messages,
                DEFAULT_PLAN_MODE_MAX_TOKENS,
                tools,
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
        let system = "You are an autonomous coding worker agent. Use native function calling for tools whenever tool use is needed. You may call multiple tools in one response when beneficial. If and only if the task is complete, respond with plain text summary.";
        
        let tools: Vec<MiniMaxToolDefinition> = req
            .tool_descriptors
            .iter()
            .map(|tool| MiniMaxToolDefinition {
                tool_type: "function".to_string(),
                function: MiniMaxFunctionDefinition {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.input_schema.clone(),
                },
            })
            .collect();

        // Reconstruct conversation history from prior observations
        let mut messages = Vec::new();
        
        // 1. System Message
        messages.push(MiniMaxMessage {
            role: "system".to_string(),
            content: Some(system.to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        // 2. Initial User Message (Task + Context)
        let tools_text = if !req.tool_descriptions.is_empty() {
            req.tool_descriptions.clone()
        } else if req.available_tools.is_empty() {
            "(none)".to_string()
        } else {
            req.available_tools.join(", ")
        };

        let user_prompt = format!(
            "Task:\n{}\n\nGoal:\n{}\n\nContext:\n{}\n\nAvailable Tools:\n{}\n\nUse native function calling when tool use is needed. If the work is complete, reply with a short plain-text completion summary.",
            req.task_prompt,
            req.goal_summary,
            req.context,
            tools_text,
        );

        messages.push(MiniMaxMessage {
            role: "user".to_string(),
            content: Some(user_prompt),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        // 3. History (Assistant Actions + Tool Observations)
        // We use a simple counter to generate stable IDs for tool calls since we don't store them in WorkerAction
        let mut tool_call_counter = 0;
        let mut pending_tool_calls: std::collections::VecDeque<String> = std::collections::VecDeque::new();

        for obs in req.prior_observations {
            // Check if this is an Assistant Turn (injected by us in worker/mod.rs)
            if let Some(role) = obs.get("role").and_then(|v| v.as_str()) {
                if role == "assistant" {
                    let content = obs.get("reasoning").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let tool_calls_json = obs.get("tool_calls").and_then(|v| v.as_array());
                    
                    let mut minimax_tool_calls = None;
                    if let Some(calls) = tool_calls_json {
                        if !calls.is_empty() {
                            let mut mapped_calls = Vec::new();
                            for call in calls {
                                let tool_name = call.get("tool_name").and_then(|v| v.as_str()).unwrap_or("unknown");
                                let tool_args = call.get("tool_args").unwrap_or(&serde_json::json!({})).to_string();
                                
                                // Generate ID and queue it for the next observation
                                let call_id = format!("call_{}", tool_call_counter);
                                tool_call_counter += 1;
                                pending_tool_calls.push_back(call_id.clone());

                                mapped_calls.push(MiniMaxToolCall {
                                    tool_type: "function".to_string(),
                                    function: MiniMaxFunctionCall {
                                        name: tool_name.to_string(),
                                        arguments: tool_args,
                                    },
                                    id: Some(call_id),
                                });
                            }
                            minimax_tool_calls = Some(mapped_calls);
                        }
                    }

                    messages.push(MiniMaxMessage {
                        role: "assistant".to_string(),
                        content: if content.is_empty() && minimax_tool_calls.is_some() { None } else { Some(content) },
                        tool_calls: minimax_tool_calls,
                        tool_call_id: None,
                        name: None,
                    });
                    continue;
                }
            }

            // Otherwise, assume it is a Tool Observation
            if let Some(tool_name) = obs.get("tool_name").and_then(|v| v.as_str()) {
                let output = if let Some(out) = obs.get("output") {
                    if out.is_string() {
                        out.as_str().unwrap().to_string()
                    } else {
                        out.to_string()
                    }
                } else if let Some(err) = obs.get("error") {
                    format!("Error: {}", err)
                } else {
                    "Success".to_string()
                };

                let actual_call_id = if let Some(id) = pending_tool_calls.pop_front() {
                    id
                } else {
                    // Orphan case
                    let id = format!("call_orphan_{}", tool_call_counter);
                    tool_call_counter += 1;
                    
                    // Fixup history
                    let needs_new_msg = if let Some(last) = messages.last() {
                        last.role != "assistant"
                    } else {
                        true
                    };

                    let call_record = MiniMaxToolCall {
                        tool_type: "function".to_string(),
                        function: MiniMaxFunctionCall {
                            name: tool_name.to_string(),
                            arguments: "{}".to_string(),
                        },
                        id: Some(id.clone()),
                    };

                    if needs_new_msg {
                        messages.push(MiniMaxMessage {
                            role: "assistant".to_string(),
                            content: None,
                            tool_calls: Some(vec![call_record]),
                            tool_call_id: None,
                            name: None,
                        });
                    } else {
                        // Append to last assistant message
                        if let Some(last) = messages.last_mut() {
                            if let Some(calls) = &mut last.tool_calls {
                                calls.push(call_record);
                            } else {
                                last.tool_calls = Some(vec![call_record]);
                            }
                        }
                    }
                    id
                };

                messages.push(MiniMaxMessage {
                    role: "tool".to_string(),
                    content: Some(output),
                    tool_calls: None,
                    tool_call_id: Some(actual_call_id),
                    name: Some(tool_name.to_string()),
                });
            }
        }

        let max_tokens = req.max_tokens.unwrap_or(WORKER_MAX_TOKENS);
        let response = self
            .run_chat_json_native_streaming(
                messages,
                max_tokens,
                if tools.is_empty() { None } else { Some(tools) },
                &mut on_delta,
            )
            .await?;

        let reasoning = response
            .reasoning_content
            .as_deref()
            .filter(|r| !r.trim().is_empty())
            .map(strip_tool_call_markup);

        tracing::debug!(
            "MiniMax worker response - content: {:?}, reasoning_content: {:?}, tool_calls: {:?}",
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
                        reasoning,
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

        tracing::debug!("MiniMax worker raw content: {}", raw);

        if raw.trim().is_empty() {
            tracing::debug!("MiniMax worker returning complete - empty response");
            return Ok(WorkerDecision {
                action: WorkerAction::Complete {
                    summary: "Task complete.".to_string(),
                },
                reasoning,
                raw_response,
            });
        }

        let summary = strip_tool_call_markup(raw.trim()).trim().to_string();

        Ok(WorkerDecision {
            action: WorkerAction::Complete {
                summary: if summary.is_empty() {
                    "Task complete.".to_string()
                } else {
                    summary
                },
            },
            reasoning,
            raw_response,
        })
    }
}

impl AgentModelClient for MiniMaxClient {
    fn model_id(&self) -> String {
        self.model.clone()
    }

    async fn decide_action(&self, req: WorkerActionRequest) -> Result<WorkerDecision, ModelError> {
        let noop = |_delta: StreamDelta| Ok::<(), String>(());
        self.decide_action_streaming(req, noop).await
    }
}

impl MiniMaxClient {
    /// Simple text completion without tools - useful for summarization and other single-turn tasks.
    pub async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String, ModelError> {
        let messages = vec![
            MiniMaxMessage {
                role: "system".to_string(),
                content: Some(system.to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            MiniMaxMessage {
                role: "user".to_string(),
                content: Some(user.to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];
        let response = self
            .run_chat_json_native(messages, max_tokens, None)
            .await?;
        Ok(preferred_response_text(
            response.content,
            response.reasoning_content,
        ))
    }

    async fn run_chat_json_native(
        &self,
        messages: Vec<MiniMaxMessage>,
        max_tokens: u32,
        tools: Option<Vec<MiniMaxToolDefinition>>,
    ) -> Result<MiniMaxResponseMessage, ModelError> {
        let endpoint = format!(
            "{}{}",
            self.base_url.trim_end_matches('/'),
            MINIMAX_CHAT_PATH
        );
        let tool_choice = tools.as_ref().map(|_| "auto".to_string());
        let parallel_tool_calls = tools.as_ref().map(|_| true);
        let body = MiniMaxChatRequest {
            model: self.model.clone(),
            messages,
            max_tokens,
            temperature: 0.1,
            stream: false,
            tools,
            tool_choice,
            parallel_tool_calls,
            // Request standard JSON output, model might output <think> tags in content
            // or we could use reasoning_split if we wanted separate fields.
            // For now, let's keep it simple.
        };

        tracing::debug!(
            "MiniMax API request to {} with model {} and tools: {:?}",
            endpoint,
            self.model,
            body.tools.as_ref().map(|t| t.len())
        );

        let response = self
            .client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        tracing::debug!("MiniMax API response: status={status}, body={}", text);

        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ModelError::Auth(format!("MiniMax auth failed ({status})")));
        }
        if !status.is_success() {
            return Err(ModelError::Request(format!(
                "MiniMax error {status}: {text}"
            )));
        }

        let parsed: MiniMaxChatResponse = serde_json::from_str(&text)
            .map_err(|e| ModelError::InvalidResponse(format!("MiniMax parse failed: {e}")))?;

        parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message)
            .ok_or_else(|| {
                ModelError::InvalidResponse(
                    "missing choices[0].message from MiniMax response".to_string(),
                )
            })
    }

    async fn run_chat_json_native_streaming(
        &self,
        messages: Vec<MiniMaxMessage>,
        max_tokens: u32,
        tools: Option<Vec<MiniMaxToolDefinition>>,
        on_delta: &mut (dyn FnMut(StreamDelta) -> Result<(), String> + Send),
    ) -> Result<MiniMaxResponseMessage, ModelError> {
        let endpoint = format!(
            "{}{}",
            self.base_url.trim_end_matches('/'),
            MINIMAX_CHAT_PATH
        );
        let tool_choice = tools.as_ref().map(|_| "auto".to_string());
        let parallel_tool_calls = tools.as_ref().map(|_| true);
        let body = MiniMaxChatRequest {
            model: self.model.clone(),
            messages,
            max_tokens,
            temperature: 0.1,
            stream: true,
            tools,
            tool_choice,
            parallel_tool_calls,
        };

        tracing::debug!(
            "MiniMax streaming request to {} with model {} and tools: {:?}",
            endpoint,
            self.model,
            body.tools.as_ref().map(|t| t.len())
        );

        let response = self
            .client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        let status = response.status();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            let body = response.text().await.unwrap_or_default();
            return Err(ModelError::Auth(format!(
                "MiniMax auth failed ({status}): {body}"
            )));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ModelError::Request(format!(
                "MiniMax error {status}: {body}"
            )));
        }

        let mut content = String::new();
        let mut reasoning = String::new();
        let mut tool_call_accumulators: Vec<MiniMaxToolCallAccumulator> = Vec::new();
        let mut full_tool_calls: Vec<MiniMaxToolCall> = Vec::new();
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

                if process_minimax_stream_line(
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
            let _ = process_minimax_stream_line(
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
                        Some(MiniMaxToolCall {
                            tool_type: if entry.tool_type.trim().is_empty() {
                                "function".to_string()
                            } else {
                                entry.tool_type
                            },
                            function: MiniMaxFunctionCall {
                                name: entry.function_name,
                                arguments: entry.arguments,
                            },
                            id: if entry.id.is_empty() { None } else { Some(entry.id) },
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        } else if !full_tool_calls.is_empty() {
            Some(full_tool_calls)
        } else {
            None
        };

        Ok(MiniMaxResponseMessage {
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
}

// ---------------------------------------------------------------------------
// MiniMax API types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct MiniMaxMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<MiniMaxToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct MiniMaxChatRequest {
    model: String,
    messages: Vec<MiniMaxMessage>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<MiniMaxToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parallel_tool_calls: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct MiniMaxChatResponse {
    choices: Vec<MiniMaxChoice>,
}

#[derive(Debug, Deserialize)]
struct MiniMaxChoice {
    message: MiniMaxResponseMessage,
}

#[derive(Debug, Deserialize)]
struct MiniMaxStreamChunk {
    #[serde(default)]
    choices: Vec<MiniMaxStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct MiniMaxStreamChoice {
    #[serde(default)]
    delta: Option<MiniMaxStreamDelta>,
    #[serde(default)]
    message: Option<MiniMaxResponseMessage>,
}

#[derive(Debug, Deserialize, Default)]
struct MiniMaxStreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<MiniMaxToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct MiniMaxToolCallDelta {
    #[serde(default)]
    index: Option<usize>,
    #[serde(rename = "type", default)]
    tool_type: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<MiniMaxFunctionCallDelta>,
}

#[derive(Debug, Deserialize, Default)]
struct MiniMaxFunctionCallDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Default)]
struct MiniMaxToolCallAccumulator {
    tool_type: String,
    function_name: String,
    arguments: String,
    id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MiniMaxResponseMessage {
    #[serde(default)]
    content: Option<String>,
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<MiniMaxToolCall>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MiniMaxToolDefinition {
    #[serde(rename = "type")]
    tool_type: String,
    function: MiniMaxFunctionDefinition,
}

#[derive(Debug, Serialize, Deserialize)]
struct MiniMaxFunctionDefinition {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct MiniMaxToolCall {
    #[serde(rename = "type")]
    tool_type: String,
    function: MiniMaxFunctionCall,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MiniMaxFunctionCall {
    name: String,
    arguments: String,
}

fn process_minimax_stream_line(
    line: &str,
    content: &mut String,
    reasoning: &mut String,
    tool_call_accumulators: &mut Vec<MiniMaxToolCallAccumulator>,
    full_tool_calls: &mut Vec<MiniMaxToolCall>,
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

    let chunk: MiniMaxStreamChunk = match serde_json::from_str(payload) {
        Ok(value) => value,
        Err(err) => {
            tracing::debug!("MiniMax stream chunk parse failed: {err}; payload={payload}");
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
                            .resize_with(idx + 1, MiniMaxToolCallAccumulator::default);
                    }

                    let entry = &mut tool_call_accumulators[idx];
                    if let Some(id) = call.id {
                        if !id.is_empty() {
                            entry.id = id;
                        }
                    }
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
