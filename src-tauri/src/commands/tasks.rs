use chrono::Utc;
use uuid::Uuid;

use crate::db::queries;
use crate::runtime::planner::emit_and_record;
use crate::{AppError, AppState, CreateTaskOptions};

#[tauri::command]
pub fn create_task(
    state: tauri::State<'_, AppState>,
    prompt: String,
    options: Option<CreateTaskOptions>,
) -> Result<queries::TaskRow, AppError> {
    let now = Utc::now().to_rfc3339();
    let parent_task_id = options
        .as_ref()
        .and_then(|opts| opts.parent_task_id.clone())
        .filter(|value| !value.trim().is_empty());

    if let Some(parent_id) = parent_task_id.as_ref() {
        if queries::get_task(&state.db, parent_id)?.is_none() {
            return Err(AppError::Other(format!(
                "parent task not found: {parent_id}"
            )));
        }
    }

    let row = queries::TaskRow {
        id: Uuid::new_v4().to_string(),
        prompt,
        parent_task_id: parent_task_id.clone(),
        status: "pending".to_string(),
        created_at: now.clone(),
        updated_at: now,
    };
    queries::insert_task(&state.db, &row)?;

    if let Some(parent_id) = parent_task_id.as_ref() {
        queries::upsert_task_link(&state.db, &row.id, parent_id, &row.created_at)?;
    }

    if let Some(reference_ids) = options.and_then(|opts| opts.reference_task_ids) {
        for reference_id in reference_ids {
            if reference_id == row.id {
                continue;
            }
            if queries::get_task(&state.db, &reference_id)?.is_none() {
                continue;
            }
            queries::upsert_task_link(&state.db, &row.id, &reference_id, &row.created_at)?;
        }
    }

    emit_and_record(
        &state.db,
        &state.bus,
        "task",
        "task.created",
        None,
        serde_json::json!({
            "task_id": &row.id,
            "prompt": &row.prompt,
            "parent_task_id": row.parent_task_id,
        }),
    )
    .map_err(AppError::Other)?;

    // Emit user message event so the prompt appears in the chat UI immediately
    emit_and_record(
        &state.db,
        &state.bus,
        "user",
        "user.message_sent",
        None,
        serde_json::json!({
            "task_id": &row.id,
            "content": &row.prompt,
        }),
    )
    .map_err(AppError::Other)?;

    Ok(row)
}

#[tauri::command]
pub fn list_tasks(state: tauri::State<'_, AppState>) -> Result<Vec<queries::TaskRow>, AppError> {
    Ok(queries::list_tasks(&state.db)?)
}

#[tauri::command]
pub fn list_task_links(
    state: tauri::State<'_, AppState>,
    task_id: String,
) -> Result<Vec<queries::TaskLinkRow>, AppError> {
    Ok(queries::list_task_links_for_task(&state.db, &task_id)?)
}

#[tauri::command]
pub fn link_tasks(
    state: tauri::State<'_, AppState>,
    task_id: String,
    related_task_id: String,
) -> Result<(), AppError> {
    if task_id == related_task_id {
        return Err(AppError::Other(
            "cannot link a conversation to itself".to_string(),
        ));
    }
    if queries::get_task(&state.db, &task_id)?.is_none() {
        return Err(AppError::Other(format!("task not found: {task_id}")));
    }
    if queries::get_task(&state.db, &related_task_id)?.is_none() {
        return Err(AppError::Other(format!(
            "task not found: {related_task_id}"
        )));
    }

    let now = Utc::now().to_rfc3339();
    queries::upsert_task_link(&state.db, &task_id, &related_task_id, &now)?;

    emit_and_record(
        &state.db,
        &state.bus,
        "task",
        "task.linked",
        None,
        serde_json::json!({
            "task_id": task_id,
            "related_task_id": related_task_id,
        }),
    )
    .map_err(AppError::Other)?;
    Ok(())
}

#[tauri::command]
pub fn unlink_tasks(
    state: tauri::State<'_, AppState>,
    task_id: String,
    related_task_id: String,
) -> Result<(), AppError> {
    queries::delete_task_link(&state.db, &task_id, &related_task_id)?;

    emit_and_record(
        &state.db,
        &state.bus,
        "task",
        "task.unlinked",
        None,
        serde_json::json!({
            "task_id": task_id,
            "related_task_id": related_task_id,
        }),
    )
    .map_err(AppError::Other)?;
    Ok(())
}

#[tauri::command]
pub fn delete_task(state: tauri::State<'_, AppState>, task_id: String) -> Result<(), AppError> {
    state.orchestrator.cancel_task(&task_id);
    queries::delete_task_cascade(&state.db, &task_id)?;

    emit_and_record(
        &state.db,
        &state.bus,
        "task",
        "task.deleted",
        None,
        serde_json::json!({ "task_id": task_id }),
    )
    .map_err(AppError::Other)?;
    Ok(())
}

#[tauri::command]
pub fn get_task(
    state: tauri::State<'_, AppState>,
    task_id: String,
) -> Result<Option<queries::TaskRow>, AppError> {
    Ok(queries::get_task(&state.db, &task_id)?)
}

#[tauri::command]
pub fn cancel_task(state: tauri::State<'_, AppState>, task_id: String) -> Result<(), AppError> {
    state.orchestrator.cancel_task(&task_id);

    queries::update_task_status(&state.db, &task_id, "cancelled", &Utc::now().to_rfc3339())?;
    emit_and_record(
        &state.db,
        &state.bus,
        "task",
        "task.status_changed",
        None,
        serde_json::json!({ "task_id": task_id, "status": "cancelled" }),
    )
    .map_err(AppError::Other)?;
    Ok(())
}

#[tauri::command]
pub async fn start_task(
    state: tauri::State<'_, AppState>,
    task_id: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<(), AppError> {
    super::runs::run_plan_mode(state, task_id, provider, model).await
}
