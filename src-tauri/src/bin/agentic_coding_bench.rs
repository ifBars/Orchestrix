//! CLI for running the agentic coding benchmark harness.
//!
//! Examples:
//!   cargo run --manifest-path src-tauri/Cargo.toml --bin agentic_coding_bench --
//!   cargo run --manifest-path src-tauri/Cargo.toml --bin agentic_coding_bench -- --provider kimi --api-key-file C:\path\to\kimi-key.txt --model kimi-k2.5
//!   cargo run --manifest-path src-tauri/Cargo.toml --bin agentic_coding_bench -- --providers minimax,kimi --model-overrides minimax=MiniMax-M2.5,kimi=kimi-k2.5 --output kimi-vs-minimax.json

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Instant;

use orchestrix_lib::bench::agentic_coding::{
    available_agentic_coding_tasks, run_agentic_coding_benchmark, AgenticCodingBenchOptions,
    AgenticCodingBenchReport,
};
use orchestrix_lib::bench::llm::{LlmProviderConfig, LlmProviderId};

#[derive(Debug, Clone)]
struct CliOptions {
    providers: Vec<LlmProviderId>,
    model_overrides: HashMap<LlmProviderId, String>,
    base_url_overrides: HashMap<LlmProviderId, String>,
    api_key_file: Option<PathBuf>,
    max_tokens: u32,
    timeout_seconds: u64,
    task_filter: Vec<String>,
    scratch_root: Option<PathBuf>,
    retain_failed_workspaces: bool,
    output_path: Option<PathBuf>,
    list_tasks: bool,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            providers: vec![LlmProviderId::MiniMax],
            model_overrides: HashMap::from([(LlmProviderId::MiniMax, "MiniMax-M2.1".to_string())]),
            base_url_overrides: HashMap::new(),
            api_key_file: None,
            max_tokens: 4096,
            timeout_seconds: 120,
            task_filter: Vec::new(),
            scratch_root: None,
            retain_failed_workspaces: false,
            output_path: None,
            list_tasks: false,
        }
    }
}

#[tokio::main]
async fn main() {
    load_dotenv();

    if let Err(error) = run().await {
        eprintln!("agentic coding benchmark failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let cli = parse_args()?;

    if cli.list_tasks {
        print_task_list();
        return Ok(());
    }

    let bench_options = build_bench_options(&cli)?;

    print_banner(&cli);
    let start = Instant::now();
    let report = run_agentic_coding_benchmark(bench_options).await;
    let duration = start.elapsed();

    if let Some(output_path) = cli.output_path.as_ref() {
        save_report(output_path, &report)?;
        println!("Saved benchmark output to {}", output_path.display());
    }

    print_summary(&report, duration);

    let any_success = report
        .providers
        .iter()
        .any(|provider| provider.aggregate.success_rate > 0.0);
    if !any_success {
        return Err("no benchmarked provider completed any tasks successfully".to_string());
    }

    Ok(())
}

fn load_dotenv() {
    if let Ok(current_dir) = env::current_dir() {
        let env_path = current_dir.join(".env");
        if env_path.exists() {
            let _ = dotenvy::from_path(&env_path);
        }
    }
}

fn parse_args() -> Result<CliOptions, String> {
    let mut cli = CliOptions::default();
    let mut args = std::env::args().skip(1).peekable();

    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            print_help();
            std::process::exit(0);
        }

        if arg == "--list-tasks" {
            cli.list_tasks = true;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--provider=") {
            cli.providers = vec![parse_provider(value)?];
            continue;
        }
        if arg == "--provider" {
            let value = args
                .next()
                .ok_or_else(|| "--provider requires a value".to_string())?;
            cli.providers = vec![parse_provider(&value)?];
            continue;
        }

        if let Some(value) = arg.strip_prefix("--providers=") {
            cli.providers = parse_provider_list(value)?;
            continue;
        }
        if arg == "--providers" {
            let value = args
                .next()
                .ok_or_else(|| "--providers requires a value".to_string())?;
            cli.providers = parse_provider_list(&value)?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--model=") {
            assign_single_provider_override(
                &mut cli.model_overrides,
                &cli.providers,
                value,
                "model",
            )?;
            continue;
        }
        if arg == "--model" {
            let value = args
                .next()
                .ok_or_else(|| "--model requires a value".to_string())?;
            assign_single_provider_override(
                &mut cli.model_overrides,
                &cli.providers,
                &value,
                "model",
            )?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--base-url=") {
            assign_single_provider_override(
                &mut cli.base_url_overrides,
                &cli.providers,
                value,
                "base URL",
            )?;
            continue;
        }
        if arg == "--base-url" {
            let value = args
                .next()
                .ok_or_else(|| "--base-url requires a value".to_string())?;
            assign_single_provider_override(
                &mut cli.base_url_overrides,
                &cli.providers,
                &value,
                "base URL",
            )?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--model-overrides=") {
            cli.model_overrides = parse_provider_overrides(value)?;
            continue;
        }
        if arg == "--model-overrides" {
            let value = args
                .next()
                .ok_or_else(|| "--model-overrides requires a value".to_string())?;
            cli.model_overrides = parse_provider_overrides(&value)?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--base-url-overrides=") {
            cli.base_url_overrides = parse_provider_overrides(value)?;
            continue;
        }
        if arg == "--base-url-overrides" {
            let value = args
                .next()
                .ok_or_else(|| "--base-url-overrides requires a value".to_string())?;
            cli.base_url_overrides = parse_provider_overrides(&value)?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--api-key-file=") {
            cli.api_key_file = Some(PathBuf::from(value));
            continue;
        }
        if arg == "--api-key-file" {
            let value = args
                .next()
                .ok_or_else(|| "--api-key-file requires a value".to_string())?;
            cli.api_key_file = Some(PathBuf::from(value));
            continue;
        }

        if let Some(value) = arg.strip_prefix("--max-tokens=") {
            cli.max_tokens = parse_positive_u32(value, "--max-tokens")?;
            continue;
        }
        if arg == "--max-tokens" {
            let value = args
                .next()
                .ok_or_else(|| "--max-tokens requires a value".to_string())?;
            cli.max_tokens = parse_positive_u32(&value, "--max-tokens")?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--timeout-seconds=") {
            cli.timeout_seconds = parse_positive_u64(value, "--timeout-seconds")?;
            continue;
        }
        if arg == "--timeout-seconds" {
            let value = args
                .next()
                .ok_or_else(|| "--timeout-seconds requires a value".to_string())?;
            cli.timeout_seconds = parse_positive_u64(&value, "--timeout-seconds")?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--tasks=") {
            cli.task_filter = parse_task_filter(value)?;
            continue;
        }
        if arg == "--tasks" || arg == "--task" {
            let value = args
                .next()
                .ok_or_else(|| format!("{arg} requires a value"))?;
            cli.task_filter = parse_task_filter(&value)?;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--task=") {
            cli.task_filter = parse_task_filter(value)?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--scratch-root=") {
            cli.scratch_root = Some(PathBuf::from(value));
            continue;
        }
        if arg == "--scratch-root" {
            let value = args
                .next()
                .ok_or_else(|| "--scratch-root requires a value".to_string())?;
            cli.scratch_root = Some(PathBuf::from(value));
            continue;
        }

        if arg == "--retain-failed-workspaces" {
            cli.retain_failed_workspaces = true;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--output=") {
            cli.output_path = Some(PathBuf::from(value));
            continue;
        }
        if arg == "--output" {
            let value = args
                .next()
                .ok_or_else(|| "--output requires a value".to_string())?;
            cli.output_path = Some(PathBuf::from(value));
            continue;
        }

        return Err(format!("unknown argument: {arg}"));
    }

    if cli.providers.is_empty() {
        return Err("at least one provider is required".to_string());
    }

    Ok(cli)
}

fn build_bench_options(cli: &CliOptions) -> Result<AgenticCodingBenchOptions, String> {
    if cli.api_key_file.is_some() && cli.providers.len() != 1 {
        return Err("--api-key-file only supports a single selected provider".to_string());
    }

    validate_task_filter(&cli.task_filter)?;
    let file_api_key = match cli.api_key_file.as_ref() {
        Some(path) => Some(read_api_key_file(path)?),
        None => None,
    };

    let provider_configs = cli
        .providers
        .iter()
        .map(|provider| LlmProviderConfig {
            provider: *provider,
            api_key: file_api_key.clone(),
            model: cli
                .model_overrides
                .get(provider)
                .cloned()
                .or_else(|| default_model_for_provider(*provider)),
            base_url: cli.base_url_overrides.get(provider).cloned(),
            max_tokens: Some(cli.max_tokens),
        })
        .collect();

    Ok(AgenticCodingBenchOptions {
        providers: cli.providers.clone(),
        provider_configs,
        max_tokens: cli.max_tokens,
        timeout_seconds: cli.timeout_seconds,
        task_filter: cli.task_filter.clone(),
        scratch_root: cli.scratch_root.clone(),
        retain_failed_workspaces: cli.retain_failed_workspaces,
    })
}

fn print_banner(cli: &CliOptions) {
    let provider_names = cli
        .providers
        .iter()
        .map(|provider| provider.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let task_summary = if cli.task_filter.is_empty() {
        "all tasks".to_string()
    } else {
        cli.task_filter.join(", ")
    };

    println!("Orchestrix Agentic Coding Benchmark");
    println!("Providers: {provider_names}");
    println!("Tasks: {task_summary}");
    println!("Max tokens: {}", cli.max_tokens);
    println!("Timeout: {}s per task", cli.timeout_seconds);
    if let Some(scratch_root) = cli.scratch_root.as_ref() {
        println!("Scratch root: {}", scratch_root.display());
    }
    println!("Retain failed workspaces: {}", cli.retain_failed_workspaces);
    println!();
}

fn print_task_list() {
    println!("Available agentic coding tasks:");
    for task in available_agentic_coding_tasks() {
        println!("  {:<24} {}", task.task_id, task.task_label);
        println!("    {}", task.description);
    }
}

fn print_summary(report: &AgenticCodingBenchReport, duration: std::time::Duration) {
    println!("Benchmark completed in {:.1}s", duration.as_secs_f64());
    println!();

    if report.tasks.is_empty() {
        println!("No tasks were selected.");
        return;
    }

    println!("Tasks in run:");
    for task in &report.tasks {
        println!("  {:<24} {}", task.task_id, task.task_label);
    }
    println!();

    for provider in &report.providers {
        println!(
            "Provider: {} (model: {})",
            provider.provider,
            provider.model.as_deref().unwrap_or("-")
        );
        println!("Status: {}", provider.status);

        if let Some(error) = &provider.error {
            println!("Error: {error}");
        }

        println!(
            "Success rate: {:.1}% | Completed: {} | Failed: {} | Tool calls: {} | Avg duration: {:.1}s",
            provider.aggregate.success_rate * 100.0,
            provider.aggregate.tasks_completed,
            provider.aggregate.tasks_failed,
            provider.aggregate.total_tool_calls,
            provider.aggregate.avg_duration_ms / 1000.0
        );

        for task in &provider.tasks {
            let status_icon = if task.success { "OK" } else { "ERR" };
            println!(
                "  [{}] {:<24} {:<18} turns={} tools={} completion={} validated={}",
                status_icon,
                task.task_id,
                task.status,
                task.turns_taken,
                task.tool_calls_made,
                task.completion_signaled,
                task.validation_passed
            );
            if let Some(error) = &task.error {
                println!("       error: {error}");
            }
            for note in &task.validation_notes {
                println!("       note: {note}");
            }
            if let Some(path) = &task.retained_workspace {
                println!("       retained_workspace: {path}");
            }
            println!("       tool_trace_entries: {}", task.tool_trace.len());
        }

        println!();
    }

    if let Some(winner) = report.overall_winner.as_ref() {
        println!("Overall winner: {winner}");
    }
}

fn save_report(path: &Path, report: &AgenticCodingBenchReport) -> Result<(), String> {
    let output_json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize benchmark output: {error}"))?;
    std::fs::write(path, output_json)
        .map_err(|error| format!("failed to write '{}': {error}", path.display()))
}

fn validate_task_filter(task_filter: &[String]) -> Result<(), String> {
    if task_filter.is_empty() {
        return Ok(());
    }

    let available = available_agentic_coding_tasks()
        .into_iter()
        .map(|task| task.task_id)
        .collect::<Vec<_>>();

    let unknown = task_filter
        .iter()
        .filter(|task_id| {
            !available
                .iter()
                .any(|known| known.eq_ignore_ascii_case(task_id))
        })
        .cloned()
        .collect::<Vec<_>>();

    if unknown.is_empty() {
        return Ok(());
    }

    Err(format!(
        "unknown task ids: {}. Use --list-tasks to inspect available tasks.",
        unknown.join(", ")
    ))
}

fn read_api_key_file(path: &Path) -> Result<String, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read API key file '{}': {error}", path.display()))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("API key file '{}' is empty", path.display()));
    }
    Ok(trimmed.to_string())
}

fn assign_single_provider_override(
    target: &mut HashMap<LlmProviderId, String>,
    providers: &[LlmProviderId],
    value: &str,
    label: &str,
) -> Result<(), String> {
    if providers.len() != 1 {
        return Err(format!(
            "--{} can only be used when exactly one provider is selected",
            label.replace(' ', "-")
        ));
    }

    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} cannot be empty"));
    }

    target.insert(providers[0], trimmed.to_string());
    Ok(())
}

fn default_model_for_provider(provider: LlmProviderId) -> Option<String> {
    match provider {
        LlmProviderId::MiniMax => Some("MiniMax-M2.1".to_string()),
        LlmProviderId::Kimi | LlmProviderId::Zhipu | LlmProviderId::Modal => None,
    }
}

fn parse_provider(value: &str) -> Result<LlmProviderId, String> {
    LlmProviderId::from_str(value)
}

fn parse_provider_list(value: &str) -> Result<Vec<LlmProviderId>, String> {
    let mut providers = Vec::new();
    for token in value.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let provider = parse_provider(trimmed)?;
        if !providers.contains(&provider) {
            providers.push(provider);
        }
    }

    if providers.is_empty() {
        return Err("at least one provider is required".to_string());
    }

    Ok(providers)
}

fn parse_provider_overrides(value: &str) -> Result<HashMap<LlmProviderId, String>, String> {
    let mut output = HashMap::new();
    for token in value.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (provider_raw, value_raw) = trimmed
            .split_once('=')
            .ok_or_else(|| format!("override entry '{trimmed}' must be provider=value"))?;
        let provider = parse_provider(provider_raw.trim())?;
        let override_value = value_raw.trim();
        if override_value.is_empty() {
            return Err(format!("override entry '{trimmed}' has an empty value"));
        }

        output.insert(provider, override_value.to_string());
    }
    Ok(output)
}

fn parse_task_filter(value: &str) -> Result<Vec<String>, String> {
    let tasks = value
        .split(',')
        .map(|task| task.trim().to_string())
        .filter(|task| !task.is_empty())
        .collect::<Vec<_>>();

    if tasks.is_empty() {
        return Err("at least one task id is required".to_string());
    }

    Ok(tasks)
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

fn parse_positive_u64(value: &str, flag: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{flag} expects a positive integer, got '{value}'"))?;
    if parsed == 0 {
        return Err(format!("{flag} expects a value greater than 0"));
    }
    Ok(parsed)
}

fn print_help() {
    println!("Orchestrix agentic coding benchmark");
    println!();
    println!("Usage:");
    println!(
        "  cargo run --manifest-path src-tauri/Cargo.toml --bin agentic_coding_bench -- [options]"
    );
    println!();
    println!("Options:");
    println!("  --provider <id>                  Single provider (default: minimax)");
    println!("  --providers a,b,c                Comma-separated providers");
    println!("  --model <name>                   Model override for a single provider");
    println!("  --base-url <url>                 Base URL override for a single provider");
    println!("  --model-overrides p=m,p2=m2      Model overrides for multiple providers");
    println!("  --base-url-overrides p=u,p2=u2   Base URL overrides for multiple providers");
    println!(
        "  --api-key-file <path>            Read API key from a local file for a single provider"
    );
    println!("  --tasks a,b,c                    Only run the specified task ids");
    println!("  --scratch-root <path>            Create temporary benchmark workspaces under this directory");
    println!("  --retain-failed-workspaces       Keep failed benchmark workspaces for inspection");
    println!("  --list-tasks                     Print available task ids and exit");
    println!("  --max-tokens <n>                 Max response tokens per decision (default: 4096)");
    println!("  --timeout-seconds <n>            Per-task timeout in seconds (default: 120)");
    println!("  --output <path>                  Save full JSON report to disk");
    println!("  -h, --help                       Show help");
    println!();
    println!("Examples:");
    println!("  cargo run --manifest-path src-tauri/Cargo.toml --bin agentic_coding_bench -- --provider kimi --api-key-file C:\\path\\to\\kimi-key.txt --model kimi-k2.5");
    println!("  cargo run --manifest-path src-tauri/Cargo.toml --bin agentic_coding_bench -- --providers minimax,kimi --model-overrides minimax=MiniMax-M2.5,kimi=kimi-k2.5 --tasks multistep_refactor,search_and_update --output harness-report.json");
}
