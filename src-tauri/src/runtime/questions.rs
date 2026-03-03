use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::db::{queries, Database};

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
    pub timeout_secs: Option<u64>,
    pub default_option_id: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuestionAnswer {
    pub selected_option_ids: Vec<String>,
    pub custom_text: Option<String>,
    pub final_text: String,
    pub response_time_secs: u64,
    pub was_default: bool,
}

#[derive(Debug)]
struct PendingQuestion {
    request: UserQuestionRequest,
    responder: oneshot::Sender<UserQuestionAnswer>,
    created_at_timestamp: i64,
}

#[derive(Clone)]
pub struct UserQuestionGate {
    pending: Arc<Mutex<std::collections::HashMap<String, PendingQuestion>>>,
    db: Option<Arc<Database>>,
}

impl Default for UserQuestionGate {
    fn default() -> Self {
        Self {
            pending: Arc::new(Mutex::new(std::collections::HashMap::new())),
            db: None,
        }
    }
}

impl UserQuestionGate {
    pub fn new(db: Option<Arc<Database>>) -> Self {
        Self {
            pending: Arc::new(Mutex::new(std::collections::HashMap::new())),
            db,
        }
    }

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

    pub fn list_pending_from_db(&self, task_id: &str) -> Vec<UserQuestionRequest> {
        let Some(db) = &self.db else {
            return self.list_pending(Some(task_id));
        };

        let rows = queries::list_pending_questions_for_task(db, task_id).unwrap_or_default();
        rows.into_iter()
            .filter_map(|row| {
                let options: Vec<UserQuestionOption> =
                    serde_json::from_str(&row.options_json).ok()?;
                Some(UserQuestionRequest {
                    id: row.id,
                    task_id: row.task_id,
                    run_id: row.run_id,
                    sub_agent_id: row.sub_agent_id,
                    tool_call_id: row.tool_call_id,
                    question: row.question,
                    options,
                    multiple: row.multiple,
                    allow_custom: row.allow_custom,
                    timeout_secs: row.timeout_secs.map(|v| v as u64),
                    default_option_id: row.default_option_id,
                    created_at: row.created_at,
                    expires_at: row.expires_at,
                })
            })
            .collect()
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
        timeout_secs: Option<u64>,
        default_option_id: Option<String>,
    ) -> (UserQuestionRequest, oneshot::Receiver<UserQuestionAnswer>) {
        let now = Utc::now();
        let created_at = now.to_rfc3339();
        let expires_at =
            timeout_secs.map(|secs| (now + chrono::Duration::seconds(secs as i64)).to_rfc3339());

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
            timeout_secs,
            default_option_id: default_option_id.clone(),
            created_at: created_at.clone(),
            expires_at: expires_at.clone(),
        };

        // Persist to database if available
        if let Some(db) = &self.db {
            let options_json = serde_json::to_string(&request.options).unwrap_or_default();
            let row = queries::PendingQuestionRow {
                id: request.id.clone(),
                task_id: request.task_id.clone(),
                run_id: request.run_id.clone(),
                sub_agent_id: request.sub_agent_id.clone(),
                tool_call_id: request.tool_call_id.clone(),
                question: request.question.clone(),
                options_json,
                multiple: request.multiple,
                allow_custom: request.allow_custom,
                timeout_secs: request.timeout_secs.map(|v| v as i64),
                default_option_id: request.default_option_id.clone(),
                created_at: request.created_at.clone(),
                expires_at: request.expires_at.clone(),
            };
            let _ = queries::insert_pending_question(db, &row);
        }

        let (tx, rx) = oneshot::channel();
        let mut guard = self.pending.lock().expect("question gate mutex poisoned");
        guard.insert(
            request.id.clone(),
            PendingQuestion {
                request: request.clone(),
                responder: tx,
                created_at_timestamp: now.timestamp(),
            },
        );
        (request, rx)
    }

    pub fn resolve(
        &self,
        question_id: &str,
        mut answer: UserQuestionAnswer,
    ) -> Result<UserQuestionRequest, String> {
        let mut guard = self.pending.lock().expect("question gate mutex poisoned");
        let Some(entry) = guard.remove(question_id) else {
            return Err(format!("question request not found: {question_id}"));
        };

        // Calculate response time
        let response_time_secs =
            (Utc::now().timestamp() - entry.created_at_timestamp).max(0) as u64;
        answer.response_time_secs = response_time_secs;

        // Check if default was used
        if let Some(ref default_id) = entry.request.default_option_id {
            answer.was_default = answer.selected_option_ids.len() == 1
                && answer.selected_option_ids.contains(default_id);
        }

        // Delete from database
        if let Some(db) = &self.db {
            let _ = queries::delete_pending_question(db, question_id);
        }

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
                    response_time_secs: 0,
                    was_default: false,
                },
            );
        }

        // Also delete from database
        if let Some(db) = &self.db {
            let _ = queries::delete_pending_questions_for_task(db, task_id);
        }
    }

    pub fn restore_from_db(&self, task_id: &str) {
        let Some(db) = &self.db else { return };

        let rows = match queries::list_pending_questions_for_task(db, task_id) {
            Ok(r) => r,
            Err(_) => return,
        };

        let mut guard = self.pending.lock().expect("question gate mutex poisoned");
        for row in rows {
            // Check if already in memory
            if guard.contains_key(&row.id) {
                continue;
            }

            let options: Vec<UserQuestionOption> = match serde_json::from_str(&row.options_json) {
                Ok(o) => o,
                Err(_) => continue,
            };

            let created_at_timestamp = chrono::DateTime::parse_from_rfc3339(&row.created_at)
                .map(|dt| dt.timestamp())
                .unwrap_or(0);

            let request = UserQuestionRequest {
                id: row.id.clone(),
                task_id: row.task_id.clone(),
                run_id: row.run_id.clone(),
                sub_agent_id: row.sub_agent_id.clone(),
                tool_call_id: row.tool_call_id.clone(),
                question: row.question.clone(),
                options,
                multiple: row.multiple,
                allow_custom: row.allow_custom,
                timeout_secs: row.timeout_secs.map(|v| v as u64),
                default_option_id: row.default_option_id.clone(),
                created_at: row.created_at.clone(),
                expires_at: row.expires_at.clone(),
            };

            // Create a channel for the restored question (it will timeout since no one is waiting)
            let (tx, _rx) = oneshot::channel();
            guard.insert(
                row.id,
                PendingQuestion {
                    request,
                    responder: tx,
                    created_at_timestamp,
                },
            );
        }
    }
}
