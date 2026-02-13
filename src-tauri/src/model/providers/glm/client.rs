use crate::model::providers::openai_compat::OpenAiCompatClient;

const DEFAULT_GLM_BASE_URL: &str = "https://api.z.ai/api/coding/paas/v4";

pub struct GlmClient(OpenAiCompatClient);

impl GlmClient {
    pub fn new(api_key: String, model: Option<String>, base_url: Option<String>) -> Self {
        Self(OpenAiCompatClient::new(
            api_key,
            model,
            base_url.or_else(|| Some(DEFAULT_GLM_BASE_URL.to_string())),
            "GLM",
            "glm-5",
        ))
    }

    pub fn model_id(&self) -> String {
        self.0.model_id()
    }

    #[allow(dead_code)]
    pub async fn complete(&self, system: &str, user: &str, max_tokens: u32) -> Result<String, crate::model::ModelError> {
        self.0.complete(system, user, max_tokens).await
    }

    pub async fn decide_action_streaming<F>(&self, req: crate::model::WorkerActionRequest, on_delta: F) -> Result<crate::model::WorkerDecision, crate::model::ModelError>
    where
        F: FnMut(crate::model::StreamDelta) -> Result<(), String> + Send,
    {
        self.0.decide_action_streaming(req, on_delta).await
    }

    #[allow(dead_code)]
    pub async fn generate_plan_markdown(
        &self,
        task_prompt: &str,
        prior_markdown_context: &str,
        tool_descriptors: Vec<crate::core::tool::ToolDescriptor>,
    ) -> Result<String, crate::model::ModelError> {
        self.0.generate_plan_markdown(task_prompt, prior_markdown_context, tool_descriptors).await
    }
}

impl crate::model::AgentModelClient for GlmClient {
    fn model_id(&self) -> String {
        self.model_id()
    }

    async fn decide_action(&self, req: crate::model::WorkerActionRequest) -> Result<crate::model::WorkerDecision, crate::model::ModelError> {
        let noop = |_delta: crate::model::StreamDelta| Ok::<(), String>(());
        self.decide_action_streaming(req, noop).await
    }
}
