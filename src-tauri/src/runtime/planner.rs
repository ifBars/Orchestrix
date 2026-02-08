use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::bus::{BusEvent, EventBus};
use crate::db::{queries, Database};
use crate::model::{kimi::KimiPlanner, minimax::MiniMaxPlanner, PlannerModel};

#[derive(Debug, Clone)]
pub struct PlanningOutcome {
    pub run_id: String,
    pub artifact_path: String,
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
            "content": "Got it. I am drafting a plan and will attach it as an artifact for your review.",
        }),
    )?;

    let planner_model: String;

    let existing_markdown = collect_existing_markdown(&db, &task_id);
    let context = if let Some(note) = revision_note.as_ref() {
        format!(
            "{}\n\nReviewer feedback to incorporate:\n- {}",
            existing_markdown,
            note
        )
    } else {
        existing_markdown
    };

    let markdown: String;

    if provider == "kimi" {
        let planner = KimiPlanner::new(api_key, model, base_url);
        planner_model = planner.model_id().to_string();
        markdown = planner
            .generate_plan_markdown(&prompt, &context)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        let planner = MiniMaxPlanner::new_with_base_url(api_key, model, base_url);
        planner_model = planner.model_id().to_string();
        markdown = planner
            .generate_plan_markdown(&prompt, &context)
            .await
            .map_err(|e| e.to_string())?;
    }

    queries::update_run_status_and_plan(
        &db,
        &run_id,
        "awaiting_review",
        None,
        None,
        None,
    )
    .map_err(|e| e.to_string())?;

    // Trim trailing whitespace and excessive blank lines from the markdown
    let trimmed_markdown = trim_excessive_blank_lines(&markdown);
    
    let plan_artifact_path = write_plan_artifact(
        &db,
        &bus,
        &run_id,
        &task_id,
        &workspace_root,
        &planner_model,
        &trimmed_markdown,
    )?;

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
) -> Result<String, String> {
    let run_dir = workspace_root.join(".orchestrix").join("runs").join(run_id);
    std::fs::create_dir_all(&run_dir).map_err(|e| e.to_string())?;

    let artifact_path = run_dir.join("plan.md");
    std::fs::write(&artifact_path, markdown).map_err(|e| e.to_string())?;

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
