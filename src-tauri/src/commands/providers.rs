use chrono::Utc;

use crate::db::queries;
use crate::{
    default_model_for_provider, load_provider_config, provider_setting_key, AppError, AppState,
    ModelCatalogEntry, ProviderConfig, ProviderConfigView,
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
    if provider != "minimax" && provider != "kimi" {
        return Err(AppError::Other(format!("unsupported provider: {provider}")));
    }

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
    let providers = ["minimax", "kimi"];
    let mut result = Vec::new();
    for provider in providers {
        let cfg = load_provider_config(&state.db, provider)?;
        result.push(ProviderConfigView {
            provider: provider.to_string(),
            configured: cfg.is_some(),
            default_model: cfg
                .as_ref()
                .and_then(|v| v.default_model.clone())
                .or_else(|| Some(default_model_for_provider(provider).to_string())),
            base_url: cfg.and_then(|v| v.base_url),
        });
    }
    Ok(result)
}

#[tauri::command]
pub fn get_model_catalog() -> Vec<ModelCatalogEntry> {
    vec![
        ModelCatalogEntry {
            provider: "minimax".to_string(),
            models: vec![
                "MiniMax-M2.1".to_string(),
                "MiniMax-M2".to_string(),
                "MiniMax-M1".to_string(),
                "MiniMax-Text-01".to_string(),
            ],
        },
        ModelCatalogEntry {
            provider: "kimi".to_string(),
            models: vec![
                "kimi-k2".to_string(),
                "kimi-k2.5".to_string(),
                "kimi-for-coding".to_string(),
            ],
        },
    ]
}
