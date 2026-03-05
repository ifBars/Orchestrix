//! Tool execution logic for the worker.
//!
//! Handles tool invocation, approval flows, result recording, and observation tracking.

use chrono::Utc;
use uuid::Uuid;

use crate::db::{queries, Database};
use crate::policy::PolicyEngine;
use crate::runtime::approval::ApprovalGate;
use crate::runtime::planner::emit_and_record;
use crate::runtime::questions::UserQuestionGate;
use crate::runtime::tool_calling::{invoke_tool_with_special_cases, resolve_human_gates};
use crate::tools::{ToolError, ToolRegistry};

/// Execute a single tool call with full lifecycle management.
///
/// This function handles:
/// - Tool call recording in database
/// - Event emission
/// - Policy/approval checking
/// - Tool invocation
/// - Result recording and observation tracking
/// - Artifact tracking for `agent.create_artifact`
pub async fn execute_tool_call(
    db: &Database,
    bus: &crate::bus::EventBus,
    tool_registry: &ToolRegistry,
    policy: &PolicyEngine,
    approval_gate: &ApprovalGate,
    question_gate: &UserQuestionGate,
    run_id: &str,
    task_id: &str,
    sub_agent_id: &str,
    step_idx: usize,
    turn: usize,
    tool_name: &str,
    tool_args: &serde_json::Value,
    rationale: Option<&str>,
    worktree_path: &std::path::Path,
    available_tools: &[String],
) -> serde_json::Value {
    // Check if tool is allowed
    if !available_tools.contains(&tool_name.to_string()) {
        return serde_json::json!({
            "tool_name": tool_name,
            "status": "denied",
            "error": "tool not allowed by delegation contract",
        });
    }

    // Record tool call in database
    let tool_call_id = Uuid::new_v4().to_string();
    let started_at = Utc::now().to_rfc3339();
    let _ = queries::insert_tool_call(
        db,
        &queries::ToolCallRow {
            id: tool_call_id.clone(),
            run_id: run_id.to_string(),
            step_idx: Some(step_idx as i64),
            tool_name: tool_name.to_string(),
            input_json: tool_args.to_string(),
            output_json: None,
            status: "running".to_string(),
            started_at: Some(started_at),
            finished_at: None,
            error: None,
        },
    );

    // Emit tool.call_started event
    let _ = emit_and_record(
        db,
        bus,
        "tool",
        "tool.call_started",
        Some(run_id.to_string()),
        serde_json::json!({
            "task_id": task_id,
            "sub_agent_id": sub_agent_id,
            "tool_call_id": tool_call_id,
            "tool_name": tool_name,
            "tool_args": tool_args,
            "step_idx": step_idx,
            "turn": turn,
            "rationale": rationale,
        }),
    );

    // Invoke the tool
    let invocation = invoke_tool_with_special_cases(
        db,
        bus,
        task_id,
        tool_registry,
        policy,
        worktree_path,
        tool_name,
        tool_args,
    );

    let invocation = match resolve_human_gates(
        db,
        bus,
        approval_gate,
        question_gate,
        policy,
        run_id,
        task_id,
        Some(sub_agent_id),
        &tool_call_id,
        tool_name,
        invocation,
        || {
            invoke_tool_with_special_cases(
                db,
                bus,
                task_id,
                tool_registry,
                policy,
                worktree_path,
                tool_name,
                tool_args,
            )
        },
    )
    .await
    {
        Ok(result) => result,
        Err(error) => Err(ToolError::Execution(error)),
    };

    // Handle invocation result
    match invocation {
        Ok(output) => {
            let output_json = output.data.to_string();
            let _ = queries::update_tool_call_result(
                db,
                &tool_call_id,
                if output.ok { "succeeded" } else { "failed" },
                Some(&output_json),
                Some(&Utc::now().to_rfc3339()),
                output.error.as_deref(),
            );

            let _ = emit_and_record(
                db,
                bus,
                "tool",
                "tool.call_finished",
                Some(run_id.to_string()),
                serde_json::json!({
                    "task_id": task_id,
                    "sub_agent_id": sub_agent_id,
                    "tool_call_id": tool_call_id,
                    "status": if output.ok { "succeeded" } else { "failed" },
                    "output": output.data,
                }),
            );

            // Track artifacts created via agent.create_artifact
            if tool_name == "agent.create_artifact" && output.ok {
                if let (Some(path), Some(kind)) = (
                    output.data.get("path").and_then(|v| v.as_str()),
                    output.data.get("kind").and_then(|v| v.as_str()),
                ) {
                    let artifact = queries::ArtifactRow {
                        id: Uuid::new_v4().to_string(),
                        run_id: run_id.to_string(),
                        kind: kind.to_string(),
                        uri_or_content: path.to_string(),
                        metadata_json: Some(
                            serde_json::json!({
                                "task_id": task_id,
                                "source": "agent.create_artifact",
                                "kind": kind,
                            })
                            .to_string(),
                        ),
                        created_at: Utc::now().to_rfc3339(),
                    };
                    let _ = queries::insert_artifact(db, &artifact);
                    let _ = emit_and_record(
                        db,
                        bus,
                        "artifact",
                        "artifact.created",
                        Some(run_id.to_string()),
                        serde_json::json!({
                            "task_id": task_id,
                            "artifact_id": artifact.id,
                            "kind": artifact.kind,
                            "uri": artifact.uri_or_content,
                        }),
                    );
                }
            }

            serde_json::json!({
                "tool_name": tool_name,
                "status": if output.ok { "succeeded" } else { "failed" },
                "output": output.data,
            })
        }
        Err(error) => {
            let _ = queries::update_tool_call_result(
                db,
                &tool_call_id,
                "denied",
                None,
                Some(&Utc::now().to_rfc3339()),
                Some(&error.to_string()),
            );

            let _ = emit_and_record(
                db,
                bus,
                "tool",
                "tool.call_finished",
                Some(run_id.to_string()),
                serde_json::json!({
                    "task_id": task_id,
                    "sub_agent_id": sub_agent_id,
                    "tool_call_id": tool_call_id,
                    "status": "denied",
                    "error": error.to_string(),
                }),
            );

            serde_json::json!({
                "tool_name": tool_name,
                "status": "denied",
                "error": error.to_string(),
            })
        }
    }
}
