use std::path::Path;
use std::time::Duration;

use serde_json::json;
use tokio::time::timeout;

use crate::db::{queries, Database};
use crate::policy::PolicyEngine;
use crate::runtime::approval::ApprovalGate;
use crate::runtime::planner::emit_and_record;
use crate::runtime::questions::UserQuestionGate;
use crate::tools::{ToolCallInput, ToolCallOutput, ToolError, ToolRegistry};

pub fn invoke_tool_with_special_cases(
    db: &Database,
    bus: &crate::bus::EventBus,
    task_id: &str,
    tool_registry: &ToolRegistry,
    policy: &PolicyEngine,
    worktree_path: &Path,
    tool_name: &str,
    tool_args: &serde_json::Value,
) -> Result<ToolCallOutput, ToolError> {
    if tool_name == "diagram.read_graph" {
        return crate::tools::canvas::handle_read_graph(db, task_id);
    }

    if tool_name == "diagram.apply_ops" {
        let batch: crate::tools::canvas::DiagramOpBatch = serde_json::from_value(tool_args.clone())
            .map_err(|e| ToolError::InvalidInput(format!("Invalid operation batch: {}", e)))?;
        return crate::tools::canvas::handle_apply_ops(db, bus, task_id, batch);
    }

    tool_registry.invoke(
        policy,
        worktree_path,
        ToolCallInput {
            name: tool_name.to_string(),
            args: tool_args.clone(),
        },
    )
}

pub async fn resolve_human_gates(
    db: &Database,
    bus: &crate::bus::EventBus,
    approval_gate: &ApprovalGate,
    question_gate: &UserQuestionGate,
    policy: &PolicyEngine,
    run_id: &str,
    task_id: &str,
    sub_agent_id: Option<&str>,
    tool_call_id: &str,
    tool_name: &str,
    mut invocation: Result<ToolCallOutput, ToolError>,
    mut reinvoke: impl FnMut() -> Result<ToolCallOutput, ToolError>,
) -> Result<Result<ToolCallOutput, ToolError>, String> {
    let sub_agent = sub_agent_id.unwrap_or("");

    if let Err(ToolError::ApprovalRequired { scope, reason }) = &invocation {
        queries::update_tool_call_result(
            db,
            tool_call_id,
            "awaiting_approval",
            None,
            None,
            Some(reason),
        )
        .map_err(|e| e.to_string())?;

        let (request, receiver) = approval_gate.request(
            task_id,
            run_id,
            sub_agent,
            tool_call_id,
            tool_name,
            scope,
            reason,
        );

        let mut payload = serde_json::Map::new();
        payload.insert("task_id".into(), json!(task_id));
        payload.insert("tool_call_id".into(), json!(tool_call_id));
        payload.insert("approval_id".into(), json!(request.id));
        payload.insert("tool_name".into(), json!(tool_name));
        payload.insert("scope".into(), json!(scope));
        payload.insert("reason".into(), json!(reason));
        if !sub_agent.is_empty() {
            payload.insert("sub_agent_id".into(), json!(sub_agent));
        }

        let _ = emit_and_record(
            db,
            bus,
            "tool",
            "tool.approval_required",
            Some(run_id.to_string()),
            serde_json::Value::Object(payload),
        );

        let approved = match timeout(Duration::from_secs(300), receiver).await {
            Ok(Ok(value)) => value,
            Ok(Err(_)) => false,
            Err(_) => false,
        };

        let mut resolved_payload = serde_json::Map::new();
        resolved_payload.insert("task_id".into(), json!(task_id));
        resolved_payload.insert("tool_call_id".into(), json!(tool_call_id));
        resolved_payload.insert("approval_id".into(), json!(request.id));
        resolved_payload.insert("approved".into(), json!(approved));
        if !sub_agent.is_empty() {
            resolved_payload.insert("sub_agent_id".into(), json!(sub_agent));
        }

        let _ = emit_and_record(
            db,
            bus,
            "tool",
            "tool.approval_resolved",
            Some(run_id.to_string()),
            serde_json::Value::Object(resolved_payload),
        );

        invocation = if approved {
            policy.allow_scope(scope);
            reinvoke()
        } else {
            Err(ToolError::PolicyDenied(format!(
                "approval denied for scope: {scope}"
            )))
        };
    }

    if let Err(ToolError::UserQuestionRequired { question }) = &invocation {
        queries::update_tool_call_result(
            db,
            tool_call_id,
            "awaiting_user",
            None,
            None,
            Some(&question.question),
        )
        .map_err(|e| e.to_string())?;

        let (request, receiver) = question_gate.request(
            task_id,
            run_id,
            sub_agent,
            tool_call_id,
            question.question.clone(),
            question.options.clone(),
            question.multiple,
            question.allow_custom,
            question.timeout_secs,
            question.default_option_id.clone(),
        );

        let fallback_payload = if sub_agent.is_empty() {
            json!({
                "task_id": task_id,
                "run_id": run_id,
                "tool_call_id": tool_call_id,
                "question": question.question,
            })
        } else {
            json!({
                "task_id": task_id,
                "run_id": run_id,
                "sub_agent_id": sub_agent,
                "tool_call_id": tool_call_id,
                "question": question.question,
            })
        };

        let _ = emit_and_record(
            db,
            bus,
            "agent",
            "agent.question_required",
            Some(run_id.to_string()),
            serde_json::to_value(&request).unwrap_or(fallback_payload),
        );

        let timeout_duration = Duration::from_secs(question.timeout_secs.unwrap_or(300));
        match timeout(timeout_duration, receiver).await {
            Ok(Ok(answer)) => {
                let mut answered_payload = serde_json::Map::new();
                answered_payload.insert("task_id".into(), json!(task_id));
                answered_payload.insert("run_id".into(), json!(run_id));
                answered_payload.insert("tool_call_id".into(), json!(tool_call_id));
                answered_payload.insert("question_id".into(), json!(request.id));
                answered_payload.insert("answer".into(), json!(answer));
                if !sub_agent.is_empty() {
                    answered_payload.insert("sub_agent_id".into(), json!(sub_agent));
                }

                let _ = emit_and_record(
                    db,
                    bus,
                    "agent",
                    "agent.question_answered",
                    Some(run_id.to_string()),
                    serde_json::Value::Object(answered_payload),
                );

                invocation = Ok(ToolCallOutput {
                    ok: true,
                    data: json!({
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

    Ok(invocation)
}
