#[cfg(test)]
mod diagram_benchmark {
    use crate::bench::diagram::{run_diagram_benchmark, DiagramBenchOptions};
    use crate::bench::llm::LlmProviderId;

    #[tokio::test]
    async fn run_diagram_benchmark_test() {
        let api_key = crate::tests::load_api_key();
        
        let options = DiagramBenchOptions {
            providers: vec![LlmProviderId::MiniMax],
            provider_configs: vec![crate::bench::llm::LlmProviderConfig {
                provider: LlmProviderId::MiniMax,
                api_key: Some(api_key),
                model: Some("MiniMax-M2.1".to_string()),
                base_url: None,
                max_tokens: Some(4096),
            }],
            max_tokens: 4096,
            timeout_seconds: 180,
            enable_diagram_tools: true,
        };

        println!("Running diagram benchmark WITH diagram tools...");
        let report = run_diagram_benchmark(options).await;

        println!("\n=== DIAGRAM BENCHMARK RESULTS ===\n");
        println!("Tasks: {}", report.tasks.len());
        
        println!("\n--- WITH Diagram Tools ---");
        println!("Provider: {}", report.with_diagram_tools.provider);
        println!("Status: {}", report.with_diagram_tools.status);
        println!("Success Rate: {:.1}%", report.with_diagram_tools.aggregate.success_rate * 100.0);
        println!("Avg Quality: {:.2}", report.with_diagram_tools.aggregate.avg_quality);
        
        println!("\n--- WITHOUT Diagram Tools ---");
        println!("Provider: {}", report.without_diagram_tools.provider);
        println!("Status: {}", report.without_diagram_tools.status);
        println!("Success Rate: {:.1}%", report.without_diagram_tools.aggregate.success_rate * 100.0);
        println!("Avg Quality: {:.2}", report.without_diagram_tools.aggregate.avg_quality);
        
        println!("\n=== COMPARISON ===");
        println!("Winner: {}", report.comparison.winner);
        println!("Quality Improvement: {:.1}%", report.comparison.quality_improvement);
        
        println!("\n=== PER-TASK RESULTS (WITH Tools) ===");
        for task in &report.with_diagram_tools.tasks {
            println!("{}: {} - Quality: {:.2}", task.task_id, task.status, task.diagram_quality.overall_quality);
        }
        
        println!("\n=== PER-TASK RESULTS (WITHOUT Tools) ===");
        for task in &report.without_diagram_tools.tasks {
            println!("{}: {} - Quality: {:.2}", task.task_id, task.status, task.diagram_quality.overall_quality);
        }
    }
}
