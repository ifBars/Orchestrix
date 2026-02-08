use serde::{Deserialize, Serialize};

use super::shared::{
    extract_json_object, normalize_worker_json, plan_markdown_system_prompt, worker_system_prompt,
};
use super::{ModelError, PlannerModel, WorkerAction, WorkerActionRequest, WorkerDecision};

// Kimi Code API endpoint
// Uses OpenAI-compatible API format
// Full endpoint will be: {base_url}/v1/chat/completions
const DEFAULT_KIMI_BASE_URL: &str = "https://api.kimi.com/coding";

#[derive(Debug, Clone)]
pub struct KimiPlanner {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl KimiPlanner {
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

    async fn run_chat_json(&self, system: &str, user: &str, max_tokens: u32) -> Result<String, ModelError> {
        // Kimi uses OpenAI-compatible API: /v1/chat/completions
        let endpoint = format!("{}/v1/chat/completions", self.base_url.trim_end_matches('/'));
        let body = OpenAiChatRequest {
            model: self.model.clone(),
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: user.to_string(),
                },
            ],
            temperature: 0.1,
            max_tokens,
            stream: false,
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
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or_else(|| ModelError::InvalidResponse("missing choices[0].message.content".to_string()))
    }

    pub async fn generate_plan_markdown(
        &self,
        task_prompt: &str,
        prior_markdown_context: &str,
    ) -> Result<String, ModelError> {
        let user = format!(
            "Task prompt:\n{}\n\nExisting markdown context (if any):\n{}\n\nWrite a revised or fresh implementation plan as a markdown artifact.",
            task_prompt,
            if prior_markdown_context.trim().is_empty() {
                "(none)"
            } else {
                prior_markdown_context
            }
        );

        let markdown = self
            .run_chat_json(plan_markdown_system_prompt(), &user, 2200)
            .await?;

        if markdown.trim().is_empty() {
            return Err(ModelError::InvalidResponse(
                "planner returned empty markdown".to_string(),
            ));
        }

        Ok(markdown)
    }
}

impl PlannerModel for KimiPlanner {
    fn model_id(&self) -> &'static str {
        "Kimi"
    }

    async fn decide_worker_action(
        &self,
        req: WorkerActionRequest,
    ) -> Result<WorkerDecision, ModelError> {
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
            "Task:\n{}\n\nGoal:\n{}\n\nContext:\n{}\n\nAvailable Tools:\n{}\n\nPrior Observations:\n{}\n\nReturn only JSON.",
            req.task_prompt,
            req.goal_summary,
            req.context,
            tools_text,
            history_text,
        );

        let system = worker_system_prompt();
        let raw = self
            .run_chat_json(&system, &user, 16000)
            .await?;
        let json_text = extract_json_object(raw.trim())
            .ok_or_else(|| ModelError::InvalidResponse("worker returned no JSON object".to_string()))?;
        let normalized = normalize_worker_json(&json_text);
        let action = serde_json::from_str::<WorkerAction>(&normalized)
            .map_err(|e| {
                let snippet = if normalized.len() > 300 {
                    format!("{}...", &normalized[..300])
                } else {
                    normalized.clone()
                };
                ModelError::InvalidResponse(format!("worker action invalid: {e}\nNormalized JSON: {snippet}"))
            })?;

        // Kimi doesn't expose a separate reasoning_content field,
        // so reasoning is always None for now.
        Ok(WorkerDecision { action, reasoning: None })
    }
}

#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}
