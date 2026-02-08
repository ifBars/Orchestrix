use serde::Serialize;

use crate::db::queries;
use crate::runtime::worktree::WorktreeInfo;
use crate::{load_workspace_root, AppError, AppState};

/// Serializable view of an active worktree for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct WorktreeView {
    path: String,
    branch: Option<String>,
    strategy: String,
    run_id: String,
    sub_agent_id: String,
    base_ref: Option<String>,
}

impl From<WorktreeInfo> for WorktreeView {
    fn from(info: WorktreeInfo) -> Self {
        Self {
            path: info.path.to_string_lossy().to_string(),
            branch: info.branch,
            strategy: info.strategy.to_string(),
            run_id: info.run_id,
            sub_agent_id: info.sub_agent_id,
            base_ref: info.base_ref,
        }
    }
}

#[tauri::command]
pub fn list_active_worktrees(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<WorktreeView>, AppError> {
    let active = state.orchestrator.worktree_manager().list_active();
    Ok(active.into_iter().map(WorktreeView::from).collect())
}

#[tauri::command]
pub fn list_run_worktrees(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<Vec<WorktreeView>, AppError> {
    let worktrees = state.orchestrator.worktree_manager().list_for_run(&run_id);
    Ok(worktrees.into_iter().map(WorktreeView::from).collect())
}

#[tauri::command]
pub fn list_worktree_logs(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<Vec<queries::WorktreeLogRow>, AppError> {
    Ok(queries::list_worktree_logs_for_run(&state.db, &run_id)?)
}

#[tauri::command]
pub fn cleanup_run_worktrees(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<Vec<String>, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    let cleaned = state
        .orchestrator
        .worktree_manager()
        .cleanup_run(&workspace_root, &run_id)
        .map_err(|e| AppError::Other(e.to_string()))?;
    Ok(cleaned)
}

#[tauri::command]
pub fn prune_stale_worktrees(state: tauri::State<'_, AppState>) -> Result<Vec<String>, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    let pruned = state
        .orchestrator
        .worktree_manager()
        .prune_stale(&workspace_root)
        .map_err(|e| AppError::Other(e.to_string()))?;
    Ok(pruned)
}

#[tauri::command]
pub fn list_git_worktrees(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::runtime::worktree::GitWorktreeEntry>, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    crate::runtime::worktree::list_git_worktrees(&workspace_root)
        .map_err(|e| AppError::Other(e.to_string()))
}
