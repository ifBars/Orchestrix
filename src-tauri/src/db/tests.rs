//! Database operations unit tests

#[cfg(test)]
mod tests {
    use crate::db::{queries, Database};
    use chrono;
    use uuid::Uuid;

    #[test]
    fn test_db_cascade_delete_cleans_everything() {
        let db = Database::open_in_memory().expect("in-memory DB");

        let task_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Insert task
        queries::insert_task(
            &db,
            &queries::TaskRow {
                id: task_id.clone(),
                prompt: "test cascade delete".to_string(),
                parent_task_id: None,
                status: "completed".to_string(),
                created_at: now.clone(),
                updated_at: now.clone(),
            },
        )
        .unwrap();

        // Insert run
        queries::insert_run(
            &db,
            &queries::RunRow {
                id: run_id.clone(),
                task_id: task_id.clone(),
                status: "completed".to_string(),
                plan_json: Some("{}".to_string()),
                started_at: Some(now.clone()),
                finished_at: Some(now.clone()),
                failure_reason: None,
            },
        )
        .unwrap();

        // Insert sub-agent
        let sub_agent_id = Uuid::new_v4().to_string();
        queries::insert_sub_agent(
            &db,
            &queries::SubAgentRow {
                id: sub_agent_id.clone(),
                run_id: run_id.clone(),
                step_idx: 0,
                name: "test-agent".to_string(),
                status: "completed".to_string(),
                worktree_path: Some("/tmp/test".to_string()),
                context_json: Some("{}".to_string()),
                started_at: Some(now.clone()),
                finished_at: Some(now.clone()),
                error: None,
            },
        )
        .unwrap();

        // Insert tool call
        queries::insert_tool_call(
            &db,
            &queries::ToolCallRow {
                id: Uuid::new_v4().to_string(),
                run_id: run_id.clone(),
                step_idx: Some(0),
                tool_name: "fs.write".to_string(),
                input_json: "{}".to_string(),
                output_json: Some("{}".to_string()),
                status: "succeeded".to_string(),
                started_at: Some(now.clone()),
                finished_at: Some(now.clone()),
                error: None,
            },
        )
        .unwrap();

        // Insert artifact
        queries::insert_artifact(
            &db,
            &queries::ArtifactRow {
                id: Uuid::new_v4().to_string(),
                run_id: run_id.clone(),
                kind: "test".to_string(),
                uri_or_content: "test".to_string(),
                metadata_json: None,
                created_at: now.clone(),
            },
        )
        .unwrap();

        // Insert checkpoint
        queries::upsert_checkpoint(
            &db,
            &queries::CheckpointRow {
                run_id: run_id.clone(),
                last_step_idx: 0,
                runtime_state_json: Some("{}".to_string()),
                updated_at: now.clone(),
            },
        )
        .unwrap();

        // Insert event
        queries::insert_event(
            &db,
            &queries::EventRow {
                id: Uuid::new_v4().to_string(),
                run_id: Some(run_id.clone()),
                seq: 0,
                category: "test".to_string(),
                event_type: "test.event".to_string(),
                payload_json: "{}".to_string(),
                created_at: now.clone(),
            },
        )
        .unwrap();

        // Verify everything exists
        assert!(queries::get_task(&db, &task_id).unwrap().is_some());
        assert!(!queries::list_sub_agents_for_run(&db, &run_id)
            .unwrap()
            .is_empty());

        // Cascade delete
        queries::delete_task_cascade(&db, &task_id).unwrap();

        // Everything should be gone
        assert!(queries::get_task(&db, &task_id).unwrap().is_none());
        assert!(queries::list_sub_agents_for_run(&db, &run_id)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_db_sub_agent_lifecycle() {
        let db = Database::open_in_memory().expect("in-memory DB");

        let task_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let sub_agent_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Create task and run first (foreign keys)
        queries::insert_task(
            &db,
            &queries::TaskRow {
                id: task_id.clone(),
                prompt: "test".to_string(),
                parent_task_id: None,
                status: "executing".to_string(),
                created_at: now.clone(),
                updated_at: now.clone(),
            },
        )
        .unwrap();
        queries::insert_run(
            &db,
            &queries::RunRow {
                id: run_id.clone(),
                task_id: task_id.clone(),
                status: "executing".to_string(),
                plan_json: None,
                started_at: Some(now.clone()),
                finished_at: None,
                failure_reason: None,
            },
        )
        .unwrap();

        // Insert sub-agent as queued
        queries::insert_sub_agent(
            &db,
            &queries::SubAgentRow {
                id: sub_agent_id.clone(),
                run_id: run_id.clone(),
                step_idx: 0,
                name: "agent-0".to_string(),
                status: "queued".to_string(),
                worktree_path: None,
                context_json: None,
                started_at: None,
                finished_at: None,
                error: None,
            },
        )
        .unwrap();

        let agents = queries::list_sub_agents_for_run(&db, &run_id).unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].status, "queued");

        // Mark started
        queries::mark_sub_agent_started(&db, &sub_agent_id, Some("/tmp/wt"), &now).unwrap();
        let agents = queries::list_sub_agents_for_run(&db, &run_id).unwrap();
        assert_eq!(agents[0].status, "running");
        assert_eq!(agents[0].worktree_path, Some("/tmp/wt".to_string()));

        // Mark completed
        queries::update_sub_agent_status(
            &db,
            &sub_agent_id,
            "completed",
            Some("/tmp/wt"),
            Some(&now),
            None,
        )
        .unwrap();
        let agents = queries::list_sub_agents_for_run(&db, &run_id).unwrap();
        assert_eq!(agents[0].status, "completed");
        assert!(agents[0].finished_at.is_some());

        // Cleanup
        queries::delete_task_cascade(&db, &task_id).unwrap();
    }

    #[test]
    fn test_db_checkpoint_resume() {
        let db = Database::open_in_memory().expect("in-memory DB");
        let task_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Create parent task + run for FK constraints
        queries::insert_task(
            &db,
            &queries::TaskRow {
                id: task_id.clone(),
                prompt: "test".to_string(),
                parent_task_id: None,
                status: "executing".to_string(),
                created_at: now.clone(),
                updated_at: now.clone(),
            },
        )
        .unwrap();
        queries::insert_run(
            &db,
            &queries::RunRow {
                id: run_id.clone(),
                task_id: task_id.clone(),
                status: "executing".to_string(),
                plan_json: None,
                started_at: Some(now.clone()),
                finished_at: None,
                failure_reason: None,
            },
        )
        .unwrap();

        // No checkpoint initially
        let cp = queries::get_checkpoint(&db, &run_id).unwrap();
        assert!(cp.is_none());

        // Upsert checkpoint at step 2
        queries::upsert_checkpoint(
            &db,
            &queries::CheckpointRow {
                run_id: run_id.clone(),
                last_step_idx: 2,
                runtime_state_json: Some(r#"{"status":"executing"}"#.to_string()),
                updated_at: now.clone(),
            },
        )
        .unwrap();

        let cp = queries::get_checkpoint(&db, &run_id).unwrap();
        assert!(cp.is_some());
        assert_eq!(cp.unwrap().last_step_idx, 2);

        // Upsert to step 4 (should update, not insert)
        queries::upsert_checkpoint(
            &db,
            &queries::CheckpointRow {
                run_id: run_id.clone(),
                last_step_idx: 4,
                runtime_state_json: Some(r#"{"status":"executing"}"#.to_string()),
                updated_at: now,
            },
        )
        .unwrap();

        let cp = queries::get_checkpoint(&db, &run_id).unwrap().unwrap();
        assert_eq!(cp.last_step_idx, 4);
    }

    #[test]
    fn test_db_worktree_log_lifecycle() {
        let db = Database::open_in_memory().expect("in-memory DB");
        let task_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let sub_agent_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Create parent task + run + sub_agent for FK constraints
        queries::insert_task(
            &db,
            &queries::TaskRow {
                id: task_id.clone(),
                prompt: "test".to_string(),
                parent_task_id: None,
                status: "executing".to_string(),
                created_at: now.clone(),
                updated_at: now.clone(),
            },
        )
        .unwrap();
        queries::insert_run(
            &db,
            &queries::RunRow {
                id: run_id.clone(),
                task_id: task_id.clone(),
                status: "executing".to_string(),
                plan_json: None,
                started_at: Some(now.clone()),
                finished_at: None,
                failure_reason: None,
            },
        )
        .unwrap();
        queries::insert_sub_agent(
            &db,
            &queries::SubAgentRow {
                id: sub_agent_id.clone(),
                run_id: run_id.clone(),
                step_idx: 0,
                name: "agent-0".to_string(),
                status: "running".to_string(),
                worktree_path: None,
                context_json: None,
                started_at: None,
                finished_at: None,
                error: None,
            },
        )
        .unwrap();

        // Insert worktree log
        queries::insert_worktree_log(
            &db,
            &queries::WorktreeLogRow {
                id: Uuid::new_v4().to_string(),
                run_id: run_id.clone(),
                sub_agent_id: sub_agent_id.clone(),
                strategy: "git-worktree".to_string(),
                branch_name: Some("orchestrix/abc/def".to_string()),
                base_ref: Some("abc123".to_string()),
                worktree_path: "/tmp/wt".to_string(),
                merge_strategy: None,
                merge_success: None,
                merge_message: None,
                conflicted_files_json: None,
                created_at: now.clone(),
                merged_at: None,
                cleaned_at: None,
            },
        )
        .unwrap();

        // List logs
        let logs = queries::list_worktree_logs_for_run(&db, &run_id).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].strategy, "git-worktree");
        assert!(logs[0].merge_success.is_none());

        // Update merge
        queries::update_worktree_log_merge(
            &db,
            &sub_agent_id,
            "fast-forward",
            true,
            "ok",
            None,
            &now,
        )
        .unwrap();

        let logs = queries::list_worktree_logs_for_run(&db, &run_id).unwrap();
        assert_eq!(logs[0].merge_success, Some(true));
        assert_eq!(logs[0].merge_strategy, Some("fast-forward".to_string()));

        // Update cleaned
        queries::update_worktree_log_cleaned(&db, &sub_agent_id, &now).unwrap();
        let logs = queries::list_worktree_logs_for_run(&db, &run_id).unwrap();
        assert!(logs[0].cleaned_at.is_some());
    }
}
