//! Task orchestrator for managing AI agent execution.
//!
//! The orchestrator is the central component for task lifecycle management:
//! - Receives tasks from the frontend via Tauri commands
//! - Coordinates planning phase with LLM (MiniMax/Kimi)
//! - Manages plan approval workflow
//! - Executes plan steps, including sub-agent delegation
//! - Handles worktree isolation for parallel execution
//! - Manages approval gates for sensitive operations
//!
//! # Sub-modules
//!
//! - `worker`: Single-threaded task execution loop
//! - `sub_agent`: Parallel sub-agent delegation and result aggregation
//! - `task_lifecycle`: Task state transitions and persistence
//!
//! # Usage
//!
//! The orchestrator is initialized once in `lib.rs` and shared via `AppState`:
//!
//! ```ignore
//! let orchestrator = Arc::new(Orchestrator::new(db, bus, workspace_root));
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::Deserialize;
use tokio::time::{sleep, timeout, Duration};
use uuid::Uuid;

use crate::bus::EventBus;
use crate::core::plan::{Plan, PlanStep, StepStatus};
use crate::db::{queries, Database};
use crate::policy::PolicyEngine;
use crate::tools::ToolRegistry;

use super::approval::{ApprovalGate, ApprovalRequest};
use super::planner::emit_and_record;
use super::worktree::{WorktreeManager, WorktreeStrategy};

mod sub_agent;
mod task_lifecycle;
mod worker;

#[derive(Clone)]
pub struct Orchestrator {
    db: Arc<Database>,
    bus: Arc<EventBus>,
    workspace_root: Arc<Mutex<PathBuf>>,
    tool_registry: Arc<ToolRegistry>,
    worktree_manager: Arc<WorktreeManager>,
    approval_gate: Arc<ApprovalGate>,
    active: Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>,
}

#[derive(Debug)]
struct SubAgentResult {
    sub_agent_id: String,
    success: bool,
    output_path: Option<String>,
    error: Option<String>,
    merge_message: Option<String>,
}

const SUB_AGENT_RETRY_BACKOFF_MS: u64 = 500;
const SUB_AGENT_ATTEMPT_TIMEOUT_SECS: u64 = 90;

#[derive(Clone)]
struct RuntimeModelConfig {
    provider: String,
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct SubAgentContract {
    #[serde(default)]
    permissions: SubAgentPermissions,
    #[serde(default)]
    execution: SubAgentExecution,
}

impl Default for SubAgentContract {
    fn default() -> Self {
        Self {
            permissions: SubAgentPermissions::default(),
            execution: SubAgentExecution::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct SubAgentPermissions {
    #[serde(default)]
    allowed_tools: Vec<String>,
    #[serde(default)]
    can_spawn_children: bool,
    #[serde(default)]
    max_delegation_depth: u32,
}

impl Default for SubAgentPermissions {
    fn default() -> Self {
        Self {
            allowed_tools: Vec::new(),
            can_spawn_children: false,
            max_delegation_depth: 0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SubAgentExecution {
    #[serde(default = "default_attempt_timeout_ms")]
    attempt_timeout_ms: u64,
    #[serde(default = "default_close_on_completion")]
    close_on_completion: bool,
}

impl Default for SubAgentExecution {
    fn default() -> Self {
        Self {
            attempt_timeout_ms: default_attempt_timeout_ms(),
            close_on_completion: default_close_on_completion(),
        }
    }
}

fn default_attempt_timeout_ms() -> u64 {
    SUB_AGENT_ATTEMPT_TIMEOUT_SECS * 1000
}

fn default_close_on_completion() -> bool {
    true
}

fn parse_sub_agent_contract(context_json: Option<&str>) -> SubAgentContract {
    let Some(raw) = context_json else {
        return SubAgentContract::default();
    };

    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return SubAgentContract::default();
    };

    let Some(contract_value) = value.get("contract") else {
        return SubAgentContract::default();
    };

    serde_json::from_value::<SubAgentContract>(contract_value.clone()).unwrap_or_default()
}

impl Orchestrator {
    pub fn new(db: Arc<Database>, bus: Arc<EventBus>, workspace_root: PathBuf) -> Self {
        Self {
            db,
            bus,
            workspace_root: Arc::new(Mutex::new(workspace_root)),
            tool_registry: Arc::new(ToolRegistry::default()),
            worktree_manager: Arc::new(WorktreeManager::new()),
            approval_gate: Arc::new(ApprovalGate::default()),
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn worktree_manager(&self) -> &Arc<WorktreeManager> {
        &self.worktree_manager
    }

    pub fn tool_registry(&self) -> &Arc<ToolRegistry> {
        &self.tool_registry
    }

    pub fn approval_gate(&self) -> &Arc<ApprovalGate> {
        &self.approval_gate
    }

    pub fn set_workspace_root(&self, workspace_root: PathBuf) {
        let mut guard = self
            .workspace_root
            .lock()
            .expect("orchestrator mutex poisoned");
        *guard = workspace_root;
    }

    pub fn list_pending_approvals(&self, task_id: Option<&str>) -> Vec<ApprovalRequest> {
        self.approval_gate.list_pending(task_id)
    }

    pub fn resolve_approval_request(
        &self,
        approval_id: &str,
        approve: bool,
    ) -> Result<ApprovalRequest, String> {
        self.approval_gate.resolve(approval_id, approve)
    }

    fn current_workspace_root(&self) -> PathBuf {
        self.workspace_root
            .lock()
            .expect("orchestrator mutex poisoned")
            .clone()
    }
}
