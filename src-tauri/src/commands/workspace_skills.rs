use crate::core::workspace_skills::{self, WorkspaceSkill};
use crate::{load_workspace_root, AppError, AppState};

#[tauri::command]
pub fn list_workspace_skills(state: tauri::State<'_, AppState>) -> Vec<WorkspaceSkill> {
    let workspace_root = load_workspace_root(&state.db);
    workspace_skills::scan_workspace_skills(&workspace_root)
}

#[tauri::command]
pub fn get_workspace_skill_content(
    state: tauri::State<'_, AppState>,
    skill_id: String,
) -> Result<WorkspaceSkill, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    let skills = workspace_skills::scan_workspace_skills(&workspace_root);
    skills
        .into_iter()
        .find(|s| s.id == skill_id)
        .ok_or_else(|| AppError::Other(format!("skill not found: {skill_id}")))
}

#[tauri::command]
pub fn read_workspace_skill_file(
    skill_dir: String,
    relative_path: String,
) -> Result<String, AppError> {
    workspace_skills::read_skill_file(&skill_dir, &relative_path).map_err(AppError::Other)
}

#[tauri::command]
pub fn get_active_skills_context(state: tauri::State<'_, AppState>) -> String {
    let workspace_root = load_workspace_root(&state.db);
    let skills = workspace_skills::scan_workspace_skills(&workspace_root);
    workspace_skills::build_skills_context(&skills)
}
