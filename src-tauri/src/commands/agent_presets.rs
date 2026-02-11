//! Agent preset commands.

use crate::core::agent_presets::{self, AgentPreset};
use crate::{load_workspace_root, AppError, AppState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentPresetInput {
    pub id: String,
    pub name: String,
    pub description: String,
    pub mode: String, // "primary" or "subagent"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<HashMap<String, serde_json::Value>>,
}

#[tauri::command]
pub fn list_agent_presets(state: tauri::State<'_, AppState>) -> Vec<AgentPreset> {
    let workspace_root = load_workspace_root(&state.db);
    agent_presets::scan_agent_presets(&workspace_root)
}

#[tauri::command]
pub fn get_agent_preset(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<AgentPreset, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    agent_presets::get_agent_preset(&workspace_root, &id)
        .ok_or_else(|| AppError::Other(format!("agent preset not found: {}", id)))
}

#[tauri::command]
pub fn search_agent_presets(state: tauri::State<'_, AppState>, query: String) -> Vec<AgentPreset> {
    let workspace_root = load_workspace_root(&state.db);
    let presets = agent_presets::scan_agent_presets(&workspace_root);

    if query.trim().is_empty() {
        return presets;
    }

    let query_lower = query.to_ascii_lowercase();
    presets
        .into_iter()
        .filter(|p| {
            p.id.to_ascii_lowercase().contains(&query_lower)
                || p.name.to_ascii_lowercase().contains(&query_lower)
                || p.description.to_ascii_lowercase().contains(&query_lower)
                || p.tags
                    .iter()
                    .any(|t| t.to_ascii_lowercase().contains(&query_lower))
        })
        .collect()
}

#[tauri::command]
pub fn create_agent_preset(
    state: tauri::State<'_, AppState>,
    input: CreateAgentPresetInput,
) -> Result<AgentPreset, AppError> {
    use crate::core::agent_presets::{AgentMode, AgentPreset, ToolPermission};

    let workspace_root = load_workspace_root(&state.db);

    // Validate ID format (kebab-case, alphanumeric, hyphens, underscores)
    let valid_id = input
        .id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid_id || input.id.is_empty() {
        return Err(AppError::Other(format!(
            "invalid agent ID '{}': must be alphanumeric with hyphens/underscores only",
            input.id
        )));
    }

    // Parse mode
    let mode = match input.mode.as_str() {
        "primary" => AgentMode::Primary,
        "subagent" => AgentMode::Subagent,
        _ => {
            return Err(AppError::Other(format!(
                "invalid mode '{}': must be 'primary' or 'subagent'",
                input.mode
            )));
        }
    };

    // Convert tools input
    let tools = input.tools.map(|t| {
        t.into_iter()
            .map(|(k, v)| {
                let perm = match v {
                    serde_json::Value::Bool(b) => ToolPermission::Bool(b),
                    _ => ToolPermission::Inherit,
                };
                (k, perm)
            })
            .collect()
    });

    // Build preset
    let preset = AgentPreset {
        id: input.id.clone(),
        name: input.name,
        description: input.description,
        mode,
        model: input.model,
        temperature: input.temperature,
        steps: input.steps,
        tools,
        permission: None, // Simplified for creation
        prompt: input.prompt,
        tags: input.tags.unwrap_or_default(),
        file_path: String::new(), // Will be set by write_agent_preset
        source: "workspace".to_string(),
        enabled: true,
        validation_issues: vec![],
    };

    // Write to file
    let file_path = agent_presets::write_agent_preset(&workspace_root, &input.id, &preset)
        .map_err(AppError::Other)?;

    // Return the saved preset
    let mut saved = preset;
    saved.file_path = file_path;
    Ok(saved)
}

#[tauri::command]
pub fn update_agent_preset(
    state: tauri::State<'_, AppState>,
    input: CreateAgentPresetInput,
) -> Result<AgentPreset, AppError> {
    // For simplicity, update is same as create (overwrites existing)
    // The write_agent_preset function will overwrite existing files
    create_agent_preset(state, input)
}

#[tauri::command]
pub fn delete_agent_preset(state: tauri::State<'_, AppState>, id: String) -> Result<(), AppError> {
    let workspace_root = load_workspace_root(&state.db);
    agent_presets::delete_agent_preset(&workspace_root, &id).map_err(AppError::Other)
}

#[tauri::command]
pub fn read_agent_preset_file(file_path: String) -> Result<String, AppError> {
    agent_presets::read_agent_file(&file_path).map_err(AppError::Other)
}

#[tauri::command]
pub fn get_agent_preset_context(
    state: tauri::State<'_, AppState>,
    preset_id: String,
) -> Result<String, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    let preset = agent_presets::get_agent_preset(&workspace_root, &preset_id)
        .ok_or_else(|| AppError::Other(format!("agent preset not found: {}", preset_id)))?;

    let context = format!(
        "## Agent: {}\n\n{}\n\nMode: {:?}\nConstraints: {}\n\n{}",
        preset.name,
        preset.description,
        preset.mode,
        preset.constraints_summary(),
        preset.prompt
    );

    Ok(context)
}
