use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::bus::{BusEvent, EventBus, CATEGORY_AGENT, EVENT_AGENT_DECIDING, EVENT_AGENT_TOOL_CALLS_PREPARING};
use crate::core::prompt_references::expand_prompt_references;
use crate::core::tool::ToolDescriptor;
use crate::db::{queries, Database};
use crate::model::{
    kimi::KimiPlanner,
    minimax::MiniMaxPlanner,
    strip_tool_call_markup,
    PlannerModel,
    WorkerAction,
    WorkerActionRequest,
    WorkerToolCall,
};
use crate::tools::ToolRegistry;
use crate::policy::PolicyEngine;
use crate::runtime::approval::ApprovalGate;

/// Returned from plan generation; run_id and artifact_path are for future API/UI use.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PlanningOutcome {
    pub run_id: String,
    pub artifact_path: String,
}

const MAX_PLANNING_TURNS: usize = 20;

/// Multi-turn planning loop: let the agent use tools autonomously before creating the plan artifact.
/// When agent.create_artifact is called, extract the content and return it as the plan markdown.
async fn run_multi_turn_planning<P: PlannerModel>(
    db: &Database,
    bus: &EventBus,
    planner: &P,
    task_id: &str,
    run_id: &str,
    prompt: &str,
    context: &str,
    skills_context: &str,
    tool_descriptors: Vec<ToolDescriptor>,
    tool_registry: &ToolRegistry,
    policy: &PolicyEngine,
    approval_gate: &ApprovalGate,
    workspace_root: &std::path::Path,
) -> Result<(String, Option<String>), String> {
    let mut observations: Vec<serde_json::Value> = Vec::new();
    let mut turn: usize = 0;

    let available_tools: Vec<String> = tool_descriptors.iter().map(|t| t.name.clone()).collect();
    let tool_descriptions = tool_registry.tool_reference_for_plan_mode();

    loop {
        turn += 1;

        if turn > MAX_PLANNING_TURNS {
            return Err(format!(
                "Planning exceeded maximum turns ({MAX_PLANNING_TURNS}) without creating artifact"
            ));
        }

        let _ = emit_and_record(
            db,
            bus,
            CATEGORY_AGENT,
            EVENT_AGENT_DECIDING,
            Some(run_id.to_string()),
            serde_json::json!({
                "task_id": task_id,
                "run_id": run_id,
                "turn": turn,
            }),
        );

        let plan_mode_instruction = "You are in PLAN mode. Use tools (e.g. fs.list, fs.read, search.rg) to explore the workspace if needed. When ready, submit your plan by calling agent.create_artifact with filename (e.g. plan.md), kind \"plan\", and content set to your full markdown plan. Do not reply with a plain-text completion summary until you have called agent.create_artifact.";
        let full_context = if skills_context.is_empty() {
            format!("{}\n\n{}", plan_mode_instruction, context)
        } else {
            format!("{}\n\n{}\n\n{}", plan_mode_instruction, context, skills_context)
        };
        let full_context = full_context.trim();

        let decision = planner
            .decide_worker_action(WorkerActionRequest {
                task_prompt: prompt.to_string(),
                goal_summary: "Draft an implementation plan and submit it via agent.create_artifact.".to_string(),
                context: full_context.to_string(),
                available_tools: available_tools.clone(),
                tool_descriptions: tool_descriptions.clone(),
                tool_descriptors: tool_descriptors.clone(),
                prior_observations: observations.clone(),
            })
            .await
            .map_err(|e| e.to_string())?;

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
                        "content": reasoning.trim(),
                    }),
                );
            }
        }

        match decision.action {
            WorkerAction::Complete { summary } => {
                // Model returned plain text completion - treat as the plan (fallback)
                let cleaned = strip_tool_call_markup(summary.trim()).trim().to_string();
                if cleaned.is_empty() {
                    return Err("Planner completed with empty summary (no plan content)".to_string());
                }
                let _ = emit_and_record(
                    db,
                    bus,
                    "agent",
                    "agent.plan_message",
                    Some(run_id.to_string()),
                    serde_json::json!({
                        "task_id": task_id,
                        "content": format!("Plan complete: {}", cleaned),
                    }),
                );
                return Ok((cleaned, None));
            }
            WorkerAction::ToolCall { tool_name, tool_args, rationale } => {
                // Convert single call to vec for uniform handling
                handle_planning_tool_calls(
                    db,
                    bus,
                    task_id,
                    run_id,
                    vec![WorkerToolCall { tool_name, tool_args, rationale }],
                    &mut observations,
                    tool_registry,
                    policy,
                    approval_gate,
                    workspace_root,
                    turn,
                )
                .await?;
            }
            WorkerAction::ToolCalls { calls } => {
                handle_planning_tool_calls(
                    db,
                    bus,
                    task_id,
                    run_id,
                    calls,
                    &mut observations,
                    tool_registry,
                    policy,
                    approval_gate,
                    workspace_root,
                    turn,
                )
                .await?;
            }
            WorkerAction::Delegate { .. } => {
                return Err("Delegation is not supported in plan mode".to_string());
            }
        }

        // Check if agent.create_artifact was called and extract the plan content
        if let Some(plan_content) = extract_plan_from_observations(&observations) {
            let markdown = strip_tool_call_markup(plan_content.trim()).trim().to_string();
            let artifact_path = extract_plan_artifact_path_from_observations(&observations);
            return Ok((markdown, artifact_path));
        }
    }
}

/// Handle tool calls during planning. If agent.create_artifact is called, the plan content
/// is added to observations and will be extracted by the caller.
async fn handle_planning_tool_calls(
    db: &Database,
    bus: &EventBus,
    task_id: &str,
    run_id: &str,
    calls: Vec<WorkerToolCall>,
    observations: &mut Vec<serde_json::Value>,
    tool_registry: &ToolRegistry,
    policy: &PolicyEngine,
    approval_gate: &ApprovalGate,
    workspace_root: &std::path::Path,
    turn: usize,
) -> Result<(), String> {
    use crate::tools::{ToolCallInput, ToolError};
    use std::time::Duration;
    use tokio::time::timeout;

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
            "turn": turn,
        }),
    );

    for call in calls {
        let tool_name = call.tool_name;
        let tool_args = call.tool_args;
        let rationale = call.rationale;

        let tool_call_id = Uuid::new_v4().to_string();
        let started_at = Utc::now().to_rfc3339();
        queries::insert_tool_call(
            db,
            &queries::ToolCallRow {
                id: tool_call_id.clone(),
                run_id: run_id.to_string(),
                step_idx: None,
                tool_name: tool_name.clone(),
                input_json: tool_args.to_string(),
                output_json: None,
                status: "running".to_string(),
                started_at: Some(started_at),
                finished_at: None,
                error: None,
            },
        )
        .map_err(|e| e.to_string())?;

        let _ = emit_and_record(
            db,
            bus,
            "tool",
            "tool.call_started",
            Some(run_id.to_string()),
            serde_json::json!({
                "task_id": task_id,
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "tool_args": tool_args,
                "turn": turn,
                "rationale": rationale,
            }),
        );

        let mut invocation = tool_registry.invoke(
            policy,
            workspace_root,
            ToolCallInput {
                name: tool_name.clone(),
                args: tool_args.clone(),
            },
        );

        // Handle approval if required
        if let Err(ToolError::ApprovalRequired { scope, reason }) = &invocation {
            queries::update_tool_call_result(
                db,
                &tool_call_id,
                "awaiting_approval",
                None,
                None,
                Some(reason),
            )
            .map_err(|e| e.to_string())?;

            let (request, receiver) = approval_gate.request(
                task_id,
                run_id,
                "", // no sub_agent_id in plan mode
                &tool_call_id,
                &tool_name,
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
                    "tool_call_id": tool_call_id,
                    "approval_id": request.id,
                    "tool_name": tool_name,
                    "scope": scope,
                    "reason": reason,
                }),
            );

            let approved = match timeout(Duration::from_secs(300), receiver).await {
                Ok(Ok(value)) => value,
                Ok(Err(_)) => false,
                Err(_) => false,
            };

            let _ = emit_and_record(
                db,
                bus,
                "tool",
                "tool.approval_resolved",
                Some(run_id.to_string()),
                serde_json::json!({
                    "task_id": task_id,
                    "tool_call_id": tool_call_id,
                    "approval_id": request.id,
                    "approved": approved,
                }),
            );

            if approved {
                policy.allow_scope(scope);
                invocation = tool_registry.invoke(
                    policy,
                    workspace_root,
                    ToolCallInput {
                        name: tool_name.clone(),
                        args: tool_args.clone(),
                    },
                );
            } else {
                invocation = Err(ToolError::PolicyDenied(format!("approval denied for scope: {scope}")));
            }
        }

        match invocation {
            Ok(output) => {
                let output_json = output.data.to_string();
                queries::update_tool_call_result(
                    db,
                    &tool_call_id,
                    if output.ok { "succeeded" } else { "failed" },
                    Some(&output_json),
                    Some(&Utc::now().to_rfc3339()),
                    output.error.as_deref(),
                )
                .map_err(|e| e.to_string())?;

                let _ = emit_and_record(
                    db,
                    bus,
                    "tool",
                    "tool.call_finished",
                    Some(run_id.to_string()),
                    serde_json::json!({
                        "task_id": task_id,
                        "tool_call_id": tool_call_id,
                        "status": if output.ok { "succeeded" } else { "failed" },
                        "output": output.data,
                    }),
                );

                // For agent.create_artifact, store path to the file the tool wrote so we can copy it
                // to run dir (avoids re-encoding the content and preserves UTF-8 box-drawing chars).
                if tool_name == "agent.create_artifact" && output.ok {
                    let artifact_path = output.data.get("path").and_then(|v| v.as_str()).map(String::from);
                    if let Some(content) = tool_args.get("content").and_then(|v| v.as_str()) {
                        let mut obs = serde_json::json!({
                            "tool_name": "agent.create_artifact",
                            "status": "succeeded",
                            "plan_content": content,
                            "output": output.data,
                        });
                        if let Some(ref path) = artifact_path {
                            obs["artifact_path"] = serde_json::Value::String(path.clone());
                        }
                        observations.push(obs);
                    } else {
                        observations.push(serde_json::json!({
                            "tool_name": tool_name,
                            "status": if output.ok { "succeeded" } else { "failed" },
                            "output": output.data,
                        }));
                    }
                } else {
                    observations.push(serde_json::json!({
                        "tool_name": tool_name,
                        "status": if output.ok { "succeeded" } else { "failed" },
                        "output": output.data,
                    }));
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                queries::update_tool_call_result(
                    db,
                    &tool_call_id,
                    "failed",
                    None,
                    Some(&Utc::now().to_rfc3339()),
                    Some(&error_msg),
                )
                .map_err(|e| e.to_string())?;

                let _ = emit_and_record(
                    db,
                    bus,
                    "tool",
                    "tool.call_finished",
                    Some(run_id.to_string()),
                    serde_json::json!({
                        "task_id": task_id,
                        "tool_call_id": tool_call_id,
                        "status": "failed",
                        "error": error_msg,
                    }),
                );

                observations.push(serde_json::json!({
                    "tool_name": tool_name,
                    "status": "failed",
                    "error": error_msg,
                }));
            }
        }
    }

    Ok(())
}

/// Extract plan content from observations if agent.create_artifact was called.
fn extract_plan_from_observations(observations: &[serde_json::Value]) -> Option<String> {
    for obs in observations.iter().rev() {
        if obs.get("tool_name").and_then(|v| v.as_str()) == Some("agent.create_artifact") {
            if let Some(content) = obs.get("plan_content").and_then(|v| v.as_str()) {
                return Some(content.to_string());
            }
        }
    }
    None
}

/// Path to the artifact file written by agent.create_artifact (tool wrote to .orchestrix/artifacts/).
fn extract_plan_artifact_path_from_observations(observations: &[serde_json::Value]) -> Option<String> {
    for obs in observations.iter().rev() {
        if obs.get("tool_name").and_then(|v| v.as_str()) == Some("agent.create_artifact") {
            if let Some(path) = obs.get("artifact_path").and_then(|v| v.as_str()) {
                return Some(path.to_string());
            }
        }
    }
    None
}

pub async fn generate_plan_markdown_artifact(
    db: Arc<Database>,
    bus: Arc<EventBus>,
    task_id: String,
    prompt: String,
    provider: String,
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
    workspace_root: std::path::PathBuf,
    existing_run_id: Option<String>,
    revision_note: Option<String>,
    plan_mode_tools: Vec<ToolDescriptor>,
    tool_registry: Arc<ToolRegistry>,
    approval_gate: Arc<ApprovalGate>,
) -> Result<PlanningOutcome, String> {
    let run_id = match existing_run_id {
        Some(value) => value,
        None => {
            let value = Uuid::new_v4().to_string();
            queries::insert_run(
                &db,
                &queries::RunRow {
                    id: value.clone(),
                    task_id: task_id.clone(),
                    status: "planning".to_string(),
                    plan_json: None,
                    started_at: Some(Utc::now().to_rfc3339()),
                    finished_at: None,
                    failure_reason: None,
                },
            )
            .map_err(|e| e.to_string())?;
            value
        }
    };

    emit_and_record(
        &db,
        &bus,
        "agent",
        "agent.planning_started",
        Some(run_id.clone()),
        serde_json::json!({ "task_id": task_id }),
    )?;

    emit_and_record(
        &db,
        &bus,
        "agent",
        "agent.plan_message",
        Some(run_id.clone()),
        serde_json::json!({
            "task_id": task_id,
            "content": "Got it. I will analyze the workspace and draft a plan for your review.",
        }),
    )?;

    let planner_model: String;

    let existing_markdown = collect_existing_markdown(&db, &task_id);

    // Load workspace skills and append their context to the planner input
    let workspace_skills = crate::core::workspace_skills::scan_workspace_skills(&workspace_root);
    let skills_context = crate::core::workspace_skills::build_skills_context(&workspace_skills);

    let context = {
        let mut ctx = if let Some(note) = revision_note.as_ref() {
            format!(
                "{}\n\nReviewer feedback to incorporate:\n- {}",
                existing_markdown,
                note
            )
        } else {
            existing_markdown
        };
        if !skills_context.is_empty() {
            ctx.push_str(&skills_context);
        }
        ctx
    };

    let prompt_with_refs = expand_prompt_references(&prompt, &workspace_root);

    // Create a policy engine for this planning session
    let policy = Arc::new(PolicyEngine::new(workspace_root.clone()));

    // Multi-turn planning: let the agent use tools before creating the artifact
    let (markdown, source_artifact_path) = if provider == "kimi" {
        let planner = KimiPlanner::new(api_key, model, base_url);
        planner_model = planner.model_id().to_string();
        run_multi_turn_planning(
            &db,
            &bus,
            &planner,
            &task_id,
            &run_id,
            &prompt_with_refs,
            &context,
            &skills_context,
            plan_mode_tools,
            tool_registry.as_ref(),
            &policy,
            approval_gate.as_ref(),
            &workspace_root,
        )
        .await?
    } else {
        let planner = MiniMaxPlanner::new_with_base_url(api_key, model, base_url);
        planner_model = planner.model_id().to_string();
        run_multi_turn_planning(
            &db,
            &bus,
            &planner,
            &task_id,
            &run_id,
            &prompt_with_refs,
            &context,
            &skills_context,
            plan_mode_tools,
            tool_registry.as_ref(),
            &policy,
            approval_gate.as_ref(),
            &workspace_root,
        )
        .await?
    };

    queries::update_run_status_and_plan(
        &db,
        &run_id,
        "awaiting_review",
        None,
        None,
        None,
    )
    .map_err(|e| e.to_string())?;

    // Trim trailing whitespace and excessive blank lines from the markdown (used for parsing and fallback write)
    let trimmed_markdown = trim_excessive_blank_lines(&markdown);

    // Prefer copying from the artifact file the tool wrote (preserves UTF-8); otherwise write trimmed markdown
    let plan_artifact_path = write_plan_artifact(
        &db,
        &bus,
        &run_id,
        &task_id,
        &workspace_root,
        &planner_model,
        &trimmed_markdown,
        source_artifact_path.as_deref(),
    )?;

    // Parse markdown into a structured plan and emit agent.plan_ready so the UI can show steps
    if let Some(structured) = parse_plan_from_markdown(&trimmed_markdown) {
        let steps_json: Vec<serde_json::Value> = structured
            .steps
            .iter()
            .map(|s| {
                serde_json::json!({
                    "title": s.title,
                    "description": s.description,
                })
            })
            .collect();
        let plan_payload = serde_json::json!({
            "task_id": task_id,
            "plan": {
                "goal_summary": structured.goal_summary,
                "steps": steps_json,
                "completion_criteria": structured.completion_criteria,
            },
        });
        let _ = emit_and_record(
            &db,
            &bus,
            "agent",
            "agent.plan_ready",
            Some(run_id.clone()),
            plan_payload,
        );
    }

    emit_and_record(
        &db,
        &bus,
        "agent",
        "agent.plan_message",
        Some(run_id.clone()),
        serde_json::json!({
            "task_id": task_id,
            "content": format!(
                "I drafted a planning artifact for review.\n\nArtifact: `{}`",
                plan_artifact_path
            ),
        }),
    )?;

    Ok(PlanningOutcome {
        run_id,
        artifact_path: plan_artifact_path,
    })
}

fn collect_existing_markdown(db: &Database, task_id: &str) -> String {
    let artifacts = queries::list_markdown_artifacts_for_task(db, task_id).unwrap_or_default();
    if artifacts.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for artifact in artifacts {
        let path = std::path::PathBuf::from(&artifact.uri_or_content);
        if !path.exists() {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            out.push_str(&format!("\n\n---\nArtifact: {}\n\n{}", artifact.uri_or_content, content));
        }
    }
    out
}

fn write_plan_artifact(
    db: &Database,
    bus: &EventBus,
    run_id: &str,
    task_id: &str,
    workspace_root: &std::path::Path,
    planner_model: &str,
    markdown: &str,
    source_artifact_path: Option<&str>,
) -> Result<String, String> {
    let run_dir = workspace_root.join(".orchestrix").join("runs").join(run_id);
    std::fs::create_dir_all(&run_dir).map_err(|e| e.to_string())?;

    let artifact_path = run_dir.join("plan.md");
    // Copy from the file the tool wrote so we preserve UTF-8 (e.g. box-drawing chars); fallback to writing markdown
    if let Some(src) = source_artifact_path {
        let src_path = std::path::Path::new(src);
        if src_path.exists() {
            std::fs::copy(src_path, &artifact_path).map_err(|e| e.to_string())?;
        } else {
            std::fs::write(&artifact_path, markdown).map_err(|e| e.to_string())?;
        }
    } else {
        std::fs::write(&artifact_path, markdown).map_err(|e| e.to_string())?;
    }

    let artifact = queries::ArtifactRow {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.to_string(),
        kind: "plan_markdown".to_string(),
        uri_or_content: artifact_path.to_string_lossy().to_string(),
        metadata_json: Some(
            serde_json::json!({
                "task_id": task_id,
                "planner_model": planner_model,
                "source": "planner_markdown",
            })
            .to_string(),
        ),
        created_at: Utc::now().to_rfc3339(),
    };
    queries::insert_artifact(db, &artifact).map_err(|e| e.to_string())?;

    emit_and_record(
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
    )?;

    Ok(artifact.uri_or_content)
}

/// Minimal structured plan parsed from markdown for agent.plan_ready.
struct ParsedPlan {
    goal_summary: String,
    steps: Vec<ParsedStep>,
    completion_criteria: Option<String>,
}

struct ParsedStep {
    title: String,
    description: String,
}

/// Parse markdown into a structured plan so the frontend can show goal + steps.
/// Tolerates common variants: "# Plan: Title", "## Implementation Steps", "1. Step" / "- Step".
fn parse_plan_from_markdown(md: &str) -> Option<ParsedPlan> {
    let mut goal_summary = String::new();
    let mut steps: Vec<ParsedStep> = Vec::new();
    let mut completion_criteria: Option<String> = None;

    let lines: Vec<&str> = md.lines().collect();
    let mut i = 0;

    // Goal: first # heading (e.g. "# Plan: X" or "# X")
    while i < lines.len() {
        let line = lines[i].trim();
        if line.starts_with("# ") {
            goal_summary = line.trim_start_matches("# ").trim().to_string();
            if goal_summary.starts_with("Plan: ") {
                goal_summary = goal_summary.trim_start_matches("Plan: ").trim().to_string();
            }
            if goal_summary.len() > 200 {
                goal_summary = format!("{}...", &goal_summary[..197]);
            }
            i += 1;
            break;
        }
        if !line.is_empty() && !line.starts_with("---") {
            goal_summary = line.to_string();
            if goal_summary.len() > 200 {
                goal_summary = format!("{}...", &goal_summary[..197]);
            }
            i += 1;
            break;
        }
        i += 1;
    }
    if goal_summary.is_empty() {
        goal_summary = "Implementation plan".to_string();
    }

    // Find "## Implementation Steps" or "## Steps"
    while i < lines.len() {
        let line = lines[i].trim();
        let lower = line.to_lowercase();
        if (lower.starts_with("## implementation steps") || lower.starts_with("## steps"))
            && line.len() > 2
        {
            i += 1;
            while i < lines.len() {
                let step_line = lines[i].trim();
                if step_line.starts_with("## ") {
                    break;
                }
                // Numbered: "1. Title" or "1) Title" or "- Title" or "* Title"
                let title = if step_line.starts_with('-') {
                    step_line.trim_start_matches('-').trim().to_string()
                } else if step_line.starts_with('*') {
                    step_line.trim_start_matches('*').trim().to_string()
                } else if step_line.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                    // Skip leading digits then ". " or ") "
                    let rest = step_line
                        .trim_start_matches(|c: char| c.is_ascii_digit())
                        .trim_start_matches(|c: char| c == '.' || c == ')' || c == ' ' || c == '\t');
                    rest.to_string()
                } else {
                    i += 1;
                    continue;
                };
                if !title.is_empty() {
                    let desc = if i + 1 < lines.len() {
                        let next = lines[i + 1].trim();
                        if !next.is_empty()
                            && !next.starts_with('#')
                            && !next.starts_with('-')
                            && !next.starts_with('*')
                            && !next.chars().next().map_or(false, |c| c.is_ascii_digit())
                        {
                            i += 1;
                            next.to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };
                    steps.push(ParsedStep {
                        title: title.clone(),
                        description: desc,
                    });
                }
                i += 1;
            }
            break;
        }
        i += 1;
    }

    // Optional: "## Acceptance Criteria"
    let mut j = 0;
    while j < lines.len() {
        let lower = lines[j].trim().to_lowercase();
        if lower.starts_with("## acceptance criteria") {
            j += 1;
            let mut criteria_lines: Vec<&str> = Vec::new();
            while j < lines.len() && !lines[j].trim().starts_with("## ") {
                if !lines[j].trim().is_empty() {
                    criteria_lines.push(lines[j].trim());
                }
                j += 1;
            }
            if !criteria_lines.is_empty() {
                completion_criteria = Some(criteria_lines.join("\n"));
            }
            break;
        }
        j += 1;
    }

    Some(ParsedPlan {
        goal_summary,
        steps,
        completion_criteria,
    })
}

/// Trim trailing whitespace and limit consecutive blank lines to 2.
fn trim_excessive_blank_lines(markdown: &str) -> String {
    // First, trim trailing whitespace from the entire string
    let trimmed = markdown.trim_end();
    
    // Split into lines
    let lines: Vec<&str> = trimmed.lines().collect();
    
    // Build result, keeping at most 2 consecutive blank lines
    let mut result = String::new();
    let mut blank_count = 0;
    
    for line in &lines {
        if line.trim().is_empty() {
            blank_count += 1;
            // Only keep up to 2 consecutive blank lines
            if blank_count <= 2 {
                result.push('\n');
            }
        } else {
            blank_count = 0;
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(line);
        }
    }
    
    // Ensure exactly one trailing newline
    result.push('\n');
    result
}

pub fn emit_and_record(
    db: &Database,
    bus: &EventBus,
    category: &str,
    event_type: &str,
    run_id: Option<String>,
    payload: serde_json::Value,
) -> Result<BusEvent, String> {
    let event = bus.emit(category, event_type, run_id, payload);
    queries::insert_event(
        db,
        &queries::EventRow {
            id: event.id.clone(),
            run_id: event.run_id.clone(),
            seq: event.seq,
            category: event.category.clone(),
            event_type: event.event_type.clone(),
            payload_json: event.payload.to_string(),
            created_at: event.created_at.clone(),
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(event)
}
