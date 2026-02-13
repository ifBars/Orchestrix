use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::bench::core::{compute_latency_stats, BenchmarkRunMetadata, LatencyStats, WorkloadKind};
use crate::embeddings::config::{EmbeddingConfig, EmbeddingProviderId};
use crate::embeddings::factory::create_provider;
use crate::embeddings::types::{
    cosine_similarity, EmbedOptions, EmbeddingProvider, EmbeddingProviderKind, EmbeddingTaskType,
};

#[derive(Debug, Clone)]
pub struct EmbeddingsBenchOptions {
    pub providers: Vec<EmbeddingProviderId>,
    pub warmup_iterations: usize,
    pub measured_iterations: usize,
    pub batch_sizes: Vec<usize>,
    pub normalize_l2: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsBenchReport {
    pub metadata: BenchmarkRunMetadata,
    pub providers: Vec<ProviderBenchmarkResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderBenchmarkResult {
    pub provider: String,
    pub kind: Option<EmbeddingProviderKind>,
    pub status: String,
    pub error: Option<String>,
    pub dims: Option<usize>,
    pub first_load_ms: Option<f64>,
    pub scenarios: Vec<ScenarioBenchmarkResult>,
    pub quality: Option<EmbeddingQualitySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioBenchmarkResult {
    pub scenario_id: String,
    pub scenario_label: String,
    pub batch_size: usize,
    pub input_chars: usize,
    pub latency: LatencyStats,
    pub throughput_texts_per_sec: f64,
    pub throughput_chars_per_sec: f64,
    pub memory_rss_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingQualitySummary {
    pub self_similarity_pass: bool,
    pub self_similarity_rank: usize,
    pub retrieval_mrr_at_10: f64,
}

#[derive(Clone)]
struct ScenarioDefinition {
    id: &'static str,
    label: &'static str,
    task: EmbeddingTaskType,
    text: String,
}

pub async fn run_embeddings_benchmark(options: EmbeddingsBenchOptions) -> EmbeddingsBenchReport {
    let metadata = BenchmarkRunMetadata::new(
        WorkloadKind::Embeddings,
        options.warmup_iterations,
        options.measured_iterations,
        options.batch_sizes.clone(),
    );

    let scenarios = default_scenarios();
    let mut results = Vec::new();

    for provider_id in &options.providers {
        let mut config = EmbeddingConfig::default();
        config.apply_env_overrides();
        config.provider = *provider_id;
        config.normalize_l2 = options.normalize_l2;

        let provider_result = match create_provider(&config) {
            Ok(provider) => run_provider_benchmark(provider, &options, &scenarios)
                .await
                .unwrap_or_else(|error| ProviderBenchmarkResult {
                    provider: provider_id.as_str().to_string(),
                    kind: None,
                    status: "error".to_string(),
                    error: Some(error.to_string()),
                    dims: None,
                    first_load_ms: None,
                    scenarios: Vec::new(),
                    quality: None,
                }),
            Err(error) => ProviderBenchmarkResult {
                provider: provider_id.as_str().to_string(),
                kind: None,
                status: "error".to_string(),
                error: Some(error.to_string()),
                dims: None,
                first_load_ms: None,
                scenarios: Vec::new(),
                quality: None,
            },
        };

        results.push(provider_result);
    }

    EmbeddingsBenchReport {
        metadata,
        providers: results,
    }
}

async fn run_provider_benchmark(
    provider: Arc<dyn EmbeddingProvider>,
    options: &EmbeddingsBenchOptions,
    scenarios: &[ScenarioDefinition],
) -> Result<ProviderBenchmarkResult, crate::embeddings::EmbeddingError> {
    let first_load_started = Instant::now();
    let mut dims = provider.dims().await?;
    if dims.is_none() {
        let probe = vec!["benchmark probe".to_string()];
        let probe_vectors = provider
            .embed(
                &probe,
                Some(EmbedOptions {
                    task: Some(EmbeddingTaskType::RetrievalDocument),
                }),
            )
            .await?;
        dims = probe_vectors.first().map(|vector| vector.len());
    }
    let first_load_ms = first_load_started.elapsed().as_secs_f64() * 1000.0;

    let mut scenario_results = Vec::new();
    for scenario in scenarios {
        for batch_size in &options.batch_sizes {
            let texts = build_batch_texts(scenario, *batch_size);
            let input_chars = texts.iter().map(|value| value.len()).sum::<usize>();
            let opts = Some(EmbedOptions {
                task: Some(scenario.task),
            });

            for _ in 0..options.warmup_iterations {
                let _ = provider.embed(&texts, opts.clone()).await?;
            }

            let memory_before = current_process_memory_bytes();
            let mut samples = Vec::with_capacity(options.measured_iterations);
            for _ in 0..options.measured_iterations {
                let started = Instant::now();
                let _ = provider.embed(&texts, opts.clone()).await?;
                samples.push(started.elapsed());
            }
            let memory_after = current_process_memory_bytes();

            let total = samples
                .iter()
                .copied()
                .fold(Duration::ZERO, |acc, value| acc + value);
            let total_seconds = total.as_secs_f64().max(f64::EPSILON);
            let throughput_texts =
                (*batch_size as f64 * options.measured_iterations as f64) / total_seconds;
            let throughput_chars =
                (input_chars as f64 * options.measured_iterations as f64) / total_seconds;

            let memory_rss_bytes = match (memory_before, memory_after) {
                (Some(before), Some(after)) if after >= before => Some(after - before),
                (Some(_), Some(after)) => Some(after),
                _ => None,
            };

            scenario_results.push(ScenarioBenchmarkResult {
                scenario_id: scenario.id.to_string(),
                scenario_label: scenario.label.to_string(),
                batch_size: *batch_size,
                input_chars,
                latency: compute_latency_stats(&samples),
                throughput_texts_per_sec: throughput_texts,
                throughput_chars_per_sec: throughput_chars,
                memory_rss_bytes,
            });
        }
    }

    let quality = run_quality_checks(provider.as_ref()).await?;

    Ok(ProviderBenchmarkResult {
        provider: provider.id().to_string(),
        kind: Some(provider.kind()),
        status: "ok".to_string(),
        error: None,
        dims,
        first_load_ms: Some(first_load_ms),
        scenarios: scenario_results,
        quality: Some(quality),
    })
}

async fn run_quality_checks(
    provider: &dyn EmbeddingProvider,
) -> Result<EmbeddingQualitySummary, crate::embeddings::EmbeddingError> {
    let (self_similarity_pass, self_similarity_rank) = run_self_similarity_check(provider).await?;
    let retrieval_mrr_at_10 = run_retrieval_mrr_check(provider).await?;

    Ok(EmbeddingQualitySummary {
        self_similarity_pass,
        self_similarity_rank,
        retrieval_mrr_at_10,
    })
}

async fn run_self_similarity_check(
    provider: &dyn EmbeddingProvider,
) -> Result<(bool, usize), crate::embeddings::EmbeddingError> {
    let docs = vec![
        "Inventory restock schedule for downtown vending machines".to_string(),
        "Quarterly supplier negotiation checklist".to_string(),
        "Machine maintenance safety checklist".to_string(),
        "Coupon redemption tracking policy".to_string(),
    ];
    let query = "Inventory restock schedule for downtown vending machines".to_string();

    let doc_vectors = provider
        .embed(
            &docs,
            Some(EmbedOptions {
                task: Some(EmbeddingTaskType::RetrievalDocument),
            }),
        )
        .await?;
    let query_vectors = provider
        .embed(
            &[query],
            Some(EmbedOptions {
                task: Some(EmbeddingTaskType::RetrievalQuery),
            }),
        )
        .await?;

    let query_vector = &query_vectors[0];
    let mut ranked: Vec<(usize, f32)> = doc_vectors
        .iter()
        .enumerate()
        .map(|(index, doc)| (index, cosine_similarity(query_vector, doc)))
        .collect();
    ranked.sort_by(|lhs, rhs| {
        rhs.1
            .partial_cmp(&lhs.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let rank = ranked
        .iter()
        .position(|(index, _)| *index == 0)
        .map(|value| value + 1)
        .unwrap_or(usize::MAX);
    Ok((rank == 1, rank))
}

async fn run_retrieval_mrr_check(
    provider: &dyn EmbeddingProvider,
) -> Result<f64, crate::embeddings::EmbeddingError> {
    let corpus_size = 20usize;
    let docs: Vec<String> = (0..corpus_size)
        .map(|index| {
            format!(
                "Machine {index} in district {index} sells snack_{index}, soda_{index}, and combo_{index}. Supplier contract_{index} controls pricing tier {index}."
            )
        })
        .collect();
    let queries: Vec<(String, usize)> = (0..corpus_size)
        .map(|index| {
            (
                format!(
                    "Find the machine that sells snack_{index} and soda_{index} with supplier contract_{index}."
                ),
                index,
            )
        })
        .collect();

    let doc_vectors = provider
        .embed(
            &docs,
            Some(EmbedOptions {
                task: Some(EmbeddingTaskType::RetrievalDocument),
            }),
        )
        .await?;

    let mut reciprocal_sum = 0.0f64;
    for (query, relevant_index) in queries {
        let query_vector = provider
            .embed(
                &[query],
                Some(EmbedOptions {
                    task: Some(EmbeddingTaskType::RetrievalQuery),
                }),
            )
            .await?;
        let query_embedding = &query_vector[0];

        let mut ranked: Vec<(usize, f32)> = doc_vectors
            .iter()
            .enumerate()
            .map(|(index, doc)| (index, cosine_similarity(query_embedding, doc)))
            .collect();
        ranked.sort_by(|lhs, rhs| {
            rhs.1
                .partial_cmp(&lhs.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if let Some(position) = ranked
            .iter()
            .position(|(index, _)| *index == relevant_index)
        {
            let rank = position + 1;
            if rank <= 10 {
                reciprocal_sum += 1.0 / rank as f64;
            }
        }
    }

    Ok(reciprocal_sum / corpus_size as f64)
}

fn default_scenarios() -> Vec<ScenarioDefinition> {
    vec![
        ScenarioDefinition {
            id: "short_query",
            label: "Short query (1-2 sentences)",
            task: EmbeddingTaskType::RetrievalQuery,
            text: "Analyze vendor contracts and suggest a pricing update for machine cluster A based on current demand and margins."
                .to_string(),
        },
        ScenarioDefinition {
            id: "medium_chunk",
            label: "Medium chunk (~1k chars)",
            task: EmbeddingTaskType::RetrievalDocument,
            text: repeat_to_length(
                "Operational note: machine cluster A shows weekday demand spikes for sugar-free beverages, while weekend snack conversion rises near transit hubs. Supplier rebates activate after monthly volume thresholds.",
                1_024,
            ),
        },
        ScenarioDefinition {
            id: "large_chunk",
            label: "Large chunk (~8k chars)",
            task: EmbeddingTaskType::RetrievalDocument,
            text: repeat_to_length(
                "Regional performance report: coordinated pricing experiments across districts reveal elasticity variance by time-of-day. Marketing channels with bundled offers improve conversion, but replenishment delays reduce realized profit.",
                8_192,
            ),
        },
    ]
}

fn build_batch_texts(scenario: &ScenarioDefinition, batch_size: usize) -> Vec<String> {
    (0..batch_size)
        .map(|index| format!("{}\nBatch item: {}", scenario.text, index + 1))
        .collect()
}

fn repeat_to_length(seed: &str, target_len: usize) -> String {
    let mut output = String::with_capacity(target_len + seed.len());
    while output.len() < target_len {
        if !output.is_empty() {
            output.push(' ');
        }
        output.push_str(seed);
    }
    output.truncate(target_len);
    output
}

fn current_process_memory_bytes() -> Option<u64> {
    use sysinfo::{Pid, ProcessesToUpdate, System};

    let pid = Pid::from_u32(std::process::id());
    let mut system = System::new_all();
    let _ = system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).map(|process| process.memory())
}
