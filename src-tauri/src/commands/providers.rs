use chrono::Utc;

use crate::db::queries;
use crate::{
    default_model_for_provider, load_provider_config, provider_setting_key, AppError, AppState,
    ModelCatalogEntry, ModelInfo, ProviderConfig, ProviderConfigView,
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

/// Returns the context window size for a given model.
fn get_model_context_window(model: &str) -> usize {
    match model {
        // MiniMax models - all have 204,800 context window
        "MiniMax-M2.1" => 204_800,
        "MiniMax-M2" => 204_800,

        // Kimi models - k2.5 has 256k context, others vary
        "kimi-k2.5" => 256_000,
        "kimi-k2" => 128_000,
        "kimi-for-coding" => 128_000,
        "kimi-k2.5-coding" => 256_000,

        // Default for unknown models
        _ => 8_192,
    }
}

#[tauri::command]
pub fn get_model_catalog() -> Vec<ModelCatalogEntry> {
    vec![
        ModelCatalogEntry {
            provider: "minimax".to_string(),
            models: vec![
                ModelInfo {
                    name: "MiniMax-M2.1".to_string(),
                    context_window: 204_800,
                },
                ModelInfo {
                    name: "MiniMax-M2".to_string(),
                    context_window: 204_800,
                },
            ],
        },
        ModelCatalogEntry {
            provider: "kimi".to_string(),
            models: vec![
                ModelInfo {
                    name: "kimi-k2.5".to_string(),
                    context_window: 256_000,
                },
                ModelInfo {
                    name: "kimi-k2".to_string(),
                    context_window: 128_000,
                },
                ModelInfo {
                    name: "kimi-for-coding".to_string(),
                    context_window: 128_000,
                },
            ],
        },
    ]
}

#[tauri::command]
pub fn get_context_window_for_model(model: String) -> usize {
    get_model_context_window(&model)
}
