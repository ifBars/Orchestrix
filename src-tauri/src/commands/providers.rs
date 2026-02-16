use chrono::Utc;
use std::str::FromStr;

use crate::db::queries;
use crate::model::{ModelCatalog, ProviderId};
use crate::{
    load_provider_config, provider_setting_key, AppError, AppState, ModelCatalogEntry, ModelInfo,
    ProviderConfig, ProviderConfigView,
};

#[tauri::command]
pub fn set_provider_config(
    state: tauri::State<'_, AppState>,
    provider: String,
    api_key: String,
    default_model: Option<String>,
    base_url: Option<String>,
) -> Result<(), AppError> {
    let provider = provider.to_ascii_lowercase();
    ProviderId::from_str(&provider)
        .map_err(|_| AppError::Other(format!("unsupported provider: {provider}")))?;

    let value = serde_json::to_string(&ProviderConfig {
        api_key,
        default_model,
        base_url,
    })
    .map_err(|e| AppError::Other(e.to_string()))?;

    queries::upsert_setting(
        &state.db,
        &provider_setting_key(&provider),
        &value,
        &Utc::now().to_rfc3339(),
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_provider_configs(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ProviderConfigView>, AppError> {
    let providers = ProviderId::all();
    let mut result = Vec::new();
    for provider in providers {
        let provider_id = provider.as_str();
        let cfg = load_provider_config(&state.db, provider_id)?;
        result.push(ProviderConfigView {
            provider: provider_id.to_string(),
            configured: cfg.is_some(),
            default_model: cfg
                .as_ref()
                .and_then(|v| v.default_model.clone())
                .or_else(|| Some(ModelCatalog::default_model_for_provider(*provider))),
            base_url: cfg.and_then(|v| v.base_url),
        });
    }
    Ok(result)
}

#[tauri::command]
pub fn remove_provider_config(
    state: tauri::State<'_, AppState>,
    provider: String,
) -> Result<(), AppError> {
    let provider = provider.to_ascii_lowercase();
    ProviderId::from_str(&provider)
        .map_err(|_| AppError::Other(format!("unsupported provider: {provider}")))?;

    let key = provider_setting_key(&provider);
    queries::delete_setting(&state.db, &key)
        .map_err(|e| AppError::Other(format!("Failed to remove provider config: {e}")))?;
    Ok(())
}

/// Returns the context window size for a given model.
fn get_model_context_window(model: &str) -> usize {
    ModelCatalog::all_models()
        .into_iter()
        .flat_map(|entry| entry.models)
        .find(|entry| entry.name == model)
        .map(|entry| entry.context_window as usize)
        .unwrap_or(8_192)
}

#[tauri::command]
pub fn get_model_catalog() -> Vec<ModelCatalogEntry> {
    ModelCatalog::all_models()
        .into_iter()
        .map(|entry| ModelCatalogEntry {
            provider: entry.provider,
            models: entry
                .models
                .into_iter()
                .map(|model| ModelInfo {
                    name: model.name,
                    context_window: model.context_window as usize,
                })
                .collect(),
        })
        .collect()
}

#[tauri::command]
pub fn get_context_window_for_model(model: String) -> usize {
    get_model_context_window(&model)
}
