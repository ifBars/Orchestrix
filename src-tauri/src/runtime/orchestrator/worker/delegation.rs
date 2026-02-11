//! Sub-agent delegation logic for the worker.
//!
//! Handles spawning child sub-agents, executing them, merging worktrees,
//! and managing the delegation lifecycle.

use std::path::Path;

use chrono::Utc;
use uuid::Uuid;

use crate::core::plan::{PlanStep, StepStatus};
use crate::db::{queries, Database};
use crate::runtime::planner::emit_and_record;
use crate::runtime::worktree::WorktreeManager;
use crate::tools::ToolRegistry;

/// Result of a sub-agent execution.
pub struct SubAgentExecutionResult {
    pub success: bool,
    pub sub_agent_id: String,
    pub output_path: Option<String>,
    pub error: Option<String>,
    pub merge_message: Option<String>,
}

/// Spawn and execute a delegated sub-agent.
///
/// Creates a child sub-agent with restricted permissions, executes it,
/// merges the worktree back to the parent, and returns the result.
pub async fn spawn_and_execute_delegated_sub_agent(
    db: &Database,
    bus: &crate::bus::EventBus,
    workspace_root: &Path,
    tool_registry: &ToolRegistry,
    worktree_manager: &WorktreeManager,
    approval_gate: &crate::runtime::approval::ApprovalGate,
    run_id: &str,
    task_id: &str,
    _sub_agent_id: &str,
    step_idx: u32,
    turn: usize,
    objective: &str,
    task_prompt: &str,
    goal_summary: &str,
    skills_context: &str,
    available_tools: &[String],
    model_config: Option<&super::model::RuntimeModelConfig>,
    can_spawn_children: bool,
    max_delegation_depth: u32,
    current_delegation_depth: u32,
) -> SubAgentExecutionResult {
    // Validate objective
    if objective.is_empty() {
        return SubAgentExecutionResult {
            success: false,
            sub_agent_id: String::new(),
            output_path: None,
            error: Some("objective is required".to_string()),
            merge_message: None,
        };
    }

    // Check delegation permissions
    if !can_spawn_children {
        return SubAgentExecutionResult {
            success: false,
            sub_agent_id: String::new(),
            output_path: None,
            error: Some("delegation disabled by contract".to_string()),
            merge_message: None,
        };
    }

    if current_delegation_depth >= max_delegation_depth {
        return SubAgentExecutionResult {
            success: false,
            sub_agent_id: String::new(),
            output_path: None,
            error: Some("max delegation depth reached".to_string()),
            merge_message: None,
        };
    }

    // Create restricted tool list for child (no subagent.spawn)
    let delegated_allowed_tools: Vec<String> = available_tools
        .iter()
        .filter(|name| name.as_str() != "subagent.spawn")
        .cloned()
        .collect();

    const SUB_AGENT_ATTEMPT_TIMEOUT_SECS: u64 = 300;

    // Create child sub-agent record
    let child = queries::SubAgentRow {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.to_string(),
        step_idx: step_idx as i64,
        name: format!("delegate-{}", turn),
        status: "created".to_string(),
        worktree_path: None,
        context_json: Some(
            serde_json::json!({
                "task_prompt": task_prompt,
                "goal_summary": goal_summary,
                "step": {
                    "title": format!("Delegated objective {}", turn),
                    "description": objective,
                },
                "contract": {
                    "permissions": {
                        "allowed_tools": delegated_allowed_tools,
                        "can_spawn_children": false,
                        "max_delegation_depth": 0,
                    },
                    "execution": {
                        "attempt_timeout_ms": SUB_AGENT_ATTEMPT_TIMEOUT_SECS * 1000,
                        "close_on_completion": true,
                    }
                }
            })
            .to_string(),
        ),
        started_at: None,
        finished_at: None,
        error: None,
    };

    if let Err(error) = queries::insert_sub_agent(db, &child) {
        return SubAgentExecutionResult {
            success: false,
            sub_agent_id: child.id,
            output_path: None,
            error: Some(format!("failed to insert sub-agent: {}", error)),
            merge_message: None,
        };
    }

    let _ = emit_and_record(
        db,
        bus,
        "agent",
        "agent.subagent_created",
        Some(run_id.to_string()),
        serde_json::json!({
            "task_id": task_id,
            "sub_agent_id": child.id,
            "step_idx": step_idx,
            "name": child.name,
            "objective": objective,
        }),
    );

    // Create delegated step
    let delegated_step = PlanStep {
        idx: step_idx,
        title: format!("Delegated objective {}", turn),
        description: objective.to_string(),
        tool_intent: None,
        status: StepStatus::Pending,
        max_retries: 0,
        result: None,
    };

    // Convert model config to orchestrator format
    let orchestrator_model_config = model_config.map(|c| super::super::RuntimeModelConfig {
        provider: c.provider.clone(),
        api_key: c.api_key.clone(),
        model: c.model.clone(),
        base_url: c.base_url.clone(),
    });

    // Execute the child sub-agent
    let mut child_result = Box::pin(super::super::sub_agent::execute_sub_agent(
        db,
        bus,
        workspace_root,
        tool_registry,
        worktree_manager,
        approval_gate,
        run_id.to_string(),
        task_id.to_string(),
        child.clone(),
        delegated_step,
        orchestrator_model_config,
        goal_summary.to_string(),
        task_prompt.to_string(),
        skills_context.to_string(),
    ))
    .await;

    // Merge worktree if successful
    if child_result.success {
        match worktree_manager.merge_worktree(workspace_root, &child_result.sub_agent_id) {
            Ok(merge_result) => {
                let _ = emit_and_record(
                    db,
                    bus,
                    "agent",
                    "agent.worktree_merged",
                    Some(run_id.to_string()),
                    serde_json::json!({
                        "task_id": task_id,
                        "sub_agent_id": child_result.sub_agent_id,
                        "step_idx": step_idx,
                        "merge_success": merge_result.success,
                        "merge_strategy": merge_result.strategy.to_string(),
                        "merge_message": merge_result.message,
                        "conflicted_files": merge_result.conflicted_files,
                    }),
                );

                let conflicted_json = if merge_result.conflicted_files.is_empty() {
                    None
                } else {
                    Some(
                        serde_json::to_string(&merge_result.conflicted_files).unwrap_or_default(),
                    )
                };
                let _ = queries::update_worktree_log_merge(
                    db,
                    &child_result.sub_agent_id,
                    &merge_result.strategy.to_string(),
                    merge_result.success,
                    &merge_result.message,
                    conflicted_json.as_deref(),
                    &Utc::now().to_rfc3339(),
                );

                child_result.merge_message = Some(merge_result.message.clone());

                if merge_result.success {
                    let _ = queries::update_sub_agent_status(
                        db,
                        &child_result.sub_agent_id,
                        "completed",
                        None,
                        Some(&Utc::now().to_rfc3339()),
                        None,
                    );
                } else {
                    child_result.success = false;
                    child_result.error =
                        Some(format!("merge failed: {}", merge_result.message));
                    let _ = queries::update_sub_agent_status(
                        db,
                        &child_result.sub_agent_id,
                        "failed",
                        None,
                        Some(&Utc::now().to_rfc3339()),
                        Some(&merge_result.message),
                    );
                    let _ = emit_and_record(
                        db,
                        bus,
                        "agent",
                        "agent.subagent_failed",
                        Some(run_id.to_string()),
                        serde_json::json!({
                            "task_id": task_id,
                            "sub_agent_id": child_result.sub_agent_id,
                            "step_idx": step_idx,
                            "error": format!("merge failed: {}", merge_result.message),
                        }),
                    );
                }
            }
            Err(error) => {
                child_result.success = false;
                child_result.error = Some(format!("merge error: {}", error));
                child_result.merge_message = Some(format!("merge error: {}", error));
                let _ = queries::update_sub_agent_status(
                    db,
                    &child_result.sub_agent_id,
                    "failed",
                    None,
                    Some(&Utc::now().to_rfc3339()),
                    Some(&error.to_string()),
                );
                let _ = emit_and_record(
                    db,
                    bus,
                    "agent",
                    "agent.subagent_failed",
                    Some(run_id.to_string()),
                    serde_json::json!({
                        "task_id": task_id,
                        "sub_agent_id": child_result.sub_agent_id,
                        "step_idx": step_idx,
                        "error": format!("merge error: {}", error),
                    }),
                );
            }
        }
    }

    // Close the sub-agent
    let final_status = if child_result.success {
        "completed"
    } else {
        "failed"
    };
    let close_reason = if child_result.success {
        "merged_and_integrated"
    } else {
        "spawn_or_merge_failed"
    };

    let _ = queries::update_sub_agent_status(
        db,
        &child_result.sub_agent_id,
        "closed",
        None,
        Some(&Utc::now().to_rfc3339()),
        child_result.error.as_deref(),
    );
    let _ = emit_and_record(
        db,
        bus,
        "agent",
        "agent.subagent_closed",
        Some(run_id.to_string()),
        serde_json::json!({
            "task_id": task_id,
            "sub_agent_id": child_result.sub_agent_id,
            "step_idx": step_idx,
            "final_status": final_status,
            "close_reason": close_reason,
        }),
    );

    // Cleanup worktree
    let _ = worktree_manager.remove_worktree(workspace_root, &child_result.sub_agent_id);
    let _ = queries::update_worktree_log_cleaned(
        db,
        &child_result.sub_agent_id,
        &Utc::now().to_rfc3339(),
    );

    SubAgentExecutionResult {
        success: child_result.success,
        sub_agent_id: child_result.sub_agent_id,
        output_path: child_result.output_path,
        error: child_result.error,
        merge_message: child_result.merge_message,
    }
}
