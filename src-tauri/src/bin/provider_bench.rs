use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use indicatif::MultiProgress;
use orchestrix_lib::bench::business_ops::{
    run_business_ops_benchmark, BusinessOpsBenchOptions, BusinessOpsBenchReport,
};
use orchestrix_lib::bench::embeddings::{EmbeddingsBenchOptions, EmbeddingsBenchReport};
use orchestrix_lib::bench::llm::{
    LlmBenchOptions, LlmBenchReport, LlmProviderConfig, LlmProviderId, LlmTaskCategory,
};
use orchestrix_lib::bench::suite::{
    run_benchmark_suite, BenchmarkSuiteOptions, BenchmarkSuiteReport,
};
use orchestrix_lib::embeddings::EmbeddingProviderId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkloadSelection {
    All,
    Embeddings,
    Llm,
    BusinessOps,
}

impl WorkloadSelection {
    fn from_flag(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "all" => Ok(Self::All),
            "embeddings" => Ok(Self::Embeddings),
            "llm" => Ok(Self::Llm),
            "business-ops" => Ok(Self::BusinessOps),
            _ => Err(format!(
                "unsupported --workload value '{value}'. Use all, embeddings, llm, or business-ops"
            )),
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("provider benchmark failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let mut workload = WorkloadSelection::All;
    let mut embedding_providers = EmbeddingProviderId::all().to_vec();
    let mut llm_providers = LlmProviderId::all().to_vec();
    let mut warmup_iterations = 1usize;
    let mut measured_iterations = 4usize;
    let mut embedding_batch_sizes = vec![1usize, 8usize, 32usize];
    let mut llm_max_tokens = 512u32;
    let mut normalize_l2 = false;
    let mut output_path = PathBuf::from("benchmark_suite_results.json");
    let mut llm_model_overrides: HashMap<LlmProviderId, String> = HashMap::new();
    let mut llm_base_url_overrides: HashMap<LlmProviderId, String> = HashMap::new();
    let mut business_ops_scenarios: Vec<String> = Vec::new();
    let mut business_ops_max_turns = 40usize;
    let mut business_ops_diagnostics = false;

    let mut args = std::env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            print_help();
            return Ok(());
        }

        if let Some(value) = arg.strip_prefix("--workload=") {
            workload = WorkloadSelection::from_flag(value)?;
            continue;
        }
        if arg == "--workload" {
            let value = args
                .next()
                .ok_or_else(|| "--workload requires a value".to_string())?;
            workload = WorkloadSelection::from_flag(&value)?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--embedding-providers=") {
            embedding_providers = parse_embedding_provider_list(value)?;
            continue;
        }
        if arg == "--embedding-providers" {
            let value = args
                .next()
                .ok_or_else(|| "--embedding-providers requires a value".to_string())?;
            embedding_providers = parse_embedding_provider_list(&value)?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--llm-providers=") {
            llm_providers = parse_llm_provider_list(value)?;
            continue;
        }
        if arg == "--llm-providers" {
            let value = args
                .next()
                .ok_or_else(|| "--llm-providers requires a value".to_string())?;
            llm_providers = parse_llm_provider_list(&value)?;
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
            embedding_batch_sizes = parse_batch_sizes(value)?;
            continue;
        }
        if arg == "--batch-sizes" {
            let value = args
                .next()
                .ok_or_else(|| "--batch-sizes requires a value".to_string())?;
            embedding_batch_sizes = parse_batch_sizes(&value)?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--llm-max-tokens=") {
            llm_max_tokens = parse_positive_u32(value, "--llm-max-tokens")?;
            continue;
        }
        if arg == "--llm-max-tokens" {
            let value = args
                .next()
                .ok_or_else(|| "--llm-max-tokens requires a value".to_string())?;
            llm_max_tokens = parse_positive_u32(&value, "--llm-max-tokens")?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--llm-model-overrides=") {
            llm_model_overrides = parse_llm_overrides(value)?;
            continue;
        }
        if arg == "--llm-model-overrides" {
            let value = args
                .next()
                .ok_or_else(|| "--llm-model-overrides requires a value".to_string())?;
            llm_model_overrides = parse_llm_overrides(&value)?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--llm-base-url-overrides=") {
            llm_base_url_overrides = parse_llm_overrides(value)?;
            continue;
        }
        if arg == "--llm-base-url-overrides" {
            let value = args
                .next()
                .ok_or_else(|| "--llm-base-url-overrides requires a value".to_string())?;
            llm_base_url_overrides = parse_llm_overrides(&value)?;
            continue;
        }

        if arg == "--normalize-l2" {
            normalize_l2 = true;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--business-ops-scenarios=") {
            business_ops_scenarios = parse_scenario_list(value)?;
            continue;
        }
        if arg == "--business-ops-scenarios" {
            let value = args
                .next()
                .ok_or_else(|| "--business-ops-scenarios requires a value".to_string())?;
            business_ops_scenarios = parse_scenario_list(&value)?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--business-ops-max-turns=") {
            business_ops_max_turns = parse_positive_usize(value, "--business-ops-max-turns")?;
            continue;
        }
        if arg == "--business-ops-max-turns" {
            let value = args
                .next()
                .ok_or_else(|| "--business-ops-max-turns requires a value".to_string())?;
            business_ops_max_turns = parse_positive_usize(&value, "--business-ops-max-turns")?;
            continue;
        }

        if arg == "--business-ops-diagnostics" {
            business_ops_diagnostics = true;
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

        return Err(format!("unknown argument: {arg}"));
    }

    // Handle business-ops workload separately
    if workload == WorkloadSelection::BusinessOps {
        let mp = MultiProgress::new();
        let llm_provider_configs = build_llm_provider_configs(
            &llm_providers,
            &llm_model_overrides,
            &llm_base_url_overrides,
        );
        let scenario_filter = business_ops_scenarios;
        let report = run_business_ops_benchmark(
            BusinessOpsBenchOptions {
                providers: llm_providers,
                warmup_iterations,
                measured_iterations,
                max_tokens: llm_max_tokens,
                provider_configs: llm_provider_configs,
                max_turns: business_ops_max_turns,
                scenario_filter,
                diagnostics: business_ops_diagnostics,
            },
            Some(&mp),
        )
        .await;

        let output_json = serde_json::to_string_pretty(&report)
            .map_err(|error| format!("failed to serialize benchmark output: {error}"))?;
        std::fs::write(&output_path, output_json)
            .map_err(|error| format!("failed to write '{}': {error}", output_path.display()))?;

        print_business_ops_summary(&report);
        println!("\nSaved benchmark output to {}", output_path.display());
        return Ok(());
    }

    let llm_provider_configs = build_llm_provider_configs(
        &llm_providers,
        &llm_model_overrides,
        &llm_base_url_overrides,
    );

    let embeddings = if workload == WorkloadSelection::Llm {
        None
    } else {
        Some(EmbeddingsBenchOptions {
            providers: embedding_providers,
            warmup_iterations,
            measured_iterations,
            batch_sizes: embedding_batch_sizes,
            normalize_l2,
        })
    };

    let llm = if workload == WorkloadSelection::Embeddings {
        None
    } else {
        Some(LlmBenchOptions {
            providers: llm_providers,
            warmup_iterations,
            measured_iterations,
            max_tokens: llm_max_tokens,
            provider_configs: llm_provider_configs,
        })
    };

    let report = run_benchmark_suite(BenchmarkSuiteOptions { embeddings, llm }).await;

    let output_json = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize benchmark output: {error}"))?;
    std::fs::write(&output_path, output_json)
        .map_err(|error| format!("failed to write '{}': {error}", output_path.display()))?;

    print_suite_summary(&report);
    println!("\nSaved benchmark output to {}", output_path.display());
    Ok(())
}

fn build_llm_provider_configs(
    providers: &[LlmProviderId],
    model_overrides: &HashMap<LlmProviderId, String>,
    base_url_overrides: &HashMap<LlmProviderId, String>,
) -> Vec<LlmProviderConfig> {
    providers
        .iter()
        .filter_map(|provider| {
            let model = model_overrides.get(provider).cloned();
            let base_url = base_url_overrides.get(provider).cloned();
            if model.is_none() && base_url.is_none() {
                return None;
            }

            Some(LlmProviderConfig {
                provider: *provider,
                api_key: None,
                model,
                base_url,
            })
        })
        .collect()
}

fn print_suite_summary(report: &BenchmarkSuiteReport) {
    if let Some(embedding_report) = report.embeddings.as_ref() {
        print_embeddings_summary(embedding_report);
    }
    if let Some(llm_report) = report.llm.as_ref() {
        print_llm_summary(llm_report);
    }

    println!("\nHighlights");
    println!("----------");

    if let Some(winner) = report.highlights.embedding_fastest_provider.as_ref() {
        println!(
            "Fastest embedding provider: {} (avg p50 {:.2} ms)",
            winner.provider, winner.average_p50_latency_ms
        );
    }
    if let Some(winner) = report.highlights.embedding_best_quality_provider.as_ref() {
        println!(
            "Best embedding quality: {} (quality {:.4})",
            winner.provider, winner.average_quality_score
        );
    }
    if let Some(winner) = report.highlights.llm_overall_winner.as_ref() {
        println!(
            "Best LLM overall: {} ({}) score {:.4}",
            winner.provider,
            winner.model.clone().unwrap_or_else(|| "-".to_string()),
            winner.weighted_score
        );
    }
}

fn print_embeddings_summary(report: &EmbeddingsBenchReport) {
    #[derive(Debug)]
    struct Row {
        provider: String,
        kind: String,
        p50_ms: f64,
        texts_per_sec: f64,
        mrr: f64,
        quality_score: f64,
        status: String,
    }

    let mut rows: Vec<Row> = report
        .providers
        .iter()
        .map(|provider| {
            let scenario_count = provider.scenarios.len().max(1) as f64;
            let p50_ms = provider
                .scenarios
                .iter()
                .map(|scenario| scenario.latency.p50_ms)
                .sum::<f64>()
                / scenario_count;
            let texts_per_sec = provider
                .scenarios
                .iter()
                .map(|scenario| scenario.throughput_texts_per_sec)
                .sum::<f64>()
                / scenario_count;

            Row {
                provider: provider.provider.clone(),
                kind: provider
                    .kind
                    .map(|kind| format!("{kind:?}").to_ascii_lowercase())
                    .unwrap_or_else(|| "-".to_string()),
                p50_ms,
                texts_per_sec,
                mrr: provider
                    .quality
                    .as_ref()
                    .map(|quality| quality.retrieval_mrr_at_10)
                    .unwrap_or(0.0),
                quality_score: provider
                    .quality
                    .as_ref()
                    .map(|quality| quality.quality_score)
                    .unwrap_or(0.0),
                status: provider.status.clone(),
            }
        })
        .collect();

    rows.sort_by(|lhs, rhs| {
        let lhs_ok = lhs.status == "ok";
        let rhs_ok = rhs.status == "ok";
        rhs_ok
            .cmp(&lhs_ok)
            .then_with(|| {
                rhs.quality_score
                    .partial_cmp(&lhs.quality_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                lhs.p50_ms
                    .partial_cmp(&rhs.p50_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    println!("\nEmbeddings Benchmark");
    println!("--------------------");
    println!(
        "{:<16} {:<10} {:>10} {:>12} {:>10} {:>10} {:<8}",
        "provider", "kind", "p50(ms)", "texts/sec", "MRR@10", "qscore", "status"
    );
    println!(
        "{:-<16} {:-<10} {:-<10} {:-<12} {:-<10} {:-<10} {:-<8}",
        "", "", "", "", "", "", ""
    );

    for row in rows {
        println!(
            "{:<16} {:<10} {:>10.2} {:>12.2} {:>10.4} {:>10.4} {:<8}",
            row.provider,
            row.kind,
            row.p50_ms,
            row.texts_per_sec,
            row.mrr,
            row.quality_score,
            row.status,
        );
    }
}

fn print_llm_summary(report: &LlmBenchReport) {
    #[derive(Debug)]
    struct Row {
        provider: String,
        model: String,
        weighted_score: f64,
        pass_rate: f64,
        success_rate: f64,
        p50_ms: f64,
        status: String,
        error: Option<String>,
    }

    let mut rows: Vec<Row> = report
        .providers
        .iter()
        .map(|provider| {
            let aggregate = provider.aggregate.as_ref();
            Row {
                provider: provider.provider.clone(),
                model: provider.model.clone().unwrap_or_else(|| "-".to_string()),
                weighted_score: aggregate.map(|value| value.weighted_score).unwrap_or(0.0),
                pass_rate: aggregate.map(|value| value.pass_rate).unwrap_or(0.0),
                success_rate: aggregate.map(|value| value.success_rate).unwrap_or(0.0),
                p50_ms: aggregate
                    .map(|value| value.avg_p50_latency_ms)
                    .unwrap_or(0.0),
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
                rhs.weighted_score
                    .partial_cmp(&lhs.weighted_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                lhs.p50_ms
                    .partial_cmp(&rhs.p50_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    println!("\nLLM Benchmark");
    println!("-------------");
    println!(
        "{:<10} {:<24} {:>8} {:>8} {:>8} {:>10} {:<8}",
        "provider", "model", "score", "pass", "success", "p50(ms)", "status"
    );
    println!(
        "{:-<10} {:-<24} {:-<8} {:-<8} {:-<8} {:-<10} {:-<8}",
        "", "", "", "", "", "", ""
    );

    for row in rows {
        println!(
            "{:<10} {:<24} {:>8.4} {:>8.2} {:>8.2} {:>10.2} {:<8}",
            row.provider,
            row.model,
            row.weighted_score,
            row.pass_rate,
            row.success_rate,
            row.p50_ms,
            row.status,
        );
        if let Some(error) = row.error {
            println!("  error: {error}");
        }
    }

    if !report.category_winners.is_empty() {
        println!("\nBest provider by task category:");
        for winner in &report.category_winners {
            println!(
                "  {:<22} {:<10} score {:.4}",
                category_label(winner.category),
                winner.provider,
                winner.average_score
            );
        }
    }
}

fn category_label(category: LlmTaskCategory) -> &'static str {
    match category {
        LlmTaskCategory::Reasoning => "reasoning",
        LlmTaskCategory::Classification => "classification",
        LlmTaskCategory::Extraction => "extraction",
        LlmTaskCategory::CodeComprehension => "code_comprehension",
        LlmTaskCategory::InstructionFollowing => "instruction_following",
        LlmTaskCategory::AgenticChoice => "agentic_choice",
        LlmTaskCategory::ToolUse => "tool_use",
    }
}

fn print_business_ops_summary(report: &BusinessOpsBenchReport) {
    println!("\nBusiness Operations Benchmark");
    println!("-----------------------------");

    #[derive(Debug)]
    struct Row {
        provider: String,
        model: String,
        avg_score: f64,
        avg_profit: f64,
        success_rate: f64,
        status: String,
        error: Option<String>,
    }

    let mut rows: Vec<Row> = report
        .providers
        .iter()
        .map(|provider| Row {
            provider: provider.provider.clone(),
            model: provider.model.clone().unwrap_or_else(|| "-".to_string()),
            avg_score: provider.aggregate.avg_score,
            avg_profit: provider.aggregate.avg_profit,
            success_rate: provider.aggregate.success_rate,
            status: provider.status.clone(),
            error: provider.error.clone(),
        })
        .collect();

    rows.sort_by(|lhs, rhs| {
        let lhs_ok = lhs.status == "ok";
        let rhs_ok = rhs.status == "ok";
        rhs_ok.cmp(&lhs_ok).then_with(|| {
            rhs.avg_score
                .partial_cmp(&lhs.avg_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    println!(
        "{:<10} {:<24} {:>8} {:>12} {:>10} {:<8}",
        "provider", "model", "score", "profit", "success", "status"
    );
    println!(
        "{:-<10} {:-<24} {:-<8} {:-<12} {:-<10} {:-<8}",
        "", "", "", "", "", ""
    );

    for row in rows {
        println!(
            "{:<10} {:<24} {:>8.4} {:>12.2} {:>10.2} {:<8}",
            row.provider, row.model, row.avg_score, row.avg_profit, row.success_rate, row.status,
        );
        if let Some(error) = row.error {
            println!("  error: {error}");
        }
    }

    if let Some(winner) = report.overall_winner.as_ref() {
        println!(
            "\nWinner: {} ({}) score {:.4}",
            winner.provider,
            winner.model.as_deref().unwrap_or("-"),
            winner.avg_score
        );
    }
}

fn parse_embedding_provider_list(value: &str) -> Result<Vec<EmbeddingProviderId>, String> {
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
        return Err("--embedding-providers must include at least one provider".to_string());
    }
    Ok(providers)
}

fn parse_llm_provider_list(value: &str) -> Result<Vec<LlmProviderId>, String> {
    let mut providers = Vec::new();
    for token in value.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let provider = LlmProviderId::from_str(trimmed)?;
        if !providers.contains(&provider) {
            providers.push(provider);
        }
    }

    if providers.is_empty() {
        return Err("--llm-providers must include at least one provider".to_string());
    }
    Ok(providers)
}

fn parse_llm_overrides(value: &str) -> Result<HashMap<LlmProviderId, String>, String> {
    let mut output = HashMap::new();
    for token in value.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (provider_raw, value_raw) = trimmed
            .split_once('=')
            .ok_or_else(|| format!("override entry '{trimmed}' must be provider=value"))?;
        let provider = LlmProviderId::from_str(provider_raw.trim())?;
        let override_value = value_raw.trim();
        if override_value.is_empty() {
            return Err(format!("override entry '{trimmed}' has an empty value"));
        }
        output.insert(provider, override_value.to_string());
    }
    Ok(output)
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

fn parse_positive_u32(value: &str, flag: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| format!("{flag} expects a positive integer, got '{value}'"))?;
    if parsed == 0 {
        return Err(format!("{flag} expects a value greater than 0"));
    }
    Ok(parsed)
}

fn parse_scenario_list(value: &str) -> Result<Vec<String>, String> {
    let scenarios: Vec<String> = value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if scenarios.is_empty() {
        return Err("--business-ops-scenarios must include at least one scenario".to_string());
    }
    Ok(scenarios)
}

fn print_help() {
    println!("Provider + embeddings benchmark suite");
    println!();
    println!("Usage:");
    println!("  cargo run --manifest-path src-tauri/Cargo.toml --bin provider_bench -- [options]");
    println!();
    println!("Options:");
    println!("  --workload all|embeddings|llm|business-ops  Workload selection (default: all)");
    println!(
        "  --embedding-providers a,b,c       Embedding providers (default: gemini,ollama,transformersjs,rust-hf)"
    );
    println!(
        "  --llm-providers a,b,c             LLM providers (default: minimax,kimi,zhipu,modal)"
    );
    println!("  --warmup <n>                      Warmup iterations per task (default: 1)");
    println!("  --iterations <n>                  Measured iterations per task (default: 4)");
    println!("  --batch-sizes 1,8,32              Embedding batch sizes (default: 1,8,32)");
    println!("  --llm-max-tokens <n>              Max output tokens for LLM tasks (default: 512)");
    println!("  --normalize-l2                    Enable shared L2 normalization for embeddings");
    println!(
        "  --llm-model-overrides p=m,p2=m2   Override model per provider (ex: minimax=MiniMax-M2.5,zhipu=glm-5)"
    );
    println!(
        "  --llm-base-url-overrides p=url    Override base URL per provider (ex: zhipu=https://api.z.ai/api/coding/paas/v4)"
    );
    println!(
        "  --business-ops-scenarios a,b,c    Scenarios to run: urban_growth,supplier_crisis,premium_focus,quick_test (default: all)"
    );
    println!(
        "  --output <path>                   Output JSON file (default: benchmark_suite_results.json)"
    );
}
