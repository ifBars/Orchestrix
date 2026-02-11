//! Run query commands

use crate::db::queries;
use crate::{AppError, AppState};

#[tauri::command]
pub fn get_latest_run(
    state: tauri::State<'_, AppState>,
    task_id: String,
) -> Result<Option<queries::RunRow>, AppError> {
    Ok(queries::get_latest_run_for_task(&state.db, &task_id)?)
}

#[tauri::command]
pub fn list_sub_agents(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<Vec<queries::SubAgentRow>, AppError> {
    Ok(queries::list_sub_agents_for_run(&state.db, &run_id)?)
}

#[tauri::command]
pub fn list_run_artifacts(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<Vec<queries::ArtifactRow>, AppError> {
    Ok(queries::list_artifacts_for_run(&state.db, &run_id)?)
}

#[tauri::command]
pub fn list_tool_calls(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<Vec<queries::ToolCallRow>, AppError> {
    Ok(queries::list_tool_calls_for_run(&state.db, &run_id)?)
}

#[tauri::command]
pub fn get_run(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<Option<queries::RunRow>, AppError> {
    Ok(queries::get_run(&state.db, &run_id)?)
}

#[tauri::command]
pub fn list_user_messages(
    state: tauri::State<'_, AppState>,
    task_id: String,
) -> Result<Vec<queries::UserMessageRow>, AppError> {
    Ok(queries::list_user_messages_for_task(&state.db, &task_id)?)
}

#[tauri::command]
pub fn get_events_after(
    state: tauri::State<'_, AppState>,
    run_id: String,
    after_seq: i64,
) -> Result<Vec<queries::EventRow>, AppError> {
    Ok(queries::get_events_after_seq(
        &state.db, &run_id, after_seq,
    )?)
}

#[tauri::command]
pub fn get_task_events(
    state: tauri::State<'_, AppState>,
    task_id: String,
) -> Result<Vec<queries::EventRow>, AppError> {
    // Get all events for all runs of this task (not just the latest run)
    // This ensures conversation history is preserved across follow-up messages
    Ok(queries::list_events_for_task(&state.db, &task_id)?)
}
