//! CLI for running agentic coding benchmark.
//!
//! Usage:
//!   cargo run --bin agentic_coding_bench
//!
//! This runs a quick 1-2 minute benchmark focused on agentic terminal usage
//! and coding tasks with real tool calls. Defaults to MiniMax M2.1.

use std::env;

#[tokio::main]
async fn main() {
    // Load environment variables from .env if present
    if let Ok(current_dir) = env::current_dir() {
        let env_path = current_dir.join(".env");
        if env_path.exists() {
            let _ = dotenvy::from_path(&env_path);
        }
    }

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║         Orchestrix Agentic Coding Benchmark                ║");
    println!("║                                                            ║");
    println!("║  Testing real tool usage: fs.*, cmd.exec, search.rg        ║");
    println!("║  Default model: MiniMax M2.1                               ║");
    println!("║  Estimated duration: 1-2 minutes                           ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Check for MiniMax API key
    let has_minimax_key = env::var("MINIMAX_API_KEY").is_ok()
        || env::var("MINIMAX_API_TOKEN").is_ok();

    if !has_minimax_key {
        eprintln!("Error: No MiniMax API key found.");
        eprintln!("Please set one of the following environment variables:");
        eprintln!("  - MINIMAX_API_KEY");
        eprintln!("  - MINIMAX_API_TOKEN");
        std::process::exit(1);
    }

    let start = std::time::Instant::now();

    // Run the benchmark
    let report = orchestrix_lib::bench::agentic_coding::run_quick_agentic_benchmark().await;

    let duration = start.elapsed();

    // Print results
    println!("\n✓ Benchmark completed in {:.1}s\n", duration.as_secs_f64());

    for provider in &report.providers {
        println!("Provider: {} (model: {:?})", provider.provider, provider.model);
        println!("Status: {}", provider.status);
        
        if let Some(error) = &provider.error {
            println!("Error: {}", error);
        }

        println!("\nAggregate Results:");
        println!("  Tasks completed: {}/{}", 
            provider.aggregate.tasks_completed,
            provider.aggregate.tasks_completed + provider.aggregate.tasks_failed);
        println!("  Success rate: {:.1}%", provider.aggregate.success_rate * 100.0);
        println!("  Total tool calls: {}", provider.aggregate.total_tool_calls);
        println!("  Avg task duration: {:.1}s", 
            provider.aggregate.avg_duration_ms / 1000.0);

        println!("\nTask Details:");
        for task in &provider.tasks {
            let status_icon = if task.success { "✓" } else { "✗" };
            println!("  {} {}: {} ({} turns, {} tool calls)",
                status_icon,
                task.task_id,
                task.status,
                task.turns_taken,
                task.tool_calls_made);
            
            if let Some(error) = &task.error {
                println!("    Error: {}", error);
            }
        }
        println!();
    }

    // Exit with error code if no providers succeeded
    let any_success = report.providers.iter()
        .any(|p| p.aggregate.success_rate > 0.0);
    
    if !any_success {
        std::process::exit(1);
    }
}
