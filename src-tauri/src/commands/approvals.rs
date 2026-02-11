//! Approval request commands

use crate::runtime::planner::emit_and_record;
use crate::{AppError, AppState, ApprovalRequestView};

#[tauri::command]
pub fn list_pending_approvals(
    state: tauri::State<'_, AppState>,
    task_id: Option<String>,
) -> Result<Vec<ApprovalRequestView>, AppError> {
    let values = state
        .orchestrator
        .list_pending_approvals(task_id.as_deref())
        .into_iter()
        .map(ApprovalRequestView::from)
        .collect();
    Ok(values)
}

#[tauri::command]
pub fn resolve_approval_request(
    state: tauri::State<'_, AppState>,
    approval_id: String,
    approve: bool,
) -> Result<(), AppError> {
    let request = state
        .orchestrator
        .resolve_approval_request(&approval_id, approve)
        .map_err(AppError::Other)?;

    emit_and_record(
        &state.db,
        &state.bus,
        "tool",
        "tool.approval_user_decision",
        Some(request.run_id.clone()),
        serde_json::json!({
            "task_id": request.task_id,
            "sub_agent_id": request.sub_agent_id,
            "tool_call_id": request.tool_call_id,
            "approval_id": request.id,
            "approved": approve,
            "scope": request.scope,
        }),
    )
    .map_err(AppError::Other)?;

    Ok(())
}
