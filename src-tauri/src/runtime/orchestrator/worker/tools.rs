//! Tool execution logic for the worker.
//!
//! Handles tool invocation, approval flows, result recording, and observation tracking.

use std::time::Duration;

use chrono::Utc;
use tokio::time::timeout;
use uuid::Uuid;

use crate::db::{queries, Database};
use crate::policy::PolicyEngine;
use crate::runtime::approval::ApprovalGate;
use crate::runtime::planner::emit_and_record;
use crate::runtime::questions::UserQuestionGate;
use crate::tools::{ToolCallInput, ToolError, ToolRegistry};

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
    let mut invocation = if tool_name == "diagram.read_graph" {
        crate::tools::canvas::handle_read_graph(db, task_id)
    } else if tool_name == "diagram.apply_ops" {
        let batch: crate::tools::canvas::DiagramOpBatch = match serde_json::from_value(tool_args.clone()) {
            Ok(b) => b,
            Err(e) => {
                return serde_json::json!({
                    "tool_name": tool_name,
                    "status": "error",
                    "error": format!("Invalid operation batch: {}", e),
                });
            }
        };
        crate::tools::canvas::handle_apply_ops(db, task_id, batch)
    } else {
        tool_registry.invoke(
            policy,
            worktree_path,
            ToolCallInput {
                name: tool_name.to_string(),
                args: tool_args.clone(),
            },
        )
    };

    // Handle approval-required errors
    if let Err(ToolError::ApprovalRequired { scope, reason }) = &invocation {
        let _ = queries::update_tool_call_result(
            db,
            &tool_call_id,
            "awaiting_approval",
            None,
            None,
            Some(reason),
        );

        let (request, receiver) = approval_gate.request(
            task_id,
            run_id,
            sub_agent_id,
            &tool_call_id,
            tool_name,
            scope,
            reason,
        );

        let _ = emit_and_record(
            db,
            bus,
            "tool",
            "tool.approval_required",
            Some(run_id.to_string()),
            serde_json::json!({
                "task_id": task_id,
                "sub_agent_id": sub_agent_id,
                "tool_call_id": tool_call_id,
                "approval_id": request.id,
                "tool_name": tool_name,
                "scope": scope,
                "reason": reason,
            }),
        );

        // Wait for approval decision (5 minute timeout)
        let approved = match timeout(Duration::from_secs(300), receiver).await {
            Ok(Ok(value)) => value,
            _ => false,
        };

        let _ = emit_and_record(
            db,
            bus,
            "tool",
            "tool.approval_resolved",
            Some(run_id.to_string()),
            serde_json::json!({
                "task_id": task_id,
                "sub_agent_id": sub_agent_id,
                "tool_call_id": tool_call_id,
                "approval_id": request.id,
                "approved": approved,
            }),
        );

        if approved {
            policy.allow_scope(scope);
            invocation = if tool_name == "diagram.read_graph" {
                crate::tools::canvas::handle_read_graph(db, task_id)
            } else if tool_name == "diagram.apply_ops" {
                match serde_json::from_value::<crate::tools::canvas::DiagramOpBatch>(tool_args.clone()) {
                    Ok(batch) => crate::tools::canvas::handle_apply_ops(db, task_id, batch),
                    Err(e) => Err(crate::tools::ToolError::InvalidInput(format!("Invalid operation batch: {}", e))),
                }
            } else {
                tool_registry.invoke(
                    policy,
                    worktree_path,
                    ToolCallInput {
                        name: tool_name.to_string(),
                        args: tool_args.clone(),
                    },
                )
            };
        } else {
            invocation = Err(ToolError::PolicyDenied(format!(
                "approval denied for scope: {scope}"
            )));
        }
    }

    if let Err(ToolError::UserQuestionRequired { question }) = &invocation {
        let _ = queries::update_tool_call_result(
            db,
            &tool_call_id,
            "awaiting_user",
            None,
            None,
            Some(&question.question),
        );

        let (request, receiver) = question_gate.request(
            task_id,
            run_id,
            sub_agent_id,
            &tool_call_id,
            question.question.clone(),
            question.options.clone(),
            question.multiple,
            question.allow_custom,
        );

        let _ = emit_and_record(
            db,
            bus,
            "agent",
            "agent.question_required",
            Some(run_id.to_string()),
            serde_json::to_value(&request).unwrap_or_else(|_| {
                serde_json::json!({
                    "task_id": task_id,
                    "run_id": run_id,
                    "sub_agent_id": sub_agent_id,
                    "tool_call_id": tool_call_id,
                    "question": question.question,
                })
            }),
        );

        match timeout(Duration::from_secs(300), receiver).await {
            Ok(Ok(answer)) => {
                let _ = emit_and_record(
                    db,
                    bus,
                    "agent",
                    "agent.question_answered",
                    Some(run_id.to_string()),
                    serde_json::json!({
                        "task_id": task_id,
                        "run_id": run_id,
                        "sub_agent_id": sub_agent_id,
                        "tool_call_id": tool_call_id,
                        "question_id": request.id,
                        "answer": answer,
                    }),
                );

                invocation = Ok(crate::tools::ToolCallOutput {
                    ok: true,
                    data: serde_json::json!({
                        "question_id": request.id,
                        "answer": answer,
                    }),
                    error: None,
                });
            }
            _ => {
                invocation = Err(ToolError::Execution(
                    "user question timed out or was cancelled".to_string(),
                ));
            }
        }
    }

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
