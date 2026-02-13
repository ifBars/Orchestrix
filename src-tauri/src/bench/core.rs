use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadKind {
    Embeddings,
    LlmAgentLoop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRunMetadata {
    pub schema_version: u32,
    pub workload: WorkloadKind,
    pub generated_at: String,
    pub warmup_iterations: usize,
    pub measured_iterations: usize,
    pub batch_sizes: Vec<usize>,
}

impl BenchmarkRunMetadata {
    pub fn new(
        workload: WorkloadKind,
        warmup_iterations: usize,
        measured_iterations: usize,
        batch_sizes: Vec<usize>,
    ) -> Self {
        Self {
            schema_version: 1,
            workload,
            generated_at: Utc::now().to_rfc3339(),
            warmup_iterations,
            measured_iterations,
            batch_sizes,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub mean_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
}

pub fn compute_latency_stats(samples: &[Duration]) -> LatencyStats {
    if samples.is_empty() {
        return LatencyStats {
            p50_ms: 0.0,
            p95_ms: 0.0,
            mean_ms: 0.0,
            min_ms: 0.0,
            max_ms: 0.0,
        };
    }

    let mut millis: Vec<f64> = samples
        .iter()
        .map(|duration| duration.as_secs_f64() * 1000.0)
        .collect();
    millis.sort_by(|lhs, rhs| lhs.partial_cmp(rhs).unwrap_or(std::cmp::Ordering::Equal));

    let sum = millis.iter().sum::<f64>();
    let len = millis.len();

    LatencyStats {
        p50_ms: percentile(&millis, 0.50),
        p95_ms: percentile(&millis, 0.95),
        mean_ms: sum / len as f64,
        min_ms: *millis.first().unwrap_or(&0.0),
        max_ms: *millis.last().unwrap_or(&0.0),
    }
}

fn percentile(values: &[f64], quantile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    if values.len() == 1 {
        return values[0];
    }

    let clamped = quantile.clamp(0.0, 1.0);
    let position = clamped * ((values.len() - 1) as f64);
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        return values[lower];
    }

    let weight = position - lower as f64;
    values[lower] * (1.0 - weight) + values[upper] * weight
}
