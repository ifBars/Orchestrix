use super::*;

pub(super) async fn execute_sub_agent(
    db: &Database,
    bus: &EventBus,
    workspace_root: &std::path::Path,
    tool_registry: &ToolRegistry,
    worktree_manager: &WorktreeManager,
    approval_gate: &ApprovalGate,
    run_id: String,
    task_id: String,
    sub_agent: queries::SubAgentRow,
    step: PlanStep,
    model_config: Option<RuntimeModelConfig>,
    goal_summary: String,
    task_prompt: String,
    skills_context: String,
) -> SubAgentResult {
    let contract = parse_sub_agent_contract(sub_agent.context_json.as_deref());

    let worktree = match worktree_manager.create_worktree(workspace_root, &run_id, &sub_agent.id) {
        Ok(value) => value,
        Err(error) => {
            let _ = queries::update_sub_agent_status(
                db,
                &sub_agent.id,
                "failed",
                None,
                Some(&Utc::now().to_rfc3339()),
                Some(&error.to_string()),
            );
            return SubAgentResult {
                sub_agent_id: sub_agent.id,
                success: false,
                output_path: None,
                error: Some(error.to_string()),
                merge_message: None,
            };
        }
    };

    let _ = queries::insert_worktree_log(
        db,
        &queries::WorktreeLogRow {
            id: Uuid::new_v4().to_string(),
            run_id: run_id.clone(),
            sub_agent_id: sub_agent.id.clone(),
            strategy: worktree.strategy.to_string(),
            branch_name: worktree.branch.clone(),
            base_ref: worktree.base_ref.clone(),
            worktree_path: worktree.path.to_string_lossy().to_string(),
            merge_strategy: None,
            merge_success: None,
            merge_message: None,
            conflicted_files_json: None,
            created_at: Utc::now().to_rfc3339(),
            merged_at: None,
            cleaned_at: None,
        },
    );

    let _ = queries::mark_sub_agent_started(
        db,
        &sub_agent.id,
        Some(&worktree.path.to_string_lossy()),
        &Utc::now().to_rfc3339(),
    );

    let _ = emit_and_record(
        db,
        bus,
        "agent",
        "agent.subagent_started",
        Some(run_id.clone()),
        serde_json::json!({
            "task_id": task_id,
            "sub_agent_id": sub_agent.id,
            "step_idx": step.idx,
            "worktree_path": worktree.path,
            "strategy": worktree.strategy.to_string(),
            "branch": worktree.branch,
            "base_ref": worktree.base_ref,
        }),
    );

    let policy = if worktree.strategy == WorktreeStrategy::GitWorktree {
        PolicyEngine::with_approved_scopes(
            worktree.path.clone(),
            approval_gate.approved_scopes_handle(),
        )
    } else {
        PolicyEngine::with_approved_scopes(
            workspace_root.to_path_buf(),
            approval_gate.approved_scopes_handle(),
        )
    };

    let mut result: Result<String, String> = Err("no-attempt".to_string());
    let attempt_timeout_ms = contract.execution.attempt_timeout_ms.max(1_000);
    for attempt in 0..=step.max_retries {
        let _ = emit_and_record(
            db,
            bus,
            "agent",
            "agent.subagent_attempt",
            Some(run_id.clone()),
            serde_json::json!({
                "task_id": task_id,
                "sub_agent_id": sub_agent.id,
                "step_idx": step.idx,
                "attempt": attempt,
            }),
        );

        // Convert model config to worker format
        let worker_model_config =
            model_config
                .as_ref()
                .map(|c| super::worker::model::RuntimeModelConfig {
                    provider: c.provider.clone(),
                    api_key: c.api_key.clone(),
                    model: c.model.clone(),
                    base_url: c.base_url.clone(),
                });

        result = match timeout(
            Duration::from_millis(attempt_timeout_ms),
            super::worker::execute_step_with_tools(
                db,
                bus,
                tool_registry,
                worktree_manager,
                &policy,
                approval_gate,
                &run_id,
                &task_id,
                &sub_agent,
                &step,
                workspace_root,
                &worktree.path,
                worker_model_config,
                goal_summary.clone(),
                task_prompt.clone(),
                0,
                &skills_context,
            ),
        )
        .await
        {
            Ok(value) => value,
            Err(_) => Err("sub-agent attempt timed out".to_string()),
        };

        if result.is_ok() {
            break;
        }

        if attempt < step.max_retries {
            sleep(Duration::from_millis(
                SUB_AGENT_RETRY_BACKOFF_MS * (attempt as u64 + 1),
            ))
            .await;
        }
    }

    match result {
        Ok(output_path) => {
            let _ = queries::update_sub_agent_status(
                db,
                &sub_agent.id,
                "waiting_for_merge",
                Some(&worktree.path.to_string_lossy()),
                None,
                None,
            );
            let _ = emit_and_record(
                db,
                bus,
                "agent",
                "agent.subagent_waiting_for_merge",
                Some(run_id.clone()),
                serde_json::json!({
                    "task_id": task_id,
                    "sub_agent_id": sub_agent.id,
                    "step_idx": step.idx,
                    "output_path": output_path,
                }),
            );
            let _ = emit_and_record(
                db,
                bus,
                "agent",
                "agent.subagent_completed",
                Some(run_id),
                serde_json::json!({
                    "task_id": task_id,
                    "sub_agent_id": sub_agent.id,
                    "step_idx": step.idx,
                    "output_path": output_path,
                    "branch": worktree.branch,
                }),
            );
            SubAgentResult {
                sub_agent_id: sub_agent.id,
                success: true,
                output_path: Some(output_path),
                error: None,
                merge_message: None,
            }
        }
        Err(error) => {
            let _ = queries::update_sub_agent_status(
                db,
                &sub_agent.id,
                "failed",
                Some(&worktree.path.to_string_lossy()),
                Some(&Utc::now().to_rfc3339()),
                Some(&error),
            );
            let _ = emit_and_record(
                db,
                bus,
                "agent",
                "agent.subagent_failed",
                Some(run_id.clone()),
                serde_json::json!({
                    "task_id": task_id,
                    "sub_agent_id": sub_agent.id,
                    "step_idx": step.idx,
                    "error": error,
                }),
            );

            if contract.execution.close_on_completion {
                let _ = queries::update_sub_agent_status(
                    db,
                    &sub_agent.id,
                    "closed",
                    Some(&worktree.path.to_string_lossy()),
                    Some(&Utc::now().to_rfc3339()),
                    Some(&error),
                );
                let _ = emit_and_record(
                    db,
                    bus,
                    "agent",
                    "agent.subagent_closed",
                    Some(run_id.clone()),
                    serde_json::json!({
                        "task_id": task_id,
                        "sub_agent_id": sub_agent.id,
                        "step_idx": step.idx,
                        "final_status": "failed",
                        "close_reason": "execution_failed",
                    }),
                );
            }

            SubAgentResult {
                sub_agent_id: sub_agent.id,
                success: false,
                output_path: None,
                error: Some(error),
                merge_message: None,
            }
        }
    }
}
