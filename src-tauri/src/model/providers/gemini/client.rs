use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::core::tool::ToolDescriptor;
use crate::model::shared::{
    completion_summary_from_content_or_reasoning, preferred_response_text,
    worker_prompt_from_request,
};
use crate::model::{
    AgentModelClient, ModelError, StreamDelta, WorkerAction, WorkerActionRequest, WorkerDecision,
    WorkerToolCall,
};
use crate::runtime::plan_mode_settings::WORKER_MAX_TOKENS;

const DEFAULT_GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Sanitizes JSON schema for Gemini API compatibility.
/// Removes fields that Gemini doesn't support: $schema, const, $ref, additionalProperties, definitions.
/// Converts type arrays to single type strings.
/// Handles items: true by converting to a proper schema object.
fn sanitize_schema_for_gemini(schema: &serde_json::Value) -> serde_json::Value {
    match schema {
        serde_json::Value::Object(obj) => {
            let mut sanitized = serde_json::Map::new();

            for (key, value) in obj.iter() {
                // Skip fields that Gemini doesn't support
                if key == "$schema"
                    || key == "const"
                    || key == "$ref"
                    || key == "definitions"
                    || key == "additionalProperties"
                {
                    continue;
                }

                // Handle "type" field - Gemini doesn't support arrays like ["string", "null"]
                if key == "type" {
                    if let serde_json::Value::Array(types) = value {
                        // If type is an array, use the first non-null type
                        let first_type = types
                            .iter()
                            .find(|t| t.as_str() != Some("null"))
                            .cloned()
                            .unwrap_or_else(|| types.first().cloned().unwrap_or_default());
                        sanitized.insert(key.clone(), first_type);
                    } else {
                        sanitized.insert(key.clone(), value.clone());
                    }
                } else if key == "items" {
                    // Gemini doesn't support items: true (boolean), it must be a schema object
                    if value.is_boolean() {
                        // Convert items: true to items: { type: "object" }
                        sanitized.insert(key.clone(), serde_json::json!({ "type": "object" }));
                    } else {
                        // Recursively sanitize the items schema
                        sanitized.insert(key.clone(), sanitize_schema_for_gemini(value));
                    }
                } else {
                    // Recursively sanitize nested objects
                    sanitized.insert(key.clone(), sanitize_schema_for_gemini(value));
                }
            }

            serde_json::Value::Object(sanitized)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(|v| sanitize_schema_for_gemini(v)).collect())
        }
        _ => schema.clone(),
    }
}

#[derive(Debug, Clone)]
pub struct GeminiClient {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl GeminiClient {
    pub fn new(api_key: String, model: Option<String>, base_url: Option<String>) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| "gemini-3-flash-preview".to_string()),
            base_url: base_url.unwrap_or_else(|| DEFAULT_GEMINI_BASE_URL.to_string()),
            client: reqwest::Client::new(),
        }
    }

    pub fn model_id(&self) -> String {
        self.model.clone()
    }

    pub async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String, ModelError> {
        let response = self
            .run_chat(system, user, max_tokens, None, false, None)
            .await?;
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
        tools: Option<Vec<ToolDescriptor>>,
        stream: bool,
        on_delta: Option<&mut (dyn FnMut(StreamDelta) -> Result<(), String> + Send)>,
    ) -> Result<GeminiResponseMessage, ModelError> {
        let model_name = if self.model.starts_with("models/") {
            self.model.clone()
        } else {
            format!("models/{}", self.model)
        };
        let method = if stream {
            "streamGenerateContent"
        } else {
            "generateContent"
        };
        let endpoint = format!(
            "{}/{}:{}",
            self.base_url.trim_end_matches('/'),
            model_name,
            method
        );

        let system_instruction = if !system.is_empty() {
            Some(GeminiContent {
                role: "system".to_string(),
                parts: vec![GeminiPart::Text {
                    text: system.to_string(),
                }],
            })
        } else {
            None
        };

        let contents = vec![GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart::Text {
                text: user.to_string(),
            }],
        }];

        let tools_value = tools.as_ref().and_then(|t| {
            if t.is_empty() {
                return None;
            }
            // Gemini API expects tools wrapped in function_declarations
            // and requires sanitized JSON schema (no $schema, const, $ref, etc.)
            let function_declarations: Vec<_> = t
                .iter()
                .map(|d| {
                    let sanitized_schema = sanitize_schema_for_gemini(&d.input_schema);
                    serde_json::json!({
                        "name": d.name,
                        "description": d.description,
                        "parameters": sanitized_schema
                    })
                })
                .collect();

            Some(serde_json::json!([{
                "function_declarations": function_declarations
            }]))
        });

        let body = GeminiGenerateRequest {
            system_instruction,
            contents,
            tools: tools_value,
            generation_config: GeminiGenerationConfig {
                temperature: 0.1,
                max_output_tokens: max_tokens as i32,
            },
        };

        let mut request = self
            .client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .query(&[("key", self.api_key.as_str())])
            .json(&body);

        if stream {
            request = request.header("Accept", "text/event-stream");
        }

        let response = request
            .send()
            .await
            .map_err(|e| ModelError::Request(e.to_string()))?;

        let status = response.status();

        if status.as_u16() == 401 || status.as_u16() == 403 {
            let text = response.text().await.unwrap_or_default();
            return Err(ModelError::Auth(format!(
                "Gemini auth failed ({status}). Please check:\n\
                1. Your GEMINI_API_KEY environment variable is set correctly\n\
                2. The API key is valid and has not expired\n\
                Response: {}",
                text
            )));
        }
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ModelError::Request(format!(
                "Gemini error {status}: {text}"
            )));
        }

        if stream {
            self.handle_streaming_response(response, on_delta).await
        } else {
            let text = response
                .text()
                .await
                .map_err(|e| ModelError::Request(e.to_string()))?;

            tracing::debug!("Gemini API response: status={}, body={}", status, text);

            let parsed: GeminiGenerateResponse = serde_json::from_str(&text)
                .map_err(|e| ModelError::InvalidResponse(format!("Gemini parse failed: {e}")))?;

            self.parse_response(parsed)
        }
    }

    fn parse_response(
        &self,
        response: GeminiGenerateResponse,
    ) -> Result<GeminiResponseMessage, ModelError> {
        let candidate = response.candidates.into_iter().next().ok_or_else(|| {
            ModelError::InvalidResponse("No candidates in Gemini response".to_string())
        })?;

        let mut content = String::new();
        let mut tool_calls = Vec::new();

        for part in &candidate.content.parts {
            match part {
                GeminiPart::Text { text } => {
                    content.push_str(text);
                }
                GeminiPart::FunctionCall { function_call } => {
                    tool_calls.push(GeminiToolCall {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: function_call.name.clone(),
                        arguments: function_call.args.clone(),
                    });
                }
                _ => {}
            }
        }

        let reasoning_content = candidate
            .grounding_metadata
            .as_ref()
            .and_then(|gm| gm.grounding_chunk.as_ref())
            .map(|_| None)
            .unwrap_or(None);

        Ok(GeminiResponseMessage {
            content: if content.is_empty() {
                None
            } else {
                Some(content)
            },
            reasoning_content,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
        })
    }

    async fn handle_streaming_response(
        &self,
        response: reqwest::Response,
        mut on_delta: Option<&mut (dyn FnMut(StreamDelta) -> Result<(), String> + Send)>,
    ) -> Result<GeminiResponseMessage, ModelError> {
        let mut content = String::new();
        let reasoning = String::new();

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| ModelError::Request(e.to_string()))?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(newline_idx) = buffer.find('\n') {
                let line = buffer[..newline_idx].to_string();
                buffer.drain(..=newline_idx);

                // Handle SSE format - lines may start with "data: "
                let line = line.trim_start_matches("data: ").trim();

                // Skip empty lines and SSE markers
                if line.is_empty() || line == "[DONE]" {
                    continue;
                }

                // Parse the JSON and extract content
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
                    if let Some(candidates) = parsed.get("candidates").and_then(|c| c.as_array()) {
                        for candidate in candidates {
                            if let Some(content_obj) = candidate
                                .get("content")
                                .and_then(|c| c.get("parts"))
                                .and_then(|p| p.as_array())
                            {
                                for part in content_obj {
                                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                        content.push_str(text);
                                        if let Some(ref mut on_delta) = on_delta {
                                            let _ =
                                                on_delta(StreamDelta::Content(text.to_string()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(GeminiResponseMessage {
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
            tool_calls: None,
        })
    }

    pub async fn decide_action_streaming<F>(
        &self,
        req: WorkerActionRequest,
        _on_delta: F,
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

        // Use non-streaming for decide_action to get complete response reliably
        let response = self
            .run_chat(&system, &user, max_tokens, tools_arg, false, None)
            .await?;

        tracing::debug!(
            "Gemini worker response - content: {:?}, reasoning_content: {:?}, tool_calls: {:?}",
            response.content,
            response.reasoning_content,
            response.tool_calls
        );

        let raw_response = serde_json::to_string(&response).ok();

        // Check if there are tool calls
        if let Some(tool_calls) = response.tool_calls {
            let calls = tool_calls
                .into_iter()
                .map(|tc| WorkerToolCall {
                    tool_name: tc.name,
                    tool_args: tc.arguments,
                    rationale: None,
                })
                .collect();

            return Ok(WorkerDecision {
                action: WorkerAction::ToolCalls { calls },
                reasoning: None,
                raw_response,
            });
        }

        Ok(WorkerDecision {
            action: WorkerAction::Complete {
                summary: completion_summary_from_content_or_reasoning(
                    response.content,
                    response.reasoning_content,
                ),
            },
            reasoning: None,
            raw_response,
        })
    }
}

impl AgentModelClient for GeminiClient {
    fn model_id(&self) -> String {
        self.model.clone()
    }

    async fn decide_action(&self, req: WorkerActionRequest) -> Result<WorkerDecision, ModelError> {
        let noop = |_delta: StreamDelta| Ok::<(), String>(());
        self.decide_action_streaming(req, noop).await
    }
}

#[derive(Debug, Serialize)]
struct GeminiGenerateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<serde_json::Value>,
    #[serde(rename = "generationConfig")]
    generation_config: GeminiGenerationConfig,
}

#[derive(Debug, Serialize, Default)]
struct GeminiGenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum GeminiPart {
    Text {
        text: String,
    },
    #[serde(rename = "inlineData")]
    InlineData {
        #[serde(rename = "mimeType")]
        mime_type: String,
        data: String,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiGenerateResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    #[allow(dead_code)]
    prompt_feedback: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiCandidate {
    #[serde(default)]
    content: GeminiContent,
    #[serde(default)]
    grounding_metadata: Option<GeminiGroundingMetadata>,
    #[serde(default)]
    #[allow(dead_code)]
    finish_reason: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    index: Option<i32>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct GeminiGroundingMetadata {
    #[serde(default)]
    grounding_chunk: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    web_search_queries: Option<Vec<String>>,
}

#[derive(Debug, Default, Serialize)]
struct GeminiResponseMessage {
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<GeminiToolCall>>,
}

#[derive(Debug, Clone, Serialize)]
struct GeminiToolCall {
    id: String,
    name: String,
    arguments: serde_json::Value,
}
