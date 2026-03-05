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
use tokio::time::{sleep, timeout, Duration};
use uuid::Uuid;

use crate::bus::EventBus;
use crate::core::plan::{Plan, PlanStep, StepStatus};
use crate::db::{queries, Database};
use crate::policy::PolicyEngine;
use crate::tools::ToolRegistry;

use super::approval::{ApprovalGate, ApprovalRequest};
use super::planner::emit_and_record;
use super::questions::{UserQuestionAnswer, UserQuestionGate, UserQuestionRequest};
use super::worktree::{WorktreeManager, WorktreeStrategy};

mod sub_agent;
mod task_lifecycle;
mod worker;

pub(crate) use worker::helpers::parse_sub_agent_contract;
pub(crate) use worker::model::RuntimeModelConfig;

#[derive(Clone)]
pub struct Orchestrator {
    db: Arc<Database>,
    bus: Arc<EventBus>,
    workspace_root: Arc<Mutex<PathBuf>>,
    tool_registry: Arc<ToolRegistry>,
    worktree_manager: Arc<WorktreeManager>,
    approval_gate: Arc<ApprovalGate>,
    question_gate: Arc<UserQuestionGate>,
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

impl Orchestrator {
    pub fn new(db: Arc<Database>, bus: Arc<EventBus>, workspace_root: PathBuf) -> Self {
        let db_clone = db.clone();
        Self {
            db,
            bus,
            workspace_root: Arc::new(Mutex::new(workspace_root)),
            tool_registry: Arc::new(ToolRegistry::default()),
            worktree_manager: Arc::new(WorktreeManager::new()),
            approval_gate: Arc::new(ApprovalGate::default()),
            question_gate: Arc::new(UserQuestionGate::new(Some(db_clone))),
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

    pub fn question_gate(&self) -> &Arc<UserQuestionGate> {
        &self.question_gate
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

    pub fn list_pending_questions(&self, task_id: Option<&str>) -> Vec<UserQuestionRequest> {
        match task_id {
            Some(tid) => self.question_gate.list_pending_from_db(tid),
            None => self.question_gate.list_pending(None),
        }
    }

    pub fn resolve_question(
        &self,
        question_id: &str,
        answer: UserQuestionAnswer,
    ) -> Result<UserQuestionRequest, String> {
        self.question_gate.resolve(question_id, answer)
    }

    fn current_workspace_root(&self) -> PathBuf {
        self.workspace_root
            .lock()
            .expect("orchestrator mutex poisoned")
            .clone()
    }
}
