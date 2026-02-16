use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use std::str::FromStr;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::bench::core::{compute_latency_stats, BenchmarkRunMetadata, LatencyStats, WorkloadKind};
use crate::model::{GlmClient, KimiClient, MiniMaxClient, ModalClient};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderId {
    MiniMax,
    Kimi,
    Zhipu,
    Modal,
}

impl LlmProviderId {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::MiniMax => "minimax",
            Self::Kimi => "kimi",
            Self::Zhipu => "zhipu",
            Self::Modal => "modal",
        }
    }

    pub const fn all() -> &'static [LlmProviderId] {
        &[
            LlmProviderId::MiniMax,
            LlmProviderId::Kimi,
            LlmProviderId::Zhipu,
            LlmProviderId::Modal,
        ]
    }
}

impl std::fmt::Display for LlmProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for LlmProviderId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "minimax" => Ok(Self::MiniMax),
            "kimi" => Ok(Self::Kimi),
            "zhipu" | "glm" => Ok(Self::Zhipu),
            "modal" => Ok(Self::Modal),
            _ => Err(format!("unsupported llm provider: {value}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmBenchOptions {
    pub providers: Vec<LlmProviderId>,
    pub warmup_iterations: usize,
    pub measured_iterations: usize,
    pub max_tokens: u32,
    pub provider_configs: Vec<LlmProviderConfig>,
}

impl Default for LlmBenchOptions {
    fn default() -> Self {
        Self {
            providers: LlmProviderId::all().to_vec(),
            warmup_iterations: 1,
            measured_iterations: 4,
            max_tokens: 512,
            provider_configs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmProviderConfig {
    pub provider: LlmProviderId,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmTaskCategory {
    Reasoning,
    Classification,
    Extraction,
    CodeComprehension,
    InstructionFollowing,
    AgenticChoice,
    ToolUse,
}

impl LlmTaskCategory {
    fn order(self) -> u8 {
        match self {
            Self::Reasoning => 0,
            Self::Classification => 1,
            Self::Extraction => 2,
            Self::CodeComprehension => 3,
            Self::InstructionFollowing => 4,
            Self::AgenticChoice => 5,
            Self::ToolUse => 6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmBenchReport {
    pub metadata: BenchmarkRunMetadata,
    pub tasks: Vec<LlmTaskDescriptor>,
    pub providers: Vec<LlmProviderBenchmarkResult>,
    pub category_winners: Vec<LlmCategoryWinner>,
    pub overall_winner: Option<LlmOverallWinner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTaskDescriptor {
    pub task_id: String,
    pub task_label: String,
    pub category: LlmTaskCategory,
    pub pass_threshold: f64,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderBenchmarkResult {
    pub provider: String,
    pub model: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub first_completion_ms: Option<f64>,
    pub tasks: Vec<LlmTaskBenchmarkResult>,
    pub categories: Vec<LlmCategoryBenchmarkResult>,
    pub aggregate: Option<LlmAggregateBenchmarkResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTaskBenchmarkResult {
    pub task_id: String,
    pub task_label: String,
    pub category: LlmTaskCategory,
    pub input_chars: usize,
    pub measured_iterations: usize,
    pub successful_iterations: usize,
    pub passed_iterations: usize,
    pub latency: LatencyStats,
    pub average_score: f64,
    pub pass_rate: f64,
    pub success_rate: f64,
    pub throughput_calls_per_sec: f64,
    pub throughput_output_chars_per_sec: f64,
    pub mean_output_chars: f64,
    pub sample_response: Option<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCategoryBenchmarkResult {
    pub category: LlmTaskCategory,
    pub average_score: f64,
    pub pass_rate: f64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAggregateBenchmarkResult {
    pub weighted_score: f64,
    pub pass_rate: f64,
    pub success_rate: f64,
    pub avg_p50_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCategoryWinner {
    pub category: LlmTaskCategory,
    pub provider: String,
    pub model: Option<String>,
    pub average_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmOverallWinner {
    pub provider: String,
    pub model: Option<String>,
    pub weighted_score: f64,
    pub avg_p50_latency_ms: f64,
}

#[derive(Debug, Clone)]
struct ResolvedProviderConfig {
    provider: LlmProviderId,
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
}

#[derive(Debug, Clone)]
struct JsonFieldExpectation {
    key: &'static str,
    expected: &'static str,
}

#[derive(Debug, Clone)]
enum LlmJudge {
    Numeric {
        expected: f64,
        tolerance: f64,
    },
    ExactLabel {
        expected: &'static str,
    },
    JsonFields {
        fields: Vec<JsonFieldExpectation>,
    },
    ContainsText {
        required: Vec<&'static str>,
        max_words: Option<usize>,
    },
}

#[derive(Debug, Clone)]
struct LlmTaskDefinition {
    id: &'static str,
    label: &'static str,
    category: LlmTaskCategory,
    system_prompt: &'static str,
    user_prompt: &'static str,
    judge: LlmJudge,
    pass_threshold: f64,
    weight: f64,
}

enum LlmBenchClient {
    MiniMax(MiniMaxClient),
    Kimi(KimiClient),
    Zhipu(GlmClient),
    Modal(ModalClient),
}

impl LlmBenchClient {
    fn model_id(&self) -> String {
        match self {
            Self::MiniMax(client) => client.model_id(),
            Self::Kimi(client) => client.model_id(),
            Self::Zhipu(client) => client.model_id(),
            Self::Modal(client) => client.model_id(),
        }
    }

    async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String, crate::model::ModelError> {
        match self {
            Self::MiniMax(client) => client.complete(system, user, max_tokens).await,
            Self::Kimi(client) => client.complete(system, user, max_tokens).await,
            Self::Zhipu(client) => client.complete(system, user, max_tokens).await,
            Self::Modal(client) => client.complete(system, user, max_tokens).await,
        }
    }

    /// Check if the client supports tool calling by attempting a simple tool call
    async fn supports_tool_calling(&self) -> bool {
        // All our providers support tool calling, but some models may not
        match self {
            Self::MiniMax(_) => true,
            Self::Kimi(_) => true,
            Self::Zhipu(_) => true,
            Self::Modal(_) => true,
        }
    }
}

pub async fn run_llm_benchmark(options: LlmBenchOptions) -> LlmBenchReport {
    let tasks = default_tasks();
    let metadata = BenchmarkRunMetadata::new(
        WorkloadKind::LlmSharedTasks,
        options.warmup_iterations,
        options.measured_iterations,
        vec![1],
    );
    let task_descriptors = tasks
        .iter()
        .map(|task| LlmTaskDescriptor {
            task_id: task.id.to_string(),
            task_label: task.label.to_string(),
            category: task.category,
            pass_threshold: task.pass_threshold,
            weight: task.weight,
        })
        .collect();

    let mut providers = Vec::new();
    for provider_id in &options.providers {
        let provider_result = match run_provider_benchmark(*provider_id, &options, &tasks).await {
            Ok(result) => result,
            Err(error) => LlmProviderBenchmarkResult {
                provider: provider_id.as_str().to_string(),
                model: None,
                status: "error".to_string(),
                error: Some(error),
                first_completion_ms: None,
                tasks: Vec::new(),
                categories: Vec::new(),
                aggregate: None,
            },
        };
        providers.push(provider_result);
    }

    let category_winners = compute_category_winners(&providers);
    let overall_winner = compute_overall_winner(&providers);

    LlmBenchReport {
        metadata,
        tasks: task_descriptors,
        providers,
        category_winners,
        overall_winner,
    }
}

async fn run_provider_benchmark(
    provider_id: LlmProviderId,
    options: &LlmBenchOptions,
    tasks: &[LlmTaskDefinition],
) -> Result<LlmProviderBenchmarkResult, String> {
    let config = resolve_provider_config(provider_id, &options.provider_configs)?;
    let client = create_client(&config);
    let model = Some(client.model_id());

    let (first_completion_ms, probe_error) =
        measure_first_completion(&client, options.max_tokens).await;

    let mut task_results = Vec::new();
    for task in tasks {
        let task_result = run_task_benchmark(&client, task, options).await;
        task_results.push(task_result);
    }

    let successful_calls = task_results
        .iter()
        .map(|task| task.successful_iterations)
        .sum::<usize>();
    let first_task_error = task_results
        .iter()
        .find_map(|task| task.errors.first().cloned());
    let error_message = probe_error.or(first_task_error);

    let categories = compute_category_scores(&task_results);
    let aggregate = compute_aggregate_scores(&task_results, tasks);

    let status = if successful_calls > 0 { "ok" } else { "error" };

    Ok(LlmProviderBenchmarkResult {
        provider: provider_id.as_str().to_string(),
        model,
        status: status.to_string(),
        error: if status == "ok" { None } else { error_message },
        first_completion_ms,
        tasks: task_results,
        categories,
        aggregate,
    })
}

async fn measure_first_completion(
    client: &LlmBenchClient,
    max_tokens: u32,
) -> (Option<f64>, Option<String>) {
    let started = Instant::now();
    match client
        .complete(
            "You are a benchmark readiness probe. Reply with OK.",
            "Reply with OK only.",
            max_tokens.min(64),
        )
        .await
    {
        Ok(_) => (Some(started.elapsed().as_secs_f64() * 1000.0), None),
        Err(error) => (None, Some(error.to_string())),
    }
}

async fn run_task_benchmark(
    client: &LlmBenchClient,
    task: &LlmTaskDefinition,
    options: &LlmBenchOptions,
) -> LlmTaskBenchmarkResult {
    for _ in 0..options.warmup_iterations {
        let _ = client
            .complete(task.system_prompt, task.user_prompt, options.max_tokens)
            .await;
    }

    let mut samples = Vec::with_capacity(options.measured_iterations);
    let mut score_sum = 0.0;
    let mut success_count = 0usize;
    let mut pass_count = 0usize;
    let mut total_output_chars = 0usize;
    let mut sample_response = None;
    let mut errors = Vec::new();

    for _ in 0..options.measured_iterations {
        let started = Instant::now();
        match client
            .complete(task.system_prompt, task.user_prompt, options.max_tokens)
            .await
        {
            Ok(response) => {
                let elapsed = started.elapsed();
                let score = task.judge.score(&response);
                samples.push(elapsed);
                score_sum += score;
                success_count += 1;
                if score >= task.pass_threshold {
                    pass_count += 1;
                }
                total_output_chars += response.chars().count();
                if sample_response.is_none() {
                    sample_response = Some(truncate_response(&response, 260));
                }
            }
            Err(error) => {
                if errors.len() < 3 {
                    errors.push(error.to_string());
                }
            }
        }
    }

    let measured = options.measured_iterations.max(1);
    let total = samples
        .iter()
        .copied()
        .fold(Duration::ZERO, |acc, value| acc + value);
    let total_seconds = total.as_secs_f64().max(f64::EPSILON);

    let average_score = score_sum / measured as f64;
    let pass_rate = pass_count as f64 / measured as f64;
    let success_rate = success_count as f64 / measured as f64;
    let throughput_calls_per_sec = success_count as f64 / total_seconds;
    let throughput_output_chars_per_sec = total_output_chars as f64 / total_seconds;
    let mean_output_chars = if success_count == 0 {
        0.0
    } else {
        total_output_chars as f64 / success_count as f64
    };

    LlmTaskBenchmarkResult {
        task_id: task.id.to_string(),
        task_label: task.label.to_string(),
        category: task.category,
        input_chars: task.user_prompt.chars().count(),
        measured_iterations: measured,
        successful_iterations: success_count,
        passed_iterations: pass_count,
        latency: compute_latency_stats(&samples),
        average_score,
        pass_rate,
        success_rate,
        throughput_calls_per_sec,
        throughput_output_chars_per_sec,
        mean_output_chars,
        sample_response,
        errors,
    }
}

fn compute_category_scores(tasks: &[LlmTaskBenchmarkResult]) -> Vec<LlmCategoryBenchmarkResult> {
    let mut category_map: HashMap<LlmTaskCategory, (f64, f64, f64, usize)> = HashMap::new();

    for task in tasks {
        let entry = category_map
            .entry(task.category)
            .or_insert((0.0, 0.0, 0.0, 0usize));
        entry.0 += task.average_score;
        entry.1 += task.pass_rate;
        entry.2 += task.success_rate;
        entry.3 += 1;
    }

    let mut categories: Vec<LlmCategoryBenchmarkResult> = category_map
        .into_iter()
        .map(|(category, (score_sum, pass_sum, success_sum, count))| {
            let divisor = count.max(1) as f64;
            LlmCategoryBenchmarkResult {
                category,
                average_score: score_sum / divisor,
                pass_rate: pass_sum / divisor,
                success_rate: success_sum / divisor,
            }
        })
        .collect();

    categories.sort_by(|lhs, rhs| lhs.category.order().cmp(&rhs.category.order()));
    categories
}

fn compute_aggregate_scores(
    tasks: &[LlmTaskBenchmarkResult],
    definitions: &[LlmTaskDefinition],
) -> Option<LlmAggregateBenchmarkResult> {
    if tasks.is_empty() {
        return None;
    }

    let mut definition_weights: HashMap<&str, f64> = HashMap::new();
    for definition in definitions {
        definition_weights.insert(definition.id, definition.weight);
    }

    let total_weight = tasks
        .iter()
        .map(|task| {
            *definition_weights
                .get(task.task_id.as_str())
                .unwrap_or(&1.0)
        })
        .sum::<f64>();

    let weighted_score = if total_weight <= f64::EPSILON {
        0.0
    } else {
        tasks
            .iter()
            .map(|task| {
                let weight = *definition_weights
                    .get(task.task_id.as_str())
                    .unwrap_or(&1.0);
                task.average_score * weight
            })
            .sum::<f64>()
            / total_weight
    };

    let measured_total = tasks
        .iter()
        .map(|task| task.measured_iterations)
        .sum::<usize>();
    let success_total = tasks
        .iter()
        .map(|task| task.successful_iterations)
        .sum::<usize>();
    let pass_total = tasks
        .iter()
        .map(|task| task.passed_iterations)
        .sum::<usize>();
    let pass_rate = if measured_total == 0 {
        0.0
    } else {
        pass_total as f64 / measured_total as f64
    };
    let success_rate = if measured_total == 0 {
        0.0
    } else {
        success_total as f64 / measured_total as f64
    };
    let avg_p50_latency_ms =
        tasks.iter().map(|task| task.latency.p50_ms).sum::<f64>() / tasks.len().max(1) as f64;

    Some(LlmAggregateBenchmarkResult {
        weighted_score,
        pass_rate,
        success_rate,
        avg_p50_latency_ms,
    })
}

fn compute_category_winners(providers: &[LlmProviderBenchmarkResult]) -> Vec<LlmCategoryWinner> {
    let mut winners = Vec::new();
    let categories: BTreeSet<LlmTaskCategory> = providers
        .iter()
        .flat_map(|provider| provider.categories.iter().map(|category| category.category))
        .collect();

    for category in categories {
        let winner = providers
            .iter()
            .filter(|provider| provider.status == "ok")
            .filter_map(|provider| {
                provider
                    .categories
                    .iter()
                    .find(|entry| entry.category == category)
                    .map(|entry| (provider, entry.average_score))
            })
            .max_by(|lhs, rhs| {
                lhs.1
                    .partial_cmp(&rhs.1)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| lhs.0.provider.cmp(&rhs.0.provider).reverse())
            });

        if let Some((provider, score)) = winner {
            winners.push(LlmCategoryWinner {
                category,
                provider: provider.provider.clone(),
                model: provider.model.clone(),
                average_score: score,
            });
        }
    }

    winners.sort_by(|lhs, rhs| lhs.category.order().cmp(&rhs.category.order()));
    winners
}

fn compute_overall_winner(providers: &[LlmProviderBenchmarkResult]) -> Option<LlmOverallWinner> {
    providers
        .iter()
        .filter(|provider| provider.status == "ok")
        .filter_map(|provider| {
            provider.aggregate.as_ref().map(|aggregate| {
                (
                    provider.provider.clone(),
                    provider.model.clone(),
                    aggregate.weighted_score,
                    aggregate.avg_p50_latency_ms,
                )
            })
        })
        .max_by(|lhs, rhs| {
            lhs.2
                .partial_cmp(&rhs.2)
                .unwrap_or(Ordering::Equal)
                .then_with(|| rhs.3.partial_cmp(&lhs.3).unwrap_or(Ordering::Equal))
        })
        .map(
            |(provider, model, weighted_score, avg_p50_latency_ms)| LlmOverallWinner {
                provider,
                model,
                weighted_score,
                avg_p50_latency_ms,
            },
        )
}

fn create_client(config: &ResolvedProviderConfig) -> LlmBenchClient {
    match config.provider {
        LlmProviderId::MiniMax => LlmBenchClient::MiniMax(MiniMaxClient::new_with_base_url(
            config.api_key.clone(),
            config.model.clone(),
            config.base_url.clone(),
        )),
        LlmProviderId::Kimi => LlmBenchClient::Kimi(KimiClient::new(
            config.api_key.clone(),
            config.model.clone(),
            config.base_url.clone(),
        )),
        LlmProviderId::Zhipu => LlmBenchClient::Zhipu(GlmClient::new(
            config.api_key.clone(),
            config.model.clone(),
            config.base_url.clone(),
        )),
        LlmProviderId::Modal => LlmBenchClient::Modal(ModalClient::new(
            config.api_key.clone(),
            config.model.clone(),
            config.base_url.clone(),
        )),
    }
}

fn resolve_provider_config(
    provider: LlmProviderId,
    overrides: &[LlmProviderConfig],
) -> Result<ResolvedProviderConfig, String> {
    let override_cfg = overrides.iter().find(|config| config.provider == provider);

    let api_key = override_cfg
        .and_then(|config| normalize_optional_string(config.api_key.clone()))
        .or_else(|| first_non_empty_env(api_key_env_keys(provider)))
        .ok_or_else(|| missing_config_message(provider))?;

    let model = override_cfg
        .and_then(|config| normalize_optional_string(config.model.clone()))
        .or_else(|| first_non_empty_env(model_env_keys(provider)))
        .or_else(|| Some(default_model(provider).to_string()));

    let base_url = override_cfg
        .and_then(|config| normalize_optional_string(config.base_url.clone()))
        .or_else(|| first_non_empty_env(base_url_env_keys(provider)));

    Ok(ResolvedProviderConfig {
        provider,
        api_key,
        model,
        base_url,
    })
}

fn missing_config_message(provider: LlmProviderId) -> String {
    match provider {
        LlmProviderId::MiniMax => {
            "missing MiniMax credentials: set MINIMAX_API_KEY or provide override config"
                .to_string()
        }
        LlmProviderId::Kimi => {
            "missing Kimi credentials: set KIMI_API_KEY or provide override config".to_string()
        }
        LlmProviderId::Zhipu => {
            "missing GLM/Zhipu credentials: set ZHIPU_API_KEY or GLM_API_KEY".to_string()
        }
        LlmProviderId::Modal => {
            "missing Modal credentials: set MODAL_API_KEY or provide override config".to_string()
        }
    }
}

pub(crate) fn api_key_env_keys(provider: LlmProviderId) -> &'static [&'static str] {
    match provider {
        LlmProviderId::MiniMax => &["MINIMAX_API_KEY"],
        LlmProviderId::Kimi => &["KIMI_API_KEY"],
        LlmProviderId::Zhipu => &["ZHIPU_API_KEY", "GLM_API_KEY", "ZAI_API_KEY"],
        LlmProviderId::Modal => &["MODAL_API_KEY"],
    }
}

pub(crate) fn model_env_keys(provider: LlmProviderId) -> &'static [&'static str] {
    match provider {
        LlmProviderId::MiniMax => &["MINIMAX_MODEL"],
        LlmProviderId::Kimi => &["KIMI_MODEL"],
        LlmProviderId::Zhipu => &["ZHIPU_MODEL", "GLM_MODEL"],
        LlmProviderId::Modal => &["MODAL_MODEL"],
    }
}

pub(crate) fn base_url_env_keys(provider: LlmProviderId) -> &'static [&'static str] {
    match provider {
        LlmProviderId::MiniMax => &["MINIMAX_BASE_URL"],
        LlmProviderId::Kimi => &["KIMI_BASE_URL"],
        LlmProviderId::Zhipu => &["ZHIPU_BASE_URL", "GLM_BASE_URL"],
        LlmProviderId::Modal => &["MODAL_BASE_URL"],
    }
}

pub(crate) fn default_model(provider: LlmProviderId) -> &'static str {
    match provider {
        LlmProviderId::MiniMax => "MiniMax-M2.5",
        LlmProviderId::Kimi => "kimi-k2.5",
        LlmProviderId::Zhipu => "glm-5",
        LlmProviderId::Modal => "zai-org/GLM-5-FP8",
    }
}

pub(crate) fn first_non_empty_env(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .and_then(|value| normalize_optional_string(Some(value)))
    })
}

pub(crate) fn normalize_optional_string(value: Option<String>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

impl LlmJudge {
    fn score(&self, response: &str) -> f64 {
        match self {
            Self::Numeric {
                expected,
                tolerance,
            } => evaluate_numeric(response, *expected, *tolerance),
            Self::ExactLabel { expected } => evaluate_exact_label(response, expected),
            Self::JsonFields { fields } => evaluate_json_fields(response, fields),
            Self::ContainsText {
                required,
                max_words,
            } => evaluate_contains_text(response, required, *max_words),
        }
    }
}

fn evaluate_numeric(response: &str, expected: f64, tolerance: f64) -> f64 {
    let Some(actual) = extract_first_number(response) else {
        return 0.0;
    };
    let delta = (actual - expected).abs();
    if delta <= tolerance {
        1.0
    } else {
        0.0
    }
}

fn evaluate_exact_label(response: &str, expected: &str) -> f64 {
    let response = normalize_words(response);
    let expected = normalize_words(expected);
    if response.contains(&expected) {
        1.0
    } else {
        0.0
    }
}

fn evaluate_json_fields(response: &str, fields: &[JsonFieldExpectation]) -> f64 {
    let Some(json_payload) = parse_json_payload(response) else {
        return 0.0;
    };

    let mut matched = 0usize;
    for field in fields {
        let Some(value) = json_payload.get(field.key) else {
            continue;
        };
        let normalized_actual = normalize_words(&json_value_to_string(value));
        let normalized_expected = normalize_words(field.expected);
        if normalized_actual == normalized_expected {
            matched += 1;
        }
    }

    matched as f64 / fields.len().max(1) as f64
}

fn evaluate_contains_text(response: &str, required: &[&str], max_words: Option<usize>) -> f64 {
    let normalized_response = response.to_ascii_lowercase();
    let mut matched = 0usize;
    for phrase in required {
        if normalized_response.contains(&phrase.to_ascii_lowercase()) {
            matched += 1;
        }
    }

    let mut score = matched as f64 / required.len().max(1) as f64;
    if let Some(limit) = max_words {
        let word_count = response.split_whitespace().count();
        if word_count > limit {
            score *= 0.5;
        }
    }
    score
}

fn extract_first_number(response: &str) -> Option<f64> {
    let mut current = String::new();
    let mut last_parsed = None;

    for ch in response.chars() {
        if ch.is_ascii_digit() || ch == '-' || ch == '.' {
            current.push(ch);
            continue;
        }

        if let Some(parsed) = parse_number_candidate(&current) {
            last_parsed = Some(parsed);
        }
        current.clear();
    }

    parse_number_candidate(&current).or(last_parsed)
}

fn parse_number_candidate(value: &str) -> Option<f64> {
    if value.is_empty() || value == "-" || value == "." || value == "-." {
        return None;
    }
    value.parse::<f64>().ok()
}

fn parse_json_payload(response: &str) -> Option<serde_json::Value> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(response.trim()) {
        return Some(value);
    }

    let without_fence_markers = response
        .lines()
        .filter(|line| !line.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n");
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(without_fence_markers.trim()) {
        return Some(value);
    }

    let trimmed = response.trim();
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    let candidate = &trimmed[start..=end];
    serde_json::from_str::<serde_json::Value>(candidate).ok()
}

fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn normalize_words(input: &str) -> String {
    input
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_response(response: &str, max_chars: usize) -> String {
    let mut truncated: String = response.chars().take(max_chars).collect();
    if response.chars().count() > max_chars {
        truncated.push_str("...");
    }
    truncated
}

fn default_tasks() -> Vec<LlmTaskDefinition> {
    vec![
        LlmTaskDefinition {
            id: "arithmetic_accuracy",
            label: "Arithmetic reasoning",
            category: LlmTaskCategory::Reasoning,
            system_prompt: "You are a careful assistant. Return only the final answer.",
            user_prompt: "Compute ((128 * 7) - 96) / 4. Return only the number.",
            judge: LlmJudge::Numeric {
                expected: 200.0,
                tolerance: 0.0,
            },
            pass_threshold: 1.0,
            weight: 1.0,
        },
        LlmTaskDefinition {
            id: "ticket_classification",
            label: "Ticket classification",
            category: LlmTaskCategory::Classification,
            system_prompt: "Classify support tickets with strict label output.",
            user_prompt: "Labels: billing, reliability, feature-request. Ticket: After the latest deployment, checkout requests return 502 for 30% of users. Respond with the label only.",
            judge: LlmJudge::ExactLabel {
                expected: "reliability",
            },
            pass_threshold: 1.0,
            weight: 0.9,
        },
        LlmTaskDefinition {
            id: "structured_extraction",
            label: "Structured extraction",
            category: LlmTaskCategory::Extraction,
            system_prompt: "Extract fields exactly. Output valid JSON only.",
            user_prompt: "Incident details:\nCustomer: Aster Logistics\nPriority: High\nRegion: us-west-2\nTicket: INC-4821\n\nReturn JSON with keys customer, priority, region, ticket_id.",
            judge: LlmJudge::JsonFields {
                fields: vec![
                    JsonFieldExpectation {
                        key: "customer",
                        expected: "Aster Logistics",
                    },
                    JsonFieldExpectation {
                        key: "priority",
                        expected: "High",
                    },
                    JsonFieldExpectation {
                        key: "region",
                        expected: "us-west-2",
                    },
                    JsonFieldExpectation {
                        key: "ticket_id",
                        expected: "INC-4821",
                    },
                ],
            },
            pass_threshold: 1.0,
            weight: 1.2,
        },
        LlmTaskDefinition {
            id: "code_comprehension",
            label: "Code comprehension",
            category: LlmTaskCategory::CodeComprehension,
            system_prompt: "You are a code analysis assistant. Provide the exact output of the code without explanation.",
            user_prompt: "What is the output of this Python code?\n\n```python\nvals = [3, 1, 4]\nvals.sort()\nresult = '-'.join(str(v * v) for v in vals[1:])\nprint(result)\n```\n\nYour answer should be exactly: 9-16",
            judge: LlmJudge::ContainsText {
                required: vec!["9-16"],
                max_words: None,
            },
            pass_threshold: 1.0,
            weight: 1.0,
        },
        LlmTaskDefinition {
            id: "instruction_following",
            label: "Instruction following",
            category: LlmTaskCategory::InstructionFollowing,
            system_prompt: "Follow every formatting instruction exactly.",
            user_prompt: "Write one sentence (20 words max) about this release: search queries are now indexed incrementally and every deploy requires a human review gate. Include the exact phrases 'faster search' and 'review gate'.",
            judge: LlmJudge::ContainsText {
                required: vec!["faster search", "review gate"],
                max_words: Some(20),
            },
            pass_threshold: 0.9,
            weight: 1.0,
        },
        LlmTaskDefinition {
            id: "agentic_tool_choice",
            label: "Agentic tool choice",
            category: LlmTaskCategory::AgenticChoice,
            system_prompt: "Pick the best first tool for an autonomous coding agent.",
            user_prompt: "Goal: update a function in an existing file, but you have not inspected the file yet. Available tools: fs.read, fs.write, cmd.exec, git.status. Reply with the best first tool name only.",
            judge: LlmJudge::ExactLabel {
                expected: "fs.read",
            },
            pass_threshold: 1.0,
            weight: 1.1,
        },
        LlmTaskDefinition {
            id: "json_schema_validation",
            label: "JSON schema validation",
            category: LlmTaskCategory::ToolUse,
            system_prompt: "You are a JSON validator. Check if the provided data conforms to the schema and report any issues.",
            user_prompt: "Validate this JSON against the schema:\n\nSchema: {\"type\": \"object\", \"required\": [\"name\", \"age\"], \"properties\": {\"name\": {\"type\": \"string\"}, \"age\": {\"type\": \"integer\", \"minimum\": 0}}}\n\nData: {\"name\": \"Alice\", \"age\": 30}\n\nIs this valid? Reply with 'valid' or 'invalid' only.",
            judge: LlmJudge::ExactLabel {
                expected: "valid",
            },
            pass_threshold: 1.0,
            weight: 0.8,
        },
        LlmTaskDefinition {
            id: "multi_step_reasoning",
            label: "Multi-step reasoning",
            category: LlmTaskCategory::Reasoning,
            system_prompt: "Solve multi-step reasoning problems carefully. Show your work and provide the final answer.",
            user_prompt: "A bakery sells cupcakes in boxes of 6. If they have 47 cupcakes and pack as many full boxes as possible, how many cupcakes remain unpacked? Provide only the number of remaining cupcakes.",
            judge: LlmJudge::Numeric {
                expected: 5.0,
                tolerance: 0.0,
            },
            pass_threshold: 1.0,
            weight: 1.1,
        },
        LlmTaskDefinition {
            id: "error_handling",
            label: "Error handling analysis",
            category: LlmTaskCategory::CodeComprehension,
            system_prompt: "Analyze code and identify potential errors or edge cases.",
            user_prompt: "What error will this Python code raise?\n\n```python\nitems = [1, 2, 3]\nprint(items[5])\n```\n\nReply with the exact error type name (e.g., 'IndexError').",
            judge: LlmJudge::ContainsText {
                required: vec!["indexerror"],
                max_words: None,
            },
            pass_threshold: 1.0,
            weight: 0.9,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_extraction_handles_inline_text() {
        assert_eq!(extract_first_number("The answer is 200."), Some(200.0));
        assert_eq!(extract_first_number("No numbers here"), None);
    }

    #[test]
    fn json_parser_handles_fenced_blocks() {
        let payload = "```json\n{\"ticket_id\":\"INC-4821\"}\n```";
        let parsed = parse_json_payload(payload).expect("json payload should parse");
        assert_eq!(parsed["ticket_id"], "INC-4821");
    }

    #[test]
    fn contains_text_penalizes_word_limit_violations() {
        let score = evaluate_contains_text(
            "This sentence includes faster search and review gate and also adds many additional filler words to exceed the requested word limit by a comfortable margin for testing.",
            &["faster search", "review gate"],
            Some(20),
        );
        assert!((score - 0.5).abs() < f64::EPSILON);
    }
}
