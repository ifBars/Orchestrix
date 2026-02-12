//! Conversation summarization service for managing chat context.
//!
//! This module provides:
//! - Transcript assembly from conversation events
//! - Model-based summarization of conversation history
//! - Token budget management and compaction triggers
//! - Persistent storage of conversation summaries

use crate::db::{queries, Database};
use crate::model::{KimiClient, MiniMaxClient, ModelCatalog};
use chrono::Utc;
use uuid::Uuid;

/// Default percentage of context window to trigger compaction (80%).
const DEFAULT_COMPACTION_PERCENTAGE: f32 = 0.8;

/// Default number of recent messages to preserve verbatim during compaction.
const DEFAULT_PRESERVE_RECENT: usize = 4;

/// Maximum tokens for summarization requests.
const SUMMARIZATION_MAX_TOKENS: u32 = 32768;

/// Settings for conversation compaction behavior.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompactionSettings {
    /// Whether to enable automatic compaction.
    pub enabled: bool,
    /// Percentage of context window to trigger compaction (0.0-1.0).
    pub threshold_percentage: f32,
    /// Number of recent messages to preserve verbatim.
    pub preserve_recent: usize,
    /// Custom prompt for summarization (optional).
    pub custom_prompt: Option<String>,
    /// Specific model to use for compaction (optional, defaults to task model).
    pub compaction_model: Option<String>,
}

impl Default for CompactionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_percentage: DEFAULT_COMPACTION_PERCENTAGE,
            preserve_recent: DEFAULT_PRESERVE_RECENT,
            custom_prompt: None,
            compaction_model: None,
        }
    }
}

impl CompactionSettings {
    /// Calculate the actual token threshold based on model context window.
    pub fn threshold_tokens(&self, model: Option<&str>) -> usize {
        let context_window = get_model_context_window(model.unwrap_or(""));
        (context_window as f32 * self.threshold_percentage) as usize
    }
}

/// Returns the context window size for a given model.
pub fn get_model_context_window(model: &str) -> usize {
    ModelCatalog::all_models()
        .into_iter()
        .flat_map(|entry| entry.models)
        .find(|entry| entry.name == model)
        .map(|entry| entry.context_window as usize)
        .unwrap_or(8_192)
}

/// A conversation message with role and content.
#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
}

/// Result of assembling a conversation transcript.
pub struct TranscriptResult {
    /// The assembled messages.
    pub messages: Vec<ConversationMessage>,
    /// Estimated token count (rough approximation: 4 chars ~= 1 token).
    pub estimated_tokens: usize,
    /// Whether compaction is recommended based on token budget.
    pub needs_compaction: bool,
}

/// Summary of a conversation segment.
#[derive(Debug, Clone)]
pub struct ConversationSummary {
    pub id: String,
    pub summary_text: String,
    pub message_count: usize,
    pub token_estimate: usize,
    pub created_at: String,
}

/// Assemble conversation transcript for a task.
pub fn assemble_transcript(db: &Database, task_id: &str) -> Result<TranscriptResult, String> {
    let messages = queries::list_conversation_messages_for_task(db, task_id)
        .map_err(|e| format!("Failed to fetch conversation: {e}"))?;

    let conversation: Vec<ConversationMessage> = messages
        .into_iter()
        .map(|m| ConversationMessage {
            role: m.role,
            content: m.content,
        })
        .collect();

    let estimated_tokens = estimate_tokens(&conversation);

    // We don't know the model yet, so return with needs_compaction=false
    // The caller will re-evaluate with the correct model
    Ok(TranscriptResult {
        messages: conversation,
        estimated_tokens,
        needs_compaction: false,
    })
}

/// Check if compaction is needed for the given model.
pub fn check_compaction_needed(
    estimated_tokens: usize,
    model: Option<&str>,
    settings: &CompactionSettings,
) -> bool {
    if !settings.enabled {
        return false;
    }
    let threshold = settings.threshold_tokens(model);
    estimated_tokens > threshold
}

/// Estimate token count from conversation (rough approximation).
fn estimate_tokens(messages: &[ConversationMessage]) -> usize {
    messages
        .iter()
        .map(|m| m.content.len() / 4 + m.role.len() / 4 + 4)
        .sum()
}

/// Generate a summary of conversation history using the specified model.
pub async fn generate_summary(
    db: &Database,
    task_id: &str,
    run_id: &str,
    provider: &str,
    api_key: &str,
    model: Option<&str>,
    base_url: Option<&str>,
    settings: &CompactionSettings,
) -> Result<ConversationSummary, String> {
    // Get the transcript
    let transcript = assemble_transcript(db, task_id)?;

    if transcript.messages.is_empty() {
        return Ok(ConversationSummary {
            id: Uuid::new_v4().to_string(),
            summary_text: "No prior conversation history.".to_string(),
            message_count: 0,
            token_estimate: 0,
            created_at: Utc::now().to_rfc3339(),
        });
    }

    // Build the conversation text for summarization
    let conversation_text = format_conversation_for_summary(&transcript.messages, settings);

    // Use custom prompt if provided, otherwise use default
    let system_prompt = settings
        .custom_prompt
        .as_ref()
        .cloned()
        .unwrap_or_else(default_summary_prompt);

    // Generate summary based on provider using simple completion (no agent loop)
    let summary_text = if provider == "kimi" {
        summarize_with_kimi(api_key, model, base_url, &system_prompt, &conversation_text).await
    } else {
        summarize_with_minimax(api_key, model, base_url, &system_prompt, &conversation_text).await
    };

    let summary = ConversationSummary {
        id: Uuid::new_v4().to_string(),
        summary_text: summary_text.trim().to_string(),
        message_count: transcript.messages.len(),
        token_estimate: estimate_summary_tokens(&summary_text),
        created_at: Utc::now().to_rfc3339(),
    };

    // Persist the summary
    let row = queries::ConversationSummaryRow {
        id: summary.id.clone(),
        task_id: task_id.to_string(),
        run_id: run_id.to_string(),
        summary: summary.summary_text.clone(),
        message_count: summary.message_count as i64,
        token_estimate: Some(summary.token_estimate as i64),
        created_at: summary.created_at.clone(),
    };
    queries::insert_conversation_summary(db, &row)
        .map_err(|e| format!("Failed to save summary: {e}"))?;

    Ok(summary)
}

/// Summarize using Kimi model with simple completion (no agent loop).
async fn summarize_with_kimi(
    api_key: &str,
    model: Option<&str>,
    base_url: Option<&str>,
    system_prompt: &str,
    conversation: &str,
) -> String {
    let planner = KimiClient::new(
        api_key.to_string(),
        model.map(String::from),
        base_url.map(String::from),
    );

    match planner
        .complete(system_prompt, conversation, SUMMARIZATION_MAX_TOKENS)
        .await
    {
        Ok(summary) => summary,
        Err(e) => {
            tracing::warn!("Kimi summarization failed: {}, falling back to basic summary", e);
            generate_fallback_summary(conversation)
        }
    }
}

/// Summarize using MiniMax model with simple completion (no agent loop).
async fn summarize_with_minimax(
    api_key: &str,
    model: Option<&str>,
    base_url: Option<&str>,
    system_prompt: &str,
    conversation: &str,
) -> String {
    let planner = MiniMaxClient::new_with_base_url(
        api_key.to_string(),
        model.map(String::from),
        base_url.map(String::from),
    );

    match planner
        .complete(system_prompt, conversation, SUMMARIZATION_MAX_TOKENS)
        .await
    {
        Ok(summary) => summary,
        Err(e) => {
            tracing::warn!("MiniMax summarization failed: {}, falling back to basic summary", e);
            generate_fallback_summary(conversation)
        }
    }
}

/// Generate a fallback summary when API calls fail.
fn generate_fallback_summary(conversation: &str) -> String {
    let char_count = conversation.len();
    let line_count = conversation.lines().count();

    format!(
        "**Previous Work Summary** ({} characters, ~{} lines of conversation)

- **Context**: Prior work established codebase foundation and requirements
- **Key Decisions**: Technical approach was agreed upon, implementation partially completed
- **Current Status**: Work in progress with ongoing refinements
- **Note**: This is a fallback summary due to API unavailability. Full conversation history is preserved in the database.

[Conversation length: {} chars]",
        char_count, line_count, char_count
    )
}

/// Get or generate a summary for a task.
pub async fn get_or_generate_summary(
    db: &Database,
    task_id: &str,
    run_id: &str,
    provider: &str,
    api_key: &str,
    model: Option<&str>,
    base_url: Option<&str>,
    settings: &CompactionSettings,
    force_regenerate: bool,
) -> Result<ConversationSummary, String> {
    // Check if we have a recent summary and don't need to regenerate
    if !force_regenerate && !settings.enabled {
        if let Some(existing) = queries::get_latest_conversation_summary(db, task_id)
            .map_err(|e| format!("Failed to fetch summary: {e}"))?
        {
            return Ok(ConversationSummary {
                id: existing.id,
                summary_text: existing.summary,
                message_count: existing.message_count as usize,
                token_estimate: existing.token_estimate.unwrap_or(0) as usize,
                created_at: existing.created_at,
            });
        }
    }

    // Check if compaction is actually needed
    let transcript = assemble_transcript(db, task_id)?;
    let needs_compaction = check_compaction_needed(transcript.estimated_tokens, model, settings);

    if !force_regenerate && !needs_compaction {
        return Ok(ConversationSummary {
            id: Uuid::new_v4().to_string(),
            summary_text: format_conversation_for_summary(&transcript.messages, settings),
            message_count: transcript.messages.len(),
            token_estimate: transcript.estimated_tokens,
            created_at: Utc::now().to_rfc3339(),
        });
    }

    // Generate new summary
    generate_summary(
        db,
        task_id,
        run_id,
        provider,
        api_key,
        model,
        base_url,
        settings,
    )
    .await
}

/// Format conversation messages for summarization.
fn format_conversation_for_summary(
    messages: &[ConversationMessage],
    settings: &CompactionSettings,
) -> String {
    if messages.len() <= settings.preserve_recent {
        // Don't compact if under threshold
        return messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n\n");
    }

    // Split into older (to summarize) and recent (to preserve verbatim)
    let split_point = messages.len().saturating_sub(settings.preserve_recent);
    let (older, recent) = messages.split_at(split_point);

    let mut result = String::new();

    // Add older messages
    result.push_str("=== Conversation History (to summarize) ===\n\n");
    for msg in older {
        result.push_str(&format!("{}: {}\n\n", msg.role, msg.content));
    }

    // Add separator
    result.push_str("\n=== Recent Messages (preserve verbatim) ===\n\n");

    // Add recent messages
    for msg in recent {
        result.push_str(&format!("{}: {}\n\n", msg.role, msg.content));
    }

    result
}

/// Default prompt for conversation summarization.
fn default_summary_prompt() -> String {
    r#"You are a conversation summarization assistant. Your task is to create a concise summary of the conversation history provided.

Focus on:
1. What was accomplished in previous turns
2. Key decisions or outputs from prior work
3. What the user is asking for in their latest message
4. Any important context needed to continue the work

Format your summary as:
- **Previous Work**: Brief description of what was done
- **Key Outputs**: Important files, decisions, or results
- **Current Request**: What the user wants now
- **Context for Next Steps**: Any important background needed

Keep the summary concise but comprehensive enough to maintain context continuity."#
    .to_string()
}

/// Estimate tokens in a summary (rough approximation).
fn estimate_summary_tokens(summary: &str) -> usize {
    summary.len() / 4 + 50 // Base overhead
}

/// Build a context string for follow-up tasks that includes summary + recent messages.
pub fn build_context_with_summary(
    summary: &ConversationSummary,
    recent_messages: &[ConversationMessage],
    task_prompt: &str,
) -> String {
    let mut context = String::new();

    // Add summary of previous work
    if !summary.summary_text.is_empty() {
        context.push_str("## Previous Conversation Summary\n\n");
        context.push_str(&summary.summary_text);
        context.push_str("\n\n");
    }

    // Add recent messages
    if !recent_messages.is_empty() {
        context.push_str("## Recent Messages\n\n");
        for msg in recent_messages {
            context.push_str(&format!("**{}**: {}\n\n", msg.role, msg.content));
        }
    }

    // Add the current task
    context.push_str("## Current Request\n\n");
    context.push_str(task_prompt);

    context
}

/// Load compaction settings from database (or return defaults).
pub fn load_compaction_settings(db: &Database) -> Result<CompactionSettings, String> {
    match queries::get_setting(db, "compaction_settings") {
        Ok(Some(json_str)) => {
            serde_json::from_str(&json_str)
                .map_err(|e| format!("Failed to parse compaction settings: {e}"))
        }
        Ok(None) => Ok(CompactionSettings::default()),
        Err(e) => Err(format!("Failed to load compaction settings: {e}")),
    }
}

/// Save compaction settings to database.
pub fn save_compaction_settings(
    db: &Database,
    settings: &CompactionSettings,
) -> Result<(), String> {
    let json_str = serde_json::to_string(settings)
        .map_err(|e| format!("Failed to serialize compaction settings: {e}"))?;

    queries::upsert_setting(
        db,
        "compaction_settings",
        &json_str,
        &Utc::now().to_rfc3339(),
    )
    .map_err(|e| format!("Failed to save compaction settings: {e}"))?;

    Ok(())
}
