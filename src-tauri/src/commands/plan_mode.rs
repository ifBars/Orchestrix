//! Plan mode settings commands for configuring token limits during planning.

use crate::runtime::plan_mode_settings::{
    get_plan_mode_max_tokens, load_plan_mode_settings, save_plan_mode_settings, PlanModeSettings,
};
use crate::{AppError, AppState};

#[tauri::command]
pub fn get_plan_mode_settings(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, AppError> {
    let settings = load_plan_mode_settings(&state.db).map_err(|e| AppError::Other(e))?;
    Ok(serde_json::to_value(settings).map_err(|e| AppError::Other(e.to_string()))?)
}

#[tauri::command]
pub fn set_plan_mode_settings(
    state: tauri::State<'_, AppState>,
    settings: serde_json::Value,
) -> Result<(), AppError> {
    let settings: PlanModeSettings = serde_json::from_value(settings)
        .map_err(|e| AppError::Other(format!("Invalid plan mode settings: {e}")))?;
    save_plan_mode_settings(&state.db, &settings).map_err(|e| AppError::Other(e))?;
    Ok(())
}

#[tauri::command]
pub fn get_plan_mode_max_tokens_command(
    state: tauri::State<'_, AppState>,
) -> Result<u32, AppError> {
    Ok(get_plan_mode_max_tokens(&state.db))
}
