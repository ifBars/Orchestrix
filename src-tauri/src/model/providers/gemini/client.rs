use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::core::tool::ToolDescriptor;
use crate::model::shared::{
    preferred_response_text, strip_tool_call_markup,
    worker_system_prompt, worker_user_prompt,
};
use crate::model::{
    AgentModelClient, ModelError, StreamDelta, WorkerAction, WorkerActionRequest, WorkerDecision,
};
use crate::runtime::plan_mode_settings::WORKER_MAX_TOKENS;

const DEFAULT_GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

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
        let response = self.run_chat(system, user, max_tokens, None, false, None).await?;
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
        let endpoint = format!(
            "{}/{}:generateContent",
            self.base_url.trim_end_matches('/'),
            model_name
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
            Some(serde_json::json!(t.iter().map(|d| {
                serde_json::json!({
                    "name": d.name,
                    "description": d.description,
                    "parameters": d.input_schema
                })
            }).collect::<Vec<_>>()))
        });

        let body = GeminiGenerateRequest {
            system_instruction,
            contents,
            tools: tools_value,
            generation_config: GeminiGenerationConfig {
                temperature: 0.1,
                max_output_tokens: max_tokens as i32,
                stream: if stream { Some(true) } else { None },
                ..Default::default()
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

        let response = request.send().await.map_err(|e| ModelError::Request(e.to_string()))?;

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
            return Err(ModelError::Request(format!("Gemini error {status}: {text}")));
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

    fn parse_response(&self, response: GeminiGenerateResponse) -> Result<GeminiResponseMessage, ModelError> {
        let candidate = response
            .candidates
            .into_iter()
            .next()
            .ok_or_else(|| {
                ModelError::InvalidResponse("No candidates in Gemini response".to_string())
            })?;

        let content = candidate
            .content
            .parts
            .iter()
            .filter_map(|p| match p {
                GeminiPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        let reasoning_content = candidate
            .grounding_metadata
            .as_ref()
            .and_then(|gm| gm.grounding_chunk.as_ref())
            .map(|_| None)
            .unwrap_or(None);

        Ok(GeminiResponseMessage {
            content: if content.is_empty() { None } else { Some(content) },
            reasoning_content,
            tool_calls: None,
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

                if let Some(ref mut on_delta) = on_delta {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&line) {
                        if let Some(candidates) = parsed.get("candidates").and_then(|c| c.as_array()) {
                            for candidate in candidates {
                                if let Some(content_obj) = candidate.get("content").and_then(|c| c.get("parts")).and_then(|p| p.as_array()) {
                                    for part in content_obj {
                                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                            content.push_str(text);
                                            let _ = on_delta(StreamDelta::Content(text.to_string()));
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
            content: if content.is_empty() { None } else { Some(content) },
            reasoning_content: if reasoning.is_empty() { None } else { Some(reasoning) },
            tool_calls: None,
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
            .run_chat(&system, &user, max_tokens, tools_arg, true, Some(&mut on_delta))
            .await?;

        tracing::debug!(
            "Gemini worker response - content: {:?}, reasoning_content: {:?}, tool_calls: {:?}",
            response.content,
            response.reasoning_content,
            response.tool_calls
        );

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
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(rename = "responseModalities")]
    response_modalities: Option<String>,
    #[serde(rename = "thoughts")]
    thoughts: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum GeminiPart {
    Text { text: String },
    #[serde(rename = "inlineData")]
    InlineData {
        #[serde(rename = "mimeType")]
        mime_type: String,
        data: String,
    },
}

#[derive(Debug, Deserialize)]
struct GeminiGenerateResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    prompt_feedback: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    #[serde(default)]
    content: GeminiContent,
    #[serde(default)]
    grounding_metadata: Option<GeminiGroundingMetadata>,
    #[serde(default)]
    finish_reason: Option<String>,
    #[serde(default)]
    index: Option<i32>,
}

#[derive(Debug, Deserialize, Default)]
struct GeminiGroundingMetadata {
    #[serde(default)]
    grounding_chunk: Option<serde_json::Value>,
    #[serde(default)]
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
