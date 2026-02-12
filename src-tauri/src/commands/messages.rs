//! User message commands with conversation summarization support.

use chrono::Utc;
use uuid::Uuid;

use crate::db::queries;
use crate::runtime::planner::emit_and_record;
use crate::runtime::summarization::{
    assemble_transcript, build_context_with_summary, get_or_generate_summary,
    load_compaction_settings, ConversationMessage,
};
use crate::{load_workspace_root, AppError, AppState};

#[tauri::command]
pub async fn send_message_to_task(
    state: tauri::State<'_, AppState>,
    task_id: String,
    message: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<(), AppError> {
    let message = message.trim();
    if message.is_empty() {
        return Err(AppError::Other("message cannot be empty".to_string()));
    }

    let task = queries::get_task(&state.db, &task_id)?
        .ok_or_else(|| AppError::Other(format!("task not found: {task_id}")))?;

    // Only allow follow-up messages on completed, failed, or cancelled tasks
    if task.status != "completed" && task.status != "failed" && task.status != "cancelled" {
        return Err(AppError::Other(format!(
            "can only send follow-up messages to completed, failed, or cancelled tasks (current status: {})",
            task.status
        )));
    }

    let workspace_root = load_workspace_root(&state.db);
    let resolved = super::execution::resolve_provider_model_for_prompt(
        &state.db,
        &workspace_root,
        &task.prompt,
        provider,
        model,
    )?;

    // Load compaction settings
    let compaction_settings =
        load_compaction_settings(&state.db).unwrap_or_default();

    // Create a new run for this continuation
    let run_id = Uuid::new_v4().to_string();
    queries::insert_run(
        &state.db,
        &queries::RunRow {
            id: run_id.clone(),
            task_id: task_id.clone(),
            status: "executing".to_string(),
            plan_json: Some(super::execution::build_run_context_json(
                "build_continue",
                &resolved.provider,
                resolved.effective_model.as_deref(),
                resolved.agent_preset_meta.as_ref(),
            )),
            started_at: Some(Utc::now().to_rfc3339()),
            finished_at: None,
            failure_reason: None,
        },
    )
    .map_err(|e| AppError::Other(e.to_string()))?;

    // Store the user message
    let message_id = Uuid::new_v4().to_string();
    queries::insert_user_message(
        &state.db,
        &queries::UserMessageRow {
            id: message_id.clone(),
            task_id: task_id.clone(),
            run_id: Some(run_id.clone()),
            content: message.to_string(),
            created_at: Utc::now().to_rfc3339(),
        },
    )
    .map_err(|e| AppError::Other(e.to_string()))?;

    // Emit event for the user message
    emit_and_record(
        &state.db,
        &state.bus,
        "user",
        "user.message_sent",
        Some(run_id.clone()),
        serde_json::json!({
            "task_id": task_id,
            "message_id": message_id,
            "content": message,
        }),
    )
    .map_err(AppError::Other)?;

    // Assemble conversation transcript and check if compaction is needed
    let transcript = assemble_transcript(&state.db, &task_id)
        .map_err(|e| AppError::Other(format!("Failed to assemble transcript: {e}")))?;

    // Determine which model to use for compaction
    let compaction_model = compaction_settings
        .compaction_model
        .as_deref()
        .or(resolved.effective_model.as_deref());

    // Generate or retrieve summary if compaction is enabled and needed
    let (summary, recent_messages) = if compaction_settings.enabled && transcript.needs_compaction {
        tracing::info!(
            "Conversation compaction triggered for task {}: {} messages, ~{} tokens",
            task_id,
            transcript.messages.len(),
            transcript.estimated_tokens
        );

        // Emit compaction started event
        let _ = emit_and_record(
            &state.db,
            &state.bus,
            "agent",
            "agent.compaction_started",
            Some(run_id.clone()),
            serde_json::json!({
                "task_id": task_id,
                "message_count": transcript.messages.len(),
                "estimated_tokens": transcript.estimated_tokens,
                "threshold": compaction_settings.threshold_tokens(compaction_model),
            }),
        );

        let summary = get_or_generate_summary(
            &state.db,
            &task_id,
            &run_id,
            &resolved.provider,
            &resolved.cfg.api_key,
            compaction_model,
            resolved.cfg.base_url.as_deref(),
            &compaction_settings,
            false, // Don't force regenerate if we have a recent one
        )
        .await
        .map_err(|e| AppError::Other(format!("Failed to generate summary: {e}")))?;

        // Emit compaction completed event
        let _ = emit_and_record(
            &state.db,
            &state.bus,
            "agent",
            "agent.compaction_completed",
            Some(run_id.clone()),
            serde_json::json!({
                "task_id": task_id,
                "summary_id": summary.id,
                "summary_length": summary.summary_text.len(),
                "messages_summarized": summary.message_count,
                "token_estimate": summary.token_estimate,
            }),
        );

        // Get recent messages to preserve verbatim
        let recent: Vec<ConversationMessage> = transcript
            .messages
            .iter()
            .rev()
            .take(compaction_settings.preserve_recent)
            .cloned()
            .collect();

        (Some(summary), recent)
    } else {
        // No compaction needed or disabled - use all messages
        let all_messages: Vec<ConversationMessage> = transcript.messages;
        (None, all_messages)
    };

    // Build the continue prompt with proper context
    let continue_prompt = if let Some(ref summary) = summary {
        // Use compacted context
        build_context_with_summary(summary, &recent_messages, message)
    } else {
        // Use full transcript
        let conversation_history = recent_messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        format!(
            "## Original Task\n\n{}\n\n## Conversation History\n\n{}\n\n## Current Request\n\n{}",
            task.prompt, conversation_history, message
        )
    };

    // Update task status to executing
    queries::update_task_status(&state.db, &task_id, "executing", &Utc::now().to_rfc3339())?;
    emit_and_record(
        &state.db,
        &state.bus,
        "task",
        "task.status_changed",
        Some(run_id.clone()),
        serde_json::json!({ "task_id": task_id, "status": "executing" }),
    )
    .map_err(AppError::Other)?;

    // Start the task in build mode (direct execution, no planning)
    state
        .orchestrator
        .continue_task_with_message(
            task,
            continue_prompt,
            resolved.provider,
            resolved.cfg.api_key,
            resolved.effective_model,
            resolved.cfg.base_url,
        )
        .map_err(AppError::Other)?;

    Ok(())
}

#[tauri::command]
pub fn get_compaction_settings(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, AppError> {
    let settings =
        load_compaction_settings(&state.db).map_err(|e| AppError::Other(e.to_string()))?;
    Ok(serde_json::to_value(settings).map_err(|e| AppError::Other(e.to_string()))?)
}

#[tauri::command]
pub fn set_compaction_settings(
    state: tauri::State<'_, AppState>,
    settings: serde_json::Value,
) -> Result<(), AppError> {
    let settings = serde_json::from_value(settings)
        .map_err(|e| AppError::Other(format!("Invalid compaction settings: {e}")))?;
    crate::runtime::summarization::save_compaction_settings(&state.db, &settings)
        .map_err(|e| AppError::Other(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn get_conversation_summary(
    state: tauri::State<'_, AppState>,
    task_id: String,
) -> Result<Option<serde_json::Value>, AppError> {
    let summary = queries::get_latest_conversation_summary(&state.db, &task_id)
        .map_err(|e| AppError::Other(e.to_string()))?;

    match summary {
        Some(s) => Ok(Some(serde_json::json!({
            "id": s.id,
            "summary": s.summary,
            "message_count": s.message_count,
            "token_estimate": s.token_estimate,
            "created_at": s.created_at,
        }))),
        None => Ok(None),
    }
}
