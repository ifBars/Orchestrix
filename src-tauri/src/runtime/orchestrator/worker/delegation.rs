//! Sub-agent delegation logic for the worker.
//!
//! Handles spawning child sub-agents, executing them, merging worktrees,
//! and managing the delegation lifecycle.

use std::path::Path;

use chrono::Utc;
use uuid::Uuid;

use crate::core::agent_presets;
use crate::core::plan::{PlanStep, StepStatus};
use crate::db::{queries, Database};
use crate::runtime::planner::emit_and_record;
use crate::runtime::worktree::WorktreeManager;
use crate::tools::ToolRegistry;

/// Result of a sub-agent execution.
pub struct SubAgentExecutionResult {
    pub success: bool,
    pub sub_agent_id: String,
    pub agent_preset_id: Option<String>,
    pub agent_preset_name: Option<String>,
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
    agent_preset_id: Option<&str>,
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
            agent_preset_id: None,
            agent_preset_name: None,
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
            agent_preset_id: None,
            agent_preset_name: None,
            output_path: None,
            error: Some("delegation disabled by contract".to_string()),
            merge_message: None,
        };
    }

    if current_delegation_depth >= max_delegation_depth {
        return SubAgentExecutionResult {
            success: false,
            sub_agent_id: String::new(),
            agent_preset_id: None,
            agent_preset_name: None,
            output_path: None,
            error: Some("max delegation depth reached".to_string()),
            merge_message: None,
        };
    }

    let requested_preset_id = agent_preset_id
        .and_then(normalize_agent_preset_reference)
        .or_else(|| agent_presets::extract_agent_preset_id_from_prompt(objective));

    let selected_agent_preset = if let Some(preset_id) = requested_preset_id.as_deref() {
        match agent_presets::get_agent_preset(workspace_root, preset_id) {
            Some(preset) => Some(preset),
            None => {
                return SubAgentExecutionResult {
                    success: false,
                    sub_agent_id: String::new(),
                    agent_preset_id: Some(preset_id.to_string()),
                    agent_preset_name: None,
                    output_path: None,
                    error: Some(format!("agent preset not found: {}", preset_id)),
                    merge_message: None,
                }
            }
        }
    } else {
        None
    };

    let preset_id = selected_agent_preset.as_ref().map(|p| p.id.clone());
    let preset_name = selected_agent_preset.as_ref().map(|p| p.name.clone());

    // Create restricted tool list for child (no subagent.spawn, but include agent.complete)
    let mut delegated_allowed_tools: Vec<String> = available_tools
        .iter()
        .filter(|name| name.as_str() != "subagent.spawn")
        .cloned()
        .collect();

    // agent.complete is exclusive to subagents - add it to their tool list
    if !delegated_allowed_tools.contains(&"agent.complete".to_string()) {
        delegated_allowed_tools.push("agent.complete".to_string());
    }

    if let Some(preset) = selected_agent_preset.as_ref() {
        apply_agent_preset_tool_constraints(&mut delegated_allowed_tools, preset);
    }

    let delegated_skills_context = if let Some(preset) = selected_agent_preset.as_ref() {
        build_delegated_agent_context(skills_context, preset)
    } else {
        skills_context.to_string()
    };

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
                "agent_preset": selected_agent_preset.as_ref().map(|preset| {
                    serde_json::json!({
                        "id": preset.id,
                        "name": preset.name,
                        "mode": preset.mode,
                        "description": preset.description,
                    })
                }),
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
            agent_preset_id: preset_id,
            agent_preset_name: preset_name,
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
            "agent_preset_id": preset_id.clone(),
            "agent_preset_name": preset_name.clone(),
        }),
    );

    // Create delegated step
    let delegated_step = PlanStep {
        idx: step_idx,
        title: format!("Delegated objective {}", turn),
        description: format!(
            "{}\n\nCompletion rule: once the objective output is produced, call agent.complete with a concise summary and outputs, then stop.",
            objective
        ),
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
        delegated_skills_context,
    ))
    .await;

    // Persist child report/output outside ephemeral worktree so the parent can
    // inspect it after merge + cleanup.
    child_result.output_path = persist_child_output_path(
        workspace_root,
        run_id,
        &child_result.sub_agent_id,
        child_result.output_path.as_deref(),
    );

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
                    Some(serde_json::to_string(&merge_result.conflicted_files).unwrap_or_default())
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
                    child_result.error = Some(format!("merge failed: {}", merge_result.message));
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
            "agent_preset_id": preset_id.clone(),
            "agent_preset_name": preset_name.clone(),
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
        agent_preset_id: preset_id,
        agent_preset_name: preset_name,
        output_path: child_result.output_path,
        error: child_result.error,
        merge_message: child_result.merge_message,
    }
}

fn persist_child_output_path(
    workspace_root: &Path,
    run_id: &str,
    sub_agent_id: &str,
    output_path: Option<&str>,
) -> Option<String> {
    let src = output_path?;
    let src_path = Path::new(src);
    if !src_path.exists() || !src_path.is_file() {
        return None;
    }

    let reports_dir = workspace_root
        .join(".orchestrix")
        .join("sub-agent-reports")
        .join(run_id);
    if std::fs::create_dir_all(&reports_dir).is_err() {
        return None;
    }

    let file_name = src_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("step-result.md");
    let dest = reports_dir.join(format!("{}-{}", sub_agent_id, file_name));
    if std::fs::copy(src_path, &dest).is_err() {
        return None;
    }

    Some(dest.to_string_lossy().to_string())
}

fn apply_agent_preset_tool_constraints(
    allowed_tools: &mut Vec<String>,
    preset: &agent_presets::AgentPreset,
) {
    let mut deny_set: std::collections::HashSet<String> = std::collections::HashSet::new();

    if let Some(tools) = &preset.tools {
        for (tool_name, permission) in tools {
            if matches!(permission, agent_presets::ToolPermission::Bool(false)) {
                for mapped in map_tool_aliases(tool_name) {
                    deny_set.insert(mapped);
                }
            }
        }
    }

    if let Some(permission) = &preset.permission {
        if matches!(
            permission.edit,
            Some(agent_presets::ToolPermission::Bool(false))
        ) || matches!(
            permission.write,
            Some(agent_presets::ToolPermission::Bool(false))
        ) {
            deny_set.insert("fs.write".to_string());
        }
        if matches!(
            permission.bash,
            Some(agent_presets::ToolPermission::Bool(false))
        ) {
            deny_set.insert("cmd.exec".to_string());
        }
        if matches!(
            permission.webfetch,
            Some(agent_presets::ToolPermission::Bool(false))
        ) {
            deny_set.insert("webfetch".to_string());
        }
    }

    if deny_set.is_empty() {
        return;
    }

    allowed_tools.retain(|tool_name| !deny_set.contains(tool_name));
}

fn map_tool_aliases(alias: &str) -> Vec<String> {
    match alias {
        "write" | "edit" => vec!["fs.write".to_string()],
        "bash" => vec!["cmd.exec".to_string()],
        "webfetch" => vec!["webfetch".to_string()],
        "read" => vec!["fs.read".to_string()],
        "list" => vec!["fs.list".to_string()],
        _ => vec![alias.to_string()],
    }
}

fn build_delegated_agent_context(
    existing_context: &str,
    preset: &agent_presets::AgentPreset,
) -> String {
    let preset_context = format!(
        "# Delegated Agent Preset\n\nID: {}\nName: {}\nMode: {:?}\n\nDescription:\n{}\n\nPrompt:\n{}",
        preset.id,
        preset.name,
        preset.mode,
        if preset.description.trim().is_empty() {
            "(no description)"
        } else {
            &preset.description
        },
        preset.prompt,
    );

    if existing_context.trim().is_empty() {
        preset_context
    } else {
        format!("{}\n\n{}", existing_context, preset_context)
    }
}

fn normalize_agent_preset_reference(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed
        .strip_prefix("@agent:")
        .or_else(|| trimmed.strip_prefix("agent:"))
        .unwrap_or(trimmed)
        .trim();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}
