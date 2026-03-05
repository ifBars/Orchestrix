use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::tool::ToolDescriptor;
use crate::model::shared::{
    completion_summary_from_content_or_reasoning, plan_markdown_system_prompt,
    preferred_response_text, strip_tool_call_markup, worker_prompt_from_request,
};
use crate::model::{
    AgentModelClient, ModelError, StreamDelta, WorkerAction, WorkerActionRequest, WorkerDecision,
    WorkerToolCall,
};
use crate::runtime::plan_mode_settings::{DEFAULT_PLAN_MODE_MAX_TOKENS, WORKER_MAX_TOKENS};

/// Configuration for OpenAI-compatible clients with provider-specific hooks.
pub struct OpenAiCompatClientConfig {
    /// Function to transform tool names when sending to the provider (e.g., encode special chars)
    pub tool_name_to_wire: Arc<dyn Fn(&str) -> String + Send + Sync>,
    /// Function to transform tool names when receiving from the provider (e.g., decode special chars)
    /// The second argument is the list of tool descriptors for reverse lookup.
    pub tool_name_from_wire: Arc<dyn Fn(&str, &[ToolDescriptor]) -> String + Send + Sync>,
    /// Function to filter/transform JSON schemas for provider compatibility
    pub schema_filter: Arc<dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync>,
    /// Extra headers to add to requests (e.g., User-Agent, custom auth)
    pub extra_headers: Vec<(&'static str, String)>,
    /// Whether to send parallel_tool_calls field (GLM doesn't support it)
    pub parallel_tool_calls: bool,
    /// Whether to retry on 429 rate limit errors
    pub retry_on_rate_limit: bool,
    /// Whether to retry on GLM error 1210 (invalid parameter)
    pub retry_on_invalid_param_1210: bool,
    /// Fallback model for error 1210 retries
    pub fallback_model: Option<String>,
    /// Maximum tokens cap (if provider has a limit)
    pub max_tokens_cap: Option<u32>,
}

impl Default for OpenAiCompatClientConfig {
    fn default() -> Self {
        Self {
            tool_name_to_wire: Arc::new(|name| name.to_string()),
            tool_name_from_wire: Arc::new(|name, _| name.to_string()),
            schema_filter: Arc::new(|schema| schema.clone()),
            extra_headers: Vec::new(),
            parallel_tool_calls: true,
            retry_on_rate_limit: false,
            retry_on_invalid_param_1210: false,
            fallback_model: None,
            max_tokens_cap: None,
        }
    }
}

pub struct OpenAiCompatClient {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    client: reqwest::Client,
    provider_name: &'static str,
    #[allow(dead_code)]
    default_model: &'static str,
    config: OpenAiCompatClientConfig,
}

impl OpenAiCompatClient {
    pub fn new(
        api_key: String,
        model: Option<String>,
        base_url: Option<String>,
        provider_name: &'static str,
        default_model: &'static str,
        config: OpenAiCompatClientConfig,
    ) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| default_model.to_string()),
            base_url: base_url.unwrap_or_else(|| String::new()),
            client: reqwest::Client::new(),
            provider_name,
            default_model,
            config,
        }
    }

    fn normalize_max_tokens(&self, requested: u32) -> u32 {
        match self.config.max_tokens_cap {
            Some(cap) => requested.min(cap),
            None => requested,
        }
    }

    fn is_rate_limit_error(&self, status: reqwest::StatusCode, text: &str) -> bool {
        status.as_u16() == 429
            || text.contains("rate limit")
            || text.contains("Rate limit")
            || (self.provider_name == "GLM"
                && (text.contains("\"code\":\"1302\"") || text.contains("\"code\":1302")))
    }

    fn is_invalid_parameter_1210(&self, status: reqwest::StatusCode, text: &str) -> bool {
        if status.as_u16() != 400 {
            return false;
        }
        text.contains("\"code\":\"1210\"")
            || text.contains("\"code\":1210")
            || text.contains("\"code\" : \"1210\"")
            || text.contains("\"code\" : 1210")
    }

    pub fn model_id(&self) -> String {
        self.model.clone()
    }

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

    async fn run_chat(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        tools: Option<&[ToolDescriptor]>,
    ) -> Result<OpenAiResponseMessage, ModelError> {
        let max_tokens = self.normalize_max_tokens(max_tokens);
        let endpoint = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let make_body = |model: String| {
            let openai_tools = tools.and_then(|t| {
                if t.is_empty() {
                    return None;
                }
                Some(
                    t.iter()
                        .map(|d| OpenAiTool {
                            type_: "function".to_string(),
                            function: OpenAiFunction {
                                name: (self.config.tool_name_to_wire)(&d.name),
                                description: d.description.clone(),
                                parameters: (self.config.schema_filter)(&d.input_schema),
                            },
                        })
                        .collect(),
                )
            });
            let has_tools = openai_tools.is_some();
            OpenAiChatRequest {
                model,
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
                parallel_tool_calls: if has_tools && self.config.parallel_tool_calls {
                    Some(true)
                } else {
                    None
                },
            }
        };

        let body = make_body(self.model.clone());

        // Build request with extra headers
        let mut request = self
            .client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body);

        for (key, value) in &self.config.extra_headers {
            request = request.header(*key, value.clone());
        }

        let mut response = request
            .send()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        let mut status = response.status();
        let mut text = response
            .text()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        tracing::debug!("{} API response: status={}", self.provider_name, status);

        // Handle rate limiting with retry
        if self.config.retry_on_rate_limit && self.is_rate_limit_error(status, &text) {
            let delay_ms = 2000;
            tracing::warn!(
                "{} rate limit hit ({}), waiting {}ms before retry",
                self.provider_name,
                status,
                delay_ms
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;

            response = self
                .client
                .post(&endpoint)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&body)
                .send()
                .await
                .map_err(|e| ModelError::Request(e.to_string()))?;
            status = response.status();
            text = response
                .text()
                .await
                .map_err(|e| ModelError::Request(e.to_string()))?;
        }

        // Handle 1210 with model fallback
        if self.config.retry_on_invalid_param_1210 && self.is_invalid_parameter_1210(status, &text)
        {
            if let Some(ref fallback_model) = self.config.fallback_model {
                if fallback_model != &self.model {
                    tracing::warn!(
                        "{} request got 1210 with model '{}', retrying with '{}'",
                        self.provider_name,
                        self.model,
                        fallback_model
                    );
                    let retry_body = make_body(fallback_model.clone());
                    response = self
                        .client
                        .post(&endpoint)
                        .header("Content-Type", "application/json")
                        .header("Authorization", format!("Bearer {}", self.api_key))
                        .json(&retry_body)
                        .send()
                        .await
                        .map_err(|e| ModelError::Request(e.to_string()))?;
                    status = response.status();
                    text = response
                        .text()
                        .await
                        .map_err(|e| ModelError::Request(e.to_string()))?;
                }
            }
        }

        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ModelError::Auth(format!(
                "{} auth failed ({}). Check API key and account access.",
                self.provider_name, status
            )));
        }
        if !status.is_success() {
            return Err(ModelError::Request(format!(
                "{} error {}: {}",
                self.provider_name, status, text
            )));
        }

        let parsed: OpenAiChatResponse = serde_json::from_str(&text).map_err(|e| {
            ModelError::InvalidResponse(format!("{} parse failed: {}", self.provider_name, e))
        })?;

        parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message)
            .ok_or_else(|| {
                ModelError::InvalidResponse(format!(
                    "missing choices[0].message from {} response",
                    self.provider_name
                ))
            })
    }

    async fn run_chat_streaming(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        tools: Option<&[ToolDescriptor]>,
        on_delta: &mut (dyn FnMut(StreamDelta) -> Result<(), String> + Send),
    ) -> Result<OpenAiResponseMessage, ModelError> {
        let max_tokens = self.normalize_max_tokens(max_tokens);
        let endpoint = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let make_body = |model: String| {
            let openai_tools = tools.and_then(|t| {
                if t.is_empty() {
                    return None;
                }
                Some(
                    t.iter()
                        .map(|d| OpenAiTool {
                            type_: "function".to_string(),
                            function: OpenAiFunction {
                                name: (self.config.tool_name_to_wire)(&d.name),
                                description: d.description.clone(),
                                parameters: (self.config.schema_filter)(&d.input_schema),
                            },
                        })
                        .collect(),
                )
            });
            let has_tools = openai_tools.is_some();
            OpenAiChatRequest {
                model,
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
                parallel_tool_calls: if has_tools && self.config.parallel_tool_calls {
                    Some(true)
                } else {
                    None
                },
            }
        };

        let body = make_body(self.model.clone());

        let mut request = self
            .client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body);

        for (key, value) in &self.config.extra_headers {
            request = request.header(*key, value.clone());
        }

        let response = request
            .send()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        let initial_status = response.status();

        // Handle rate limiting with retry
        let response = if self.config.retry_on_rate_limit
            && self.is_rate_limit_error(initial_status, "")
        {
            let delay_ms = 2000;
            tracing::warn!(
                "{} streaming rate limit hit ({}), waiting {}ms before retry",
                self.provider_name,
                initial_status,
                delay_ms
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;

            let retry_response = self
                .client
                .post(&endpoint)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&body)
                .send()
                .await
                .map_err(|e| ModelError::Request(e.to_string()))?;

            if retry_response.status().is_success() {
                retry_response
            } else {
                let retry_status = retry_response.status();
                let retry_text = retry_response.text().await.unwrap_or_default();
                return Err(ModelError::Request(format!(
                    "{} error {} after rate limit retry: {}",
                    self.provider_name, retry_status, retry_text
                )));
            }
        } else if self.config.retry_on_invalid_param_1210
            && self.is_invalid_parameter_1210(initial_status, "")
        {
            if let Some(ref fallback_model) = self.config.fallback_model {
                if fallback_model != &self.model {
                    tracing::warn!(
                        "{} streaming request got 1210 with model '{}', retrying with '{}'",
                        self.provider_name,
                        self.model,
                        fallback_model
                    );
                    let retry_body = make_body(fallback_model.clone());
                    let retry_response = self
                        .client
                        .post(&endpoint)
                        .header("Content-Type", "application/json")
                        .header("Authorization", format!("Bearer {}", self.api_key))
                        .json(&retry_body)
                        .send()
                        .await
                        .map_err(|e| ModelError::Request(e.to_string()))?;

                    if retry_response.status().is_success() {
                        retry_response
                    } else {
                        let retry_status = retry_response.status();
                        let retry_text = retry_response.text().await.unwrap_or_default();
                        if retry_status.as_u16() == 401 || retry_status.as_u16() == 403 {
                            return Err(ModelError::Auth(format!(
                                "{} auth failed ({}). Response: {}",
                                self.provider_name, retry_status, retry_text
                            )));
                        }
                        return Err(ModelError::Request(format!(
                            "{} error {} (model={}, endpoint={}): {}",
                            self.provider_name, retry_status, self.model, self.base_url, retry_text
                        )));
                    }
                } else {
                    response
                }
            } else {
                response
            }
        } else {
            response
        };

        let status = response.status();

        if status.as_u16() == 401 || status.as_u16() == 403 {
            let text = response.text().await.unwrap_or_default();
            return Err(ModelError::Auth(format!(
                "{} auth failed ({}). Response: {}",
                self.provider_name, status, text
            )));
        }
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ModelError::Request(format!(
                "{} error {}: {}",
                self.provider_name, status, text
            )));
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
            Some(req.tool_descriptors.as_slice())
        };
        let max_tokens = req.max_tokens.unwrap_or(WORKER_MAX_TOKENS);

        // Handle GLM-specific fallback for plan mode (no tools retry)
        let is_plan_mode = req
            .goal_summary
            .to_ascii_lowercase()
            .contains("draft an implementation plan");
        let response = match self
            .run_chat_streaming(&system, &user, max_tokens, tools_arg, &mut on_delta)
            .await
        {
            Ok(resp) => resp,
            Err(ModelError::Request(msg))
                if self.config.retry_on_invalid_param_1210
                    && (msg.contains("\"code\":\"1210\"") || msg.contains("\"code\":1210")) =>
            {
                tracing::warn!(
                    "{} streaming request rejected parameters (1210); retrying non-streaming",
                    self.provider_name
                );
                if is_plan_mode {
                    tracing::warn!(
                        "{} plan-mode fallback: retrying without tools",
                        self.provider_name
                    );
                    self.run_chat(&system, &user, max_tokens, None).await?
                } else {
                    self.run_chat(&system, &user, max_tokens, tools_arg).await?
                }
            }
            Err(err) => return Err(err),
        };

        tracing::debug!(
            "{} worker response - content: {:?}, tool_calls: {:?}",
            self.provider_name,
            response.content,
            response.tool_calls
        );

        Ok(openai_response_to_worker_decision(
            response,
            &req.tool_descriptors,
            &*self.config.tool_name_from_wire,
        ))
    }

    #[allow(dead_code)]
    pub async fn generate_plan_markdown(
        &self,
        task_prompt: &str,
        prior_markdown_context: &str,
        tool_descriptors: Vec<ToolDescriptor>,
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

        let tools_arg = if tool_descriptors.is_empty() {
            None
        } else {
            Some(tool_descriptors.as_slice())
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
                // Decode the tool name using the from-wire hook (empty descriptors for lookup)
                let tool_name = (self.config.tool_name_from_wire)(&call.function.name, &[]);
                if tool_name == "agent.create_artifact" {
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
                strip_tool_call_markup(response.content.unwrap_or_default().trim())
                    .trim()
                    .to_string()
            }
        } else {
            strip_tool_call_markup(response.content.unwrap_or_default().trim())
                .trim()
                .to_string()
        };

        if markdown.trim().is_empty() {
            return Err(ModelError::InvalidResponse(
                "planner returned empty markdown".to_string(),
            ));
        }

        Ok(markdown)
    }
}

impl AgentModelClient for OpenAiCompatClient {
    fn model_id(&self) -> String {
        self.model.clone()
    }

    async fn decide_action(&self, req: WorkerActionRequest) -> Result<WorkerDecision, ModelError> {
        let noop = |_delta: StreamDelta| Ok::<(), String>(());
        self.decide_action_streaming(req, noop).await
    }
}

#[derive(Debug, Default)]
pub struct OpenAiToolCallAccumulator {
    pub tool_type: String,
    pub function_name: String,
    pub arguments: String,
}

pub fn process_openai_stream_line(
    line: &str,
    content: &mut String,
    reasoning: &mut String,
    tool_call_accumulators: &mut Vec<OpenAiToolCallAccumulator>,
    full_tool_calls: &mut Vec<OpenAiToolCall>,
    saw_content_delta: &mut bool,
    on_delta: &mut (dyn FnMut(StreamDelta) -> Result<(), String> + Send),
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

pub fn openai_response_to_worker_decision(
    response: OpenAiResponseMessage,
    tool_descriptors: &[ToolDescriptor],
    decode_tool_name: &dyn Fn(&str, &[ToolDescriptor]) -> String,
) -> WorkerDecision {
    if let Some(tool_calls) = response.tool_calls.as_ref() {
        if !tool_calls.is_empty() {
            let mut calls = Vec::with_capacity(tool_calls.len());
            for call in tool_calls {
                if call.tool_type != "function" {
                    continue;
                }
                let args_json = serde_json::from_str::<serde_json::Value>(&call.function.arguments)
                    .unwrap_or_else(|_| serde_json::json!({}));
                calls.push(WorkerToolCall {
                    tool_name: decode_tool_name(&call.function.name, tool_descriptors),
                    tool_args: args_json,
                    rationale: None,
                });
            }

            if !calls.is_empty() {
                let raw_response = serde_json::to_string(&response).ok();
                return WorkerDecision {
                    action: WorkerAction::ToolCalls { calls },
                    reasoning: response.reasoning_content,
                    raw_response,
                };
            }
        }
    }

    let raw_response = serde_json::to_string(&response).ok();
    WorkerDecision {
        action: WorkerAction::Complete {
            summary: completion_summary_from_content_or_reasoning(
                response.content,
                response.reasoning_content,
            ),
        },
        reasoning: None,
        raw_response,
    }
}

#[derive(Debug, Serialize)]
pub struct OpenAiChatRequest {
    pub model: String,
    pub messages: Vec<OpenAiRequestMessage>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenAiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct OpenAiTool {
    #[serde(rename = "type")]
    pub type_: String,
    pub function: OpenAiFunction,
}

#[derive(Debug, Serialize)]
pub struct OpenAiFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAiRequestMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAiResponseMessage {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct OpenAiChatResponse {
    pub choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct OpenAiChoice {
    pub message: OpenAiResponseMessage,
}

#[derive(Debug, Deserialize)]
pub struct OpenAiStreamChunk {
    #[serde(default)]
    pub choices: Vec<OpenAiStreamChoice>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAiStreamChoice {
    #[serde(default)]
    pub delta: Option<OpenAiStreamDelta>,
    #[serde(default)]
    pub message: Option<OpenAiResponseMessage>,
}

#[derive(Debug, Deserialize, Default)]
pub struct OpenAiStreamDelta {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<OpenAiToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAiToolCallDelta {
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(rename = "type", default)]
    pub tool_type: Option<String>,
    #[serde(default)]
    pub function: Option<OpenAiFunctionCallDelta>,
}

#[derive(Debug, Deserialize, Default)]
pub struct OpenAiFunctionCallDelta {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAiToolCall {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAiFunctionCall {
    pub name: String,
    pub arguments: String,
}
