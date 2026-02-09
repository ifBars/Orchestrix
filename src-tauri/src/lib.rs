//! Orchestrator backend library.
//!
//! This is the main entry point for the Tauri application backend. It handles:
//! - Application initialization and state management
//! - Tauri command registration and IPC handling
//! - Database setup and migration
//! - Event system initialization
//! - Provider configuration management
//!
//! # Architecture
//!
//! The backend follows a modular architecture:
//! - `commands`: Tauri command handlers (IPC entry points)
//! - `runtime`: Task orchestration, planning, and execution
//! - `db`: Database layer with SQLite
//! - `bus`: Event bus for real-time communication
//! - `tools`: Tool registry and implementations
//! - `model`: LLM API clients (MiniMax, Kimi)
//! - `policy`: Permission and sandboxing engine
//! - `core`: Shared types and utilities

mod bus;
mod commands;
mod core;
mod db;
mod model;
mod policy;
mod runtime;
mod tools;

#[cfg(test)]
mod testing;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use bus::{EventBatcher, EventBus};
use db::queries;
use db::Database;
use runtime::approval::ApprovalRequest;
use runtime::orchestrator::Orchestrator;

// ---------------------------------------------------------------------------
// Shared error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub(crate) enum AppError {
    #[error("{0}")]
    Db(#[from] db::DbError),
    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderConfig {
    pub api_key: String,
    pub default_model: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProviderConfigView {
    pub provider: String,
    pub configured: bool,
    pub default_model: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ModelCatalogEntry {
    pub provider: String,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WorkspaceRootView {
    pub workspace_root: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ArtifactContentView {
    pub path: String,
    pub content: String,
    pub is_markdown: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateTaskOptions {
    pub parent_task_id: Option<String>,
    pub reference_task_ids: Option<Vec<String>>,
}

pub(crate) struct AppState {
    pub db: Arc<Database>,
    pub bus: Arc<EventBus>,
    pub orchestrator: Arc<Orchestrator>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApprovalRequestView {
    pub id: String,
    pub task_id: String,
    pub run_id: String,
    pub sub_agent_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub scope: String,
    pub reason: String,
    pub created_at: String,
}

impl From<ApprovalRequest> for ApprovalRequestView {
    fn from(value: ApprovalRequest) -> Self {
        Self {
            id: value.id,
            task_id: value.task_id,
            run_id: value.run_id,
            sub_agent_id: value.sub_agent_id,
            tool_call_id: value.tool_call_id,
            tool_name: value.tool_name,
            scope: value.scope,
            reason: value.reason,
            created_at: value.created_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helper functions
// ---------------------------------------------------------------------------

pub(crate) fn provider_setting_key(provider: &str) -> String {
    format!("provider_config:{provider}")
}

pub(crate) fn default_model_for_provider(provider: &str) -> &'static str {
    match provider {
        "kimi" => "kimi-for-coding",
        _ => "MiniMax-M2.1",
    }
}

fn env_for_provider(provider: &str) -> (Option<String>, Option<String>, Option<String>) {
    match provider {
        "kimi" => (
            std::env::var("KIMI_API_KEY").ok(),
            std::env::var("KIMI_MODEL").ok(),
            std::env::var("KIMI_BASE_URL").ok(),
        ),
        _ => (
            std::env::var("MINIMAX_API_KEY").ok(),
            std::env::var("MINIMAX_MODEL").ok(),
            std::env::var("MINIMAX_BASE_URL").ok(),
        ),
    }
}

pub(crate) fn load_provider_config(db: &Database, provider: &str) -> Result<Option<ProviderConfig>, AppError> {
    let (env_key, env_model, env_base_url) = env_for_provider(provider);
    if let Some(api_key) = env_key {
        if !api_key.trim().is_empty() {
            return Ok(Some(ProviderConfig {
                api_key,
                default_model: env_model,
                base_url: env_base_url,
            }));
        }
    }

    let raw = queries::get_setting(db, &provider_setting_key(provider))?;
    if let Some(raw) = raw {
        let cfg: ProviderConfig = serde_json::from_str(&raw)
            .map_err(|e| AppError::Other(format!("invalid {provider} provider config: {e}")))?;
        return Ok(Some(cfg));
    }

    // Backward compatibility with previous minimax key.
    if provider == "minimax" {
        if let Some(raw) = queries::get_setting(db, "minimax_config")? {
            #[derive(Deserialize)]
            struct LegacyMiniMax {
                api_key: String,
                model: Option<String>,
            }
            if let Ok(legacy) = serde_json::from_str::<LegacyMiniMax>(&raw) {
                return Ok(Some(ProviderConfig {
                    api_key: legacy.api_key,
                    default_model: legacy.model,
                    base_url: None,
                }));
            }
        }
    }

    Ok(None)
}

fn default_workspace_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.ends_with("src-tauri") {
        cwd.parent().unwrap_or(&cwd).to_path_buf()
    } else {
        cwd
    }
}

fn orchestrix_data_dir() -> PathBuf {
    if let Ok(path) = std::env::var("ORCHESTRIX_DATA_DIR") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            return PathBuf::from(app_data).join("Orchestrix");
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".orchestrix");
    }

    if let Ok(home) = std::env::var("USERPROFILE") {
        return PathBuf::from(home).join(".orchestrix");
    }

    PathBuf::from(".orchestrix")
}

fn stable_db_path() -> Result<PathBuf, String> {
    let data_dir = orchestrix_data_dir();
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("failed to create app data directory {}: {e}", data_dir.display()))?;

    let db_path = data_dir.join("orchestrix.db");
    if db_path.exists() {
        return Ok(db_path);
    }

    let legacy_candidates = [
        PathBuf::from("orchestrix.db"),
        PathBuf::from("../orchestrix.db"),
        default_workspace_root().join("orchestrix.db"),
    ];

    for candidate in legacy_candidates {
        if candidate == db_path || !candidate.exists() {
            continue;
        }
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!("failed to create database parent directory {}: {e}", parent.display())
            })?;
        }
        std::fs::copy(&candidate, &db_path).map_err(|e| {
            format!(
                "failed to migrate legacy db from {} to {}: {e}",
                candidate.display(),
                db_path.display()
            )
        })?;
        tracing::info!(
            "migrated legacy database from {} to {}",
            candidate.display(),
            db_path.display()
        );
        break;
    }

    Ok(db_path)
}

pub(crate) fn load_workspace_root(db: &Database) -> PathBuf {
    let Some(raw) = queries::get_setting(db, "workspace_root").ok().flatten() else {
        return default_workspace_root();
    };

    let parsed = PathBuf::from(raw);
    if parsed.exists() {
        parsed
    } else {
        default_workspace_root()
    }
}

// ---------------------------------------------------------------------------
// Application entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "orchestrix=debug,info".parse().expect("valid env filter")),
        )
        .init();

    let db_path = stable_db_path().expect("failed to resolve stable database path");
    let db = Arc::new(Database::open(&db_path).expect("failed to open database"));
    let bus = Arc::new(EventBus::new());
    let workspace_root = load_workspace_root(&db);
    let orchestrator = Arc::new(Orchestrator::new(db.clone(), bus.clone(), workspace_root));

    // Best-effort MCP tool discovery cache refresh at startup.
    let _ = core::mcp::refresh_mcp_tools_cache();

    let state = AppState {
        db: db.clone(),
        bus: bus.clone(),
        orchestrator: orchestrator.clone(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            // tasks
            commands::tasks::create_task,
            commands::tasks::list_tasks,
            commands::tasks::list_task_links,
            commands::tasks::link_tasks,
            commands::tasks::unlink_tasks,
            commands::tasks::delete_task,
            commands::tasks::get_task,
            commands::tasks::start_task,
            commands::tasks::cancel_task,
            // runs
            commands::runs::get_latest_run,
            commands::runs::get_run,
            commands::runs::list_sub_agents,
            commands::runs::list_run_artifacts,
            commands::runs::list_tool_calls,
            commands::runs::list_user_messages,
            commands::runs::run_plan_mode,
            commands::runs::run_build_mode,
            commands::runs::approve_plan,
            commands::runs::submit_plan_feedback,
            commands::runs::list_pending_approvals,
            commands::runs::resolve_approval_request,
            commands::runs::get_events_after,
            commands::runs::get_task_events,
            commands::runs::send_message_to_task,
            // providers
            commands::providers::set_provider_config,
            commands::providers::get_provider_configs,
            commands::providers::get_model_catalog,
            // mcp
            commands::mcp::list_mcp_server_configs,
            commands::mcp::upsert_mcp_server_config,
            commands::mcp::remove_mcp_server_config,
            commands::mcp::refresh_mcp_tools,
            commands::mcp::list_cached_mcp_tools,
            commands::mcp::call_mcp_tool,
            // skills
            commands::skills::list_available_skills,
            commands::skills::search_skills,
            commands::skills::add_custom_skill,
            commands::skills::remove_custom_skill,
            commands::skills::import_context7_skill,
            commands::skills::import_vercel_skill,
            // workspace
            commands::workspace::set_workspace_root,
            commands::workspace::get_workspace_root,
            commands::workspace::search_workspace_references,
            commands::workspace::read_artifact_content,
            commands::workspace::open_local_path,
            // worktrees
            commands::worktrees::list_active_worktrees,
            commands::worktrees::list_run_worktrees,
            commands::worktrees::list_worktree_logs,
            commands::worktrees::cleanup_run_worktrees,
            commands::worktrees::prune_stale_worktrees,
            commands::worktrees::list_git_worktrees,
            // workspace skills
            commands::workspace_skills::list_workspace_skills,
            commands::workspace_skills::get_workspace_skill_content,
            commands::workspace_skills::read_workspace_skill_file,
            commands::workspace_skills::get_active_skills_context,
        ])
        .setup(move |app| {
            let rx = bus.subscribe();
            let handle = app.handle().clone();
            EventBatcher::start(rx, handle);

            let orchestrator_for_recovery = orchestrator.clone();
            tauri::async_runtime::spawn(async move {
                runtime::recovery::recover(orchestrator_for_recovery.as_ref()).await;
            });

            tracing::info!("Orchestrix started");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
