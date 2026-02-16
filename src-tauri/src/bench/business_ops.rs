//! Business operations benchmark for LLM evaluation using Orchestrix harness.
//!
//! Multi-turn profit-driven simulation where models act as COO of a fictional
//! vending company using native tool calling through the AgentModelClient trait.

use std::time::Instant;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};

use crate::bench::core::{compute_latency_stats, BenchmarkRunMetadata, WorkloadKind};
use crate::bench::llm::{
    api_key_env_keys, base_url_env_keys, default_model, first_non_empty_env, model_env_keys,
    normalize_optional_string, LlmProviderConfig, LlmProviderId,
};
use crate::bench::simulation::{load_scenario, Simulator};
use crate::core::tool::ToolDescriptor;
use crate::model::{
    AgentModelClient, GlmClient, KimiClient, MiniMaxClient, ModalClient, WorkerAction,
    WorkerActionRequest, WorkerDecision,
};

// ---------------------------------------------------------------------------
// Scenario data (inline JSON strings)
// ---------------------------------------------------------------------------

const URBAN_GROWTH_SCENARIO: &str = include_str!("scenarios/urban_growth.json");
const SUPPLIER_CRISIS_SCENARIO: &str = include_str!("scenarios/supplier_crisis.json");
const PREMIUM_FOCUS_SCENARIO: &str = include_str!("scenarios/premium_focus.json");

// ---------------------------------------------------------------------------
// Business tool definitions (native tool calling)
// ---------------------------------------------------------------------------

fn business_tools() -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            name: "purchase_supply".to_string(),
            description:
                "Purchase inventory from a supplier. Orders will arrive after supplier lead time."
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "supplier_id": {
                        "type": "string",
                        "description": "ID of the supplier to purchase from"
                    },
                    "sku": {
                        "type": "string",
                        "description": "Product SKU to purchase"
                    },
                    "qty": {
                        "type": "integer",
                        "description": "Quantity to purchase",
                        "minimum": 1
                    }
                },
                "required": ["supplier_id", "sku", "qty"]
            }),
            output_schema: None,
        },
        ToolDescriptor {
            name: "restock_machine".to_string(),
            description: "Restock a vending machine with inventory from warehouse.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "machine_id": {
                        "type": "string",
                        "description": "ID of the machine to restock"
                    },
                    "sku": {
                        "type": "string",
                        "description": "Product SKU to stock"
                    },
                    "qty": {
                        "type": "integer",
                        "description": "Quantity to add to machine",
                        "minimum": 1
                    }
                },
                "required": ["machine_id", "sku", "qty"]
            }),
            output_schema: None,
        },
        ToolDescriptor {
            name: "set_price".to_string(),
            description: "Set the selling price for a product at a specific machine.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "machine_id": {
                        "type": "string",
                        "description": "ID of the machine"
                    },
                    "sku": {
                        "type": "string",
                        "description": "Product SKU"
                    },
                    "unit_price": {
                        "type": "number",
                        "description": "Selling price per unit",
                        "minimum": 0.01
                    }
                },
                "required": ["machine_id", "sku", "unit_price"]
            }),
            output_schema: None,
        },
        ToolDescriptor {
            name: "email_supplier".to_string(),
            description: "Send an email to a supplier to negotiate terms or request information."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "supplier_id": {
                        "type": "string",
                        "description": "ID of the supplier to contact"
                    },
                    "subject": {
                        "type": "string",
                        "description": "Email subject line"
                    },
                    "body": {
                        "type": "string",
                        "description": "Email body content"
                    }
                },
                "required": ["supplier_id", "subject", "body"]
            }),
            output_schema: None,
        },
        ToolDescriptor {
            name: "view_reports".to_string(),
            description: "View business reports including sales, inventory, and financial status."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "report_type": {
                        "type": "string",
                        "enum": ["cashflow", "demand_forecast", "stockouts", "machine_health", "supplier_sla"],
                        "description": "Type of report to view: cashflow (financial), demand_forecast (sales trends), stockouts (empty machines), machine_health (uptime/utilization), supplier_sla (supplier reliability)"
                    }
                },
                "required": ["report_type"]
            }),
            output_schema: None,
        },
        ToolDescriptor {
            name: "end_turn".to_string(),
            description: "End the current turn and advance to the next day.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// Business ops benchmark options
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BusinessOpsBenchOptions {
    pub providers: Vec<LlmProviderId>,
    pub warmup_iterations: usize,
    pub measured_iterations: usize,
    pub max_tokens: u32,
    pub provider_configs: Vec<LlmProviderConfig>,
    pub max_turns: usize,
    /// Filter to run only specific scenarios (if empty, runs all)
    pub scenario_filter: Vec<String>,
    /// Enable verbose diagnostics output
    pub diagnostics: bool,
}

impl Default for BusinessOpsBenchOptions {
    fn default() -> Self {
        Self {
            providers: LlmProviderId::all().to_vec(),
            warmup_iterations: 1,
            measured_iterations: 3,
            max_tokens: 2048,
            provider_configs: Vec::new(),
            max_turns: 40,
            scenario_filter: Vec::new(),
            diagnostics: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Report structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessOpsBenchReport {
    pub metadata: BenchmarkRunMetadata,
    pub scenarios: Vec<ScenarioInfo>,
    pub providers: Vec<BusinessOpsProviderResult>,
    pub overall_winner: Option<BusinessOpsWinner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioInfo {
    pub scenario_id: String,
    pub seed: u64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessOpsProviderResult {
    pub provider: String,
    pub model: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub scenarios: Vec<ScenarioRunResult>,
    pub aggregate: BusinessOpsAggregateResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioRunResult {
    pub scenario_id: String,
    pub seed: u64,
    pub final_score: f64,
    pub raw_profit: f64,
    pub service_level: f64,
    pub solvency_score: f64,
    pub compliance_score: f64,
    pub stockout_rate: f64,
    pub turns_completed: usize,
    pub bankrupt_turn: Option<usize>,
    pub total_emails_sent: usize,
    pub tool_call_count: usize,
    pub avg_p50_latency_ms: f64,
    pub success_rate: f64,
    pub error: Option<String>,
    pub sample_response: Option<String>,
    pub parsing_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessOpsAggregateResult {
    pub avg_score: f64,
    pub avg_profit: f64,
    pub avg_service_level: f64,
    pub avg_solvency: f64,
    pub avg_compliance: f64,
    pub avg_stockout_rate: f64,
    pub success_rate: f64,
    pub bankruptcy_rate: f64,
    pub avg_tool_calls: f64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessOpsWinner {
    pub provider: String,
    pub model: Option<String>,
    pub avg_score: f64,
    pub avg_profit: f64,
}

// ---------------------------------------------------------------------------
// Resolved provider config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ResolvedProviderConfig {
    provider: LlmProviderId,
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
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
        LlmProviderId::MiniMax => "missing MiniMax credentials: set MINIMAX_API_KEY".to_string(),
        LlmProviderId::Kimi => "missing Kimi credentials: set KIMI_API_KEY".to_string(),
        LlmProviderId::Zhipu => "missing GLM credentials: set ZHIPU_API_KEY".to_string(),
        LlmProviderId::Modal => "missing Modal credentials: set MODAL_API_KEY".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Benchmark runner
// ---------------------------------------------------------------------------

pub async fn run_business_ops_benchmark(
    options: BusinessOpsBenchOptions,
    multi_progress: Option<&MultiProgress>,
) -> BusinessOpsBenchReport {
    let all_scenario_jsons = vec![
        ("urban_growth", URBAN_GROWTH_SCENARIO),
        ("supplier_crisis", SUPPLIER_CRISIS_SCENARIO),
        ("premium_focus", PREMIUM_FOCUS_SCENARIO),
    ];

    // Filter scenarios if specified
    let scenario_jsons: Vec<&'static str> = if options.scenario_filter.is_empty() {
        all_scenario_jsons.iter().map(|(_, json)| *json).collect()
    } else {
        all_scenario_jsons
            .iter()
            .filter(|(name, _)| options.scenario_filter.iter().any(|f| f == *name))
            .map(|(_, json)| *json)
            .collect()
    };

    if options.diagnostics {
        eprintln!(
            "[DIAGNOSTICS] Running scenarios: {:?}",
            options.scenario_filter
        );
        eprintln!("[DIAGNOSTICS] Max turns: {}", options.max_turns);
        eprintln!("[DIAGNOSTICS] Providers: {:?}", options.providers);
    }

    let mut scenarios = Vec::new();
    for json in &scenario_jsons {
        if let Ok(scenario) = load_scenario(json) {
            scenarios.push(ScenarioInfo {
                scenario_id: scenario.scenario_id.clone(),
                seed: scenario.seed,
                description: scenario.description.clone(),
            });
        }
    }

    let metadata = BenchmarkRunMetadata::new(
        WorkloadKind::LlmBusinessOps,
        options.warmup_iterations,
        options.measured_iterations,
        vec![1],
    );

    let total_work = options.providers.len() * scenarios.len() * options.measured_iterations;
    let overall_pb = multi_progress.map(|mp| {
        let pb = mp.add(ProgressBar::new(total_work as u64));
        pb.set_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
            )
            .unwrap()
            .progress_chars("#>-"),
        );
        pb.set_message("business ops benchmark");
        pb
    });

    let mut provider_results = Vec::new();

    for provider_id in &options.providers {
        let result = run_provider_business_ops(
            *provider_id,
            &options,
            &scenario_jsons,
            multi_progress,
            overall_pb.as_ref(),
        )
        .await;
        provider_results.push(result);
    }

    if let Some(pb) = overall_pb {
        pb.finish_with_message("complete");
    }

    let overall_winner = determine_overall_winner(&provider_results);

    BusinessOpsBenchReport {
        metadata,
        scenarios,
        providers: provider_results,
        overall_winner,
    }
}

async fn run_provider_business_ops(
    provider_id: LlmProviderId,
    options: &BusinessOpsBenchOptions,
    scenario_jsons: &[&'static str],
    multi_progress: Option<&MultiProgress>,
    overall_pb: Option<&ProgressBar>,
) -> BusinessOpsProviderResult {
    let config = match resolve_provider_config(provider_id, &options.provider_configs) {
        Ok(cfg) => cfg,
        Err(e) => {
            return BusinessOpsProviderResult {
                provider: provider_id.as_str().to_string(),
                model: None,
                status: "error".to_string(),
                error: Some(e),
                scenarios: Vec::new(),
                aggregate: BusinessOpsAggregateResult {
                    avg_score: 0.0,
                    avg_profit: 0.0,
                    avg_service_level: 0.0,
                    avg_solvency: 0.0,
                    avg_compliance: 0.0,
                    avg_stockout_rate: 0.0,
                    success_rate: 0.0,
                    bankruptcy_rate: 0.0,
                    avg_tool_calls: 0.0,
                    avg_latency_ms: 0.0,
                },
            };
        }
    };

    let client = create_business_ops_client(&config);
    let model = Some(client.model_id());

    let provider_pb = multi_progress.map(|mp| {
        let pb = mp.add(ProgressBar::new(
            (scenario_jsons.len() * options.measured_iterations) as u64,
        ));
        pb.set_style(
            ProgressStyle::with_template(
                "{prefix:.bold.dim} [{bar:30.cyan/blue}] {pos}/{len} {msg}",
            )
            .unwrap()
            .progress_chars("#>-"),
        );
        pb.set_prefix(format!(
            "{}/{}",
            provider_id.as_str(),
            model.as_deref().unwrap_or("-")
        ));
        pb.set_message("initializing");
        pb
    });

    let mut scenario_results = Vec::new();

    for (scenario_idx, scenario_json) in scenario_jsons.iter().enumerate() {
        for iteration in 0..options.measured_iterations {
            if let Some(pb) = provider_pb.as_ref() {
                pb.set_message(format!("s{}/i{}", scenario_idx + 1, iteration + 1));
            }

            let run_result = run_single_scenario(
                &client,
                scenario_json,
                options.max_tokens,
                options.max_turns,
                provider_pb.as_ref(),
                options.diagnostics,
            )
            .await;

            scenario_results.push(run_result);

            if let Some(pb) = provider_pb.as_ref() {
                pb.inc(1);
            }
            if let Some(pb) = overall_pb {
                pb.inc(1);
            }
        }
    }

    if let Some(pb) = provider_pb {
        let success_rate = scenario_results
            .iter()
            .filter(|r| r.error.is_none())
            .count() as f64
            / scenario_results.len().max(1) as f64;
        pb.finish_with_message(format!("{:.0}%", success_rate * 100.0));
    }

    let aggregate = compute_scenario_aggregates(&scenario_results);
    let successful_scenarios = scenario_results
        .iter()
        .filter(|r| r.error.is_none())
        .count();
    let status = if successful_scenarios > 0 {
        "ok"
    } else {
        "error"
    };

    BusinessOpsProviderResult {
        provider: provider_id.as_str().to_string(),
        model,
        status: status.to_string(),
        error: if status == "ok" {
            None
        } else {
            scenario_results.iter().find_map(|r| r.error.clone())
        },
        scenarios: scenario_results,
        aggregate,
    }
}

async fn run_single_scenario(
    client: &BusinessOpsClient,
    scenario_json: &str,
    max_tokens: u32,
    max_turns: usize,
    progress_bar: Option<&ProgressBar>,
    diagnostics: bool,
) -> ScenarioRunResult {
    let scenario = match load_scenario(scenario_json) {
        Ok(s) => s,
        Err(e) => {
            return ScenarioRunResult {
                scenario_id: "unknown".to_string(),
                seed: 0,
                final_score: 0.0,
                raw_profit: 0.0,
                service_level: 0.0,
                solvency_score: 0.0,
                compliance_score: 0.0,
                stockout_rate: 0.0,
                turns_completed: 0,
                bankrupt_turn: None,
                total_emails_sent: 0,
                tool_call_count: 0,
                avg_p50_latency_ms: 0.0,
                success_rate: 0.0,
                error: Some(format!("failed to load scenario: {}", e)),
                sample_response: None,
                parsing_errors: Vec::new(),
            };
        }
    };

    let scenario_id = scenario.scenario_id.clone();
    let seed = scenario.seed;
    let mut simulator = Simulator::new(scenario);
    let mut latencies: Vec<std::time::Duration> = Vec::new();
    let mut tool_call_count = 0usize;
    let mut bankrupt_turn: Option<usize> = None;
    let mut sample_response: Option<String> = None;
    let mut parsing_errors: Vec<String> = Vec::new();

    let tools = business_tools();
    let available_tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
    // Increased to 3 to give model room for: 1) initial analysis, 2) correction with tool calls, 3) end_turn
    // With reasoning feedback loop, model should self-correct instead of forcing end_turn
    let max_actions_per_turn = 3;

    if diagnostics {
        eprintln!(
            "[DIAGNOSTICS] Starting scenario: {} (seed: {})",
            scenario_id, seed
        );
        eprintln!("[DIAGNOSTICS] Available tools: {:?}", available_tool_names);
        eprintln!("[DIAGNOSTICS] Max turns: {}", max_turns);
    }

    while !simulator.is_complete() {
        // Update progress bar with current turn
        if let Some(pb) = progress_bar {
            pb.set_message(format!("turn {}/{}", simulator.turn + 1, max_turns));
        }

        // MICRO-LOOP: Within each turn, allow multiple decision-action-observation cycles
        let mut turn_complete = false;
        let mut action_count = 0;
        let mut turn_observations: Vec<serde_json::Value> = Vec::new();
        let mut turn_tool_calls: Vec<String> = Vec::new();

        while !turn_complete && action_count < max_actions_per_turn {
            let state_json = simulator.current_state_json();

            // Build context with observations from this turn
            let observations_text = if turn_observations.is_empty() {
                String::new()
            } else {
                format!(
                    "\n\n**Results from your previous actions this turn:**\n{}",
                    turn_observations
                        .iter()
                        .map(|o| o.to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            };

            let enhanced_context = format!(
                "Current business state:\n{}{}\n\n## CRITICAL INSTRUCTIONS:\n\
                You are the COO actively running this business.\n\n\
                **YOU MUST USE TOOLS TO TAKE ACTIONS**\n\
                - Do NOT just analyze - call the actual tools\n\
                - Only tool calls affect the simulation\n\
                - When done with actions for this turn, call: end_turn\n\n\
                **EFFICIENCY RULES**\n\
                - You have LIMITED actions per turn (3 max)\n\
                - DO NOT call view_reports - all info is in the state above\n\
                - Take IMMEDIATE action: restock, purchase, or adjust prices\n\
                - Analysis without tool calls is USELESS - ACT instead\n\n\
                **IMPORTANT: THINKING IS NOT ENOUGH**\n\
                - If you think 'I should restock' → you MUST call restock_machine tool\n\
                - If you think 'I need to buy inventory' → you MUST call purchase_supply tool\n\
                - Reasoning/planning alone does NOTHING - only tool calls work\n\n\
                **Use these tools:**\n\
                - restock_machine: Fill machines from warehouse (if warehouse has stock)\n\
                - purchase_supply: Order from suppliers (takes 5 days to arrive)\n\
                - set_price: Adjust selling prices\n\
                - end_turn: Finish this turn",
                state_json, observations_text
            );

            let action_request = WorkerActionRequest {
                task_prompt: "You are the COO of a vending machine business. Use tools to take actions and maximize profit.".to_string(),
                goal_summary: "Maximize cumulative operating profit".to_string(),
                context: enhanced_context,
                available_tools: available_tool_names.clone(),
                tool_descriptions: format_tool_descriptions(&tools),
                tool_descriptors: tools.clone(),
                prior_observations: turn_observations.clone(),
                max_tokens: Some(max_tokens),
            };

            let started = Instant::now();
            let decision = match client.decide_action(action_request).await {
                Ok(d) => d,
                Err(e) => {
                    parsing_errors.push(format!("Turn {}: {}", simulator.turn, e));
                    let final_score = simulator.compute_final_score();
                    return ScenarioRunResult {
                        scenario_id,
                        seed,
                        final_score: final_score.weighted_score,
                        raw_profit: final_score.raw_profit,
                        service_level: final_score.service_level,
                        solvency_score: final_score.solvency_score,
                        compliance_score: final_score.compliance_score,
                        stockout_rate: final_score.stockout_rate,
                        turns_completed: simulator.turn,
                        bankrupt_turn,
                        total_emails_sent: final_score.total_emails_sent,
                        tool_call_count,
                        avg_p50_latency_ms: 0.0,
                        success_rate: 0.0,
                        error: Some(format!("model error: {}", e)),
                        sample_response: sample_response.clone(),
                        parsing_errors: parsing_errors.clone(),
                    };
                }
            };
            latencies.push(started.elapsed());
            action_count += 1;

            // Execute tool calls and collect observations
            match decision.action {
                WorkerAction::ToolCalls { calls } => {
                    if diagnostics && !calls.is_empty() {
                        eprintln!(
                            "[DIAGNOSTICS] Turn {} (action {}): {} tool calls",
                            simulator.turn,
                            action_count,
                            calls.len()
                        );
                    }

                    for call in calls {
                        let call_desc = format!("{}({})", call.tool_name, call.tool_args);
                        if diagnostics {
                            eprintln!("[DIAGNOSTICS]   -> {}", call_desc);
                        }
                        turn_tool_calls.push(call_desc.clone());

                        if call.tool_name == "end_turn" {
                            turn_complete = true;
                            let result = simulator.tool_call(&call.tool_name, &call.tool_args);
                            turn_observations.push(serde_json::json!({
                                "tool": call.tool_name,
                                "result": result.message
                            }));
                            tool_call_count += 1;
                        } else {
                            let result = simulator.tool_call(&call.tool_name, &call.tool_args);
                            turn_observations.push(serde_json::json!({
                                "tool": call.tool_name,
                                "args": call.tool_args,
                                "success": result.success,
                                "result": result.message
                            }));
                            tool_call_count += 1;
                        }
                    }
                }
                WorkerAction::ToolCall {
                    tool_name,
                    tool_args,
                    ..
                } => {
                    let call_desc = format!("{}({})", tool_name, tool_args);
                    if diagnostics {
                        eprintln!(
                            "[DIAGNOSTICS] Turn {} (action {}): 1 tool call -> {}",
                            simulator.turn, action_count, call_desc
                        );
                    }
                    turn_tool_calls.push(call_desc.clone());

                    if tool_name == "end_turn" {
                        turn_complete = true;
                    }

                    let result = simulator.tool_call(&tool_name, &tool_args);
                    turn_observations.push(serde_json::json!({
                        "tool": tool_name,
                        "args": tool_args,
                        "success": result.success,
                        "result": result.message
                    }));
                    tool_call_count += 1;
                }
                other => {
                    if diagnostics {
                        eprintln!(
                            "[DIAGNOSTICS] Turn {} (action {}): ❌ NO TOOL CALLS - {:?}",
                            simulator.turn, action_count, other
                        );
                    }

                    // Check if model provided reasoning about intended actions
                    let reasoning_hint = decision.reasoning.as_ref().map(|r| {
                        // Extract first sentence or first 200 chars of reasoning
                        let preview = r.chars().take(200).collect::<String>();
                        preview
                    });

                    if let Some(ref reasoning) = reasoning_hint {
                        if diagnostics {
                            eprintln!("[DIAGNOSTICS]   Reasoning: {}", reasoning);
                        }
                        // Add observation that model was thinking but not acting
                        turn_observations.push(serde_json::json!({
                            "system": "You thought about actions but didn't call tools. REASONING DOESN'T EXECUTE - only tool calls work!",
                            "your_thoughts": reasoning,
                            "instruction": "In your next action, actually CALL the tools (restock_machine, purchase_supply, etc.) instead of just thinking about them."
                        }));
                    }

                    // If model doesn't make tool calls, force end_turn after a few tries
                    if action_count >= 2 {
                        if diagnostics {
                            eprintln!(
                                "[DIAGNOSTICS]   Forcing end_turn after {} no-action attempts",
                                action_count
                            );
                        }
                        turn_complete = true;
                        let _result = simulator.tool_call("end_turn", &serde_json::json!({}));
                    }
                }
            }
        }

        // Ensure turn always advances even if model got stuck
        if !turn_complete {
            let _result = simulator.tool_call("end_turn", &serde_json::json!({}));
        }

        // Store sample of actual tool calls for debugging
        if sample_response.is_none() && !turn_tool_calls.is_empty() {
            sample_response = Some(turn_tool_calls.join(", "));
        }

        // Check for bankruptcy
        if bankrupt_turn.is_none()
            && simulator.state.cash < simulator.scenario.constraints.min_cash_floor
        {
            bankrupt_turn = Some(simulator.turn);
        }

        // Safety limit
        if simulator.turn >= max_turns {
            break;
        }
    }

    let final_score = simulator.compute_final_score();
    let stats = compute_latency_stats(&latencies.iter().map(|d| *d).collect::<Vec<_>>());

    if diagnostics {
        eprintln!("\n[DIAGNOSTICS] === SCENARIO COMPLETE: {} ===", scenario_id);
        eprintln!("[DIAGNOSTICS]   Total Turns: {}", simulator.turn);
        eprintln!("[DIAGNOSTICS]   Total Tool Calls: {}", tool_call_count);
        eprintln!(
            "[DIAGNOSTICS]   Final Profit: ${:.2}",
            final_score.raw_profit
        );
        eprintln!(
            "[DIAGNOSTICS]   Service Level: {:.1}%",
            final_score.service_level * 100.0
        );
        eprintln!("[DIAGNOSTICS]   Score: {:.3}", final_score.weighted_score);
        eprintln!(
            "[DIAGNOSTICS]   Bankrupt: {}",
            bankrupt_turn
                .map(|t| format!("Yes (turn {})", t))
                .unwrap_or_else(|| "No".to_string())
        );
        if tool_call_count > 0 {
            eprintln!(
                "[DIAGNOSTICS]   Sample Actions: {}",
                sample_response.as_deref().unwrap_or("N/A")
            );
        } else {
            eprintln!("[DIAGNOSTICS]   ⚠️  WARNING: Model made NO tool calls - it only analyzed!");
        }
        if !parsing_errors.is_empty() {
            eprintln!("[DIAGNOSTICS]   Errors: {:?}", parsing_errors);
        }
        eprintln!("[DIAGNOSTICS] ====================================\n");
    }

    ScenarioRunResult {
        scenario_id,
        seed,
        final_score: final_score.weighted_score,
        raw_profit: final_score.raw_profit,
        service_level: final_score.service_level,
        solvency_score: final_score.solvency_score,
        compliance_score: final_score.compliance_score,
        stockout_rate: final_score.stockout_rate,
        turns_completed: simulator.turn,
        bankrupt_turn,
        total_emails_sent: final_score.total_emails_sent,
        tool_call_count,
        avg_p50_latency_ms: stats.p50_ms,
        success_rate: 1.0,
        error: None,
        sample_response,
        parsing_errors,
    }
}

fn format_tool_descriptions(tools: &[ToolDescriptor]) -> String {
    let mut descriptions = String::new();
    for tool in tools {
        descriptions.push_str(&format!("\n## {}\n{}", tool.name, tool.description));
        if let Some(schema) = tool.input_schema.get("properties") {
            if let Some(props) = schema.as_object() {
                descriptions.push_str("\nParameters:");
                for (name, prop) in props {
                    let desc = prop
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("");
                    let required = tool
                        .input_schema
                        .get("required")
                        .and_then(|r| r.as_array())
                        .map(|arr| arr.iter().any(|v| v.as_str() == Some(name)))
                        .unwrap_or(false);
                    let req_str = if required { "(required)" } else { "(optional)" };
                    descriptions.push_str(&format!("\n  - {}: {} {}", name, desc, req_str));
                }
            }
        }
        descriptions.push('\n');
    }
    descriptions
}

fn compute_scenario_aggregates(results: &[ScenarioRunResult]) -> BusinessOpsAggregateResult {
    if results.is_empty() {
        return BusinessOpsAggregateResult {
            avg_score: 0.0,
            avg_profit: 0.0,
            avg_service_level: 0.0,
            avg_solvency: 0.0,
            avg_compliance: 0.0,
            avg_stockout_rate: 0.0,
            success_rate: 0.0,
            bankruptcy_rate: 0.0,
            avg_tool_calls: 0.0,
            avg_latency_ms: 0.0,
        };
    }

    let count = results.len() as f64;
    let successful = results.iter().filter(|r| r.error.is_none()).count() as f64;
    let bankruptcies = results.iter().filter(|r| r.bankrupt_turn.is_some()).count() as f64;

    BusinessOpsAggregateResult {
        avg_score: results.iter().map(|r| r.final_score).sum::<f64>() / count,
        avg_profit: results.iter().map(|r| r.raw_profit).sum::<f64>() / count,
        avg_service_level: results.iter().map(|r| r.service_level).sum::<f64>() / count,
        avg_solvency: results.iter().map(|r| r.solvency_score).sum::<f64>() / count,
        avg_compliance: results.iter().map(|r| r.compliance_score).sum::<f64>() / count,
        avg_stockout_rate: results.iter().map(|r| r.stockout_rate).sum::<f64>() / count,
        success_rate: successful / count,
        bankruptcy_rate: bankruptcies / count,
        avg_tool_calls: results
            .iter()
            .map(|r| r.tool_call_count as f64)
            .sum::<f64>()
            / count,
        avg_latency_ms: results.iter().map(|r| r.avg_p50_latency_ms).sum::<f64>() / count,
    }
}

fn determine_overall_winner(results: &[BusinessOpsProviderResult]) -> Option<BusinessOpsWinner> {
    results
        .iter()
        .filter(|r| r.status == "ok")
        .max_by(|a, b| {
            a.aggregate
                .avg_score
                .partial_cmp(&b.aggregate.avg_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|r| BusinessOpsWinner {
            provider: r.provider.clone(),
            model: r.model.clone(),
            avg_score: r.aggregate.avg_score,
            avg_profit: r.aggregate.avg_profit,
        })
}

// ---------------------------------------------------------------------------
// Business ops client using AgentModelClient trait
// ---------------------------------------------------------------------------

enum BusinessOpsClient {
    MiniMax(MiniMaxClient),
    Kimi(KimiClient),
    Zhipu(GlmClient),
    Modal(ModalClient),
}

impl BusinessOpsClient {
    fn model_id(&self) -> String {
        match self {
            Self::MiniMax(c) => c.model_id(),
            Self::Kimi(c) => c.model_id(),
            Self::Zhipu(c) => c.model_id(),
            Self::Modal(c) => c.model_id(),
        }
    }
}

impl AgentModelClient for BusinessOpsClient {
    fn model_id(&self) -> String {
        match self {
            Self::MiniMax(c) => c.model_id(),
            Self::Kimi(c) => c.model_id(),
            Self::Zhipu(c) => c.model_id(),
            Self::Modal(c) => c.model_id(),
        }
    }

    async fn decide_action(
        &self,
        req: WorkerActionRequest,
    ) -> Result<WorkerDecision, crate::model::ModelError> {
        match self {
            Self::MiniMax(c) => c.decide_action(req).await,
            Self::Kimi(c) => c.decide_action(req).await,
            Self::Zhipu(c) => c.decide_action(req).await,
            Self::Modal(c) => c.decide_action(req).await,
        }
    }
}

fn create_business_ops_client(config: &ResolvedProviderConfig) -> BusinessOpsClient {
    match config.provider {
        LlmProviderId::MiniMax => BusinessOpsClient::MiniMax(MiniMaxClient::new_with_base_url(
            config.api_key.clone(),
            config.model.clone(),
            config.base_url.clone(),
        )),
        LlmProviderId::Kimi => BusinessOpsClient::Kimi(KimiClient::new(
            config.api_key.clone(),
            config.model.clone(),
            config.base_url.clone(),
        )),
        LlmProviderId::Zhipu => BusinessOpsClient::Zhipu(GlmClient::new(
            config.api_key.clone(),
            config.model.clone(),
            config.base_url.clone(),
        )),
        LlmProviderId::Modal => BusinessOpsClient::Modal(ModalClient::new(
            config.api_key.clone(),
            config.model.clone(),
            config.base_url.clone(),
        )),
    }
}
