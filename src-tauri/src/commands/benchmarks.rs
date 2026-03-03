use std::collections::HashMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::bench::agentic_coding::{
    available_agentic_coding_scenarios, run_agentic_coding_benchmark, AgenticCodingBenchOptions,
    AgenticCodingBenchReport, AgenticCodingScenarioDescriptor,
};
use crate::bench::business_ops::{
    available_business_ops_scenarios, run_business_ops_benchmark, BusinessOpsBenchEvent,
    BusinessOpsBenchOptions, BusinessOpsBenchReport, BusinessOpsEventSink,
    BusinessOpsScenarioDescriptor,
};
use crate::bench::diagram::{
    available_diagram_scenarios, run_diagram_benchmark, DiagramBenchOptions, DiagramBenchReport,
    DiagramScenarioDescriptor,
};
use crate::bench::llm::{
    run_llm_benchmark, LlmBenchOptions, LlmBenchReport, LlmProviderConfig, LlmProviderId,
};
use crate::model::ModelCatalog;
use crate::{load_provider_config, AppError, AppState};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RunModelBenchmarkRequest {
    pub run_id: Option<String>,
    pub workload: BenchmarkWorkload,
    pub providers: Option<Vec<String>>,
    pub provider_models: Option<HashMap<String, String>>,
    pub warmup_iterations: Option<usize>,
    pub measured_iterations: Option<usize>,
    pub business_ops_max_turns: Option<usize>,
    pub business_ops_prompts_per_day: Option<usize>,
    pub business_ops_scenarios: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkWorkload {
    Llm,
    BusinessOps,
    LlmAndBusinessOps,
    AgenticCoding,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelBenchmarkReport {
    pub llm: Option<LlmBenchReport>,
    pub business_ops: Option<BusinessOpsBenchReport>,
    pub agentic_coding: Option<AgenticCodingBenchReport>,
}

#[tauri::command]
pub fn list_business_ops_scenarios_command() -> Vec<BusinessOpsScenarioDescriptor> {
    available_business_ops_scenarios()
}

#[tauri::command]
pub fn list_agentic_coding_scenarios_command() -> Vec<AgenticCodingScenarioDescriptor> {
    available_agentic_coding_scenarios()
}

#[tauri::command]
pub async fn run_model_benchmark(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    request: RunModelBenchmarkRequest,
) -> Result<ModelBenchmarkReport, AppError> {
    let run_id = request
        .run_id
        .clone()
        .unwrap_or_else(|| format!("bench-{}", uuid::Uuid::new_v4()));

    let providers = resolve_providers(request.providers)?;
    if providers.is_empty() {
        return Err(AppError::Other(
            "at least one provider is required".to_string(),
        ));
    }

    let provider_configs = resolve_provider_configs(&state, &providers, request.provider_models)?;
    let warmup_iterations = request.warmup_iterations.unwrap_or(1);
    let measured_iterations = request.measured_iterations.unwrap_or(4).max(1);
    let business_ops_max_turns = request.business_ops_max_turns.unwrap_or(40).max(1);
    let business_ops_prompts_per_day = request.business_ops_prompts_per_day.unwrap_or(3).max(1);
    let business_ops_scenarios = request.business_ops_scenarios.unwrap_or_default();

    let llm = if matches!(
        request.workload,
        BenchmarkWorkload::Llm | BenchmarkWorkload::LlmAndBusinessOps
    ) {
        Some(
            run_llm_benchmark(LlmBenchOptions {
                providers: providers.clone(),
                warmup_iterations,
                measured_iterations,
                max_tokens: default_benchmark_max_tokens(),
                provider_configs: provider_configs.clone(),
            })
            .await,
        )
    } else {
        None
    };

    let business_ops = if matches!(
        request.workload,
        BenchmarkWorkload::BusinessOps | BenchmarkWorkload::LlmAndBusinessOps
    ) {
        let sink = TauriBenchEventSink { app: app.clone() };
        Some(
            run_business_ops_benchmark(
                BusinessOpsBenchOptions {
                    providers,
                    warmup_iterations,
                    measured_iterations,
                    max_tokens: default_benchmark_max_tokens(),
                    provider_configs,
                    max_turns: business_ops_max_turns,
                    max_prompts_per_turn: business_ops_prompts_per_day,
                    scenario_filter: business_ops_scenarios,
                    diagnostics: false,
                },
                None,
                Some(run_id.as_str()),
                Some(&sink),
            )
            .await,
        )
    } else {
        None
    };

    let agentic_coding = if matches!(request.workload, BenchmarkWorkload::AgenticCoding) {
        Some(run_agentic_coding_benchmark(AgenticCodingBenchOptions::default()).await)
    } else {
        None
    };

    Ok(ModelBenchmarkReport {
        llm,
        business_ops,
        agentic_coding,
    })
}

struct TauriBenchEventSink {
    app: tauri::AppHandle,
}

impl BusinessOpsEventSink for TauriBenchEventSink {
    fn on_event(&self, event: BusinessOpsBenchEvent) {
        let _ = self.app.emit("benchmark:event", event);
    }
}

fn resolve_providers(raw: Option<Vec<String>>) -> Result<Vec<LlmProviderId>, AppError> {
    let Some(raw_providers) = raw else {
        return Ok(LlmProviderId::all().to_vec());
    };

    let mut providers = Vec::new();
    for item in raw_providers {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }

        let provider = LlmProviderId::from_str(trimmed)
            .map_err(|error| AppError::Other(format!("invalid provider '{trimmed}': {error}")))?;
        if !providers.contains(&provider) {
            providers.push(provider);
        }
    }

    Ok(providers)
}

fn resolve_provider_configs(
    state: &tauri::State<'_, AppState>,
    providers: &[LlmProviderId],
    provider_models: Option<HashMap<String, String>>,
) -> Result<Vec<LlmProviderConfig>, AppError> {
    let provider_models = provider_models.unwrap_or_default();
    let mut configs_by_provider: HashMap<LlmProviderId, LlmProviderConfig> = HashMap::new();

    for provider in providers {
        let provider_key = provider.as_str();
        let cfg = load_provider_config(&state.db, provider_key)?;

        if let Some(cfg) = cfg {
            let model_override = provider_models
                .get(provider_key)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let model = model_override.or(cfg.default_model);
            let max_tokens = model
                .as_deref()
                .and_then(model_context_window)
                .or_else(|| model_context_window_by_provider(provider_key))
                .unwrap_or(default_benchmark_max_tokens());
            let max_tokens = cap_provider_max_tokens(provider_key, max_tokens);

            configs_by_provider.insert(
                *provider,
                LlmProviderConfig {
                    provider: *provider,
                    api_key: Some(cfg.api_key),
                    model,
                    base_url: cfg.base_url,
                    max_tokens: Some(max_tokens),
                },
            );
        } else {
            let model = provider_models
                .get(provider_key)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());

            let max_tokens = model
                .as_deref()
                .and_then(model_context_window)
                .or_else(|| model_context_window_by_provider(provider_key))
                .unwrap_or(default_benchmark_max_tokens());
            let max_tokens = cap_provider_max_tokens(provider_key, max_tokens);

            configs_by_provider.insert(
                *provider,
                LlmProviderConfig {
                    provider: *provider,
                    api_key: None,
                    model,
                    base_url: None,
                    max_tokens: Some(max_tokens),
                },
            );
        }
    }

    Ok(configs_by_provider.into_values().collect())
}

fn default_benchmark_max_tokens() -> u32 {
    4096
}

fn model_context_window(model_name: &str) -> Option<u32> {
    let trimmed = model_name.trim();
    if trimmed.is_empty() {
        return None;
    }

    ModelCatalog::all_models()
        .into_iter()
        .flat_map(|entry| entry.models.into_iter())
        .find(|model| model.name == trimmed)
        .map(|model| model.context_window)
}

fn model_context_window_by_provider(provider: &str) -> Option<u32> {
    ModelCatalog::all_models()
        .into_iter()
        .find(|entry| entry.provider == provider)
        .and_then(|entry| entry.models.into_iter().next())
        .map(|model| model.context_window)
}

fn cap_provider_max_tokens(provider: &str, requested: u32) -> u32 {
    let capped = match provider {
        "minimax" => requested.min(196_608),
        _ => requested,
    };
    capped.max(1)
}

#[tauri::command]
pub fn list_diagram_scenarios_command() -> Vec<DiagramScenarioDescriptor> {
    available_diagram_scenarios()
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunDiagramBenchmarkRequest {
    #[allow(dead_code)]
    pub run_id: Option<String>,
    pub provider: Option<String>,
    pub provider_model: Option<String>,
    pub max_tokens: Option<u32>,
    pub timeout_seconds: Option<u64>,
    pub enable_diagram_tools: Option<bool>,
}

#[tauri::command]
pub async fn run_diagram_benchmark_command(
    state: tauri::State<'_, AppState>,
    request: RunDiagramBenchmarkRequest,
) -> Result<DiagramBenchReport, AppError> {
    let provider_id = if let Some(p) = &request.provider {
        LlmProviderId::from_str(p)
            .map_err(|e| AppError::Other(format!("invalid provider: {}", e)))?
    } else {
        LlmProviderId::MiniMax
    };

    let provider_key = provider_id.as_str();
    let cfg = load_provider_config(&state.db, provider_key)?;

    let config = if let Some(cfg) = cfg {
        let model = request.provider_model.clone().or(cfg.default_model);
        LlmProviderConfig {
            provider: provider_id,
            api_key: Some(cfg.api_key),
            model,
            base_url: cfg.base_url,
            max_tokens: request.max_tokens,
        }
    } else {
        LlmProviderConfig {
            provider: provider_id,
            api_key: None,
            model: request.provider_model.clone(),
            base_url: None,
            max_tokens: request.max_tokens,
        }
    };

    let options = DiagramBenchOptions {
        providers: vec![provider_id],
        provider_configs: vec![config],
        max_tokens: request.max_tokens.unwrap_or(4096),
        timeout_seconds: request.timeout_seconds.unwrap_or(180),
        enable_diagram_tools: request.enable_diagram_tools.unwrap_or(true),
    };

    Ok(run_diagram_benchmark(options).await)
}
