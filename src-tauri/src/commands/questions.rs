//! User question commands

use crate::runtime::questions::{UserQuestionAnswer, UserQuestionRequest};
use crate::AppState;

#[tauri::command]
pub fn list_pending_questions(
    state: tauri::State<'_, AppState>,
    task_id: Option<String>,
) -> Result<Vec<UserQuestionRequest>, String> {
    Ok(state
        .orchestrator
        .list_pending_questions(task_id.as_deref()))
}

#[tauri::command]
pub fn resolve_question(
    state: tauri::State<'_, AppState>,
    question_id: String,
    answer: UserQuestionAnswer,
) -> Result<(), String> {
    state.orchestrator.resolve_question(&question_id, answer)?;
    Ok(())
}
