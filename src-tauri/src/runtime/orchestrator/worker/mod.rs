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
use uuid::Uuid;

use crate::bus::{
    CATEGORY_AGENT, EVENT_AGENT_DECIDING, EVENT_AGENT_MESSAGE_DELTA,
    EVENT_AGENT_MESSAGE_STREAM_CANCELLED, EVENT_AGENT_MESSAGE_STREAM_COMPLETED,
    EVENT_AGENT_MESSAGE_STREAM_STARTED, EVENT_AGENT_TOOL_CALLS_PREPARING,
};
use crate::core::plan::PlanStep;
use crate::db::{queries, Database};
use crate::model::{WorkerAction, WorkerActionRequest};
use crate::policy::PolicyEngine;
use crate::runtime::approval::ApprovalGate;
use crate::runtime::planner::emit_and_record;
use crate::runtime::worktree::WorktreeManager;
use crate::tools::{infer_tool_call, ToolRegistry};

use crate::tools::dev_server::stop_all_dev_servers_for_run;
use delegation::spawn_and_execute_delegated_sub_agent;
use helpers::{open_todos_in_latest_todo_observation, parse_sub_agent_contract};
use model::{RuntimeModelConfig, StreamDelta, WorkerModelClient};
use tools::execute_tool_call;

const STREAM_DELTA_FLUSH_CHARS: usize = 120;
const THINKING_DELTA_FLUSH_CHARS: usize = 120;

struct MessageStreamEmitter<'a> {
    db: &'a Database,
    bus: &'a crate::bus::EventBus,
    run_id: &'a str,
    task_id: &'a str,
    sub_agent_id: &'a str,
    step_idx: u32,
    turn: usize,
    stream_id: Option<String>,
    pending: String,
    thinking_stream_id: Option<String>,
    thinking_pending: String,
}

impl<'a> MessageStreamEmitter<'a> {
    fn new(
        db: &'a Database,
        bus: &'a crate::bus::EventBus,
        run_id: &'a str,
        task_id: &'a str,
        sub_agent_id: &'a str,
        step_idx: u32,
        turn: usize,
    ) -> Self {
        Self {
            db,
            bus,
            run_id,
            task_id,
            sub_agent_id,
            step_idx,
            turn,
            stream_id: None,
            pending: String::new(),
            thinking_stream_id: None,
            thinking_pending: String::new(),
        }
    }

    fn append_delta(&mut self, delta: &str) -> Result<(), String> {
        if delta.is_empty() {
            return Ok(());
        }
        if self.stream_id.is_none() {
            self.start_stream()?;
        }
        self.pending.push_str(delta);
        if self.pending.len() >= STREAM_DELTA_FLUSH_CHARS || self.pending.contains('\n') {
            self.flush_pending()?;
        }
        Ok(())
    }

    fn append_thinking_delta(&mut self, delta: &str) -> Result<(), String> {
        if delta.is_empty() {
            return Ok(());
        }
        if self.thinking_stream_id.is_none() {
            self.start_thinking_stream()?;
        }
        self.thinking_pending.push_str(delta);
        if self.thinking_pending.len() >= THINKING_DELTA_FLUSH_CHARS
            || self.thinking_pending.contains('\n')
        {
            self.flush_thinking_pending()?;
        }
        Ok(())
    }

    fn complete(mut self) -> Result<(), String> {
        // Complete thinking stream first if active
        if self.thinking_stream_id.is_some() {
            self.flush_thinking_pending()?;
            let thinking_stream_id = self.thinking_stream_id.clone().unwrap_or_default();
            let _ = emit_and_record(
                self.db,
                self.bus,
                "agent",
                "agent.thinking_stream_completed",
                Some(self.run_id.to_string()),
                serde_json::json!({
                    "task_id": self.task_id,
                    "sub_agent_id": self.sub_agent_id,
                    "step_idx": self.step_idx,
                    "turn": self.turn,
                    "stream_id": thinking_stream_id,
                }),
            );
        }

        if self.stream_id.is_none() {
            return Ok(());
        }
        self.flush_pending()?;
        let stream_id = self.stream_id.clone().unwrap_or_default();
        let _ = emit_and_record(
            self.db,
            self.bus,
            "agent",
            EVENT_AGENT_MESSAGE_STREAM_COMPLETED,
            Some(self.run_id.to_string()),
            serde_json::json!({
                "task_id": self.task_id,
                "sub_agent_id": self.sub_agent_id,
                "step_idx": self.step_idx,
                "turn": self.turn,
                "stream_id": stream_id,
            }),
        );
        Ok(())
    }

    fn cancel(mut self, reason: &str) -> Result<(), String> {
        // Cancel thinking stream first if active
        if self.thinking_stream_id.is_some() {
            self.thinking_pending.clear();
            let thinking_stream_id = self.thinking_stream_id.clone().unwrap_or_default();
            let _ = emit_and_record(
                self.db,
                self.bus,
                "agent",
                "agent.thinking_stream_cancelled",
                Some(self.run_id.to_string()),
                serde_json::json!({
                    "task_id": self.task_id,
                    "sub_agent_id": self.sub_agent_id,
                    "step_idx": self.step_idx,
                    "turn": self.turn,
                    "stream_id": thinking_stream_id,
                    "reason": reason,
                }),
            );
        }

        if self.stream_id.is_none() {
            return Ok(());
        }
        self.pending.clear();
        let stream_id = self.stream_id.clone().unwrap_or_default();
        let _ = emit_and_record(
            self.db,
            self.bus,
            "agent",
            EVENT_AGENT_MESSAGE_STREAM_CANCELLED,
            Some(self.run_id.to_string()),
            serde_json::json!({
                "task_id": self.task_id,
                "sub_agent_id": self.sub_agent_id,
                "step_idx": self.step_idx,
                "turn": self.turn,
                "stream_id": stream_id,
                "reason": reason,
            }),
        );
        Ok(())
    }

    fn start_stream(&mut self) -> Result<(), String> {
        if self.stream_id.is_some() {
            return Ok(());
        }
        let stream_id = Uuid::new_v4().to_string();
        self.stream_id = Some(stream_id.clone());
        let _ = emit_and_record(
            self.db,
            self.bus,
            "agent",
            EVENT_AGENT_MESSAGE_STREAM_STARTED,
            Some(self.run_id.to_string()),
            serde_json::json!({
                "task_id": self.task_id,
                "sub_agent_id": self.sub_agent_id,
                "step_idx": self.step_idx,
                "turn": self.turn,
                "stream_id": stream_id,
            }),
        );
        Ok(())
    }

    fn flush_pending(&mut self) -> Result<(), String> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let content = std::mem::take(&mut self.pending);
        let stream_id = self.stream_id.clone().unwrap_or_default();
        let _ = emit_and_record(
            self.db,
            self.bus,
            "agent",
            EVENT_AGENT_MESSAGE_DELTA,
            Some(self.run_id.to_string()),
            serde_json::json!({
                "task_id": self.task_id,
                "sub_agent_id": self.sub_agent_id,
                "step_idx": self.step_idx,
                "turn": self.turn,
                "stream_id": stream_id,
                "content": content,
            }),
        );
        Ok(())
    }

    fn start_thinking_stream(&mut self) -> Result<(), String> {
        if self.thinking_stream_id.is_some() {
            return Ok(());
        }
        let stream_id = Uuid::new_v4().to_string();
        self.thinking_stream_id = Some(stream_id.clone());
        let _ = emit_and_record(
            self.db,
            self.bus,
            "agent",
            "agent.thinking_stream_started",
            Some(self.run_id.to_string()),
            serde_json::json!({
                "task_id": self.task_id,
                "sub_agent_id": self.sub_agent_id,
                "step_idx": self.step_idx,
                "turn": self.turn,
                "stream_id": stream_id,
            }),
        );
        Ok(())
    }

    fn flush_thinking_pending(&mut self) -> Result<(), String> {
        if self.thinking_pending.is_empty() {
            return Ok(());
        }
        let content = std::mem::take(&mut self.thinking_pending);
        let stream_id = self.thinking_stream_id.clone().unwrap_or_default();
        let _ = emit_and_record(
            self.db,
            self.bus,
            "agent",
            "agent.thinking_delta",
            Some(self.run_id.to_string()),
            serde_json::json!({
                "task_id": self.task_id,
                "sub_agent_id": self.sub_agent_id,
                "step_idx": self.step_idx,
                "turn": self.turn,
                "stream_id": stream_id,
                "content": content,
            }),
        );
        Ok(())
    }
}

/// Normalize legacy delegate actions into canonical subagent.spawn tool calls.
///
/// This keeps a single delegation semantic path in the worker while preserving
/// backward compatibility for providers/tests that may still emit `delegate`.
fn normalize_worker_action(action: WorkerAction) -> WorkerAction {
    match action {
        WorkerAction::Delegate { objective } => WorkerAction::ToolCall {
            tool_name: "subagent.spawn".to_string(),
            tool_args: serde_json::json!({
                "objective": objective,
            }),
            rationale: Some("normalized_from_delegate_action".to_string()),
        },
        other => other,
    }
}

fn completion_summary_from_observation(observation: &serde_json::Value) -> Option<String> {
    let tool_name = observation.get("tool_name")?.as_str()?;
    let status = observation.get("status")?.as_str()?;
    if tool_name != "agent.complete" || status != "succeeded" {
        return None;
    }

    observation
        .get("output")
        .and_then(|v| v.get("summary"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

async fn execute_subagent_spawn_observation(
    db: &Database,
    bus: &crate::bus::EventBus,
    workspace_root: &Path,
    tool_registry: &ToolRegistry,
    worktree_manager: &WorktreeManager,
    approval_gate: &ApprovalGate,
    run_id: &str,
    task_id: &str,
    sub_agent_id: &str,
    step_idx: u32,
    turn: usize,
    objective: &str,
    agent_preset_id: Option<&str>,
    task_prompt: &str,
    goal_summary: &str,
    skills_context: &str,
    available_tools: &[String],
    model_config: Option<&RuntimeModelConfig>,
    can_spawn_children: bool,
    max_delegation_depth: u32,
    delegation_depth: u32,
) -> serde_json::Value {
    let result = spawn_and_execute_delegated_sub_agent(
        db,
        bus,
        workspace_root,
        tool_registry,
        worktree_manager,
        approval_gate,
        run_id,
        task_id,
        sub_agent_id,
        step_idx,
        turn,
        objective,
        agent_preset_id,
        task_prompt,
        goal_summary,
        skills_context,
        available_tools,
        model_config,
        can_spawn_children,
        max_delegation_depth,
        delegation_depth,
    )
    .await;

    if result.success {
        serde_json::json!({
            "tool_name": "subagent.spawn",
            "status": "succeeded",
            "objective": objective,
            "sub_agent_id": result.sub_agent_id,
            "output_path": result.output_path,
            "agent_preset_id": result.agent_preset_id,
            "agent_preset_name": result.agent_preset_name,
            "merge": result.merge_message,
        })
    } else {
        serde_json::json!({
            "tool_name": "subagent.spawn",
            "status": "failed",
            "objective": objective,
            "sub_agent_id": result.sub_agent_id,
            "agent_preset_id": result.agent_preset_id,
            "agent_preset_name": result.agent_preset_name,
            "error": result.error,
            "merge": result.merge_message,
        })
    }
}

/// Execute a step with tool calling and optional model assistance.
///
/// This is the main worker entry point that runs the decision-action-observation
/// loop until the step completes or max turns.
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
    include_embeddings: bool,
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
        .list_for_build_mode(include_embeddings)
        .into_iter()
        .map(|v| v.name)
        .collect();

    if !contract.permissions.allowed_tools.is_empty() {
        available_tools.retain(|name| contract.permissions.allowed_tools.contains(name));
    }

    let tool_descriptions = tool_registry.tool_reference_for_build_mode(include_embeddings);
    let mut tool_descriptors = tool_registry.list_for_build_mode(include_embeddings);
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
            let mut stream_emitter =
                MessageStreamEmitter::new(db, bus, run_id, task_id, &sub_agent.id, step.idx, turn);

            let decision = model
                .decide_streaming(
                    WorkerActionRequest {
                        task_prompt: task_prompt.clone(),
                        goal_summary: goal_summary.clone(),
                        context: if skills_context.is_empty() {
                            format!("{}\n\n{}", step.title, step.description)
                        } else {
                            format!(
                                "{}\n\n{}\n\n{}",
                                step.title, step.description, skills_context
                            )
                        },
                        available_tools: available_tools.clone(),
                        tool_descriptions: tool_descriptions.clone(),
                        tool_descriptors: tool_descriptors.clone(),
                        prior_observations: observations.clone(),
                        max_tokens: None, // Worker mode uses default (180k)
                    },
                    |delta| match delta {
                        StreamDelta::Content(text) => stream_emitter.append_delta(&text),
                        StreamDelta::Reasoning(text) => stream_emitter.append_thinking_delta(&text),
                    },
                )
                .await?;

            match &decision.action {
                WorkerAction::Complete { .. } => {
                    stream_emitter.complete()?;
                }
                WorkerAction::ToolCall { .. }
                | WorkerAction::ToolCalls { .. }
                | WorkerAction::Delegate { .. } => {
                    stream_emitter.cancel("model selected non-message action")?;
                }
            }

            decision
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

        // Note: Reasoning is now streamed in real-time via thinking_delta events
        // during the model's response, so we don't emit it again here.

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

        // Process action (with legacy Delegate normalized to subagent.spawn)
        let action = normalize_worker_action(decision.action);

        // Record the assistant's decision/action in observations for history tracking
        // This is crucial for multi-turn agent providers (like MiniMax) to reconstruct state.
        match &action {
            WorkerAction::ToolCalls { calls } => {
                observations.push(serde_json::json!({
                    "role": "assistant",
                    "reasoning": decision.reasoning,
                    "tool_calls": calls
                }));
            }
            WorkerAction::ToolCall {
                tool_name,
                tool_args,
                rationale,
            } => {
                observations.push(serde_json::json!({
                    "role": "assistant",
                    "reasoning": decision.reasoning,
                    "tool_calls": [{
                        "tool_name": tool_name,
                        "tool_args": tool_args,
                        "rationale": rationale
                    }]
                }));
            }
            // Complete/Delegate don't necessarily need to be recorded as "tool calls"
            // in the history the same way, or they terminate the loop.
            _ => {}
        }

        match action {
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

                // If the model requested multiple sub-agent spawns in one turn,
                // execute them concurrently for real parallel delegation.
                let all_subagent_spawns =
                    calls.iter().all(|call| call.tool_name == "subagent.spawn");
                if all_subagent_spawns && calls.len() > 1 {
                    let spawn_observations = futures::future::join_all(calls.iter().map(|call| {
                        let objective = call
                            .tool_args
                            .get("objective")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim();
                        let agent_preset_id = call
                            .tool_args
                            .get("agent_preset_id")
                            .and_then(|v| v.as_str())
                            .map(str::trim)
                            .filter(|v| !v.is_empty());

                        execute_subagent_spawn_observation(
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
                            agent_preset_id,
                            &task_prompt,
                            &goal_summary,
                            skills_context,
                            &available_tools,
                            model_config.as_ref(),
                            contract.permissions.can_spawn_children,
                            contract.permissions.max_delegation_depth,
                            delegation_depth,
                        )
                    }))
                    .await;
                    observations.extend(spawn_observations);
                } else {
                    let mut completion_requested = false;
                    // Execute each tool call sequentially when there are mixed tool
                    // dependencies in the same turn.
                    for call in calls {
                        if call.tool_name == "subagent.spawn" {
                            let objective = call
                                .tool_args
                                .get("objective")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .trim();
                            let agent_preset_id = call
                                .tool_args
                                .get("agent_preset_id")
                                .and_then(|v| v.as_str())
                                .map(str::trim)
                                .filter(|v| !v.is_empty());

                            let observation = execute_subagent_spawn_observation(
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
                                agent_preset_id,
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
                            observations.push(observation);
                            continue;
                        }

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
                        if let Some(summary) = completion_summary_from_observation(&observation) {
                            completion_summary = Some(summary);
                            completion_requested = true;
                        }
                        observations.push(observation);
                        if completion_requested {
                            break;
                        }
                    }

                    if completion_requested {
                        break;
                    }
                }
            }
            WorkerAction::Delegate { .. } => {
                unreachable!("delegate action should be normalized to subagent.spawn")
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
                    let agent_preset_id = tool_args
                        .get("agent_preset_id")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|v| !v.is_empty());

                    let observation = execute_subagent_spawn_observation(
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
                        agent_preset_id,
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
                    observations.push(observation);
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
                if let Some(summary) = completion_summary_from_observation(&observation) {
                    completion_summary = Some(summary);
                    observations.push(observation);
                    break;
                }
                observations.push(observation);
            }
        }
    }

    // Cleanup: Stop all dev servers associated with this run
    // This ensures no orphaned processes when a step completes
    let cleanup_results = stop_all_dev_servers_for_run(run_id).await;
    if !cleanup_results.is_empty() {
        let stopped_count = cleanup_results.len();
        let _ = emit_and_record(
            db,
            bus,
            "agent",
            "agent.dev_servers_cleaned",
            Some(run_id.to_string()),
            serde_json::json!({
                "task_id": task_id,
                "sub_agent_id": sub_agent.id,
                "step_idx": step.idx,
                "servers_stopped": stopped_count,
                "details": cleanup_results.iter().map(|r| {
                    serde_json::json!({
                        "server_id": &r.server_id,
                        "success": r.success,
                        "runtime_secs": r.runtime_secs,
                    })
                }).collect::<Vec<_>>(),
            }),
        );
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
