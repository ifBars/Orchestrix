use std::path::PathBuf;

use crate::core::preferences_memory::{self, PreferenceEntry};
use crate::core::tool::ToolDescriptor;
use crate::db::queries;
use crate::model::ModelCatalog;
use crate::runtime::summarization::{
    assemble_transcript, check_compaction_needed, load_compaction_settings, ConversationMessage,
};
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextUsageSegmentView {
    pub key: String,
    pub label: String,
    pub tokens: usize,
    pub percentage: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskContextSnapshotView {
    pub task_id: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub context_window: usize,
    pub used_tokens: usize,
    pub free_tokens: usize,
    pub usage_percentage: f32,
    pub segments: Vec<ContextUsageSegmentView>,
    pub updated_at: String,
    pub estimated: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct RunContextMetadata {
    mode: Option<String>,
    provider: Option<String>,
    model: Option<String>,
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

#[tauri::command]
pub fn get_task_context_snapshot(
    state: tauri::State<'_, AppState>,
    task_id: String,
) -> Result<TaskContextSnapshotView, AppError> {
    let latest_run = queries::get_latest_run_for_task(&state.db, &task_id)?;
    let metadata = latest_run
        .as_ref()
        .and_then(|run| run.plan_json.as_deref())
        .and_then(parse_run_context_metadata)
        .unwrap_or_default();

    let mode = metadata.mode;
    let mut provider = metadata.provider;
    let model = metadata
        .model
        .or_else(|| provider.as_deref().and_then(default_model_for_provider))
        .or_else(default_catalog_model);
    if provider.is_none() {
        provider = model.as_deref().and_then(provider_for_model);
    }

    let context_window = model
        .as_deref()
        .map(context_window_for_model)
        .unwrap_or(200_000);

    let include_embeddings = crate::embeddings::is_semantic_search_configured(&state.db);
    let tool_registry = state.orchestrator.tool_registry();
    // Using unified tool list (list_all) for cache-safe execution.
    // Mode-specific restrictions are enforced at tool execution time.
    let tool_descriptors = tool_registry.list_all(include_embeddings);
    let (tool_definition_tokens, mcp_tools_tokens) =
        estimate_tool_descriptor_tokens(&tool_descriptors);

    let system_prompt_tokens = estimate_system_prompt_tokens(mode.as_deref());

    let transcript = assemble_transcript(&state.db, &task_id).map_err(AppError::Other)?;
    let compaction_settings = load_compaction_settings(&state.db).unwrap_or_default();
    let needs_compaction = check_compaction_needed(
        transcript.estimated_tokens,
        model.as_deref(),
        &compaction_settings,
    );

    let latest_summary = queries::get_latest_conversation_summary(&state.db, &task_id)?;
    let autocompact_tokens = latest_summary
        .as_ref()
        .and_then(|row| row.token_estimate)
        .map(|value| value.max(0) as usize)
        .unwrap_or(0);

    let message_tokens = if needs_compaction && autocompact_tokens > 0 {
        estimate_recent_message_tokens(&transcript.messages, compaction_settings.preserve_recent)
    } else {
        transcript.estimated_tokens
    };

    // Include tool call I/O in token calculation
    let tool_calls = queries::list_tool_calls_for_task(&state.db, &task_id)?;
    let tool_call_tokens = tool_calls
        .iter()
        .map(|tc| {
            let input_tokens = estimate_text_tokens(&tc.input_json);
            let output_tokens = tc
                .output_json
                .as_ref()
                .map(|o| estimate_text_tokens(o))
                .unwrap_or(0);
            let error_tokens = tc
                .error
                .as_ref()
                .map(|e| estimate_text_tokens(e))
                .unwrap_or(0);
            input_tokens
                .saturating_add(output_tokens)
                .saturating_add(error_tokens)
        })
        .sum::<usize>();

    let used_tokens = system_prompt_tokens
        .saturating_add(tool_definition_tokens)
        .saturating_add(mcp_tools_tokens)
        .saturating_add(message_tokens)
        .saturating_add(autocompact_tokens)
        .saturating_add(tool_call_tokens);
    let free_tokens = context_window.saturating_sub(used_tokens);

    let segments = vec![
        build_segment(
            "system_prompt",
            "System instructions",
            system_prompt_tokens,
            context_window,
        ),
        build_segment(
            "tool_definitions",
            "Tool definitions",
            tool_definition_tokens,
            context_window,
        ),
        build_segment("mcp_tools", "MCP tools", mcp_tools_tokens, context_window),
        build_segment("messages", "Messages", message_tokens, context_window),
        build_segment(
            "tool_calls",
            "Tool calls & responses",
            tool_call_tokens,
            context_window,
        ),
        build_segment(
            "autocompact_buffer",
            "Autocompact buffer",
            autocompact_tokens,
            context_window,
        ),
        build_segment("free_space", "Free space", free_tokens, context_window),
    ];

    Ok(TaskContextSnapshotView {
        task_id,
        provider,
        model,
        mode,
        context_window,
        used_tokens,
        free_tokens,
        usage_percentage: percentage(used_tokens, context_window),
        segments,
        updated_at: chrono::Utc::now().to_rfc3339(),
        estimated: true,
    })
}

fn parse_run_context_metadata(raw: &str) -> Option<RunContextMetadata> {
    serde_json::from_str(raw).ok()
}

fn context_window_for_model(model: &str) -> usize {
    ModelCatalog::all_models()
        .into_iter()
        .flat_map(|entry| entry.models)
        .find(|entry| entry.name == model)
        .map(|entry| entry.context_window as usize)
        .unwrap_or(200_000)
}

fn default_model_for_provider(provider: &str) -> Option<String> {
    ModelCatalog::all_models()
        .into_iter()
        .find(|entry| entry.provider == provider)
        .and_then(|entry| entry.models.into_iter().next().map(|model| model.name))
}

fn default_catalog_model() -> Option<String> {
    ModelCatalog::all_models()
        .into_iter()
        .next()
        .and_then(|entry| entry.models.into_iter().next().map(|model| model.name))
}

fn provider_for_model(model: &str) -> Option<String> {
    ModelCatalog::all_models()
        .into_iter()
        .find(|entry| entry.models.iter().any(|candidate| candidate.name == model))
        .map(|entry| entry.provider)
}

fn estimate_tool_descriptor_tokens(tools: &[ToolDescriptor]) -> (usize, usize) {
    let mut tool_definition_tokens = 0usize;
    let mut mcp_tokens = 0usize;

    for descriptor in tools {
        let serialized = serde_json::to_string(descriptor)
            .unwrap_or_else(|_| format!("{} {}", descriptor.name, descriptor.description));
        let tokens = estimate_text_tokens(&serialized);
        if descriptor.name.starts_with("mcp.") {
            mcp_tokens = mcp_tokens.saturating_add(tokens);
        } else {
            tool_definition_tokens = tool_definition_tokens.saturating_add(tokens);
        }
    }

    (tool_definition_tokens, mcp_tokens)
}

fn estimate_system_prompt_tokens(mode: Option<&str>) -> usize {
    const PLAN_SYSTEM_PROMPT: &str = "You are in PLAN mode. Use read-only tools to explore the workspace and create a plan artifact before requesting build mode.";
    const BUILD_SYSTEM_PROMPT: &str = "You are in BUILD mode. Execute the approved plan using tools, keep the user informed, and stop when the objective is complete.";
    let reference = if mode == Some("plan") {
        PLAN_SYSTEM_PROMPT
    } else {
        BUILD_SYSTEM_PROMPT
    };
    estimate_text_tokens(reference)
}

fn estimate_recent_message_tokens(
    messages: &[ConversationMessage],
    preserve_recent: usize,
) -> usize {
    if messages.is_empty() {
        return 0;
    }
    let start = messages.len().saturating_sub(preserve_recent.max(1));
    messages[start..].iter().map(estimate_message_tokens).sum()
}

fn estimate_message_tokens(message: &ConversationMessage) -> usize {
    estimate_text_tokens(&message.content)
        .saturating_add(estimate_text_tokens(&message.role))
        .saturating_add(4)
}

fn estimate_text_tokens(value: &str) -> usize {
    (value.chars().count().saturating_add(3)) / 4
}

fn build_segment(
    key: &str,
    label: &str,
    tokens: usize,
    context_window: usize,
) -> ContextUsageSegmentView {
    ContextUsageSegmentView {
        key: key.to_string(),
        label: label.to_string(),
        tokens,
        percentage: percentage(tokens, context_window),
    }
}

fn percentage(value: usize, total: usize) -> f32 {
    if total == 0 {
        return 0.0;
    }
    ((value as f64 / total as f64) * 100.0) as f32
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
