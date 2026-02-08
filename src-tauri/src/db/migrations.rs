use rusqlite::Connection;

use super::DbError;

struct Migration {
    version: i64,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: r#"
CREATE TABLE tasks (
    id          TEXT PRIMARY KEY,
    prompt      TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    metadata_json TEXT
);

CREATE TABLE runs (
    id              TEXT PRIMARY KEY,
    task_id         TEXT NOT NULL REFERENCES tasks(id),
    status          TEXT NOT NULL DEFAULT 'queued',
    plan_json       TEXT,
    started_at      TEXT,
    finished_at     TEXT,
    failure_reason  TEXT
);

CREATE TABLE steps (
    id              TEXT PRIMARY KEY,
    run_id          TEXT NOT NULL REFERENCES runs(id),
    idx             INTEGER NOT NULL,
    title           TEXT NOT NULL,
    description     TEXT,
    tool_intent_json TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    max_retries     INTEGER NOT NULL DEFAULT 1,
    result_json     TEXT,
    started_at      TEXT,
    finished_at     TEXT
);

CREATE TABLE agent_messages (
    id          TEXT PRIMARY KEY,
    run_id      TEXT NOT NULL REFERENCES runs(id),
    role        TEXT NOT NULL,
    content     TEXT NOT NULL,
    tokens_in   INTEGER,
    tokens_out  INTEGER,
    created_at  TEXT NOT NULL
);

CREATE TABLE tool_calls (
    id          TEXT PRIMARY KEY,
    run_id      TEXT NOT NULL REFERENCES runs(id),
    step_idx    INTEGER,
    tool_name   TEXT NOT NULL,
    input_json  TEXT NOT NULL,
    output_json TEXT,
    status      TEXT NOT NULL DEFAULT 'pending',
    started_at  TEXT,
    finished_at TEXT,
    error       TEXT
);

CREATE TABLE artifacts (
    id              TEXT PRIMARY KEY,
    run_id          TEXT NOT NULL REFERENCES runs(id),
    kind            TEXT NOT NULL,
    uri_or_content  TEXT NOT NULL,
    metadata_json   TEXT,
    created_at      TEXT NOT NULL
);

CREATE TABLE events (
    id           TEXT PRIMARY KEY,
    run_id       TEXT,
    seq          INTEGER NOT NULL,
    category     TEXT NOT NULL,
    event_type   TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at   TEXT NOT NULL
);

CREATE TABLE checkpoints (
    run_id              TEXT PRIMARY KEY REFERENCES runs(id),
    last_step_idx       INTEGER NOT NULL,
    runtime_state_json  TEXT,
    updated_at          TEXT NOT NULL
);

CREATE TABLE settings (
    key         TEXT PRIMARY KEY,
    value_json  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
"#,
    },
    Migration {
        version: 2,
        sql: r#"
CREATE INDEX idx_runs_task_status ON runs(task_id, status);
CREATE INDEX idx_steps_run_idx ON steps(run_id, idx);
CREATE INDEX idx_tool_calls_run_step ON tool_calls(run_id, step_idx);
CREATE INDEX idx_events_run_seq ON events(run_id, seq);
CREATE INDEX idx_tasks_status ON tasks(status, updated_at);
"#,
    },
    Migration {
        version: 3,
        sql: r#"
CREATE TABLE sub_agents (
    id            TEXT PRIMARY KEY,
    run_id        TEXT NOT NULL REFERENCES runs(id),
    step_idx      INTEGER NOT NULL,
    name          TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'queued',
    worktree_path TEXT,
    context_json  TEXT,
    started_at    TEXT,
    finished_at   TEXT,
    error         TEXT
);

CREATE INDEX idx_sub_agents_run_status ON sub_agents(run_id, status);
CREATE INDEX idx_sub_agents_run_step ON sub_agents(run_id, step_idx);
"#,
    },
    Migration {
        version: 4,
        sql: r#"
ALTER TABLE tasks ADD COLUMN parent_task_id TEXT REFERENCES tasks(id);

CREATE TABLE task_links (
    source_task_id TEXT NOT NULL REFERENCES tasks(id),
    target_task_id TEXT NOT NULL REFERENCES tasks(id),
    created_at TEXT NOT NULL,
    PRIMARY KEY (source_task_id, target_task_id),
    CHECK (source_task_id < target_task_id)
);

CREATE INDEX idx_tasks_parent_task ON tasks(parent_task_id);
CREATE INDEX idx_task_links_source ON task_links(source_task_id);
CREATE INDEX idx_task_links_target ON task_links(target_task_id);
"#,
    },
    Migration {
        version: 5,
        sql: r#"
CREATE TABLE worktree_log (
    id              TEXT PRIMARY KEY,
    run_id          TEXT NOT NULL REFERENCES runs(id),
    sub_agent_id    TEXT NOT NULL,
    strategy        TEXT NOT NULL,
    branch_name     TEXT,
    base_ref        TEXT,
    worktree_path   TEXT NOT NULL,
    merge_strategy  TEXT,
    merge_success   INTEGER,
    merge_message   TEXT,
    conflicted_files_json TEXT,
    created_at      TEXT NOT NULL,
    merged_at       TEXT,
    cleaned_at      TEXT
);

CREATE INDEX idx_worktree_log_run ON worktree_log(run_id);
CREATE INDEX idx_worktree_log_subagent ON worktree_log(sub_agent_id);

ALTER TABLE sub_agents ADD COLUMN branch_name TEXT;
ALTER TABLE sub_agents ADD COLUMN merge_status TEXT;
"#,
    },
    Migration {
        version: 6,
        sql: r#"
CREATE TABLE user_messages (
    id          TEXT PRIMARY KEY,
    task_id     TEXT NOT NULL REFERENCES tasks(id),
    run_id      TEXT REFERENCES runs(id),
    content     TEXT NOT NULL,
    created_at  TEXT NOT NULL
);

CREATE INDEX idx_user_messages_task ON user_messages(task_id, created_at);
"#,
    },
];

pub(super) fn run_migrations(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version     INTEGER PRIMARY KEY,
            applied_at  TEXT NOT NULL
        );",
    )?;

    let applied: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT version FROM _migrations ORDER BY version")?;
        let result = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    for migration in MIGRATIONS {
        if applied.contains(&migration.version) {
            continue;
        }

        tracing::info!("applying migration v{}", migration.version);

        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(migration.sql)
            .map_err(|e| DbError::Migration(format!("v{}: {e}", migration.version)))?;
        tx.execute(
            "INSERT INTO _migrations (version, applied_at) VALUES (?1, datetime('now'))",
            rusqlite::params![migration.version],
        )?;
        tx.commit()?;
    }

    Ok(())
}
