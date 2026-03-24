use crate::embeddings::config::load_embedding_config;
use crate::embeddings::{
    gpu_probe::{detect_hardware_profile, HardwareProfile},
    is_semantic_search_configured, EmbedOptions, EmbeddingConfig, EmbeddingConfigView,
    EmbeddingIndexStatus, EmbeddingProviderId, EmbeddingProviderInfo,
};
use crate::{load_workspace_root, AppError, AppState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EmbeddingAutoConfigPreference {
    Local,
    Quality,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecommendedEmbeddingConfig {
    pub config: EmbeddingConfig,
    pub notes: Vec<String>,
    pub hardware: HardwareProfile,
}

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
pub fn get_recommended_embedding_config(
    state: tauri::State<'_, AppState>,
    preference: Option<EmbeddingAutoConfigPreference>,
) -> Result<RecommendedEmbeddingConfig, AppError> {
    let preference = preference.unwrap_or(EmbeddingAutoConfigPreference::Local);
    let hardware = detect_hardware_profile();
    let mut config =
        load_embedding_config(&state.db).map_err(|error| AppError::Other(error.to_string()))?;
    let mut notes = Vec::new();

    let ram_gb = hardware.ram_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    let has_any_gpu = !hardware.gpus.is_empty();

    if matches!(preference, EmbeddingAutoConfigPreference::Quality)
        && config.effective_gemini_api_key().is_some()
    {
        config.enabled = true;
        config.provider = EmbeddingProviderId::Gemini;
        notes.push(
            "Configured for best quality using Gemini embeddings. Free tier can rate-limit quickly; paid usage is recommended for sustained indexing.".to_string(),
        );
    } else {
        config.enabled = true;
        config.normalize_l2 = true;

        if ram_gb < 4.0 {
            config.provider = EmbeddingProviderId::Transformersjs;
            config.transformersjs.device = "cpu".to_string();
            notes.push("Low-memory system detected (<4 GB RAM), using Transformers.js on CPU for stability.".to_string());
        } else if ram_gb < 8.0 {
            config.provider = EmbeddingProviderId::Transformersjs;
            config.transformersjs.device = if has_any_gpu {
                "webgpu".to_string()
            } else {
                "cpu".to_string()
            };
            notes.push("Mid-memory system detected (4-8 GB RAM), using Transformers.js with WebGPU when available.".to_string());
        } else {
            config.provider = EmbeddingProviderId::RustHf;
            config.rust_hf.threads = Some(hardware.physical_cores.max(1));
            notes.push("Higher-memory system detected (>=8 GB RAM), using Rust HF for best local throughput.".to_string());

            if hardware.ort_backends.cuda
                || hardware.ort_backends.directml
                || hardware.ort_backends.coreml
            {
                notes.push("GPU execution providers detected for ONNX Runtime; Rust HF will try GPU acceleration automatically with CPU fallback.".to_string());
            } else {
                notes.push(
                    "No ONNX Runtime GPU execution provider detected; Rust HF will run on CPU."
                        .to_string(),
                );
            }
        }

        if matches!(preference, EmbeddingAutoConfigPreference::Quality) {
            notes.push("Gemini quality mode requested, but no API key is configured. Falling back to optimized local embeddings.".to_string());
        }
    }

    notes.push(format!(
        "Detected {:.1} GB RAM, {} logical cores, {} physical cores, {} GPU(s).",
        ram_gb,
        hardware.logical_cores,
        hardware.physical_cores,
        hardware.gpus.len()
    ));

    Ok(RecommendedEmbeddingConfig {
        config,
        notes,
        hardware,
    })
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
        .inspect(|view| {
            if view.enabled {
                let workspace_root = load_workspace_root(&state.db);
                state
                    .embedding_index_service
                    .ensure_workspace_index_started(workspace_root);
            }
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

#[tauri::command]
pub fn get_embedding_index_status(
    state: tauri::State<'_, AppState>,
) -> Result<Option<EmbeddingIndexStatus>, AppError> {
    if !is_semantic_search_configured(&state.db) {
        return Ok(None);
    }
    let workspace_root = load_workspace_root(&state.db);
    Ok(state.embedding_index_service.index_status(&workspace_root))
}
