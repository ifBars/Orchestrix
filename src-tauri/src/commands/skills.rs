use crate::core::skills::{
    add_custom_skill as add_custom_skill_record,
    import_context7_skill as import_context7_skill_record,
    import_vercel_skill as import_vercel_skill_record,
    install_agent_skill as install_agent_skill_record, list_all_skills,
    remove_custom_skill as remove_custom_skill_record,
    search_agent_skills as search_agent_skills_record, search_skills as search_skills_catalog,
    AgentSkillInstallResult, AgentSkillSearchItem, NewCustomSkill, SkillCatalogItem,
};
use crate::{load_workspace_root, AppError, AppState};

#[tauri::command]
pub fn list_available_skills() -> Vec<SkillCatalogItem> {
    list_all_skills()
}

#[tauri::command]
pub fn search_skills(
    query: String,
    source: Option<String>,
    limit: Option<usize>,
) -> Vec<SkillCatalogItem> {
    search_skills_catalog(&query, source.as_deref(), limit.unwrap_or(50).clamp(1, 250))
}

#[tauri::command]
pub fn add_custom_skill(skill: NewCustomSkill) -> Result<SkillCatalogItem, AppError> {
    add_custom_skill_record(skill).map_err(AppError::Other)
}

#[tauri::command]
pub fn remove_custom_skill(skill_id: String) -> Result<(), AppError> {
    remove_custom_skill_record(&skill_id).map_err(AppError::Other)?;
    Ok(())
}

#[tauri::command]
pub fn import_context7_skill(
    library_id: String,
    title: Option<String>,
) -> Result<SkillCatalogItem, AppError> {
    import_context7_skill_record(&library_id, title.as_deref()).map_err(AppError::Other)
}

#[tauri::command]
pub fn import_vercel_skill(skill_name: String) -> Result<SkillCatalogItem, AppError> {
    import_vercel_skill_record(&skill_name).map_err(AppError::Other)
}

#[tauri::command]
pub async fn search_agent_skills(
    query: String,
    limit: Option<usize>,
) -> Result<Vec<AgentSkillSearchItem>, AppError> {
    search_agent_skills_record(&query, limit.unwrap_or(25).clamp(1, 100))
        .await
        .map_err(AppError::Other)
}

#[tauri::command]
pub async fn install_agent_skill(
    state: tauri::State<'_, AppState>,
    skill_name: String,
    repo_url: Option<String>,
) -> Result<AgentSkillInstallResult, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    install_agent_skill_record(&workspace_root, &skill_name, repo_url)
        .await
        .map_err(AppError::Other)
}
