use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::core::tool::ToolDescriptor;
use crate::model::shared::{
    plan_markdown_system_prompt, strip_tool_call_markup, worker_system_prompt,
};
use crate::model::{
    AgentModelClient, ModelError, StreamDelta, WorkerAction, WorkerActionRequest, WorkerDecision,
    WorkerToolCall,
};
use crate::runtime::plan_mode_settings::{DEFAULT_PLAN_MODE_MAX_TOKENS, WORKER_MAX_TOKENS};

const DEFAULT_GLM_BASE_URL: &str = "https://api.z.ai/api/coding/paas/v4";
const GLM_CODING_MAX_TOKENS: u32 = 25_000;

#[derive(Debug, Clone)]
pub struct GlmClient {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl GlmClient {
    pub fn new(api_key: String, model: Option<String>, base_url: Option<String>) -> Self {
        let resolved_base_url = base_url.unwrap_or_else(|| DEFAULT_GLM_BASE_URL.to_string());
        let is_coding_endpoint = resolved_base_url
            .to_ascii_lowercase()
            .contains("/api/coding/paas/v4");
        let resolved_model = model
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| {
                if is_coding_endpoint {
                    "glm-4.7".to_string()
                } else {
                    "glm-5".to_string()
                }
            });

        Self {
            api_key,
            model: resolved_model,
            base_url: resolved_base_url,
            client: reqwest::Client::new(),
        }
    }

    pub fn model_id(&self) -> String {
        self.model.clone()
    }

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
                                    filtered_props
                                        .insert(k.clone(), Self::filter_schema_for_glm(v));
                                }
                                filtered.insert(
                                    key.to_string(),
                                    serde_json::Value::Object(filtered_props),
                                );
                            } else {
                                filtered.insert(key.to_string(), val.clone());
                            }
                        } else if key == &"items" {
                            // Recursively filter items schema
                            filtered.insert(key.to_string(), Self::filter_schema_for_glm(val));
                        } else {
                            filtered.insert(key.to_string(), val.clone());
                        }
                    }
                }
                serde_json::Value::Object(filtered)
            }
            serde_json::Value::Array(arr) => serde_json::Value::Array(
                arr.iter().map(|v| Self::filter_schema_for_glm(v)).collect(),
            ),
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

    fn is_invalid_parameter_1210(status: reqwest::StatusCode, text: &str) -> bool {
        if status.as_u16() != 400 {
            return false;
        }
        text.contains("\"code\":\"1210\"")
            || text.contains("\"code\":1210")
            || text.contains("\"code\" : \"1210\"")
            || text.contains("\"code\" : 1210")
    }

    fn is_rate_limit_error(status: reqwest::StatusCode, text: &str) -> bool {
        status.as_u16() == 429
            || text.contains("\"code\":\"1302\"")
            || text.contains("\"code\":1302")
            || text.contains("rate limit")
            || text.contains("Rate limit")
    }

    fn fallback_model_for_invalid_parameter(&self) -> Option<String> {
        if !self
            .base_url
            .to_ascii_lowercase()
            .contains("/api/coding/paas/v4")
        {
            return None;
        }

        let current = self.model.trim();
        if current.is_empty() {
            return Some("glm-4.7".to_string());
        }

        // If current is uppercase GLM-X, try lowercase glm-X (Z.AI expects lowercase)
        if current.starts_with("GLM-") {
            let suffix = current.strip_prefix("GLM-").unwrap_or(current);
            let candidate = format!("glm-{suffix}");
            if candidate != current {
                return Some(candidate);
            }
        }

        // Fallback to standard glm-4.7 if other variants fail
        if !current.eq_ignore_ascii_case("glm-4.7") {
            return Some("glm-4.7".to_string());
        }

        None
    }

    fn normalize_max_tokens(&self, requested: u32) -> u32 {
        let is_coding_endpoint = self
            .base_url
            .to_ascii_lowercase()
            .contains("/api/coding/paas/v4");
        if !is_coding_endpoint {
            return requested;
        }

        let normalized = requested.min(GLM_CODING_MAX_TOKENS);
        if normalized != requested {
            tracing::warn!(
                "GLM requested max_tokens={} exceeds coding endpoint limit; clamping to {}",
                requested,
                normalized
            );
        }
        normalized
    }

    #[allow(dead_code)]
    pub async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String, ModelError> {
        let response = self.run_chat(system, user, max_tokens, None).await?;
        Ok(response.content.unwrap_or_default())
    }

    async fn run_chat(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        tools: Option<Vec<ToolDescriptor>>,
    ) -> Result<OpenAiResponseMessage, ModelError> {
        let max_tokens = self.normalize_max_tokens(max_tokens);
        let endpoint = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let openai_tools = tools.and_then(|t| {
            if t.is_empty() {
                return None;
            }
            Some(
                t.iter()
                    .map(|d| OpenAiTool {
                        type_: "function".to_string(),
                        function: OpenAiFunction {
                            name: Self::encode_tool_name_for_glm(&d.name),
                            description: d.description.clone(),
                            parameters: Self::filter_schema_for_glm(&d.input_schema),
                        },
                    })
                    .collect::<Vec<_>>(),
            )
        });
        let has_tools = openai_tools.is_some();
        let make_body = |model: String| OpenAiChatRequest {
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
            tools: openai_tools.clone(),
            tool_choice: if has_tools {
                Some("auto".to_string())
            } else {
                None
            },
        };
        let body = make_body(self.model.clone());

        let mut response = self
            .client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        let mut status = response.status();
        let mut text = response
            .text()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        // Handle rate limiting with exponential backoff
        if Self::is_rate_limit_error(status, &text) {
            let delay_ms = 2000; // Start with 2 second delay
            tracing::warn!(
                "GLM rate limit hit ({}), waiting {}ms before retry",
                status,
                delay_ms
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;

            // Retry the request
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
        if Self::is_invalid_parameter_1210(status, &text) {
            if let Some(fallback_model) = self.fallback_model_for_invalid_parameter() {
                if fallback_model != self.model {
                    tracing::warn!(
                        "GLM request got 1210 with model '{}', retrying with '{}'",
                        self.model,
                        fallback_model
                    );
                    let retry_body = make_body(fallback_model);
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

        tracing::debug!("GLM API response: status={}", status);

        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ModelError::Auth(format!(
                "GLM auth failed ({status}). Check API key and account access."
            )));
        }
        if !status.is_success() {
            return Err(ModelError::Request(format!(
                "GLM error {status} (model={}, endpoint={}): {text}",
                self.model, self.base_url
            )));
        }

        let parsed: OpenAiChatResponse = serde_json::from_str(&text)
            .map_err(|e| ModelError::InvalidResponse(format!("GLM parse failed: {e}")))?;

        parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message)
            .ok_or_else(|| {
                ModelError::InvalidResponse(
                    "missing choices[0].message from GLM response".to_string(),
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
        let max_tokens = self.normalize_max_tokens(max_tokens);
        let endpoint = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let openai_tools = tools.and_then(|t| {
            if t.is_empty() {
                return None;
            }
            Some(
                t.iter()
                    .map(|d| OpenAiTool {
                        type_: "function".to_string(),
                        function: OpenAiFunction {
                            name: Self::encode_tool_name_for_glm(&d.name),
                            description: d.description.clone(),
                            parameters: Self::filter_schema_for_glm(&d.input_schema),
                        },
                    })
                    .collect::<Vec<_>>(),
            )
        });
        let has_tools = openai_tools.is_some();
        let make_body = |model: String| OpenAiChatRequest {
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
            tools: openai_tools.clone(),
            tool_choice: if has_tools {
                Some("auto".to_string())
            } else {
                None
            },
        };
        let body = make_body(self.model.clone());

        let initial_response = self
            .client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        let initial_status = initial_response.status();
        let response = if initial_status.is_success() {
            initial_response
        } else {
            let text = initial_response.text().await.unwrap_or_default();

            // Handle rate limiting with backoff
            if Self::is_rate_limit_error(initial_status, &text) {
                let delay_ms = 2000;
                tracing::warn!(
                    "GLM streaming rate limit hit ({}), waiting {}ms before retry",
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

                let retry_status = retry_response.status();
                if retry_status.is_success() {
                    // Return the successful retry response for streaming
                    retry_response
                } else {
                    let retry_text = retry_response.text().await.unwrap_or_default();
                    return Err(ModelError::Request(format!(
                        "GLM error {retry_status} after rate limit retry (model={}, endpoint={}): {retry_text}",
                        self.model, self.base_url
                    )));
                }
            } else if Self::is_invalid_parameter_1210(initial_status, &text) {
                if let Some(fallback_model) = self.fallback_model_for_invalid_parameter() {
                    if fallback_model != self.model {
                        tracing::warn!(
                            "GLM streaming request got 1210 with model '{}', retrying with '{}'",
                            self.model,
                            fallback_model
                        );
                        let retry_body = make_body(fallback_model);
                        let retry_response = self
                            .client
                            .post(&endpoint)
                            .header("Content-Type", "application/json")
                            .header("Authorization", format!("Bearer {}", self.api_key))
                            .json(&retry_body)
                            .send()
                            .await
                            .map_err(|e| ModelError::Request(e.to_string()))?;

                        let retry_status = retry_response.status();
                        if retry_status.is_success() {
                            retry_response
                        } else {
                            let retry_text = retry_response.text().await.unwrap_or_default();
                            if retry_status.as_u16() == 401 || retry_status.as_u16() == 403 {
                                return Err(ModelError::Auth(format!(
                                    "GLM auth failed ({retry_status}). Response: {}",
                                    retry_text
                                )));
                            }
                            return Err(ModelError::Request(format!(
                                "GLM error {retry_status} (model={}, endpoint={}): {retry_text}",
                                self.model, self.base_url
                            )));
                        }
                    } else {
                        if initial_status.as_u16() == 401 || initial_status.as_u16() == 403 {
                            return Err(ModelError::Auth(format!(
                                "GLM auth failed ({initial_status}). Response: {}",
                                text
                            )));
                        }
                        return Err(ModelError::Request(format!(
                            "GLM error {initial_status} (model={}, endpoint={}): {text}",
                            self.model, self.base_url
                        )));
                    }
                } else {
                    if initial_status.as_u16() == 401 || initial_status.as_u16() == 403 {
                        return Err(ModelError::Auth(format!(
                            "GLM auth failed ({initial_status}). Response: {}",
                            text
                        )));
                    }
                    return Err(ModelError::Request(format!(
                        "GLM error {initial_status} (model={}, endpoint={}): {text}",
                        self.model, self.base_url
                    )));
                }
            } else {
                if initial_status.as_u16() == 401 || initial_status.as_u16() == 403 {
                    return Err(ModelError::Auth(format!(
                        "GLM auth failed ({initial_status}). Response: {}",
                        text
                    )));
                }
                return Err(ModelError::Request(format!(
                    "GLM error {initial_status} (model={}, endpoint={}): {text}",
                    self.model, self.base_url
                )));
            }
        };

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

    pub async fn decide_action_streaming<F>(
        &self,
        req: WorkerActionRequest,
        mut on_delta: F,
    ) -> Result<WorkerDecision, ModelError>
    where
        F: FnMut(StreamDelta) -> Result<(), String> + Send,
    {
        let is_plan_mode = req
            .goal_summary
            .to_ascii_lowercase()
            .contains("draft an implementation plan");

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
            req.task_prompt, req.goal_summary, req.context, tools_text, history_text
        );

        let system = worker_system_prompt();
        let tools_arg = if req.tool_descriptors.is_empty() {
            None
        } else {
            Some(req.tool_descriptors.clone())
        };
        let max_tokens = req.max_tokens.unwrap_or(WORKER_MAX_TOKENS);
        let response = match self
            .run_chat_streaming(&system, &user, max_tokens, tools_arg.clone(), &mut on_delta)
            .await
        {
            Ok(resp) => resp,
            Err(ModelError::Request(msg))
                if msg.contains("\"code\":\"1210\"") || msg.contains("\"code\":1210") =>
            {
                tracing::warn!(
                    "GLM streaming request rejected parameters (1210); retrying non-streaming"
                );
                if is_plan_mode {
                    tracing::warn!(
                        "GLM plan-mode fallback: retrying without tools to bypass strict param validation"
                    );
                    self.run_chat(&system, &user, max_tokens, None).await?
                } else {
                    self.run_chat(&system, &user, max_tokens, tools_arg).await?
                }
            }
            Err(err) => return Err(err),
        };

        tracing::debug!(
            "GLM worker response - content: {:?}, tool_calls: {:?}",
            response.content,
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
                        tool_name: Self::decode_tool_name_from_glm(&call.function.name),
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
            if prior_markdown_context.trim().is_empty() { "(none)" } else { prior_markdown_context }
        );

        let tools_arg = if tool_descriptors.is_empty() {
            None
        } else {
            Some(tool_descriptors)
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
                let tool_name = Self::decode_tool_name_from_glm(&call.function.name);
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

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn normalize_max_tokens_clamps_for_coding_endpoint() {
        let client = GlmClient::new(
            "test-key".to_string(),
            Some("glm-4.7".to_string()),
            Some("https://api.z.ai/api/coding/paas/v4".to_string()),
        );

        assert_eq!(client.normalize_max_tokens(8_192), 8_192);
        assert_eq!(client.normalize_max_tokens(25_000), 25_000);
        assert_eq!(client.normalize_max_tokens(180_000), 25_000);
    }

    #[test]
    fn normalize_max_tokens_keeps_value_for_non_coding_endpoint() {
        let client = GlmClient::new(
            "test-key".to_string(),
            Some("glm-5".to_string()),
            Some("https://api.z.ai/api/paas/v4".to_string()),
        );

        assert_eq!(client.normalize_max_tokens(180_000), 180_000);
    }
}

impl AgentModelClient for GlmClient {
    fn model_id(&self) -> String {
        self.model.clone()
    }

    async fn decide_action(&self, req: WorkerActionRequest) -> Result<WorkerDecision, ModelError> {
        let noop = |_delta: StreamDelta| Ok::<(), String>(());
        self.decide_action_streaming(req, noop).await
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

#[derive(Debug, Clone, Serialize)]
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
}

#[derive(Debug, Clone, Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    type_: String,
    function: OpenAiFunction,
}

#[derive(Debug, Clone, Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
