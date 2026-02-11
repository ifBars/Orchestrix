//! Worker execution engine for orchestrating step-level tool calls.
//!
//! This module provides the core worker loop that:
//! - Initializes tool context and available tools
//! - Runs the decision-action-observation loop
//! - Handles tool execution, approvals, and sub-agent delegation
//! - Generates step completion reports
//!
//! # Sub-modules
//!
//! - `model`: Model client abstraction (MiniMax/Kimi)
//! - `tools`: Tool invocation and lifecycle management
//! - `delegation`: Sub-agent spawning and worktree merging
//! - `helpers`: Utility functions for observations and contracts

pub mod delegation;
pub mod helpers;
pub mod model;
pub mod tools;

use std::path::Path;

use crate::bus::{CATEGORY_AGENT, EVENT_AGENT_DECIDING, EVENT_AGENT_TOOL_CALLS_PREPARING};
use crate::core::plan::PlanStep;
use crate::db::{queries, Database};
use crate::policy::PolicyEngine;
use crate::runtime::approval::ApprovalGate;
use crate::runtime::planner::emit_and_record;
use crate::runtime::worktree::WorktreeManager;
use crate::tools::{infer_tool_call, ToolRegistry};
use crate::model::{WorkerAction, WorkerActionRequest};

use delegation::spawn_and_execute_delegated_sub_agent;
use helpers::{open_todos_in_latest_todo_observation, parse_sub_agent_contract};
use model::{RuntimeModelConfig, WorkerModelClient};
use tools::execute_tool_call;

/// Execute a step with tool calling and optional model assistance.
///
/// This is the main worker entry point that runs the decision-action-observation
/// loop until the step completes or reaches max turns.
pub async fn execute_step_with_tools(
    db: &Database,
    bus: &crate::bus::EventBus,
    tool_registry: &ToolRegistry,
    worktree_manager: &WorktreeManager,
    policy: &PolicyEngine,
    approval_gate: &ApprovalGate,
    run_id: &str,
    task_id: &str,
    sub_agent: &queries::SubAgentRow,
    step: &PlanStep,
    workspace_root: &Path,
    worktree_path: &Path,
    model_config: Option<RuntimeModelConfig>,
    goal_summary: String,
    task_prompt: String,
    delegation_depth: u32,
    skills_context: &str,
) -> Result<String, String> {
    // Parse delegation contract
    let contract = parse_sub_agent_contract(sub_agent.context_json.as_deref());

    // Setup orchestrix directory
    let orchestrix_dir = worktree_path.join(".orchestrix");
    std::fs::create_dir_all(&orchestrix_dir).map_err(|e| e.to_string())?;

    let context_path = orchestrix_dir.join("context.json");
    if let Some(content) = &sub_agent.context_json {
        std::fs::write(&context_path, content).map_err(|e| e.to_string())?;
    }

    // Filter available tools by contract
    let mut available_tools: Vec<String> = tool_registry
        .list_for_build_mode()
        .into_iter()
        .map(|v| v.name)
        .collect();

    if !contract.permissions.allowed_tools.is_empty() {
        available_tools.retain(|name| contract.permissions.allowed_tools.contains(name));
    }

    let tool_descriptions = tool_registry.tool_reference_for_build_mode();
    let mut tool_descriptors = tool_registry.list_for_build_mode();
    tool_descriptors.retain(|tool| available_tools.contains(&tool.name));

    // Create model client if config provided
    let worker_model = model_config.as_ref().map(WorkerModelClient::from_config);

    let mut observations: Vec<serde_json::Value> = Vec::new();
    #[allow(unused_assignments)]
    let mut completion_summary: Option<String> = None;
    let mut turn: usize = 0;

    loop {
        turn += 1;

        // Emit deciding event
        let _ = emit_and_record(
            db,
            bus,
            CATEGORY_AGENT,
            EVENT_AGENT_DECIDING,
            Some(run_id.to_string()),
            serde_json::json!({
                "task_id": task_id,
                "run_id": run_id,
                "step_idx": step.idx,
                "sub_agent_id": sub_agent.id,
                "turn": turn,
            }),
        );

        // Get decision from model or fallback
        let decision = if let Some(model) = &worker_model {
            model
                .decide(WorkerActionRequest {
                    task_prompt: task_prompt.clone(),
                    goal_summary: goal_summary.clone(),
                    context: if skills_context.is_empty() {
                        format!("{}\n\n{}", step.title, step.description)
                    } else {
                        format!("{}\n\n{}\n\n{}", step.title, step.description, skills_context)
                    },
                    available_tools: available_tools.clone(),
                    tool_descriptions: tool_descriptions.clone(),
                    tool_descriptors: tool_descriptors.clone(),
                    prior_observations: observations.clone(),
                })
                .await?
        } else {
            let fallback_action = match infer_tool_call(&step.title, step.tool_intent.as_deref()) {
                Some(call) => WorkerAction::ToolCall {
                    tool_name: call.name,
                    tool_args: call.args,
                    rationale: Some("fallback tool inference".to_string()),
                },
                None => WorkerAction::Complete {
                    summary: "No tool intent found in fallback mode".to_string(),
                },
            };
            crate::model::WorkerDecision {
                action: fallback_action,
                reasoning: None,
                raw_response: None,
            }
        };

        // Emit reasoning if present
        if let Some(reasoning) = &decision.reasoning {
            if !reasoning.trim().is_empty() {
                let _ = emit_and_record(
                    db,
                    bus,
                    "agent",
                    "agent.thinking_delta",
                    Some(run_id.to_string()),
                    serde_json::json!({
                        "task_id": task_id,
                        "sub_agent_id": sub_agent.id,
                        "step_idx": step.idx,
                        "content": reasoning.trim(),
                    }),
                );
            }
        }

        // Emit raw response if present
        if let Some(raw) = &decision.raw_response {
            let _ = emit_and_record(
                db,
                bus,
                "agent",
                "agent.raw_response",
                Some(run_id.to_string()),
                serde_json::json!({
                    "task_id": task_id,
                    "sub_agent_id": sub_agent.id,
                    "step_idx": step.idx,
                    "turn": turn,
                    "content": raw,
                }),
            );
        }

        // Process action
        match decision.action {
            WorkerAction::Complete { summary } => {
                // Check for incomplete todos
                if let Some(open_todos) = open_todos_in_latest_todo_observation(&observations) {
                    if open_todos > 0 {
                        observations.push(serde_json::json!({
                            "system": "todo_guard",
                            "status": "incomplete",
                            "open_todos": open_todos,
                            "instruction": "agent.todo still has pending or in_progress items; continue with next tool call",
                        }));
                        continue;
                    }
                }

                let content = if summary.trim().is_empty() {
                    "Step completed.".to_string()
                } else {
                    summary.clone()
                };
                let _ = emit_and_record(
                    db,
                    bus,
                    "agent",
                    "agent.message",
                    Some(run_id.to_string()),
                    serde_json::json!({
                        "task_id": task_id,
                        "sub_agent_id": sub_agent.id,
                        "step_idx": step.idx,
                        "content": content,
                    }),
                );
                completion_summary = Some(summary);
                break;
            }
            WorkerAction::ToolCalls { calls } => {
                // Emit tool calls preparing event
                let tool_names: Vec<String> = calls.iter().map(|c| c.tool_name.clone()).collect();
                let _ = emit_and_record(
                    db,
                    bus,
                    CATEGORY_AGENT,
                    EVENT_AGENT_TOOL_CALLS_PREPARING,
                    Some(run_id.to_string()),
                    serde_json::json!({
                        "task_id": task_id,
                        "run_id": run_id,
                        "tool_names": tool_names,
                        "step_idx": step.idx,
                        "sub_agent_id": sub_agent.id,
                    }),
                );

                // Execute each tool call
                for call in calls {
                    let observation = execute_tool_call(
                        db,
                        bus,
                        tool_registry,
                        policy,
                        approval_gate,
                        run_id,
                        task_id,
                        &sub_agent.id,
                        step.idx as usize,
                        turn,
                        &call.tool_name,
                        &call.tool_args,
                        call.rationale.as_deref(),
                        worktree_path,
                        &available_tools,
                    )
                    .await;
                    observations.push(observation);
                }
            }
            WorkerAction::Delegate { objective } => {
                observations.push(serde_json::json!({
                    "delegate": "rejected",
                    "reason": "implicit delegate action disabled; call tool subagent.spawn instead",
                    "objective": objective,
                }));
            }
            WorkerAction::ToolCall {
                tool_name,
                tool_args,
                rationale,
            } => {
                // Handle subagent.spawn
                if tool_name == "subagent.spawn" {
                    let objective = tool_args
                        .get("objective")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();

                    let result = spawn_and_execute_delegated_sub_agent(
                        db,
                        bus,
                        workspace_root,
                        tool_registry,
                        worktree_manager,
                        approval_gate,
                        run_id,
                        task_id,
                        &sub_agent.id,
                        step.idx,
                        turn,
                        objective,
                        &task_prompt,
                        &goal_summary,
                        skills_context,
                        &available_tools,
                        model_config.as_ref(),
                        contract.permissions.can_spawn_children,
                        contract.permissions.max_delegation_depth,
                        delegation_depth,
                    )
                    .await;

                    if result.success {
                        observations.push(serde_json::json!({
                            "tool_name": "subagent.spawn",
                            "status": "succeeded",
                            "sub_agent_id": result.sub_agent_id,
                            "output_path": result.output_path,
                            "merge": result.merge_message,
                        }));
                    } else {
                        observations.push(serde_json::json!({
                            "tool_name": "subagent.spawn",
                            "status": "failed",
                            "sub_agent_id": result.sub_agent_id,
                            "error": result.error,
                            "merge": result.merge_message,
                        }));
                    }
                    continue;
                }

                // Execute regular tool call
                let observation = execute_tool_call(
                    db,
                    bus,
                    tool_registry,
                    policy,
                    approval_gate,
                    run_id,
                    task_id,
                    &sub_agent.id,
                    step.idx as usize,
                    turn,
                    &tool_name,
                    &tool_args,
                    rationale.as_deref(),
                    worktree_path,
                    &available_tools,
                )
                .await;
                observations.push(observation);
            }
        }
    }

    // Generate step report
    let tool_summary = completion_summary.unwrap_or_else(|| {
        if observations.is_empty() {
            "No tool actions executed".to_string()
        } else {
            format!(
                "Worker stopped without explicit completion. Final observation count: {}",
                observations.len()
            )
        }
    });

    let report_path = orchestrix_dir.join(format!("step-{}-result.md", step.idx));
    let report = format!(
        "# Sub-agent Step Report\n\n## Title\n{}\n\n## Description\n{}\n\n## Tool Intent\n{}\n\n## Result\n{}\n\n## Observations\n{}\n",
        step.title,
        step.description,
        step.tool_intent.clone().unwrap_or_else(|| "none".to_string()),
        tool_summary,
        serde_json::to_string_pretty(&observations).unwrap_or_else(|_| "[]".to_string()),
    );
    std::fs::write(&report_path, report).map_err(|e| e.to_string())?;

    Ok(report_path.to_string_lossy().to_string())
}
