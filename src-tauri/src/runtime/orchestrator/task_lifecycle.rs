use super::*;
use crate::core::prompt_references::expand_prompt_references;

impl Orchestrator {
    /// Legacy: unified plan+build entry. Current flow uses run_plan_mode then run_build_mode separately.
    #[allow(dead_code)]
    pub fn start_task(
        &self,
        task: queries::TaskRow,
        provider: String,
        api_key: String,
        model: Option<String>,
        base_url: Option<String>,
    ) -> Result<(), String> {
        let task_id = task.id.clone();
        let task_id_key = task.id.clone();
        let orchestrator = self.clone();

        let handle = tauri::async_runtime::spawn(async move {
            let result = orchestrator
                .run_full_orchestration(task, provider, api_key, model, base_url)
                .await;
            if let Err(error) = result {
                let failed_at = Utc::now().to_rfc3339();
                let _ = queries::update_task_status(&orchestrator.db, &task_id, "failed", &failed_at);
                let _ = emit_and_record(
                    &orchestrator.db,
                    &orchestrator.bus,
                    "task",
                    "task.status_changed",
                    None,
                    serde_json::json!({ "task_id": task_id, "status": "failed", "error": error }),
                );
            }
            let mut guard = orchestrator.active.lock().expect("orchestrator mutex poisoned");
            guard.remove(&task_id);
        });

        let mut guard = self.active.lock().expect("orchestrator mutex poisoned");
        guard.insert(task_id_key, handle);
        Ok(())
    }

    pub fn cancel_task(&self, task_id: &str) {
        self.approval_gate.reject_all_for_task(task_id);
        let mut guard = self.active.lock().expect("orchestrator mutex poisoned");
        if let Some(handle) = guard.remove(task_id) {
            handle.abort();
        }
    }

    pub fn approve_plan(
        &self,
        task: queries::TaskRow,
        provider: String,
        api_key: String,
        model: Option<String>,
        base_url: Option<String>,
    ) -> Result<(), String> {
        let run = if let Some(existing) =
            queries::get_latest_run_for_task(&self.db, &task.id).map_err(|e| e.to_string())?
        {
            existing
        } else {
            let workspace_root = self.current_workspace_root();
            let agent_preset_meta = crate::core::agent_presets::resolve_agent_preset_from_prompt(
                &task.prompt,
                &workspace_root,
            )
            .map(|preset| {
                serde_json::json!({
                    "id": preset.id,
                    "name": preset.name,
                    "mode": preset.mode,
                    "model": preset.model,
                })
            });

            let run = queries::RunRow {
                id: Uuid::new_v4().to_string(),
                task_id: task.id.clone(),
                status: "executing".to_string(),
                plan_json: Some(
                    serde_json::json!({
                        "metadata_version": 1,
                        "mode": "build",
                        "provider": provider.clone(),
                        "model": model.clone(),
                        "agent_preset": agent_preset_meta,
                    })
                    .to_string(),
                ),
                started_at: Some(Utc::now().to_rfc3339()),
                finished_at: None,
                failure_reason: None,
            };
            queries::insert_run(&self.db, &run).map_err(|e| e.to_string())?;
            run
        };

        let artifacts =
            queries::list_markdown_artifacts_for_task(&self.db, &task.id).map_err(|e| e.to_string())?;

        let mut artifact_bundle = String::new();
        for artifact in artifacts {
            let path = std::path::PathBuf::from(&artifact.uri_or_content);
            if !path.exists() {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                artifact_bundle.push_str(&format!(
                    "\n\n---\nArtifact: {}\n\n{}",
                    artifact.uri_or_content, content
                ));
            }
        }
        let has_artifacts = !artifact_bundle.trim().is_empty();

        let run_uuid = Uuid::parse_str(&run.id).map_err(|e| format!("invalid run id: {e}"))?;
        let plan = Plan {
            id: Uuid::new_v4(),
            run_id: run_uuid,
            goal_summary: "Implement task using reviewed markdown artifacts".to_string(),
            steps: vec![PlanStep {
                idx: 0,
                title: "Implement from artifacts".to_string(),
                description: format!(
                    "Implement the task using the user prompt and all markdown artifacts as source-of-truth context when available.\n\nTask prompt:\n{}\n\nMarkdown artifacts:{}",
                    task.prompt,
                    if has_artifacts {
                        artifact_bundle
                    } else {
                        "\n(none found; implement directly from prompt)".to_string()
                    }
                ),
                tool_intent: None,
                status: StepStatus::Pending,
                max_retries: 1,
                result: None,
            }],
            completion_criteria: vec![
                "Implementation matches the markdown plan artifacts".to_string(),
                "Changes are applied and validated where possible".to_string(),
            ],
        };

        let task_id = task.id.clone();
        let task_id_key = task.id.clone();
        let run_id = run.id.clone();
        let task_prompt = task.prompt.clone();
        let orchestrator = self.clone();

        let handle = tauri::async_runtime::spawn(async move {
            let _ = emit_and_record(
                &orchestrator.db,
                &orchestrator.bus,
                "task",
                "task.review_approved",
                Some(run_id.clone()),
                serde_json::json!({ "task_id": task_id, "run_id": run_id }),
            );

            let result = orchestrator
                .execute_plan(
                    run_id,
                    task_id.clone(),
                    task_prompt,
                    plan,
                    Some(RuntimeModelConfig {
                        provider,
                        api_key,
                        model,
                        base_url,
                    }),
                )
                .await;

            if let Err(error) = result {
                let failed_at = Utc::now().to_rfc3339();
                let _ = queries::update_task_status(&orchestrator.db, &task_id, "failed", &failed_at);
                let _ = emit_and_record(
                    &orchestrator.db,
                    &orchestrator.bus,
                    "task",
                    "task.status_changed",
                    None,
                    serde_json::json!({ "task_id": task_id, "status": "failed", "error": error }),
                );
            }

            let mut guard = orchestrator.active.lock().expect("orchestrator mutex poisoned");
            guard.remove(&task_id);
        });

        let mut guard = self.active.lock().expect("orchestrator mutex poisoned");
        guard.insert(task_id_key, handle);
        Ok(())
    }

    pub fn continue_task_with_message(
        &self,
        task: queries::TaskRow,
        continue_prompt: String,
        provider: String,
        api_key: String,
        model: Option<String>,
        base_url: Option<String>,
    ) -> Result<(), String> {
        let run = if let Some(existing) =
            queries::get_latest_run_for_task(&self.db, &task.id).map_err(|e| e.to_string())?
        {
            existing
        } else {
            return Err("no run found for task".to_string());
        };

        let artifacts =
            queries::list_markdown_artifacts_for_task(&self.db, &task.id).map_err(|e| e.to_string())?;

        let mut artifact_bundle = String::new();
        for artifact in artifacts {
            let path = std::path::PathBuf::from(&artifact.uri_or_content);
            if !path.exists() {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                artifact_bundle.push_str(&format!(
                    "\n\n---\nArtifact: {}\n\n{}",
                    artifact.uri_or_content, content
                ));
            }
        }
        let has_artifacts = !artifact_bundle.trim().is_empty();

        let run_uuid = Uuid::parse_str(&run.id).map_err(|e| format!("invalid run id: {e}"))?;
        let plan = Plan {
            id: Uuid::new_v4(),
            run_id: run_uuid,
            goal_summary: "Continue task with follow-up request".to_string(),
            steps: vec![PlanStep {
                idx: 0,
                title: "Process follow-up request".to_string(),
                description: format!(
                    "Continue working on the task with the new follow-up message. Review previous work and artifacts if available.\n\nContinue prompt:\n{}\n\nPrevious artifacts:{}",
                    continue_prompt,
                    if has_artifacts {
                        artifact_bundle
                    } else {
                        "\n(none found)".to_string()
                    }
                ),
                tool_intent: None,
                status: StepStatus::Pending,
                max_retries: 1,
                result: None,
            }],
            completion_criteria: vec![
                "Follow-up request has been addressed".to_string(),
                "Changes are applied and validated".to_string(),
            ],
        };

        let task_id = task.id.clone();
        let task_id_key = task.id.clone();
        let run_id = run.id.clone();
        let orchestrator = self.clone();

        let handle = tauri::async_runtime::spawn(async move {
            let _ = emit_and_record(
                &orchestrator.db,
                &orchestrator.bus,
                "task",
                "task.continued",
                Some(run_id.clone()),
                serde_json::json!({ "task_id": task_id, "run_id": run_id }),
            );

            let result = orchestrator
                .execute_plan(
                    run_id,
                    task_id.clone(),
                    continue_prompt,
                    plan,
                    Some(RuntimeModelConfig {
                        provider,
                        api_key,
                        model,
                        base_url,
                    }),
                )
                .await;

            if let Err(error) = result {
                let failed_at = Utc::now().to_rfc3339();
                let _ = queries::update_task_status(&orchestrator.db, &task_id, "failed", &failed_at);
                let _ = emit_and_record(
                    &orchestrator.db,
                    &orchestrator.bus,
                    "task",
                    "task.status_changed",
                    None,
                    serde_json::json!({ "task_id": task_id, "status": "failed", "error": error }),
                );
            }

            let mut guard = orchestrator.active.lock().expect("orchestrator mutex poisoned");
            guard.remove(&task_id);
        });

        let mut guard = self.active.lock().expect("orchestrator mutex poisoned");
        guard.insert(task_id_key, handle);
        Ok(())
    }

    pub async fn recover_active_runs(&self) {
        let active_runs = match queries::get_active_runs(&self.db) {
            Ok(value) => value,
            Err(error) => {
                tracing::error!("failed to load active runs for recovery: {error}");
                return;
            }
        };

        for run in active_runs {
            let Some(task) = (match queries::get_task(&self.db, &run.task_id) {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!("failed to load task {}: {}", run.task_id, error);
                    None
                }
            }) else {
                continue;
            };

            if run.status == "planning" {
                let _ = queries::update_run_status(
                    &self.db,
                    &run.id,
                    "failed",
                    Some(&Utc::now().to_rfc3339()),
                    Some("recovery: planning interrupted before plan persisted"),
                );
                let _ =
                    queries::update_task_status(&self.db, &task.id, "failed", &Utc::now().to_rfc3339());
                continue;
            }

            if run.status == "executing" {
                let Some(plan_json) = run.plan_json.as_ref() else {
                    continue;
                };

                let Ok(plan) = serde_json::from_str::<Plan>(plan_json) else {
                    continue;
                };

                let _ = emit_and_record(
                    &self.db,
                    &self.bus,
                    "task",
                    "task.resumed",
                    Some(run.id.clone()),
                    serde_json::json!({ "task_id": task.id, "run_id": run.id }),
                );

                let _ = self.execute_plan(run.id, task.id, task.prompt, plan, None).await;
            }
        }
    }

    #[allow(dead_code)]
    async fn run_full_orchestration(
        &self,
        task: queries::TaskRow,
        provider: String,
        api_key: String,
        model: Option<String>,
        base_url: Option<String>,
    ) -> Result<(), String> {
        let run = queries::RunRow {
            id: Uuid::new_v4().to_string(),
            task_id: task.id.clone(),
            status: "executing".to_string(),
            plan_json: None,
            started_at: Some(Utc::now().to_rfc3339()),
            finished_at: None,
            failure_reason: None,
        };
        queries::insert_run(&self.db, &run).map_err(|e| e.to_string())?;

        let run_uuid = Uuid::parse_str(&run.id).map_err(|e| format!("invalid run id: {e}"))?;
        let plan = Plan {
            id: Uuid::new_v4(),
            run_id: run_uuid,
            goal_summary: "Autonomous conversational execution".to_string(),
            steps: vec![PlanStep {
                idx: 0,
                title: "Autonomous execution".to_string(),
                description: task.prompt.clone(),
                tool_intent: None,
                status: StepStatus::Pending,
                max_retries: 0,
                result: None,
            }],
            completion_criteria: vec!["Worker completes autonomously".to_string()],
        };

        self.execute_plan(
            run.id,
            task.id,
            task.prompt,
            plan,
            Some(RuntimeModelConfig {
                provider,
                api_key,
                model,
                base_url,
            }),
        )
        .await
    }

    async fn execute_plan(
        &self,
        run_id: String,
        task_id: String,
        task_prompt: String,
        plan: Plan,
        model_config: Option<RuntimeModelConfig>,
    ) -> Result<(), String> {
        queries::update_task_status(&self.db, &task_id, "executing", &Utc::now().to_rfc3339())
            .map_err(|e| e.to_string())?;

        emit_and_record(
            &self.db,
            &self.bus,
            "task",
            "task.status_changed",
            Some(run_id.clone()),
            serde_json::json!({ "task_id": task_id, "status": "executing" }),
        )?;

        let workspace_root = self.current_workspace_root();
        let resolved_task_prompt = expand_prompt_references(&task_prompt, &workspace_root);
        let policy = PolicyEngine::with_approved_scopes(
            workspace_root.clone(),
            self.approval_gate.approved_scopes_handle(),
        );

        // Load workspace skills and build context for agent injection
        let workspace_skills = crate::core::workspace_skills::scan_workspace_skills(&workspace_root);
        let skills_context = crate::core::workspace_skills::build_skills_context(&workspace_skills);

        let checkpoint = queries::get_checkpoint(&self.db, &run_id).map_err(|e| e.to_string())?;
        let mut failed: Vec<SubAgentResult> = Vec::new();

        for step in &plan.steps {
            if let Some(cp) = checkpoint.as_ref() {
                if step.idx as i64 <= cp.last_step_idx {
                    continue;
                }
            }

            let virtual_parent = queries::SubAgentRow {
                id: format!("parent-{}-step-{}", run_id, step.idx),
                run_id: run_id.clone(),
                step_idx: step.idx as i64,
                name: format!("parent-step-{}", step.idx),
                status: "running".to_string(),
                worktree_path: Some(workspace_root.to_string_lossy().to_string()),
                context_json: Some(
                    serde_json::json!({
                        "task_prompt": resolved_task_prompt.clone(),
                        "goal_summary": &plan.goal_summary,
                        "step": step,
                        "contract": {
                            "permissions": {
                                "allowed_tools": self.tool_registry.list().into_iter().map(|tool| tool.name).collect::<Vec<String>>(),
                                "can_spawn_children": true,
                                "max_delegation_depth": 1,
                            },
                            "execution": {
                                "attempt_timeout_ms": SUB_AGENT_ATTEMPT_TIMEOUT_SECS * 1000,
                                "close_on_completion": true,
                            }
                        }
                    })
                    .to_string(),
                ),
                started_at: Some(Utc::now().to_rfc3339()),
                finished_at: None,
                error: None,
            };

            // Convert model config to worker format
            let worker_model_config = model_config.as_ref().map(|c| {
                super::worker::model::RuntimeModelConfig {
                    provider: c.provider.clone(),
                    api_key: c.api_key.clone(),
                    model: c.model.clone(),
                    base_url: c.base_url.clone(),
                }
            });

            let step_result = super::worker::execute_step_with_tools(
                &self.db,
                &self.bus,
                &self.tool_registry,
                &self.worktree_manager,
                &policy,
                &self.approval_gate,
                &run_id,
                &task_id,
                &virtual_parent,
                step,
                &workspace_root,
                &workspace_root,
                worker_model_config,
                plan.goal_summary.clone(),
                resolved_task_prompt.clone(),
                0,
                &skills_context,
            )
            .await;

            match step_result {
                Ok(_) => {
                    let _ = queries::upsert_checkpoint(
                        &self.db,
                        &queries::CheckpointRow {
                            run_id: run_id.clone(),
                            last_step_idx: step.idx as i64,
                            runtime_state_json: Some(
                                serde_json::json!({
                                    "status": "executing",
                                    "step_idx": step.idx,
                                })
                                .to_string(),
                            ),
                            updated_at: Utc::now().to_rfc3339(),
                        },
                    );
                }
                Err(error) => {
                    failed.push(SubAgentResult {
                        sub_agent_id: virtual_parent.id,
                        success: false,
                        output_path: None,
                        error: Some(error),
                        merge_message: None,
                    });
                    break;
                }
            }
        }

        if failed.is_empty() {
            queries::update_run_status(
                &self.db,
                &run_id,
                "completed",
                Some(&Utc::now().to_rfc3339()),
                None,
            )
            .map_err(|e| e.to_string())?;
            queries::update_task_status(&self.db, &task_id, "completed", &Utc::now().to_rfc3339())
                .map_err(|e| e.to_string())?;

            emit_and_record(
                &self.db,
                &self.bus,
                "task",
                "task.status_changed",
                Some(run_id),
                serde_json::json!({ "task_id": task_id, "status": "completed" }),
            )?;
            return Ok(());
        }

        let failures_json: Vec<serde_json::Value> = failed
            .iter()
            .map(|value| {
                serde_json::json!({
                    "sub_agent_id": value.sub_agent_id,
                    "error": value.error,
                    "output_path": value.output_path,
                })
            })
            .collect();

        let failure_reason = format!("{} sub-agent(s) failed", failed.len());

        queries::update_run_status(
            &self.db,
            &run_id,
            "failed",
            Some(&Utc::now().to_rfc3339()),
            Some(&failure_reason),
        )
        .map_err(|e| e.to_string())?;
        queries::update_task_status(&self.db, &task_id, "failed", &Utc::now().to_rfc3339())
            .map_err(|e| e.to_string())?;

        emit_and_record(
            &self.db,
            &self.bus,
            "task",
            "task.status_changed",
            Some(run_id),
            serde_json::json!({ "task_id": task_id, "status": "failed", "failures": failures_json }),
        )?;

        Err(failure_reason)
    }
}
