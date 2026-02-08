use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
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

#[derive(Debug)]
struct PendingApproval {
    request: ApprovalRequest,
    responder: oneshot::Sender<bool>,
}

#[derive(Clone, Default)]
pub struct ApprovalGate {
    pending: Arc<Mutex<HashMap<String, PendingApproval>>>,
    approved_scopes: Arc<Mutex<HashSet<String>>>,
}

impl ApprovalGate {
    pub fn approved_scopes_handle(&self) -> Arc<Mutex<HashSet<String>>> {
        self.approved_scopes.clone()
    }

    pub fn request(
        &self,
        task_id: &str,
        run_id: &str,
        sub_agent_id: &str,
        tool_call_id: &str,
        tool_name: &str,
        scope: &str,
        reason: &str,
    ) -> (ApprovalRequest, oneshot::Receiver<bool>) {
        let request = ApprovalRequest {
            id: Uuid::new_v4().to_string(),
            task_id: task_id.to_string(),
            run_id: run_id.to_string(),
            sub_agent_id: sub_agent_id.to_string(),
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            scope: scope.to_string(),
            reason: reason.to_string(),
            created_at: Utc::now().to_rfc3339(),
        };

        let (tx, rx) = oneshot::channel();
        let mut guard = self.pending.lock().expect("approval gate mutex poisoned");
        guard.insert(
            request.id.clone(),
            PendingApproval {
                request: request.clone(),
                responder: tx,
            },
        );
        (request, rx)
    }

    pub fn list_pending(&self, task_id: Option<&str>) -> Vec<ApprovalRequest> {
        let guard = self.pending.lock().expect("approval gate mutex poisoned");
        let mut values: Vec<ApprovalRequest> = guard
            .values()
            .map(|entry| entry.request.clone())
            .filter(|entry| match task_id {
                Some(filter) => entry.task_id == filter,
                None => true,
            })
            .collect();
        values.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        values
    }

    pub fn resolve(&self, approval_id: &str, approve: bool) -> Result<ApprovalRequest, String> {
        let mut guard = self.pending.lock().expect("approval gate mutex poisoned");
        let Some(entry) = guard.remove(approval_id) else {
            return Err(format!("approval request not found: {approval_id}"));
        };

        if approve {
            let mut approved = self
                .approved_scopes
                .lock()
                .expect("approval scope mutex poisoned");
            approved.insert(entry.request.scope.clone());
        }

        let _ = entry.responder.send(approve);
        Ok(entry.request)
    }

    pub fn reject_all_for_task(&self, task_id: &str) {
        let ids: Vec<String> = {
            let guard = self.pending.lock().expect("approval gate mutex poisoned");
            guard
                .iter()
                .filter(|(_, pending)| pending.request.task_id == task_id)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for id in ids {
            let _ = self.resolve(&id, false);
        }
    }
}
