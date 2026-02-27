use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuestionOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuestionRequest {
    pub id: String,
    pub task_id: String,
    pub run_id: String,
    pub sub_agent_id: String,
    pub tool_call_id: String,
    pub question: String,
    pub options: Vec<UserQuestionOption>,
    pub multiple: bool,
    pub allow_custom: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuestionAnswer {
    pub selected_option_ids: Vec<String>,
    pub custom_text: Option<String>,
    pub final_text: String,
}

#[derive(Debug)]
struct PendingQuestion {
    request: UserQuestionRequest,
    responder: oneshot::Sender<UserQuestionAnswer>,
}

#[derive(Clone, Default)]
pub struct UserQuestionGate {
    pending: Arc<Mutex<HashMap<String, PendingQuestion>>>,
}

impl UserQuestionGate {
    pub fn list_pending(&self, task_id: Option<&str>) -> Vec<UserQuestionRequest> {
        let guard = self.pending.lock().expect("question gate mutex poisoned");
        let mut values: Vec<UserQuestionRequest> = guard
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

    pub fn request(
        &self,
        task_id: &str,
        run_id: &str,
        sub_agent_id: &str,
        tool_call_id: &str,
        question: String,
        options: Vec<UserQuestionOption>,
        multiple: bool,
        allow_custom: bool,
    ) -> (UserQuestionRequest, oneshot::Receiver<UserQuestionAnswer>) {
        let request = UserQuestionRequest {
            id: Uuid::new_v4().to_string(),
            task_id: task_id.to_string(),
            run_id: run_id.to_string(),
            sub_agent_id: sub_agent_id.to_string(),
            tool_call_id: tool_call_id.to_string(),
            question,
            options,
            multiple,
            allow_custom,
            created_at: Utc::now().to_rfc3339(),
        };

        let (tx, rx) = oneshot::channel();
        let mut guard = self.pending.lock().expect("question gate mutex poisoned");
        guard.insert(
            request.id.clone(),
            PendingQuestion {
                request: request.clone(),
                responder: tx,
            },
        );
        (request, rx)
    }

    pub fn resolve(
        &self,
        question_id: &str,
        answer: UserQuestionAnswer,
    ) -> Result<UserQuestionRequest, String> {
        let mut guard = self.pending.lock().expect("question gate mutex poisoned");
        let Some(entry) = guard.remove(question_id) else {
            return Err(format!("question request not found: {question_id}"));
        };

        let _ = entry.responder.send(answer);
        Ok(entry.request)
    }

    pub fn reject_all_for_task(&self, task_id: &str) {
        let ids: Vec<String> = {
            let guard = self.pending.lock().expect("question gate mutex poisoned");
            guard
                .iter()
                .filter(|(_, pending)| pending.request.task_id == task_id)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for id in ids {
            let _ = self.resolve(
                &id,
                UserQuestionAnswer {
                    selected_option_ids: Vec::new(),
                    custom_text: None,
                    final_text: String::new(),
                },
            );
        }
    }
}
