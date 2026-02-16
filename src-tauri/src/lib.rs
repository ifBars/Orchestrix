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

pub mod bench;
mod bus;
mod commands;
mod core;
mod db;
pub mod embeddings;
pub mod mcp;
mod model;
mod policy;
mod runtime;
mod tools;

// Test modules
#[cfg(test)]
mod tests;

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
pub(crate) struct ModelInfo {
    pub name: String,
    pub context_window: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ModelCatalogEntry {
    pub provider: String,
    pub models: Vec<ModelInfo>,
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
    pub mcp_manager: Arc<mcp::McpClientManager>,
    pub embedding_manager: Arc<embeddings::EmbeddingManager>,
    pub embedding_index_service: Arc<embeddings::SemanticIndexService>,
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

fn env_for_provider(provider: &str) -> (Option<String>, Option<String>, Option<String>) {
    match provider {
        "kimi" => (
            std::env::var("KIMI_API_KEY").ok(),
            std::env::var("KIMI_MODEL").ok(),
            std::env::var("KIMI_BASE_URL").ok(),
        ),
        "zhipu" => (
            std::env::var("ZHIPU_API_KEY").ok(),
            std::env::var("ZHIPU_MODEL").ok(),
            std::env::var("ZHIPU_BASE_URL").ok(),
        ),
        "modal" => (
            std::env::var("MODAL_API_KEY").ok(),
            std::env::var("MODAL_MODEL").ok(),
            std::env::var("MODAL_BASE_URL").ok(),
        ),
        "minimax" => (
            std::env::var("MINIMAX_API_KEY").ok(),
            std::env::var("MINIMAX_MODEL").ok(),
            std::env::var("MINIMAX_BASE_URL").ok(),
        ),
        _ => (
            std::env::var("MINIMAX_API_KEY").ok(),
            std::env::var("MINIMAX_MODEL").ok(),
            std::env::var("MINIMAX_BASE_URL").ok(),
        ),
    }
}

pub(crate) fn load_provider_config(
    db: &Database,
    provider: &str,
) -> Result<Option<ProviderConfig>, AppError> {
    // Check database first - this is the authoritative source for UI
    let raw = queries::get_setting(db, &provider_setting_key(provider))?;
    if let Some(raw) = raw {
        let cfg: ProviderConfig = serde_json::from_str(&raw)
            .map_err(|e| AppError::Other(format!("invalid {provider} provider config: {e}")))?;
        return Ok(Some(cfg));
    }

    // Fall back to environment variables only if no DB config exists
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
    std::fs::create_dir_all(&data_dir).map_err(|e| {
        format!(
            "failed to create app data directory {}: {e}",
            data_dir.display()
        )
    })?;

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
                format!(
                    "failed to create database parent directory {}: {e}",
                    parent.display()
                )
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
    let embedding_manager = Arc::new(embeddings::EmbeddingManager::new(db.clone()));
    let embedding_index_service =
        embeddings::SemanticIndexService::new(db.clone(), bus.clone(), embedding_manager.clone());
    tools::set_semantic_index_service(embedding_index_service.clone());

    // Initialize enhanced MCP client manager
    let mcp_manager = tauri::async_runtime::block_on(async {
        // Migrate legacy config if present
        let _ = mcp::migrate_legacy_config().await;

        // Create and initialize the manager
        let mut manager = mcp::McpClientManager::new()
            .await
            .expect("failed to create MCP manager");

        // Set up event emitter to forward MCP events to the event bus
        let bus_for_events = bus.clone();
        manager.set_event_emitter(move |event| {
            let payload = event.to_payload();
            let event_type = event.event_type();
            let category = event.category().to_string();

            // Emit to the event bus
            let _ = bus_for_events.emit(&category, &event_type, None, payload);
        });

        Arc::new(manager)
    });

    let state = AppState {
        db: db.clone(),
        bus: bus.clone(),
        orchestrator: orchestrator.clone(),
        mcp_manager: mcp_manager.clone(),
        embedding_manager,
        embedding_index_service: embedding_index_service.clone(),
    };

    if embeddings::is_semantic_search_configured(&db) {
        embedding_index_service.ensure_workspace_index_started(load_workspace_root(&db));
    }

    // Initialize MCP in the background so a slow/unhealthy server cannot block app startup.
    let mcp_manager_for_init = mcp_manager.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = mcp_manager_for_init.initialize().await {
            tracing::warn!("MCP manager initialization partially failed: {}", e);
        }
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            // tasks
            commands::tasks::create_task,
            commands::tasks::fork_task,
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
            commands::runs::get_events_after,
            commands::runs::get_task_events,
            // execution
            commands::execution::run_plan_mode,
            commands::execution::run_build_mode,
            commands::execution::approve_plan,
            commands::execution::submit_plan_feedback,
            // approvals
            commands::approvals::list_pending_approvals,
            commands::approvals::resolve_approval_request,
            // messages
            commands::messages::send_message_to_task,
            commands::messages::get_compaction_settings,
            commands::messages::set_compaction_settings,
            commands::messages::get_conversation_summary,
            // plan mode settings
            commands::plan_mode::get_plan_mode_settings,
            commands::plan_mode::set_plan_mode_settings,
            commands::plan_mode::get_plan_mode_max_tokens_command,
            // providers
            commands::providers::set_provider_config,
            commands::providers::remove_provider_config,
            commands::providers::get_provider_configs,
            commands::providers::get_model_catalog,
            commands::providers::get_context_window_for_model,
            // embeddings
            commands::embeddings::get_embedding_config,
            commands::embeddings::set_embedding_config,
            commands::embeddings::get_embedding_provider_info,
            commands::embeddings::embedding_dims,
            commands::embeddings::embed_texts,
            commands::embeddings::get_embedding_index_status,
            // mcp
            commands::mcp::list_mcp_servers,
            commands::mcp::get_mcp_server,
            commands::mcp::upsert_mcp_server,
            commands::mcp::remove_mcp_server,
            commands::mcp::refresh_mcp_tools_cache,
            commands::mcp::list_mcp_tools,
            commands::mcp::call_mcp_tool,
            commands::mcp::test_mcp_server_connection,
            commands::mcp::get_mcp_statistics,
            commands::mcp::migrate_mcp_config,
            // mcp resources
            commands::mcp::list_mcp_resources,
            commands::mcp::read_mcp_resource,
            commands::mcp::subscribe_mcp_resource,
            commands::mcp::unsubscribe_mcp_resource,
            // mcp prompts
            commands::mcp::list_mcp_prompts,
            commands::mcp::get_mcp_prompt,
            // mcp legacy (backward compatibility)
            commands::mcp::list_mcp_server_configs,
            commands::mcp::upsert_mcp_server_config,
            commands::mcp::remove_mcp_server_config,
            commands::mcp::refresh_mcp_tools,
            commands::mcp::list_cached_mcp_tools,
            // skills
            commands::skills::list_available_skills,
            commands::skills::search_skills,
            commands::skills::add_custom_skill,
            commands::skills::remove_custom_skill,
            commands::skills::import_context7_skill,
            commands::skills::import_vercel_skill,
            commands::skills::search_agent_skills,
            commands::skills::install_agent_skill,
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
            // agent presets
            commands::agent_presets::list_agent_presets,
            commands::agent_presets::get_agent_preset,
            commands::agent_presets::search_agent_presets,
            commands::agent_presets::create_agent_preset,
            commands::agent_presets::update_agent_preset,
            commands::agent_presets::delete_agent_preset,
            commands::agent_presets::read_agent_preset_file,
            commands::agent_presets::get_agent_preset_context,
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
