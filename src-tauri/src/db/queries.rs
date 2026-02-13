use rusqlite::params;
use serde::Serialize;

use super::{Database, DbError};

// ---------------------------------------------------------------------------
// Row types — flat structs that map directly to table columns
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct TaskRow {
    pub id: String,
    pub prompt: String,
    pub parent_task_id: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskLinkRow {
    pub source_task_id: String,
    pub target_task_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunRow {
    pub id: String,
    pub task_id: String,
    pub status: String,
    pub plan_json: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubAgentRow {
    pub id: String,
    pub run_id: String,
    pub step_idx: i64,
    pub name: String,
    pub status: String,
    pub worktree_path: Option<String>,
    pub context_json: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventRow {
    pub id: String,
    pub run_id: Option<String>,
    pub seq: i64,
    pub category: String,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactRow {
    pub id: String,
    pub run_id: String,
    pub kind: String,
    pub uri_or_content: String,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserMessageRow {
    pub id: String,
    pub task_id: String,
    pub run_id: Option<String>,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckpointRow {
    pub run_id: String,
    pub last_step_idx: i64,
    pub runtime_state_json: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolCallRow {
    pub id: String,
    pub run_id: String,
    pub step_idx: Option<i64>,
    pub tool_name: String,
    pub input_json: String,
    pub output_json: Option<String>,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Task queries
// ---------------------------------------------------------------------------

pub fn insert_task(db: &Database, row: &TaskRow) -> Result<(), DbError> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO tasks (id, prompt, parent_task_id, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.id,
            row.prompt,
            row.parent_task_id,
            row.status,
            row.created_at,
            row.updated_at
        ],
    )?;
    Ok(())
}

pub fn list_tasks(db: &Database) -> Result<Vec<TaskRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, prompt, parent_task_id, status, created_at, updated_at FROM tasks ORDER BY updated_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TaskRow {
                id: row.get(0)?,
                prompt: row.get(1)?,
                parent_task_id: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_task(db: &Database, id: &str) -> Result<Option<TaskRow>, DbError> {
    let conn = db.conn();
    let mut stmt =
        conn.prepare("SELECT id, prompt, parent_task_id, status, created_at, updated_at FROM tasks WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![id], |row| {
        Ok(TaskRow {
            id: row.get(0)?,
            prompt: row.get(1)?,
            parent_task_id: row.get(2)?,
            status: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Conversation transcript queries (from events)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ConversationMessageRow {
    pub id: String,
    pub run_id: Option<String>,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

/// Fetch conversation transcript for a task (user messages + agent responses).
/// Returns messages ordered chronologically (oldest first).
pub fn list_conversation_messages_for_task(
    db: &Database,
    task_id: &str,
) -> Result<Vec<ConversationMessageRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        r#"SELECT e.id, e.run_id,
            CASE 
                WHEN e.event_type = 'user.message_sent' THEN 'user'
                ELSE 'assistant'
            END as role,
            json_extract(e.payload_json, '$.content') as content,
            e.created_at
         FROM events e
         INNER JOIN runs r ON r.id = e.run_id
         WHERE r.task_id = ?1
           AND e.event_type IN ('user.message_sent', 'agent.message', 'agent.plan_message')
         ORDER BY e.created_at ASC, e.seq ASC"#,
    )?;
    let rows = stmt
        .query_map(params![task_id], |row| {
            Ok(ConversationMessageRow {
                id: row.get(0)?,
                run_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Conversation summary queries
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ConversationSummaryRow {
    pub id: String,
    pub task_id: String,
    pub run_id: String,
    pub summary: String,
    pub message_count: i64,
    pub token_estimate: Option<i64>,
    pub created_at: String,
}

pub fn insert_conversation_summary(
    db: &Database,
    row: &ConversationSummaryRow,
) -> Result<(), DbError> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO conversation_summaries (id, task_id, run_id, summary, message_count, token_estimate, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            row.id,
            row.task_id,
            row.run_id,
            row.summary,
            row.message_count,
            row.token_estimate,
            row.created_at,
        ],
    )?;
    Ok(())
}

pub fn get_latest_conversation_summary(
    db: &Database,
    task_id: &str,
) -> Result<Option<ConversationSummaryRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, task_id, run_id, summary, message_count, token_estimate, created_at
         FROM conversation_summaries
         WHERE task_id = ?1
         ORDER BY created_at DESC
         LIMIT 1",
    )?;
    let mut rows = stmt.query_map(params![task_id], |row| {
        Ok(ConversationSummaryRow {
            id: row.get(0)?,
            task_id: row.get(1)?,
            run_id: row.get(2)?,
            summary: row.get(3)?,
            message_count: row.get(4)?,
            token_estimate: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn update_task_status(
    db: &Database,
    id: &str,
    status: &str,
    updated_at: &str,
) -> Result<(), DbError> {
    let conn = db.conn();
    let changed = conn.execute(
        "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status, updated_at, id],
    )?;
    if changed == 0 {
        return Err(DbError::NotFound(format!("task {id}")));
    }
    Ok(())
}

pub fn upsert_task_link(db: &Database, a: &str, b: &str, created_at: &str) -> Result<(), DbError> {
    if a == b {
        return Ok(());
    }
    let (source, target) = if a < b { (a, b) } else { (b, a) };
    let conn = db.conn();
    conn.execute(
        "INSERT INTO task_links (source_task_id, target_task_id, created_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(source_task_id, target_task_id) DO NOTHING",
        params![source, target, created_at],
    )?;
    Ok(())
}

pub fn delete_task_link(db: &Database, a: &str, b: &str) -> Result<(), DbError> {
    if a == b {
        return Ok(());
    }
    let (source, target) = if a < b { (a, b) } else { (b, a) };
    let conn = db.conn();
    conn.execute(
        "DELETE FROM task_links WHERE source_task_id = ?1 AND target_task_id = ?2",
        params![source, target],
    )?;
    Ok(())
}

pub fn list_task_links_for_task(db: &Database, task_id: &str) -> Result<Vec<TaskLinkRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT source_task_id, target_task_id, created_at
         FROM task_links
         WHERE source_task_id = ?1 OR target_task_id = ?1
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![task_id], |row| {
            Ok(TaskLinkRow {
                source_task_id: row.get(0)?,
                target_task_id: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn delete_task_cascade(db: &Database, task_id: &str) -> Result<(), DbError> {
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;

    let run_ids = {
        let mut stmt = tx.prepare("SELECT id FROM runs WHERE task_id = ?1")?;
        let rows = stmt.query_map(params![task_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    for run_id in &run_ids {
        tx.execute(
            "DELETE FROM user_messages WHERE run_id = ?1",
            params![run_id],
        )?;
        tx.execute(
            "DELETE FROM worktree_log WHERE run_id = ?1",
            params![run_id],
        )?;
        tx.execute("DELETE FROM artifacts WHERE run_id = ?1", params![run_id])?;
        tx.execute("DELETE FROM tool_calls WHERE run_id = ?1", params![run_id])?;
        tx.execute("DELETE FROM sub_agents WHERE run_id = ?1", params![run_id])?;
        tx.execute("DELETE FROM events WHERE run_id = ?1", params![run_id])?;
        tx.execute("DELETE FROM checkpoints WHERE run_id = ?1", params![run_id])?;
        tx.execute(
            "DELETE FROM agent_messages WHERE run_id = ?1",
            params![run_id],
        )?;
        tx.execute("DELETE FROM steps WHERE run_id = ?1", params![run_id])?;
        tx.execute("DELETE FROM runs WHERE id = ?1", params![run_id])?;
    }

    tx.execute(
        "UPDATE tasks SET parent_task_id = NULL WHERE parent_task_id = ?1",
        params![task_id],
    )?;
    tx.execute(
        "DELETE FROM task_links WHERE source_task_id = ?1 OR target_task_id = ?1",
        params![task_id],
    )?;
    tx.execute(
        "DELETE FROM user_messages WHERE task_id = ?1",
        params![task_id],
    )?;
    tx.execute("DELETE FROM tasks WHERE id = ?1", params![task_id])?;

    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Run queries
// ---------------------------------------------------------------------------

pub fn insert_run(db: &Database, row: &RunRow) -> Result<(), DbError> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO runs (id, task_id, status, plan_json, started_at, finished_at, failure_reason)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            row.id,
            row.task_id,
            row.status,
            row.plan_json,
            row.started_at,
            row.finished_at,
            row.failure_reason,
        ],
    )?;
    Ok(())
}

pub fn get_active_runs(db: &Database) -> Result<Vec<RunRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, task_id, status, plan_json, started_at, finished_at, failure_reason
         FROM runs WHERE status IN ('planning', 'executing')",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(RunRow {
                id: row.get(0)?,
                task_id: row.get(1)?,
                status: row.get(2)?,
                plan_json: row.get(3)?,
                started_at: row.get(4)?,
                finished_at: row.get(5)?,
                failure_reason: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_run(db: &Database, run_id: &str) -> Result<Option<RunRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, task_id, status, plan_json, started_at, finished_at, failure_reason
         FROM runs WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![run_id], |row| {
        Ok(RunRow {
            id: row.get(0)?,
            task_id: row.get(1)?,
            status: row.get(2)?,
            plan_json: row.get(3)?,
            started_at: row.get(4)?,
            finished_at: row.get(5)?,
            failure_reason: row.get(6)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn get_latest_run_for_task(db: &Database, task_id: &str) -> Result<Option<RunRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, task_id, status, plan_json, started_at, finished_at, failure_reason
         FROM runs
         WHERE task_id = ?1
         ORDER BY started_at DESC
         LIMIT 1",
    )?;
    let mut rows = stmt.query_map(params![task_id], |row| {
        Ok(RunRow {
            id: row.get(0)?,
            task_id: row.get(1)?,
            status: row.get(2)?,
            plan_json: row.get(3)?,
            started_at: row.get(4)?,
            finished_at: row.get(5)?,
            failure_reason: row.get(6)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn list_runs_for_task(db: &Database, task_id: &str) -> Result<Vec<RunRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, task_id, status, plan_json, started_at, finished_at, failure_reason
         FROM runs
         WHERE task_id = ?1
         ORDER BY started_at ASC",
    )?;
    let rows = stmt
        .query_map(params![task_id], |row| {
            Ok(RunRow {
                id: row.get(0)?,
                task_id: row.get(1)?,
                status: row.get(2)?,
                plan_json: row.get(3)?,
                started_at: row.get(4)?,
                finished_at: row.get(5)?,
                failure_reason: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[allow(dead_code)]
pub fn list_events_for_run(db: &Database, run_id: &str) -> Result<Vec<EventRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, run_id, seq, category, event_type, payload_json, created_at
         FROM events
         WHERE run_id = ?1
         ORDER BY seq ASC",
    )?;
    let rows = stmt
        .query_map(params![run_id], |row| {
            Ok(EventRow {
                id: row.get(0)?,
                run_id: row.get(1)?,
                seq: row.get(2)?,
                category: row.get(3)?,
                event_type: row.get(4)?,
                payload_json: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn list_events_for_task(db: &Database, task_id: &str) -> Result<Vec<EventRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT e.id, e.run_id, e.seq, e.category, e.event_type, e.payload_json, e.created_at
         FROM events e
         INNER JOIN runs r ON r.id = e.run_id
         WHERE r.task_id = ?1
         ORDER BY e.created_at ASC, e.seq ASC",
    )?;
    let rows = stmt
        .query_map(params![task_id], |row| {
            Ok(EventRow {
                id: row.get(0)?,
                run_id: row.get(1)?,
                seq: row.get(2)?,
                category: row.get(3)?,
                event_type: row.get(4)?,
                payload_json: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn update_run_status_and_plan(
    db: &Database,
    run_id: &str,
    status: &str,
    plan_json: Option<&str>,
    finished_at: Option<&str>,
    failure_reason: Option<&str>,
) -> Result<(), DbError> {
    let conn = db.conn();
    let changed = conn.execute(
        "UPDATE runs
         SET status = ?1,
             plan_json = COALESCE(?2, plan_json),
             finished_at = ?3,
             failure_reason = ?4
         WHERE id = ?5",
        params![status, plan_json, finished_at, failure_reason, run_id],
    )?;
    if changed == 0 {
        return Err(DbError::NotFound(format!("run {run_id}")));
    }
    Ok(())
}

pub fn update_run_status(
    db: &Database,
    run_id: &str,
    status: &str,
    finished_at: Option<&str>,
    failure_reason: Option<&str>,
) -> Result<(), DbError> {
    let conn = db.conn();
    let changed = conn.execute(
        "UPDATE runs
         SET status = ?1,
             finished_at = ?2,
             failure_reason = ?3
         WHERE id = ?4",
        params![status, finished_at, failure_reason, run_id],
    )?;
    if changed == 0 {
        return Err(DbError::NotFound(format!("run {run_id}")));
    }
    Ok(())
}

pub fn insert_sub_agent(db: &Database, row: &SubAgentRow) -> Result<(), DbError> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO sub_agents
         (id, run_id, step_idx, name, status, worktree_path, context_json, started_at, finished_at, error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            row.id,
            row.run_id,
            row.step_idx,
            row.name,
            row.status,
            row.worktree_path,
            row.context_json,
            row.started_at,
            row.finished_at,
            row.error,
        ],
    )?;
    Ok(())
}

pub fn list_sub_agents_for_run(db: &Database, run_id: &str) -> Result<Vec<SubAgentRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, run_id, step_idx, name, status, worktree_path, context_json, started_at, finished_at, error
         FROM sub_agents
         WHERE run_id = ?1
         ORDER BY step_idx ASC",
    )?;
    let rows = stmt
        .query_map(params![run_id], |row| {
            Ok(SubAgentRow {
                id: row.get(0)?,
                run_id: row.get(1)?,
                step_idx: row.get(2)?,
                name: row.get(3)?,
                status: row.get(4)?,
                worktree_path: row.get(5)?,
                context_json: row.get(6)?,
                started_at: row.get(7)?,
                finished_at: row.get(8)?,
                error: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn update_sub_agent_status(
    db: &Database,
    id: &str,
    status: &str,
    worktree_path: Option<&str>,
    finished_at: Option<&str>,
    error: Option<&str>,
) -> Result<(), DbError> {
    let conn = db.conn();
    let changed = conn.execute(
        "UPDATE sub_agents
         SET status = ?1,
             worktree_path = COALESCE(?2, worktree_path),
             finished_at = ?3,
             error = ?4
         WHERE id = ?5",
        params![status, worktree_path, finished_at, error, id],
    )?;
    if changed == 0 {
        return Err(DbError::NotFound(format!("sub_agent {id}")));
    }
    Ok(())
}

pub fn mark_sub_agent_started(
    db: &Database,
    id: &str,
    worktree_path: Option<&str>,
    started_at: &str,
) -> Result<(), DbError> {
    let conn = db.conn();
    let changed = conn.execute(
        "UPDATE sub_agents
         SET status = 'running',
             worktree_path = COALESCE(?1, worktree_path),
             started_at = ?2
         WHERE id = ?3",
        params![worktree_path, started_at, id],
    )?;
    if changed == 0 {
        return Err(DbError::NotFound(format!("sub_agent {id}")));
    }
    Ok(())
}

pub fn upsert_setting(
    db: &Database,
    key: &str,
    value_json: &str,
    updated_at: &str,
) -> Result<(), DbError> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO settings (key, value_json, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key)
         DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
        params![key, value_json, updated_at],
    )?;
    Ok(())
}

pub fn get_setting(db: &Database, key: &str) -> Result<Option<String>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT value_json FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query_map(params![key], |row| row.get(0))?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Event queries
// ---------------------------------------------------------------------------

pub fn insert_event(db: &Database, row: &EventRow) -> Result<(), DbError> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO events (id, run_id, seq, category, event_type, payload_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            row.id,
            row.run_id,
            row.seq,
            row.category,
            row.event_type,
            row.payload_json,
            row.created_at,
        ],
    )?;
    Ok(())
}

pub fn insert_artifact(db: &Database, row: &ArtifactRow) -> Result<(), DbError> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO artifacts (id, run_id, kind, uri_or_content, metadata_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.id,
            row.run_id,
            row.kind,
            row.uri_or_content,
            row.metadata_json,
            row.created_at,
        ],
    )?;
    Ok(())
}

pub fn list_artifacts_for_run(db: &Database, run_id: &str) -> Result<Vec<ArtifactRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, run_id, kind, uri_or_content, metadata_json, created_at
         FROM artifacts
         WHERE run_id = ?1
         ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![run_id], |row| {
            Ok(ArtifactRow {
                id: row.get(0)?,
                run_id: row.get(1)?,
                kind: row.get(2)?,
                uri_or_content: row.get(3)?,
                metadata_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn list_markdown_artifacts_for_task(
    db: &Database,
    task_id: &str,
) -> Result<Vec<ArtifactRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT a.id, a.run_id, a.kind, a.uri_or_content, a.metadata_json, a.created_at
         FROM artifacts a
         INNER JOIN runs r ON r.id = a.run_id
         WHERE r.task_id = ?1
           AND (
             LOWER(a.uri_or_content) LIKE '%.md'
             OR LOWER(a.uri_or_content) LIKE '%.markdown'
             OR LOWER(a.uri_or_content) LIKE '%.mdx'
           )
         ORDER BY a.created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![task_id], |row| {
            Ok(ArtifactRow {
                id: row.get(0)?,
                run_id: row.get(1)?,
                kind: row.get(2)?,
                uri_or_content: row.get(3)?,
                metadata_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn insert_user_message(db: &Database, row: &UserMessageRow) -> Result<(), DbError> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO user_messages (id, task_id, run_id, content, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![row.id, row.task_id, row.run_id, row.content, row.created_at,],
    )?;
    Ok(())
}

pub fn list_user_messages_for_task(
    db: &Database,
    task_id: &str,
) -> Result<Vec<UserMessageRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, task_id, run_id, content, created_at
         FROM user_messages
         WHERE task_id = ?1
         ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![task_id], |row| {
            Ok(UserMessageRow {
                id: row.get(0)?,
                task_id: row.get(1)?,
                run_id: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn upsert_checkpoint(db: &Database, row: &CheckpointRow) -> Result<(), DbError> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO checkpoints (run_id, last_step_idx, runtime_state_json, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(run_id)
         DO UPDATE SET
             last_step_idx = excluded.last_step_idx,
             runtime_state_json = excluded.runtime_state_json,
             updated_at = excluded.updated_at",
        params![
            row.run_id,
            row.last_step_idx,
            row.runtime_state_json,
            row.updated_at,
        ],
    )?;
    Ok(())
}

pub fn get_checkpoint(db: &Database, run_id: &str) -> Result<Option<CheckpointRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT run_id, last_step_idx, runtime_state_json, updated_at
         FROM checkpoints
         WHERE run_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![run_id], |row| {
        Ok(CheckpointRow {
            run_id: row.get(0)?,
            last_step_idx: row.get(1)?,
            runtime_state_json: row.get(2)?,
            updated_at: row.get(3)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn insert_tool_call(db: &Database, row: &ToolCallRow) -> Result<(), DbError> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO tool_calls
         (id, run_id, step_idx, tool_name, input_json, output_json, status, started_at, finished_at, error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            row.id,
            row.run_id,
            row.step_idx,
            row.tool_name,
            row.input_json,
            row.output_json,
            row.status,
            row.started_at,
            row.finished_at,
            row.error,
        ],
    )?;
    Ok(())
}

pub fn update_tool_call_result(
    db: &Database,
    id: &str,
    status: &str,
    output_json: Option<&str>,
    finished_at: Option<&str>,
    error: Option<&str>,
) -> Result<(), DbError> {
    let conn = db.conn();
    let changed = conn.execute(
        "UPDATE tool_calls
         SET status = ?1,
             output_json = ?2,
             finished_at = ?3,
             error = ?4
         WHERE id = ?5",
        params![status, output_json, finished_at, error, id],
    )?;
    if changed == 0 {
        return Err(DbError::NotFound(format!("tool_call {id}")));
    }
    Ok(())
}

pub fn list_tool_calls_for_run(db: &Database, run_id: &str) -> Result<Vec<ToolCallRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, run_id, step_idx, tool_name, input_json, output_json, status, started_at, finished_at, error
         FROM tool_calls
         WHERE run_id = ?1
         ORDER BY started_at ASC",
    )?;
    let rows = stmt
        .query_map(params![run_id], |row| {
            Ok(ToolCallRow {
                id: row.get(0)?,
                run_id: row.get(1)?,
                step_idx: row.get(2)?,
                tool_name: row.get(3)?,
                input_json: row.get(4)?,
                output_json: row.get(5)?,
                status: row.get(6)?,
                started_at: row.get(7)?,
                finished_at: row.get(8)?,
                error: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Worktree log queries
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeLogRow {
    pub id: String,
    pub run_id: String,
    pub sub_agent_id: String,
    pub strategy: String,
    pub branch_name: Option<String>,
    pub base_ref: Option<String>,
    pub worktree_path: String,
    pub merge_strategy: Option<String>,
    pub merge_success: Option<bool>,
    pub merge_message: Option<String>,
    pub conflicted_files_json: Option<String>,
    pub created_at: String,
    pub merged_at: Option<String>,
    pub cleaned_at: Option<String>,
}

pub fn insert_worktree_log(db: &Database, row: &WorktreeLogRow) -> Result<(), DbError> {
    let conn = db.conn();
    let merge_success_int = row.merge_success.map(|b| if b { 1i64 } else { 0i64 });
    conn.execute(
        "INSERT INTO worktree_log
         (id, run_id, sub_agent_id, strategy, branch_name, base_ref, worktree_path,
          merge_strategy, merge_success, merge_message, conflicted_files_json,
          created_at, merged_at, cleaned_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            row.id,
            row.run_id,
            row.sub_agent_id,
            row.strategy,
            row.branch_name,
            row.base_ref,
            row.worktree_path,
            row.merge_strategy,
            merge_success_int,
            row.merge_message,
            row.conflicted_files_json,
            row.created_at,
            row.merged_at,
            row.cleaned_at,
        ],
    )?;
    Ok(())
}

pub fn update_worktree_log_merge(
    db: &Database,
    sub_agent_id: &str,
    merge_strategy: &str,
    merge_success: bool,
    merge_message: &str,
    conflicted_files_json: Option<&str>,
    merged_at: &str,
) -> Result<(), DbError> {
    let conn = db.conn();
    let merge_success_int: i64 = if merge_success { 1 } else { 0 };
    conn.execute(
        "UPDATE worktree_log
         SET merge_strategy = ?1,
             merge_success = ?2,
             merge_message = ?3,
             conflicted_files_json = ?4,
             merged_at = ?5
         WHERE sub_agent_id = ?6",
        params![
            merge_strategy,
            merge_success_int,
            merge_message,
            conflicted_files_json,
            merged_at,
            sub_agent_id,
        ],
    )?;
    Ok(())
}

pub fn update_worktree_log_cleaned(
    db: &Database,
    sub_agent_id: &str,
    cleaned_at: &str,
) -> Result<(), DbError> {
    let conn = db.conn();
    conn.execute(
        "UPDATE worktree_log SET cleaned_at = ?1 WHERE sub_agent_id = ?2",
        params![cleaned_at, sub_agent_id],
    )?;
    Ok(())
}

pub fn list_worktree_logs_for_run(
    db: &Database,
    run_id: &str,
) -> Result<Vec<WorktreeLogRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, run_id, sub_agent_id, strategy, branch_name, base_ref, worktree_path,
                merge_strategy, merge_success, merge_message, conflicted_files_json,
                created_at, merged_at, cleaned_at
         FROM worktree_log
         WHERE run_id = ?1
         ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![run_id], |row| {
            let merge_success_int: Option<i64> = row.get(8)?;
            Ok(WorktreeLogRow {
                id: row.get(0)?,
                run_id: row.get(1)?,
                sub_agent_id: row.get(2)?,
                strategy: row.get(3)?,
                branch_name: row.get(4)?,
                base_ref: row.get(5)?,
                worktree_path: row.get(6)?,
                merge_strategy: row.get(7)?,
                merge_success: merge_success_int.map(|i| i != 0),
                merge_message: row.get(9)?,
                conflicted_files_json: row.get(10)?,
                created_at: row.get(11)?,
                merged_at: row.get(12)?,
                cleaned_at: row.get(13)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Event queries
// ---------------------------------------------------------------------------

pub fn get_events_after_seq(
    db: &Database,
    run_id: &str,
    after_seq: i64,
) -> Result<Vec<EventRow>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, run_id, seq, category, event_type, payload_json, created_at
         FROM events WHERE run_id = ?1 AND seq > ?2 ORDER BY seq ASC",
    )?;
    let rows = stmt
        .query_map(params![run_id, after_seq], |row| {
            Ok(EventRow {
                id: row.get(0)?,
                run_id: row.get(1)?,
                seq: row.get(2)?,
                category: row.get(3)?,
                event_type: row.get(4)?,
                payload_json: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
