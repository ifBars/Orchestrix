//! Prompt suggestion service for generating contextual follow-up suggestions.
//!
//! This module provides:
//! - Settings management for suggestion behavior
//! - Conversation context extraction for suggestions
//! - Model-based suggestion generation

use crate::db::{queries, Database};
use crate::model::{GeminiClient, GlmClient, KimiClient, MiniMaxClient, ModalClient};
use chrono::Utc;

/// Default number of conversation turns to include in context.
const DEFAULT_CONTEXT_TURNS: u32 = 2;

/// Maximum tokens for suggestion requests.
const SUGGESTION_MAX_TOKENS: u32 = 4096;

/// Settings for prompt suggestion behavior.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PromptSuggestionSettings {
    /// Whether to enable prompt suggestions.
    pub enabled: bool,
    /// Number of recent message pairs (user + assistant) to include in context.
    pub context_turns: u32,
    /// Specific model to use for suggestions (None = use current task model).
    pub suggestion_model: Option<String>,
    /// Custom system prompt for suggestion generation (None = use default).
    pub system_prompt: Option<String>,
}

impl Default for PromptSuggestionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            context_turns: DEFAULT_CONTEXT_TURNS,
            suggestion_model: None,
            system_prompt: None,
        }
    }
}

/// A conversation message with role and content.
#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
}

/// Extract recent conversation context for suggestion generation.
pub fn extract_conversation_context(
    db: &Database,
    task_id: &str,
    context_turns: u32,
) -> Result<String, String> {
    let messages = queries::list_conversation_messages_for_task(db, task_id)
        .map_err(|e| format!("Failed to fetch conversation: {e}"))?;

    if messages.is_empty() {
        return Ok(String::new());
    }

    // Convert to conversation format and take last N turns (each turn = user + assistant)
    let conversation: Vec<ConversationMessage> = messages
        .into_iter()
        .map(|m| ConversationMessage {
            role: m.role,
            content: m.content,
        })
        .collect();

    // Calculate how many messages to take: 2 * context_turns (user + assistant pairs)
    let take_count = (context_turns as usize * 2).min(conversation.len());
    let recent: Vec<_> = conversation
        .into_iter()
        .rev()
        .take(take_count)
        .rev()
        .collect();

    // Format for the suggestion prompt
    let context = recent
        .into_iter()
        .map(|m| {
            let role = match m.role.as_str() {
                "user" => "User",
                _ => "Assistant",
            };
            format!("{}: {}", role, m.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    Ok(context)
}

/// Generate a prompt suggestion based on conversation context.
pub async fn generate_suggestion(
    db: &Database,
    task_id: &str,
    provider: &str,
    api_key: &str,
    model: Option<&str>,
    base_url: Option<&str>,
    settings: &PromptSuggestionSettings,
) -> Result<String, String> {
    // Extract conversation context
    let context = extract_conversation_context(db, task_id, settings.context_turns)?;

    if context.is_empty() {
        return Err("No conversation history available for suggestion".to_string());
    }

    // Use custom prompt if provided, otherwise use default
    let system_prompt = settings
        .system_prompt
        .as_ref()
        .cloned()
        .unwrap_or_else(default_suggestion_prompt);

    // Use specified model or fall back to current model
    let effective_model = settings.suggestion_model.as_deref().or(model);

    // Generate suggestion based on provider
    let suggestion = match provider {
        "kimi" => {
            suggest_with_kimi(api_key, effective_model, base_url, &system_prompt, &context).await
        }
        "zhipu" | "glm" => {
            suggest_with_glm(api_key, effective_model, base_url, &system_prompt, &context).await
        }
        "modal" => {
            suggest_with_modal(api_key, effective_model, base_url, &system_prompt, &context).await
        }
        "gemini" => {
            suggest_with_gemini(api_key, effective_model, base_url, &system_prompt, &context).await
        }
        _ => {
            suggest_with_minimax(api_key, effective_model, base_url, &system_prompt, &context).await
        }
    };

    Ok(suggestion.trim().to_string())
}

/// Suggest using Kimi model with simple completion.
async fn suggest_with_kimi(
    api_key: &str,
    model: Option<&str>,
    base_url: Option<&str>,
    system_prompt: &str,
    conversation: &str,
) -> String {
    let client = KimiClient::new(
        api_key.to_string(),
        model.map(String::from),
        base_url.map(String::from),
    );

    match client
        .complete(system_prompt, conversation, SUGGESTION_MAX_TOKENS)
        .await
    {
        Ok(suggestion) => suggestion,
        Err(e) => {
            tracing::warn!("Kimi suggestion failed: {}", e);
            String::new()
        }
    }
}

/// Suggest using MiniMax model with simple completion.
async fn suggest_with_minimax(
    api_key: &str,
    model: Option<&str>,
    base_url: Option<&str>,
    system_prompt: &str,
    conversation: &str,
) -> String {
    let client = MiniMaxClient::new_with_base_url(
        api_key.to_string(),
        model.map(String::from),
        base_url.map(String::from),
    );

    match client
        .complete(system_prompt, conversation, SUGGESTION_MAX_TOKENS)
        .await
    {
        Ok(suggestion) => suggestion,
        Err(e) => {
            tracing::warn!("MiniMax suggestion failed: {}", e);
            String::new()
        }
    }
}

/// Suggest using GLM (Zhipu) model with simple completion.
#[allow(dead_code)]
async fn suggest_with_glm(
    api_key: &str,
    model: Option<&str>,
    base_url: Option<&str>,
    system_prompt: &str,
    conversation: &str,
) -> String {
    let client = GlmClient::new(
        api_key.to_string(),
        model.map(String::from),
        base_url.map(String::from),
    );

    match client
        .complete(system_prompt, conversation, SUGGESTION_MAX_TOKENS)
        .await
    {
        Ok(suggestion) => suggestion,
        Err(e) => {
            tracing::warn!("GLM suggestion failed: {}", e);
            String::new()
        }
    }
}

/// Suggest using Modal model with simple completion.
#[allow(dead_code)]
async fn suggest_with_modal(
    api_key: &str,
    model: Option<&str>,
    base_url: Option<&str>,
    system_prompt: &str,
    conversation: &str,
) -> String {
    let client = ModalClient::new(
        api_key.to_string(),
        model.map(String::from),
        base_url.map(String::from),
    );

    match client
        .complete(system_prompt, conversation, SUGGESTION_MAX_TOKENS)
        .await
    {
        Ok(suggestion) => suggestion,
        Err(e) => {
            tracing::warn!("Modal suggestion failed: {}", e);
            String::new()
        }
    }
}

/// Suggest using Gemini model with simple completion.
#[allow(dead_code)]
async fn suggest_with_gemini(
    api_key: &str,
    model: Option<&str>,
    base_url: Option<&str>,
    system_prompt: &str,
    conversation: &str,
) -> String {
    let client = GeminiClient::new(
        api_key.to_string(),
        model.map(String::from),
        base_url.map(String::from),
    );

    match client
        .complete(system_prompt, conversation, SUGGESTION_MAX_TOKENS)
        .await
    {
        Ok(suggestion) => suggestion,
        Err(e) => {
            tracing::warn!("Gemini suggestion failed: {}", e);
            String::new()
        }
    }
}

/// Default prompt for suggestion generation.
fn default_suggestion_prompt() -> String {
    r#"Based on the recent conversation, suggest a brief (1-2 sentence) follow-up prompt the user might want to send next. Focus on natural next steps, clarifications, or related tasks. Be concise and practical.

Respond with ONLY the suggested prompt text, nothing else. Start directly with the prompt."#
        .to_string()
}

/// Load suggestion settings from database (or return defaults).
pub fn load_suggestion_settings(db: &Database) -> Result<PromptSuggestionSettings, String> {
    match queries::get_setting(db, "prompt_suggestion_settings") {
        Ok(Some(json_str)) => serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse suggestion settings: {e}")),
        Ok(None) => Ok(PromptSuggestionSettings::default()),
        Err(e) => Err(format!("Failed to load suggestion settings: {e}")),
    }
}

/// Save suggestion settings to database.
pub fn save_suggestion_settings(
    db: &Database,
    settings: &PromptSuggestionSettings,
) -> Result<(), String> {
    let json_str = serde_json::to_string(settings)
        .map_err(|e| format!("Failed to serialize suggestion settings: {e}"))?;

    queries::upsert_setting(
        db,
        "prompt_suggestion_settings",
        &json_str,
        &Utc::now().to_rfc3339(),
    )
    .map_err(|e| format!("Failed to save suggestion settings: {e}"))?;

    Ok(())
}
