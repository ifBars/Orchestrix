use super::*;

pub(super) async fn execute_step_with_tools(
    db: &Database,
    bus: &EventBus,
    tool_registry: &ToolRegistry,
    worktree_manager: &WorktreeManager,
    policy: &PolicyEngine,
    approval_gate: &ApprovalGate,
    run_id: &str,
    task_id: &str,
    sub_agent: &queries::SubAgentRow,
    step: &PlanStep,
    workspace_root: &std::path::Path,
    worktree_path: &std::path::Path,
    model_config: Option<RuntimeModelConfig>,
    goal_summary: String,
    task_prompt: String,
    delegation_depth: u32,
    skills_context: &str,
) -> Result<String, String> {
    let contract = parse_sub_agent_contract(sub_agent.context_json.as_deref());

    let orchestrix_dir = worktree_path.join(".orchestrix");
    std::fs::create_dir_all(&orchestrix_dir).map_err(|e| e.to_string())?;

    let context_path = orchestrix_dir.join("context.json");
    if let Some(content) = &sub_agent.context_json {
        std::fs::write(&context_path, content).map_err(|e| e.to_string())?;
    }

    // Use BUILD mode specific tools (excludes agent.request_build_mode, includes agent.request_plan_mode)
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
    enum WorkerModelClient {
        MiniMax(MiniMaxPlanner),
        Kimi(KimiPlanner),
    }

    impl WorkerModelClient {
        async fn decide(&self, req: WorkerActionRequest) -> Result<WorkerDecision, String> {
            match self {
                Self::MiniMax(model) => model.decide_worker_action(req).await.map_err(|e| e.to_string()),
                Self::Kimi(model) => model.decide_worker_action(req).await.map_err(|e| e.to_string()),
            }
        }
    }

    let worker_model = model_config.as_ref().map(|cfg| {
        if cfg.provider == "kimi" {
            WorkerModelClient::Kimi(KimiPlanner::new(
                cfg.api_key.clone(),
                cfg.model.clone(),
                cfg.base_url.clone(),
            ))
        } else {
            WorkerModelClient::MiniMax(MiniMaxPlanner::new_with_base_url(
                cfg.api_key.clone(),
                cfg.model.clone(),
                cfg.base_url.clone(),
            ))
        }
    });

    let mut observations: Vec<serde_json::Value> = Vec::new();
    let mut completion_summary: Option<String> = None;
    let mut turn: usize = 0;

    loop {
        turn += 1;
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
            WorkerDecision { action: fallback_action, reasoning: None }
        };

        // Emit model reasoning (chain-of-thought) as thinking if present
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

        let action = decision.action;

        match action {
            WorkerAction::Complete { summary } => {
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
                for call in calls {
                    let tool_name = call.tool_name;
                    let tool_args = call.tool_args;
                    let rationale = call.rationale;

                    if !available_tools.contains(&tool_name) {
                        observations.push(serde_json::json!({
                            "tool_name": tool_name,
                            "status": "denied",
                            "error": "tool not allowed by delegation contract",
                        }));
                        continue;
                    }

                    let tool_call_id = Uuid::new_v4().to_string();
                    let started_at = Utc::now().to_rfc3339();
                    queries::insert_tool_call(
                        db,
                        &queries::ToolCallRow {
                            id: tool_call_id.clone(),
                            run_id: run_id.to_string(),
                            step_idx: Some(step.idx as i64),
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
                            "sub_agent_id": sub_agent.id,
                            "tool_call_id": tool_call_id,
                            "tool_name": tool_name,
                            "tool_args": tool_args,
                            "step_idx": step.idx,
                            "turn": turn,
                            "rationale": rationale,
                        }),
                    );

                    let mut invocation = tool_registry.invoke(
                        policy,
                        worktree_path,
                        crate::tools::ToolCallInput {
                            name: tool_name.clone(),
                            args: tool_args.clone(),
                        },
                    );

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
                            &sub_agent.id,
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
                                "sub_agent_id": sub_agent.id,
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
                                "sub_agent_id": sub_agent.id,
                                "tool_call_id": tool_call_id,
                                "approval_id": request.id,
                                "approved": approved,
                            }),
                        );

                        if approved {
                            policy.allow_scope(scope);
                            invocation = tool_registry.invoke(
                                policy,
                                worktree_path,
                                crate::tools::ToolCallInput {
                                    name: tool_name.clone(),
                                    args: tool_args.clone(),
                                },
                            );
                        } else {
                            invocation =
                                Err(ToolError::PolicyDenied(format!("approval denied for scope: {scope}")));
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
                                    "sub_agent_id": sub_agent.id,
                                    "tool_call_id": tool_call_id,
                                    "status": if output.ok { "succeeded" } else { "failed" },
                                    "output": output.data,
                                }),
                            );

                            observations.push(serde_json::json!({
                                "tool_name": tool_name,
                                "status": if output.ok { "succeeded" } else { "failed" },
                                "output": output.data,
                            }));

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
                        }
                        Err(error) => {
                            queries::update_tool_call_result(
                                db,
                                &tool_call_id,
                                "denied",
                                None,
                                Some(&Utc::now().to_rfc3339()),
                                Some(&error.to_string()),
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
                                    "sub_agent_id": sub_agent.id,
                                    "tool_call_id": tool_call_id,
                                    "status": "denied",
                                    "error": error.to_string(),
                                }),
                            );

                            observations.push(serde_json::json!({
                                "tool_name": tool_name,
                                "status": "denied",
                                "error": error.to_string(),
                            }));
                        }
                    }
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
                if tool_name == "subagent.spawn" {
                    let objective = tool_args
                        .get("objective")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    if objective.is_empty() {
                        observations.push(serde_json::json!({
                            "tool_name": "subagent.spawn",
                            "status": "error",
                            "error": "objective is required",
                        }));
                        continue;
                    }

                    if !contract.permissions.can_spawn_children {
                        observations.push(serde_json::json!({
                            "tool_name": "subagent.spawn",
                            "status": "denied",
                            "error": "delegation disabled by contract",
                        }));
                        continue;
                    }

                    if delegation_depth >= contract.permissions.max_delegation_depth {
                        observations.push(serde_json::json!({
                            "tool_name": "subagent.spawn",
                            "status": "denied",
                            "error": "max delegation depth reached",
                        }));
                        continue;
                    }

                    let delegated_allowed_tools: Vec<String> = available_tools
                        .iter()
                        .filter(|name| name.as_str() != "subagent.spawn")
                        .cloned()
                        .collect();

                    let child = queries::SubAgentRow {
                        id: Uuid::new_v4().to_string(),
                        run_id: run_id.to_string(),
                        step_idx: step.idx as i64,
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
                        observations.push(serde_json::json!({
                            "tool_name": "subagent.spawn",
                            "status": "error",
                            "error": format!("failed to insert sub-agent: {}", error),
                        }));
                        continue;
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
                            "step_idx": step.idx,
                            "name": child.name,
                            "objective": objective,
                        }),
                    );

                    let delegated_step = PlanStep {
                        idx: step.idx,
                        title: format!("Delegated objective {}", turn),
                        description: tool_args
                            .get("objective")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_string(),
                        tool_intent: None,
                        status: StepStatus::Pending,
                        max_retries: tool_args
                            .get("max_retries")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0)
                            .clamp(0, 3) as u32,
                        result: None,
                    };

                    let mut child_result = Box::pin(super::sub_agent::execute_sub_agent(
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
                        model_config.clone(),
                        goal_summary.clone(),
                        task_prompt.clone(),
                        skills_context.to_string(),
                    ))
                    .await;

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
                                        "step_idx": step.idx,
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
                                        serde_json::to_string(&merge_result.conflicted_files)
                                            .unwrap_or_default(),
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
                                            "step_idx": step.idx,
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
                                        "step_idx": step.idx,
                                        "error": format!("merge error: {}", error),
                                    }),
                                );
                            }
                        }
                    }

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
                            "step_idx": step.idx,
                            "final_status": final_status,
                            "close_reason": close_reason,
                        }),
                    );

                    let _ = worktree_manager.remove_worktree(workspace_root, &child_result.sub_agent_id);
                    let _ = queries::update_worktree_log_cleaned(
                        db,
                        &child_result.sub_agent_id,
                        &Utc::now().to_rfc3339(),
                    );

                    if child_result.success {
                        if let Some(path) = child_result.output_path {
                            observations.push(serde_json::json!({
                                "tool_name": "subagent.spawn",
                                "status": "succeeded",
                                "sub_agent_id": child_result.sub_agent_id,
                                "output_path": path,
                                "merge": child_result.merge_message,
                            }));
                        } else {
                            observations.push(serde_json::json!({
                                "tool_name": "subagent.spawn",
                                "status": "failed",
                                "sub_agent_id": child_result.sub_agent_id,
                                "error": "missing child output path",
                            }));
                        }
                    } else {
                        observations.push(serde_json::json!({
                            "tool_name": "subagent.spawn",
                            "status": "failed",
                            "sub_agent_id": child_result.sub_agent_id,
                            "error": child_result.error,
                            "merge": child_result.merge_message,
                        }));
                    }

                    continue;
                }

                if !available_tools.contains(&tool_name) {
                    observations.push(serde_json::json!({
                        "tool_name": tool_name,
                        "status": "denied",
                        "error": "tool not allowed by delegation contract",
                    }));
                    continue;
                }

                let tool_call_id = Uuid::new_v4().to_string();
                let started_at = Utc::now().to_rfc3339();
                queries::insert_tool_call(
                    db,
                    &queries::ToolCallRow {
                        id: tool_call_id.clone(),
                        run_id: run_id.to_string(),
                        step_idx: Some(step.idx as i64),
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
                        "sub_agent_id": sub_agent.id,
                        "tool_call_id": tool_call_id,
                        "tool_name": tool_name,
                        "tool_args": tool_args,
                        "step_idx": step.idx,
                        "turn": turn,
                        "rationale": rationale,
                    }),
                );

                let mut invocation = tool_registry.invoke(
                    policy,
                    worktree_path,
                    crate::tools::ToolCallInput {
                        name: tool_name.clone(),
                        args: tool_args.clone(),
                    },
                );

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

                    let (request, receiver) =
                        approval_gate.request(task_id, run_id, &sub_agent.id, &tool_call_id, &tool_name, scope, reason);

                    let _ = emit_and_record(
                        db,
                        bus,
                        "tool",
                        "tool.approval_required",
                        Some(run_id.to_string()),
                        serde_json::json!({
                            "task_id": task_id,
                            "sub_agent_id": sub_agent.id,
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
                            "sub_agent_id": sub_agent.id,
                            "tool_call_id": tool_call_id,
                            "approval_id": request.id,
                            "approved": approved,
                        }),
                    );

                    if approved {
                        policy.allow_scope(scope);
                        invocation = tool_registry.invoke(
                            policy,
                            worktree_path,
                            crate::tools::ToolCallInput {
                                name: tool_name.clone(),
                                args: tool_args.clone(),
                            },
                        );
                    } else {
                        invocation =
                            Err(ToolError::PolicyDenied(format!("approval denied for scope: {scope}")));
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
                                "sub_agent_id": sub_agent.id,
                                "tool_call_id": tool_call_id,
                                "status": if output.ok { "succeeded" } else { "failed" },
                                "output": output.data,
                            }),
                        );

                        observations.push(serde_json::json!({
                            "tool_name": tool_name,
                            "status": if output.ok { "succeeded" } else { "failed" },
                            "output": output.data,
                        }));

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
                    }
                    Err(error) => {
                        queries::update_tool_call_result(
                            db,
                            &tool_call_id,
                            "denied",
                            None,
                            Some(&Utc::now().to_rfc3339()),
                            Some(&error.to_string()),
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
                                "sub_agent_id": sub_agent.id,
                                "tool_call_id": tool_call_id,
                                "status": "denied",
                                "error": error.to_string(),
                            }),
                        );

                        observations.push(serde_json::json!({
                            "tool_name": tool_name,
                            "status": "denied",
                            "error": error.to_string(),
                        }));
                    }
                }
            }
        }
    }

    let tool_summary = completion_summary.unwrap_or_else(|| {
        if observations.is_empty() {
            "No tool actions executed".to_string()
        } else {
            format!("Worker stopped without explicit completion. Final observation count: {}", observations.len())
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

fn open_todos_in_latest_todo_observation(observations: &[serde_json::Value]) -> Option<usize> {
    let last = observations.last()?;
    let tool_name = last.get("tool_name")?.as_str()?;
    if tool_name != "agent.todo" {
        return None;
    }

    let status = last.get("status")?.as_str()?;
    if status != "succeeded" {
        return None;
    }

    let todos = last.get("output")?.get("todos")?.as_array()?;
    let open = todos
        .iter()
        .filter(|todo| {
            let state = todo
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("pending");
            state != "completed" && state != "cancelled"
        })
        .count();

    Some(open)
}
