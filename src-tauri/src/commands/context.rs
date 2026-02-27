use std::path::PathBuf;

use crate::core::preferences_memory::{self, PreferenceEntry};
use crate::{load_workspace_root, AppError, AppState};

#[derive(Debug, Clone, serde::Serialize)]
pub struct AutoMemorySettingsView {
    pub enabled: bool,
    pub source: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AutoMemoryPathView {
    pub path: String,
}

#[tauri::command]
pub fn get_auto_memory_settings(
    state: tauri::State<'_, AppState>,
) -> Result<AutoMemorySettingsView, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    let (enabled, source) = resolve_auto_memory_enabled(&workspace_root)?;
    Ok(AutoMemorySettingsView { enabled, source })
}

#[tauri::command]
pub fn set_auto_memory_settings(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), AppError> {
    let workspace_root = load_workspace_root(&state.db);
    let settings_path = workspace_root.join(".orchestrix").join("settings.json");

    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Other(format!("failed to create settings dir: {e}")))?;
    }

    let mut json = if settings_path.exists() {
        let raw = std::fs::read_to_string(&settings_path)
            .map_err(|e| AppError::Other(format!("failed to read settings: {e}")))?;
        serde_json::from_str::<serde_json::Value>(&raw).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if !json.is_object() {
        json = serde_json::json!({});
    }
    if let Some(obj) = json.as_object_mut() {
        obj.insert(
            "autoMemoryEnabled".to_string(),
            serde_json::Value::Bool(enabled),
        );
    }

    let serialized = serde_json::to_string_pretty(&json)
        .map_err(|e| AppError::Other(format!("failed to serialize settings: {e}")))?;
    std::fs::write(&settings_path, serialized)
        .map_err(|e| AppError::Other(format!("failed to write settings: {e}")))?;
    Ok(())
}

#[tauri::command]
pub fn get_auto_memory_entrypoint(
    state: tauri::State<'_, AppState>,
) -> Result<AutoMemoryPathView, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    let path = preferences_memory::memory_entrypoint_path_for_workspace(&workspace_root);
    Ok(AutoMemoryPathView {
        path: path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn list_auto_memory_preferences(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PreferenceEntry>, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    preferences_memory::list_preferences(&workspace_root).map_err(AppError::Other)
}

#[tauri::command]
pub fn read_auto_memory_context(state: tauri::State<'_, AppState>) -> Result<String, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    preferences_memory::startup_memory_context(&workspace_root).map_err(AppError::Other)
}

#[tauri::command]
pub fn compact_auto_memory(state: tauri::State<'_, AppState>) -> Result<usize, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    preferences_memory::compact_preferences(&workspace_root).map_err(AppError::Other)
}

#[tauri::command]
pub fn upsert_auto_memory_preference(
    state: tauri::State<'_, AppState>,
    key: String,
    value: String,
    category: Option<String>,
) -> Result<PreferenceEntry, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    preferences_memory::upsert_preference(&workspace_root, &key, &value, category.as_deref())
        .map_err(AppError::Other)
}

#[tauri::command]
pub fn delete_auto_memory_preference(
    state: tauri::State<'_, AppState>,
    key: String,
) -> Result<bool, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    preferences_memory::delete_preference(&workspace_root, &key).map_err(AppError::Other)
}

fn resolve_auto_memory_enabled(workspace_root: &PathBuf) -> Result<(bool, String), AppError> {
    if let Ok(value) = std::env::var("ORCHESTRIX_DISABLE_AUTO_MEMORY") {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized == "1" || normalized == "true" || normalized == "yes" {
            return Ok((false, "env:ORCHESTRIX_DISABLE_AUTO_MEMORY".to_string()));
        }
    }

    if let Ok(value) = std::env::var("ORCHESTRIX_AUTO_MEMORY_ENABLED") {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized == "0" || normalized == "false" || normalized == "no" {
            return Ok((false, "env:ORCHESTRIX_AUTO_MEMORY_ENABLED".to_string()));
        }
        if normalized == "1" || normalized == "true" || normalized == "yes" {
            return Ok((true, "env:ORCHESTRIX_AUTO_MEMORY_ENABLED".to_string()));
        }
    }

    let project_settings_path = workspace_root.join(".orchestrix").join("settings.json");
    if project_settings_path.exists() {
        let raw = std::fs::read_to_string(&project_settings_path)
            .map_err(|e| AppError::Other(format!("failed to read settings: {e}")))?;
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(enabled) = value.get("autoMemoryEnabled").and_then(|v| v.as_bool()) {
                return Ok((enabled, "project".to_string()));
            }
        }
    }

    Ok((true, "default".to_string()))
}
