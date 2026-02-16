use std::cmp::Ordering;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::bench::embeddings::{EmbeddingsBenchOptions, EmbeddingsBenchReport};
use crate::bench::llm::{
    run_llm_benchmark, LlmBenchOptions, LlmBenchReport, LlmCategoryWinner, LlmTaskCategory,
};

use super::embeddings::run_embeddings_benchmark;

#[derive(Debug, Clone)]
pub struct BenchmarkSuiteOptions {
    pub embeddings: Option<EmbeddingsBenchOptions>,
    pub llm: Option<LlmBenchOptions>,
}

impl Default for BenchmarkSuiteOptions {
    fn default() -> Self {
        Self {
            embeddings: Some(EmbeddingsBenchOptions {
                providers: crate::embeddings::EmbeddingProviderId::all().to_vec(),
                warmup_iterations: 1,
                measured_iterations: 4,
                batch_sizes: vec![1, 8, 32],
                normalize_l2: false,
            }),
            llm: Some(LlmBenchOptions::default()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSuiteReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub embeddings: Option<EmbeddingsBenchReport>,
    pub llm: Option<LlmBenchReport>,
    pub highlights: BenchmarkSuiteHighlights,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BenchmarkSuiteHighlights {
    pub llm_overall_winner: Option<LlmOverallHighlight>,
    pub llm_category_winners: Vec<LlmCategoryHighlight>,
    pub embedding_fastest_provider: Option<EmbeddingHighlight>,
    pub embedding_best_quality_provider: Option<EmbeddingHighlight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmOverallHighlight {
    pub provider: String,
    pub model: Option<String>,
    pub weighted_score: f64,
    pub avg_p50_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCategoryHighlight {
    pub category: LlmTaskCategory,
    pub provider: String,
    pub model: Option<String>,
    pub average_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingHighlight {
    pub provider: String,
    pub average_p50_latency_ms: f64,
    pub average_quality_score: f64,
}

pub async fn run_benchmark_suite(options: BenchmarkSuiteOptions) -> BenchmarkSuiteReport {
    let embeddings = if let Some(options) = options.embeddings {
        Some(run_embeddings_benchmark(options).await)
    } else {
        None
    };

    let llm = if let Some(options) = options.llm {
        Some(run_llm_benchmark(options).await)
    } else {
        None
    };

    let highlights = build_highlights(embeddings.as_ref(), llm.as_ref());

    BenchmarkSuiteReport {
        schema_version: 1,
        generated_at: Utc::now().to_rfc3339(),
        embeddings,
        llm,
        highlights,
    }
}

fn build_highlights(
    embeddings: Option<&EmbeddingsBenchReport>,
    llm: Option<&LlmBenchReport>,
) -> BenchmarkSuiteHighlights {
    let mut highlights = BenchmarkSuiteHighlights::default();

    if let Some(llm_report) = llm {
        highlights.llm_overall_winner =
            llm_report
                .overall_winner
                .as_ref()
                .map(|winner| LlmOverallHighlight {
                    provider: winner.provider.clone(),
                    model: winner.model.clone(),
                    weighted_score: winner.weighted_score,
                    avg_p50_latency_ms: winner.avg_p50_latency_ms,
                });
        highlights.llm_category_winners = llm_report
            .category_winners
            .iter()
            .map(convert_category_winner)
            .collect();
    }

    if let Some(embedding_report) = embeddings {
        let ranked = rank_embedding_providers(embedding_report);
        highlights.embedding_fastest_provider = ranked
            .iter()
            .min_by(|lhs, rhs| {
                lhs.average_p50_latency_ms
                    .partial_cmp(&rhs.average_p50_latency_ms)
                    .unwrap_or(Ordering::Equal)
            })
            .cloned();
        highlights.embedding_best_quality_provider = ranked
            .iter()
            .max_by(|lhs, rhs| {
                lhs.average_quality_score
                    .partial_cmp(&rhs.average_quality_score)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| {
                        rhs.average_p50_latency_ms
                            .partial_cmp(&lhs.average_p50_latency_ms)
                            .unwrap_or(Ordering::Equal)
                    })
            })
            .cloned();
    }

    highlights
}

fn convert_category_winner(winner: &LlmCategoryWinner) -> LlmCategoryHighlight {
    LlmCategoryHighlight {
        category: winner.category,
        provider: winner.provider.clone(),
        model: winner.model.clone(),
        average_score: winner.average_score,
    }
}

fn rank_embedding_providers(report: &EmbeddingsBenchReport) -> Vec<EmbeddingHighlight> {
    report
        .providers
        .iter()
        .filter(|provider| provider.status == "ok")
        .map(|provider| {
            let scenario_count = provider.scenarios.len().max(1) as f64;
            let average_p50_latency_ms = provider
                .scenarios
                .iter()
                .map(|scenario| scenario.latency.p50_ms)
                .sum::<f64>()
                / scenario_count;

            let average_quality_score = provider
                .quality
                .as_ref()
                .map(|quality| quality.quality_score)
                .unwrap_or(0.0);

            EmbeddingHighlight {
                provider: provider.provider.clone(),
                average_p50_latency_ms,
                average_quality_score,
            }
        })
        .collect()
}
