//! Plan and build mode execution commands

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use chrono::Utc;
use uuid::Uuid;

use crate::core::agent_presets;
use crate::db::queries;
use crate::runtime::planner::{emit_and_record, generate_plan_markdown_artifact};
use crate::{load_provider_config, load_workspace_root, AppError, AppState};

pub(crate) struct ResolvedModelConfig {
    pub provider: String,
    pub cfg: crate::ProviderConfig,
    pub effective_model: Option<String>,
    pub agent_preset_meta: Option<serde_json::Value>,
}

pub(crate) fn resolve_provider_model_for_prompt(
    db: &crate::db::Database,
    workspace_root: &Path,
    task_prompt: &str,
    provider: Option<String>,
    model: Option<String>,
) -> Result<ResolvedModelConfig, AppError> {
    let mut resolved_provider = provider
        .unwrap_or_else(|| "minimax".to_string())
        .to_ascii_lowercase();
    let mut requested_model = model;
    let mut agent_preset_meta = None;

    if let Some(preset) = agent_presets::resolve_agent_preset_from_prompt(task_prompt, workspace_root) {
        agent_preset_meta = Some(serde_json::json!({
            "id": preset.id,
            "name": preset.name,
            "mode": preset.mode,
            "model": preset.model,
        }));

        if let Some(preset_model) = preset.model.as_deref() {
            let trimmed = preset_model.trim();
            if !trimmed.is_empty() {
                let (provider_override, model_override) =
                    agent_presets::parse_model_override(trimmed);
                if let Some(provider_override) = provider_override {
                    resolved_provider = provider_override;
                }
                requested_model = Some(model_override);
            }
        }
    }

    let cfg = load_provider_config(db, &resolved_provider)?.ok_or_else(|| {
        AppError::Other(format!(
            "{} not configured. Set provider config in settings or env vars.",
            resolved_provider
        ))
    })?;

    let effective_model = requested_model.or(cfg.default_model.clone());

    Ok(ResolvedModelConfig {
        provider: resolved_provider,
        cfg,
        effective_model,
        agent_preset_meta,
    })
}

pub(crate) fn build_run_context_json(
    mode: &str,
    provider: &str,
    model: Option<&str>,
    agent_preset_meta: Option<&serde_json::Value>,
) -> String {
    serde_json::json!({
        "metadata_version": 1,
        "mode": mode,
        "provider": provider,
        "model": model,
        "agent_preset": agent_preset_meta,
    })
    .to_string()
}

#[tauri::command]
pub async fn run_plan_mode(
    state: tauri::State<'_, AppState>,
    task_id: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<(), AppError> {
    let task = queries::get_task(&state.db, &task_id)?
        .ok_or_else(|| AppError::Other(format!("task not found: {task_id}")))?;
    let workspace_root = load_workspace_root(&state.db);

    let resolved = resolve_provider_model_for_prompt(
        &state.db,
        &workspace_root,
        &task.prompt,
        provider,
        model,
    )?;

    // Immediately update task status to planning and emit event
    // This ensures the UI refreshes to show the planning state
    queries::update_task_status(&state.db, &task_id, "planning", &Utc::now().to_rfc3339())?;

    // Create a temporary run to get the run_id for the event
    let run_id = Uuid::new_v4().to_string();
    queries::insert_run(
        &state.db,
        &queries::RunRow {
            id: run_id.clone(),
            task_id: task_id.clone(),
            status: "planning".to_string(),
            plan_json: Some(build_run_context_json(
                "plan",
                &resolved.provider,
                resolved.effective_model.as_deref(),
                resolved.agent_preset_meta.as_ref(),
            )),
            started_at: Some(Utc::now().to_rfc3339()),
            finished_at: None,
            failure_reason: None,
        },
    )?;

    emit_and_record(
        &state.db,
        &state.bus,
        "task",
        "task.status_changed",
        Some(run_id.clone()),
        serde_json::json!({ "task_id": task_id, "status": "planning" }),
    )
    .map_err(AppError::Other)?;

    // Generate the plan markdown artifact for user review.
    // Plan mode uses its own tool set (read-only + agent.create_artifact + agent.request_build_mode).
    let plan_mode_tools = state.orchestrator.tool_registry().list_for_plan_mode();
    let _ = generate_plan_markdown_artifact(
        state.db.clone(),
        state.bus.clone(),
        task_id.clone(),
        task.prompt,
        resolved.provider,
        resolved.cfg.api_key,
        resolved.effective_model,
        resolved.cfg.base_url,
        workspace_root.clone(),
        Some(run_id.clone()),
        None,
        plan_mode_tools,
        state.orchestrator.tool_registry().clone(),
        state.orchestrator.approval_gate().clone(),
    )
    .await
    .map_err(AppError::Other)?;

    // Transition task to awaiting_review so the UI shows the plan and Build button
    queries::update_task_status(
        &state.db,
        &task_id,
        "awaiting_review",
        &Utc::now().to_rfc3339(),
    )?;
    emit_and_record(
        &state.db,
        &state.bus,
        "task",
        "task.status_changed",
        Some(run_id),
        serde_json::json!({ "task_id": task_id, "status": "awaiting_review" }),
    )
    .map_err(AppError::Other)?;

    Ok(())
}

#[tauri::command]
pub fn run_build_mode(
    state: tauri::State<'_, AppState>,
    task_id: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<(), AppError> {
    let task = queries::get_task(&state.db, &task_id)?
        .ok_or_else(|| AppError::Other(format!("task not found: {task_id}")))?;
    let workspace_root = load_workspace_root(&state.db);
    let resolved = resolve_provider_model_for_prompt(
        &state.db,
        &workspace_root,
        &task.prompt,
        provider,
        model,
    )?;

    state
        .orchestrator
        .approve_plan(
            task,
            resolved.provider,
            resolved.cfg.api_key,
            resolved.effective_model,
            resolved.cfg.base_url,
        )
        .map_err(AppError::Other)?;
    Ok(())
}

#[tauri::command]
pub fn approve_plan(
    state: tauri::State<'_, AppState>,
    task_id: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<(), AppError> {
    run_build_mode(state, task_id, provider, model)
}

#[tauri::command]
pub async fn submit_plan_feedback(
    state: tauri::State<'_, AppState>,
    task_id: String,
    note: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<(), AppError> {
    let note = note.trim();
    if note.is_empty() {
        return Err(AppError::Other("feedback note cannot be empty".to_string()));
    }

    let task = queries::get_task(&state.db, &task_id)?
        .ok_or_else(|| AppError::Other(format!("task not found: {task_id}")))?;
    let workspace_root = load_workspace_root(&state.db);
    let resolved = resolve_provider_model_for_prompt(
        &state.db,
        &workspace_root,
        &task.prompt,
        provider,
        model,
    )?;

    let run = queries::get_latest_run_for_task(&state.db, &task_id)?
        .ok_or_else(|| AppError::Other("no run found for task".to_string()))?;
    if run.status != "awaiting_review" {
        return Err(AppError::Other(format!(
            "task is not awaiting review (current status: {})",
            run.status
        )));
    }

    let run_dir = workspace_root
        .join(".orchestrix")
        .join("runs")
        .join(&run.id);
    std::fs::create_dir_all(&run_dir)
        .map_err(|e| AppError::Other(format!("failed to create run dir: {e}")))?;

    let feedback_path = run_dir.join("plan-feedback.md");
    let created = !feedback_path.exists();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&feedback_path)
        .map_err(|e| AppError::Other(format!("failed to open feedback file: {e}")))?;

    if created {
        writeln!(file, "# Plan Feedback\n")
            .map_err(|e| AppError::Other(format!("failed to write feedback header: {e}")))?;
    }

    writeln!(
        file,
        "- {} - {}",
        Utc::now().to_rfc3339(),
        note.replace('\n', " ")
    )
    .map_err(|e| AppError::Other(format!("failed to append feedback: {e}")))?;

    if created {
        let artifact = queries::ArtifactRow {
            id: Uuid::new_v4().to_string(),
            run_id: run.id.clone(),
            kind: "plan_feedback".to_string(),
            uri_or_content: feedback_path.to_string_lossy().to_string(),
            metadata_json: Some(serde_json::json!({ "task_id": task_id }).to_string()),
            created_at: Utc::now().to_rfc3339(),
        };
        queries::insert_artifact(&state.db, &artifact)?;
        emit_and_record(
            &state.db,
            &state.bus,
            "artifact",
            "artifact.created",
            Some(run.id.clone()),
            serde_json::json!({
                "task_id": task_id,
                "artifact_id": artifact.id,
                "kind": artifact.kind,
                "uri": artifact.uri_or_content,
            }),
        )
        .map_err(AppError::Other)?;
    }

    emit_and_record(
        &state.db,
        &state.bus,
        "agent",
        "agent.plan_message",
        Some(run.id.clone()),
        serde_json::json!({
            "task_id": task_id,
            "content": format!("Plan review note recorded. Revising plan: {}", note),
        }),
    )
    .map_err(AppError::Other)?;

    queries::update_task_status(&state.db,
        &task.id,
        "planning",
        &Utc::now().to_rfc3339(),
    )?;
    emit_and_record(
        &state.db,
        &state.bus,
        "task",
        "task.status_changed",
        Some(run.id.clone()),
        serde_json::json!({ "task_id": task.id, "status": "planning" }),
    )
    .map_err(AppError::Other)?;

    queries::update_run_status(&state.db, &run.id, "planning", None, None
    )?;

    let revised_prompt = format!(
        "{}\n\nPlan review feedback to incorporate before implementation:\n- {}\n\nRevise the plan so it addresses the review feedback. Return an updated plan.",
        task.prompt, note
    );

    let plan_mode_tools = state.orchestrator.tool_registry().list_for_plan_mode();
    let _ = generate_plan_markdown_artifact(
        state.db.clone(),
        state.bus.clone(),
        task.id.clone(),
        revised_prompt,
        resolved.provider,
        resolved.cfg.api_key,
        resolved.effective_model,
        resolved.cfg.base_url,
        workspace_root.clone(),
        Some(run.id.clone()),
        Some(note.to_string()),
        plan_mode_tools,
        state.orchestrator.tool_registry().clone(),
        state.orchestrator.approval_gate().clone(),
    )
    .await
    .map_err(AppError::Other)?;

    queries::update_task_status(
        &state.db,
        &task.id,
        "awaiting_review",
        &Utc::now().to_rfc3339(),
    )?;
    emit_and_record(
        &state.db,
        &state.bus,
        "task",
        "task.status_changed",
        Some(run.id.clone()),
        serde_json::json!({ "task_id": task.id, "status": "awaiting_review" }),
    )
    .map_err(AppError::Other)?;

    emit_and_record(
        &state.db,
        &state.bus,
        "agent",
        "agent.plan_message",
        Some(run.id),
        serde_json::json!({
            "task_id": task.id,
            "content": "Plan updated from your feedback. Review the revised plan artifact, then press Build to execute.",
        }),
    )
    .map_err(AppError::Other)?;

    Ok(())
}
