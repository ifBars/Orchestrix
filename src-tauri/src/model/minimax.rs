use serde::{Deserialize, Serialize};

use crate::core::tool::ToolDescriptor;
use super::shared::{
    extract_json_object, normalize_worker_json, plan_markdown_system_prompt,
    strip_tool_call_markup,
};
use super::{ModelError, PlannerModel, WorkerAction, WorkerActionRequest, WorkerDecision, WorkerToolCall};

const DEFAULT_MINIMAX_BASE_URL: &str = "https://api.minimaxi.chat";
const MINIMAX_CHAT_PATH: &str = "/v1/text/chatcompletion_v2";

#[derive(Debug, Clone)]
pub struct MiniMaxPlanner {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl MiniMaxPlanner {
    #[allow(dead_code)]
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self::new_with_base_url(api_key, model, None)
    }

    pub fn new_with_base_url(api_key: String, model: Option<String>, base_url: Option<String>) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| "MiniMax-M2.1".to_string()),
            base_url: base_url.unwrap_or_else(|| DEFAULT_MINIMAX_BASE_URL.to_string()),
            client: reqwest::Client::new(),
        }
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
        let response = self
            .run_chat_json_native(
                &plan_markdown_system_prompt(),
                &user,
                2200,
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
}

impl PlannerModel for MiniMaxPlanner {
    fn model_id(&self) -> &'static str {
        "MiniMax-M2.1"
    }

    async fn decide_worker_action(
        &self,
        req: WorkerActionRequest,
    ) -> Result<WorkerDecision, ModelError> {
        let system = "You are an autonomous coding worker agent. Use native function calling for tools whenever tool use is needed. You may call multiple tools in one response when beneficial. If and only if the task is complete, respond with plain text summary.";
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
            "Task:\n{}\n\nGoal:\n{}\n\nContext:\n{}\n\nAvailable Tools:\n{}\n\nPrior Observations:\n{}\n\nUse native function calling when tool use is needed. If the work is complete, reply with a short plain-text completion summary.",
            req.task_prompt,
            req.goal_summary,
            req.context,
            tools_text,
            history_text,
        );

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

        let response = self
            .run_chat_json_native(system, &user, 16000, if tools.is_empty() { None } else { Some(tools) })
            .await?;

        // Capture model reasoning (chain-of-thought) before we consume the content.
        // Strip any tool-call XML that MiniMax may leak into reasoning_content.
        let reasoning = response
            .reasoning_content
            .as_deref()
            .filter(|r| !r.trim().is_empty())
            .map(|r| strip_tool_call_markup(r));

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

        // Capture raw response before consuming fields
        let raw_response = serde_json::to_string(&response).ok();

        let raw = if response.content.as_deref().unwrap_or(""
        ).trim().is_empty() {
            response.reasoning_content.unwrap_or_default()
        } else {
            response.content.unwrap_or_default()
        };

        if raw.trim().is_empty() {
            return Ok(WorkerDecision {
                action: WorkerAction::Complete {
                    summary: "Task complete.".to_string(),
                },
                reasoning,
                raw_response,
            });
        }

        let json_text = extract_json_object(raw.trim())
            .ok_or_else(|| {
                ModelError::InvalidResponse("worker returned no JSON object".to_string())
            })?;
        let normalized = normalize_worker_json(&json_text);
        let action = serde_json::from_str::<WorkerAction>(&normalized)
            .map_err(|e| {
                // Truncate the normalized JSON for the error message to avoid huge dumps.
                let snippet = if normalized.len() > 300 {
                    format!("{}...", &normalized[..300])
                } else {
                    normalized.clone()
                };
                ModelError::InvalidResponse(format!("worker action invalid: {e}\nNormalized JSON: {snippet}"))
            })?;

        Ok(WorkerDecision { action, reasoning, raw_response })
    }
}

impl MiniMaxPlanner {
    async fn run_chat_json_native(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        tools: Option<Vec<MiniMaxToolDefinition>>,
    ) -> Result<MiniMaxResponseMessage, ModelError> {
        let endpoint = format!("{}{}", self.base_url.trim_end_matches('/'), MINIMAX_CHAT_PATH);
        let tool_choice = tools.as_ref().map(|_| "auto".to_string());
        let parallel_tool_calls = tools.as_ref().map(|_| true);
        let body = MiniMaxChatRequest {
            model: self.model.clone(),
            messages: vec![
                MiniMaxMessage {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                MiniMaxMessage {
                    role: "user".to_string(),
                    content: user.to_string(),
                },
            ],
            max_tokens,
            temperature: 0.1,
            stream: false,
            tools,
            tool_choice,
            parallel_tool_calls,
        };

        let response = self
            .client
            .post(endpoint)
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

        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ModelError::Auth(format!("MiniMax auth failed ({status})")));
        }
        if !status.is_success() {
            return Err(ModelError::Request(format!("MiniMax error {status}: {text}")));
        }

        let parsed: MiniMaxChatResponse = serde_json::from_str(&text)
            .map_err(|e| ModelError::InvalidResponse(format!("MiniMax parse failed: {e}")))?;

        parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message)
            .ok_or_else(|| {
                ModelError::InvalidResponse("missing choices[0].message from MiniMax response".to_string())
            })
    }
}

// ---------------------------------------------------------------------------
// MiniMax API types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct MiniMaxMessage {
    role: String,
    content: String,
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
}

#[derive(Debug, Serialize, Deserialize)]
struct MiniMaxFunctionCall {
    name: String,
    arguments: String,
}
