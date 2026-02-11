//! User message commands

use chrono::Utc;
use uuid::Uuid;

use crate::db::queries;
use crate::runtime::planner::emit_and_record;
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

    // Start the orchestrator with the follow-up message
    // Build context that includes the original prompt and the new message
    let continue_prompt = format!(
        "Previous task: {}\n\nFollow-up request: {}\n\nPlease continue from where we left off and address the follow-up request. Review the conversation history and previous artifacts if needed.",
        task.prompt, message
    );

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
