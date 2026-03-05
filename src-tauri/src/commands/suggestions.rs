use crate::runtime::prompt_suggestion::{self, PromptSuggestionSettings};
use crate::{load_provider_config, AppError, AppState};

#[tauri::command]
pub fn get_prompt_suggestion_settings(
    state: tauri::State<'_, AppState>,
) -> Result<PromptSuggestionSettings, AppError> {
    prompt_suggestion::load_suggestion_settings(&state.db)
        .map_err(|e| AppError::Other(e.to_string()))
}

#[tauri::command]
pub fn set_prompt_suggestion_settings(
    state: tauri::State<'_, AppState>,
    settings: PromptSuggestionSettings,
) -> Result<(), AppError> {
    prompt_suggestion::save_suggestion_settings(&state.db, &settings)
        .map_err(|e| AppError::Other(e.to_string()))
}

#[tauri::command]
pub async fn generate_prompt_suggestion(
    state: tauri::State<'_, AppState>,
    task_id: String,
    provider: String,
    model: Option<String>,
) -> Result<String, AppError> {
    // Load suggestion settings
    let settings = prompt_suggestion::load_suggestion_settings(&state.db)
        .map_err(|e| AppError::Other(e.to_string()))?;

    if !settings.enabled {
        return Err(AppError::Other(
            "Prompt suggestions are disabled".to_string(),
        ));
    }

    // Load provider config to get API key
    let provider = provider.to_ascii_lowercase();
    let provider_config = load_provider_config(&state.db, &provider)
        .map_err(|e| AppError::Other(e.to_string()))?
        .ok_or_else(|| AppError::Other(format!("Provider {} not configured", provider)))?;

    let api_key = provider_config.api_key;
    let base_url = provider_config.base_url;
    let effective_model = settings.suggestion_model.as_deref().or(model.as_deref());

    // Generate suggestion
    let suggestion = prompt_suggestion::generate_suggestion(
        &state.db,
        &task_id,
        &provider,
        &api_key,
        effective_model,
        base_url.as_deref(),
        &settings,
    )
    .await
    .map_err(|e| AppError::Other(e.to_string()))?;

    Ok(suggestion)
}
