use std::path::PathBuf;
use std::str::FromStr;

use orchestrix_lib::bench::embeddings::{
    run_embeddings_benchmark, EmbeddingsBenchOptions, EmbeddingsBenchReport,
};
use orchestrix_lib::embeddings::EmbeddingProviderId;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("embeddings benchmark failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let mut providers = EmbeddingProviderId::all().to_vec();
    let mut warmup_iterations = 2usize;
    let mut measured_iterations = 8usize;
    let mut batch_sizes = vec![1usize, 8usize, 32usize];
    let mut output_path = PathBuf::from("bench_results.json");
    let mut normalize_l2 = false;

    let mut args = std::env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            print_help();
            return Ok(());
        }

        if let Some(value) = arg.strip_prefix("--providers=") {
            providers = parse_provider_list(value)?;
            continue;
        }
        if arg == "--providers" {
            let value = args
                .next()
                .ok_or_else(|| "--providers requires a value".to_string())?;
            providers = parse_provider_list(&value)?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--warmup=") {
            warmup_iterations = parse_positive_usize(value, "--warmup")?;
            continue;
        }
        if arg == "--warmup" {
            let value = args
                .next()
                .ok_or_else(|| "--warmup requires a value".to_string())?;
            warmup_iterations = parse_positive_usize(&value, "--warmup")?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--iterations=") {
            measured_iterations = parse_positive_usize(value, "--iterations")?;
            continue;
        }
        if arg == "--iterations" {
            let value = args
                .next()
                .ok_or_else(|| "--iterations requires a value".to_string())?;
            measured_iterations = parse_positive_usize(&value, "--iterations")?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--batch-sizes=") {
            batch_sizes = parse_batch_sizes(value)?;
            continue;
        }
        if arg == "--batch-sizes" {
            let value = args
                .next()
                .ok_or_else(|| "--batch-sizes requires a value".to_string())?;
            batch_sizes = parse_batch_sizes(&value)?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--output=") {
            output_path = PathBuf::from(value);
            continue;
        }
        if arg == "--output" {
            let value = args
                .next()
                .ok_or_else(|| "--output requires a value".to_string())?;
            output_path = PathBuf::from(value);
            continue;
        }

        if arg == "--normalize-l2" {
            normalize_l2 = true;
            continue;
        }

        return Err(format!("unknown argument: {arg}"));
    }

    let report = run_embeddings_benchmark(EmbeddingsBenchOptions {
        providers,
        warmup_iterations,
        measured_iterations,
        batch_sizes,
        normalize_l2,
    })
    .await;

    let output_json = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize benchmark output: {error}"))?;
    std::fs::write(&output_path, output_json)
        .map_err(|error| format!("failed to write '{}': {error}", output_path.display()))?;

    print_summary_table(&report);
    println!("\nSaved benchmark output to {}", output_path.display());
    Ok(())
}

fn print_help() {
    println!("Embeddings benchmark runner");
    println!();
    println!("Usage:");
    println!(
        "  cargo run --manifest-path src-tauri/Cargo.toml --bin embeddings_bench -- [options]"
    );
    println!();
    println!("Options:");
    println!("  --providers gemini,ollama,transformersjs,rust-hf");
    println!("  --warmup <n>            Warmup iterations per scenario (default: 2)");
    println!("  --iterations <n>        Measured iterations per scenario (default: 8)");
    println!("  --batch-sizes 1,8,32    Batch sizes to benchmark (default: 1,8,32)");
    println!("  --normalize-l2          Apply shared L2 normalization to outputs");
    println!("  --output <path>         Output JSON file path (default: bench_results.json)");
}

fn parse_provider_list(value: &str) -> Result<Vec<EmbeddingProviderId>, String> {
    let mut providers = Vec::new();
    for token in value.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let provider = EmbeddingProviderId::from_str(trimmed).map_err(|error| error.to_string())?;
        if !providers.contains(&provider) {
            providers.push(provider);
        }
    }

    if providers.is_empty() {
        return Err("--providers must include at least one provider".to_string());
    }
    Ok(providers)
}

fn parse_batch_sizes(value: &str) -> Result<Vec<usize>, String> {
    let mut values = Vec::new();
    for token in value.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed = parse_positive_usize(trimmed, "--batch-sizes")?;
        if !values.contains(&parsed) {
            values.push(parsed);
        }
    }
    if values.is_empty() {
        return Err("--batch-sizes must include at least one value".to_string());
    }
    Ok(values)
}

fn parse_positive_usize(value: &str, flag: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{flag} expects a positive integer, got '{value}'"))?;
    if parsed == 0 {
        return Err(format!("{flag} expects a value greater than 0"));
    }
    Ok(parsed)
}

fn print_summary_table(report: &EmbeddingsBenchReport) {
    #[derive(Debug)]
    struct Row {
        provider: String,
        kind: String,
        dims: String,
        p50_ms: f64,
        throughput_texts: f64,
        throughput_chars: f64,
        mrr: f64,
        self_sim: String,
        status: String,
        error: Option<String>,
    }

    let mut rows: Vec<Row> = report
        .providers
        .iter()
        .map(|provider| {
            let count = provider.scenarios.len().max(1) as f64;
            let avg_p50 = provider
                .scenarios
                .iter()
                .map(|row| row.latency.p50_ms)
                .sum::<f64>()
                / count;
            let avg_texts = provider
                .scenarios
                .iter()
                .map(|row| row.throughput_texts_per_sec)
                .sum::<f64>()
                / count;
            let avg_chars = provider
                .scenarios
                .iter()
                .map(|row| row.throughput_chars_per_sec)
                .sum::<f64>()
                / count;

            Row {
                provider: provider.provider.clone(),
                kind: provider
                    .kind
                    .map(|kind| format!("{kind:?}").to_lowercase())
                    .unwrap_or_else(|| "-".to_string()),
                dims: provider
                    .dims
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                p50_ms: avg_p50,
                throughput_texts: avg_texts,
                throughput_chars: avg_chars,
                mrr: provider
                    .quality
                    .as_ref()
                    .map(|quality| quality.retrieval_mrr_at_10)
                    .unwrap_or(0.0),
                self_sim: provider
                    .quality
                    .as_ref()
                    .map(|quality| {
                        if quality.self_similarity_pass {
                            format!("pass(rank={})", quality.self_similarity_rank)
                        } else {
                            format!("fail(rank={})", quality.self_similarity_rank)
                        }
                    })
                    .unwrap_or_else(|| "-".to_string()),
                status: provider.status.clone(),
                error: provider.error.clone(),
            }
        })
        .collect();

    rows.sort_by(|lhs, rhs| {
        let lhs_ok = lhs.status == "ok";
        let rhs_ok = rhs.status == "ok";
        rhs_ok
            .cmp(&lhs_ok)
            .then_with(|| {
                lhs.p50_ms
                    .partial_cmp(&rhs.p50_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                rhs.throughput_texts
                    .partial_cmp(&lhs.throughput_texts)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    println!(
        "{:<16} {:<8} {:<6} {:>10} {:>12} {:>14} {:>8} {:<16} {:<8}",
        "provider",
        "kind",
        "dims",
        "p50(ms)",
        "texts/sec",
        "chars/sec",
        "MRR@10",
        "self-sim",
        "status"
    );
    println!(
        "{:-<16} {:-<8} {:-<6} {:-<10} {:-<12} {:-<14} {:-<8} {:-<16} {:-<8}",
        "", "", "", "", "", "", "", "", ""
    );

    for row in rows {
        println!(
            "{:<16} {:<8} {:<6} {:>10.2} {:>12.2} {:>14.2} {:>8.4} {:<16} {:<8}",
            row.provider,
            row.kind,
            row.dims,
            row.p50_ms,
            row.throughput_texts,
            row.throughput_chars,
            row.mrr,
            row.self_sim,
            row.status,
        );

        if let Some(error) = row.error {
            println!("  error: {error}");
        }
    }
}
