use std::fs::OpenOptions;
use std::io::Write;

use chrono::Utc;
use uuid::Uuid;

use crate::db::queries;
use crate::runtime::planner::{emit_and_record, generate_plan_markdown_artifact};
use crate::{load_provider_config, load_workspace_root, AppError, AppState, ApprovalRequestView};

#[tauri::command]
pub fn get_latest_run(
    state: tauri::State<'_, AppState>,
    task_id: String,
) -> Result<Option<queries::RunRow>, AppError> {
    Ok(queries::get_latest_run_for_task(&state.db, &task_id)?)
}

#[tauri::command]
pub fn list_sub_agents(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<Vec<queries::SubAgentRow>, AppError> {
    Ok(queries::list_sub_agents_for_run(&state.db, &run_id)?)
}

#[tauri::command]
pub fn list_run_artifacts(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<Vec<queries::ArtifactRow>, AppError> {
    Ok(queries::list_artifacts_for_run(&state.db, &run_id)?)
}

#[tauri::command]
pub fn list_tool_calls(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<Vec<queries::ToolCallRow>, AppError> {
    Ok(queries::list_tool_calls_for_run(&state.db, &run_id)?)
}

#[tauri::command]
pub fn get_run(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<Option<queries::RunRow>, AppError> {
    Ok(queries::get_run(&state.db, &run_id)?)
}

#[tauri::command]
pub fn list_user_messages(
    state: tauri::State<'_, AppState>,
    task_id: String,
) -> Result<Vec<queries::UserMessageRow>, AppError> {
    Ok(queries::list_user_messages_for_task(&state.db, &task_id)?)
}

#[tauri::command]
pub async fn run_plan_mode(
    state: tauri::State<'_, AppState>,
    task_id: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<(), AppError> {
    let provider = provider.unwrap_or_else(|| "minimax".to_string()).to_ascii_lowercase();

    let task = queries::get_task(&state.db, &task_id)?
        .ok_or_else(|| AppError::Other(format!("task not found: {task_id}")))?;
    let cfg = load_provider_config(&state.db, &provider)?.ok_or_else(|| {
        AppError::Other(format!(
            "{provider} not configured. Set provider config in settings or env vars."
        ))
    })?;

    let effective_model = model.or(cfg.default_model.clone());
    let workspace_root = load_workspace_root(&state.db);

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
            plan_json: None,
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

    // Generate the plan markdown artifact for user review
    // This uses the PLAN mode system prompt which directs the AI to ONLY write markdown
    let _ = generate_plan_markdown_artifact(
        state.db.clone(),
        state.bus.clone(),
        task_id,
        task.prompt,
        provider,
        cfg.api_key,
        effective_model,
        cfg.base_url,
        workspace_root,
        Some(run_id),
        None,
    )
    .await
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
    let provider = provider.unwrap_or_else(|| "minimax".to_string()).to_ascii_lowercase();

    let task = queries::get_task(&state.db, &task_id)?
        .ok_or_else(|| AppError::Other(format!("task not found: {task_id}")))?;
    let cfg = load_provider_config(&state.db, &provider)?.ok_or_else(|| {
        AppError::Other(format!(
            "{provider} not configured. Set provider config in settings or env vars."
        ))
    })?;

    let effective_model = model.or(cfg.default_model.clone());

    state
        .orchestrator
        .approve_plan(task, provider, cfg.api_key, effective_model, cfg.base_url)
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

    let provider = provider.unwrap_or_else(|| "minimax".to_string()).to_ascii_lowercase();
    let task = queries::get_task(&state.db, &task_id)?
        .ok_or_else(|| AppError::Other(format!("task not found: {task_id}")))?;
    let cfg = load_provider_config(&state.db, &provider)?.ok_or_else(|| {
        AppError::Other(format!(
            "{provider} not configured. Set provider config in settings or env vars."
        ))
    })?;
    let effective_model = model.or(cfg.default_model.clone());

    let run = queries::get_latest_run_for_task(&state.db, &task_id)?
        .ok_or_else(|| AppError::Other("no run found for task".to_string()))?;
    if run.status != "awaiting_review" {
        return Err(AppError::Other(format!(
            "task is not awaiting review (current status: {})",
            run.status
        )));
    }

    let workspace_root = load_workspace_root(&state.db);
    let run_dir = workspace_root.join(".orchestrix").join("runs").join(&run.id);
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

    queries::update_task_status(&state.db, &task.id, "planning", &Utc::now().to_rfc3339())?;
    emit_and_record(
        &state.db,
        &state.bus,
        "task",
        "task.status_changed",
        Some(run.id.clone()),
        serde_json::json!({ "task_id": task.id, "status": "planning" }),
    )
    .map_err(AppError::Other)?;

    queries::update_run_status(&state.db, &run.id, "planning", None, None)?;

    let revised_prompt = format!(
        "{}\n\nPlan review feedback to incorporate before implementation:\n- {}\n\nRevise the plan so it addresses the review feedback. Return an updated plan.",
        task.prompt,
        note
    );

    let _ = generate_plan_markdown_artifact(
        state.db.clone(),
        state.bus.clone(),
        task.id.clone(),
        revised_prompt,
        provider,
        cfg.api_key,
        effective_model,
        cfg.base_url,
        workspace_root,
        Some(run.id.clone()),
        Some(note.to_string()),
    )
    .await
    .map_err(AppError::Other)?;

    queries::update_task_status(&state.db, &task.id, "awaiting_review", &Utc::now().to_rfc3339())?;
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

#[tauri::command]
pub fn list_pending_approvals(
    state: tauri::State<'_, AppState>,
    task_id: Option<String>,
) -> Result<Vec<ApprovalRequestView>, AppError> {
    let values = state
        .orchestrator
        .list_pending_approvals(task_id.as_deref())
        .into_iter()
        .map(ApprovalRequestView::from)
        .collect();
    Ok(values)
}

#[tauri::command]
pub fn resolve_approval_request(
    state: tauri::State<'_, AppState>,
    approval_id: String,
    approve: bool,
) -> Result<(), AppError> {
    let request = state
        .orchestrator
        .resolve_approval_request(&approval_id, approve)
        .map_err(AppError::Other)?;

    emit_and_record(
        &state.db,
        &state.bus,
        "tool",
        "tool.approval_user_decision",
        Some(request.run_id.clone()),
        serde_json::json!({
            "task_id": request.task_id,
            "sub_agent_id": request.sub_agent_id,
            "tool_call_id": request.tool_call_id,
            "approval_id": request.id,
            "approved": approve,
            "scope": request.scope,
        }),
    )
    .map_err(AppError::Other)?;

    Ok(())
}

#[tauri::command]
pub fn get_events_after(
    state: tauri::State<'_, AppState>,
    run_id: String,
    after_seq: i64,
) -> Result<Vec<queries::EventRow>, AppError> {
    Ok(queries::get_events_after_seq(&state.db, &run_id, after_seq)?)
}

#[tauri::command]
pub fn get_task_events(
    state: tauri::State<'_, AppState>,
    task_id: String,
) -> Result<Vec<queries::EventRow>, AppError> {
    // Get all events for all runs of this task (not just the latest run)
    // This ensures conversation history is preserved across follow-up messages
    Ok(queries::list_events_for_task(&state.db, &task_id)?)
}

#[tauri::command]
pub async fn send_message_to_task(
    state: tauri::State<'_, AppState>,
    task_id: String,
    message: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<(), AppError> {
    let message = message.trim();
    if message.is_empty() {
        return Err(AppError::Other("message cannot be empty".to_string()));
    }

    let provider = provider.unwrap_or_else(|| "minimax".to_string()).to_ascii_lowercase();

    let task = queries::get_task(&state.db, &task_id)?
        .ok_or_else(|| AppError::Other(format!("task not found: {task_id}")))?;

    // Only allow follow-up messages on completed, failed, or cancelled tasks
    if task.status != "completed" && task.status != "failed" && task.status != "cancelled" {
        return Err(AppError::Other(format!(
            "can only send follow-up messages to completed, failed, or cancelled tasks (current status: {})",
            task.status
        )));
    }

    let cfg = load_provider_config(&state.db, &provider)?.ok_or_else(|| {
        AppError::Other(format!(
            "{} not configured. Set provider config in settings or env vars.",
            provider
        ))
    })?;

    let effective_model = model.or(cfg.default_model.clone());

    // Create a new run for this continuation
    let run_id = Uuid::new_v4().to_string();
    queries::insert_run(
        &state.db,
        &queries::RunRow {
            id: run_id.clone(),
            task_id: task_id.clone(),
            status: "executing".to_string(),
            plan_json: None,
            started_at: Some(Utc::now().to_rfc3339()),
            finished_at: None,
            failure_reason: None,
        },
    )
    .map_err(|e| AppError::Other(e.to_string()))?;

    // Store the user message
    let message_id = Uuid::new_v4().to_string();
    queries::insert_user_message(
        &state.db,
        &queries::UserMessageRow {
            id: message_id.clone(),
            task_id: task_id.clone(),
            run_id: Some(run_id.clone()),
            content: message.to_string(),
            created_at: Utc::now().to_rfc3339(),
        },
    )
    .map_err(|e| AppError::Other(e.to_string()))?;

    // Emit event for the user message
    emit_and_record(
        &state.db,
        &state.bus,
        "user",
        "user.message_sent",
        Some(run_id.clone()),
        serde_json::json!({
            "task_id": task_id,
            "message_id": message_id,
            "content": message,
        }),
    )
    .map_err(AppError::Other)?;

    // Update task status to executing
    queries::update_task_status(&state.db, &task_id, "executing", &Utc::now().to_rfc3339())?;
    emit_and_record(
        &state.db,
        &state.bus,
        "task",
        "task.status_changed",
        Some(run_id.clone()),
        serde_json::json!({ "task_id": task_id, "status": "executing" }),
    )
    .map_err(AppError::Other)?;

    // Start the orchestrator with the follow-up message
    // Build context that includes the original prompt and the new message
    let continue_prompt = format!(
        "Previous task: {}\n\nFollow-up request: {}\n\nPlease continue from where we left off and address the follow-up request. Review the conversation history and previous artifacts if needed.",
        task.prompt,
        message
    );

    // Start the task in build mode (direct execution, no planning)
    state
        .orchestrator
        .continue_task_with_message(
            task,
            continue_prompt,
            provider,
            cfg.api_key,
            effective_model,
            cfg.base_url,
        )
        .map_err(AppError::Other)?;

    Ok(())
}
