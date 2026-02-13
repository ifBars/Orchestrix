use crate::embeddings::config::load_embedding_config;
use crate::embeddings::{
    EmbedOptions, EmbeddingConfig, EmbeddingConfigView, EmbeddingProviderInfo,
};
use crate::{load_workspace_root, AppError, AppState};

#[tauri::command]
pub fn get_embedding_config(
    state: tauri::State<'_, AppState>,
) -> Result<EmbeddingConfigView, AppError> {
    state
        .embedding_manager
        .get_config_view()
        .map_err(|error| AppError::Other(error.to_string()))
}

#[tauri::command]
pub async fn set_embedding_config(
    state: tauri::State<'_, AppState>,
    mut config: EmbeddingConfig,
) -> Result<EmbeddingConfigView, AppError> {
    let incoming_key = config.gemini.api_key.as_deref().unwrap_or("").trim();
    if incoming_key.is_empty() {
        if let Ok(existing) = load_embedding_config(&state.db) {
            let existing_key = existing.gemini.api_key.as_deref().unwrap_or("").trim();
            if !existing_key.is_empty() {
                config.gemini.api_key = Some(existing_key.to_string());
            }
        }
    }

    state
        .embedding_manager
        .set_config(config)
        .await
        .map_err(|error| AppError::Other(error.to_string()))
        .inspect(|_| {
            let workspace_root = load_workspace_root(&state.db);
            state
                .embedding_index_service
                .ensure_workspace_index_started(workspace_root);
        })
}

#[tauri::command]
pub async fn get_embedding_provider_info(
    state: tauri::State<'_, AppState>,
) -> Result<EmbeddingProviderInfo, AppError> {
    state
        .embedding_manager
        .provider_info()
        .await
        .map_err(|error| AppError::Other(error.to_string()))
}

#[tauri::command]
pub async fn embedding_dims(state: tauri::State<'_, AppState>) -> Result<Option<usize>, AppError> {
    state
        .embedding_manager
        .dims()
        .await
        .map_err(|error| AppError::Other(error.to_string()))
}

#[tauri::command]
pub async fn embed_texts(
    state: tauri::State<'_, AppState>,
    texts: Vec<String>,
    opts: Option<EmbedOptions>,
) -> Result<Vec<Vec<f32>>, AppError> {
    state
        .embedding_manager
        .embed(&texts, opts)
        .await
        .map_err(|error| AppError::Other(error.to_string()))
}
