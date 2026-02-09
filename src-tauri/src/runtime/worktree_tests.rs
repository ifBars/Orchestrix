//! WorktreeManager unit tests

#[cfg(test)]
mod tests {
    use crate::runtime::worktree::{WorktreeManager, WorktreeStrategy};
    use crate::tests::{cleanup, init_git_repo, temp_workspace};
    use uuid::Uuid;

    #[test]
    fn test_worktree_manager_creates_isolated_dir_without_git() {
        let workspace = temp_workspace();
        let manager = WorktreeManager::new();
        let run_id = Uuid::new_v4().to_string();
        let agent_id = Uuid::new_v4().to_string();

        let info = manager
            .create_worktree(&workspace, &run_id, &agent_id)
            .expect("create_worktree failed");

        assert_eq!(info.strategy, WorktreeStrategy::IsolatedDir);
        assert!(info.path.exists());
        assert!(info.branch.is_none());
        assert_eq!(info.run_id, run_id);
        assert_eq!(info.sub_agent_id, agent_id);

        let active = manager.list_active();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].sub_agent_id, agent_id);

        cleanup(&workspace);
    }

    #[test]
    fn test_worktree_manager_creates_git_worktree_with_branch() {
        let workspace = temp_workspace();
        init_git_repo(&workspace);

        let manager = WorktreeManager::new();
        let run_id = Uuid::new_v4().to_string();
        let agent_id = Uuid::new_v4().to_string();

        let info = manager
            .create_worktree(&workspace, &run_id, &agent_id)
            .expect("create_worktree failed");

        assert_eq!(info.strategy, WorktreeStrategy::GitWorktree);
        assert!(info.branch.is_some());
        assert!(info.path.exists());
        assert!(info.base_ref.is_some());

        let branch = info.branch.as_ref().unwrap();
        assert!(branch.starts_with("orchestrix/"), "branch name: {branch}");

        let output = std::process::Command::new("git")
            .args(["worktree", "list"])
            .current_dir(&workspace)
            .output()
            .expect("git worktree list failed");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let normalized_path = info.path.to_string_lossy().replace('\\', "/");
        let normalized_stdout = stdout.replace('\\', "/");
        assert!(normalized_stdout.contains(&normalized_path),
            "worktree path should appear in git worktree list.\n  looking for: {normalized_path}\n  in: {normalized_stdout}");

        cleanup(&workspace);
    }

    #[test]
    fn test_worktree_manager_reclaims_existing() {
        let workspace = temp_workspace();
        init_git_repo(&workspace);

        let manager = WorktreeManager::new();
        let run_id = Uuid::new_v4().to_string();
        let agent_id = Uuid::new_v4().to_string();

        let info1 = manager
            .create_worktree(&workspace, &run_id, &agent_id)
            .expect("first");
        let info2 = manager
            .create_worktree(&workspace, &run_id, &agent_id)
            .expect("second");

        assert_eq!(info1.path, info2.path);
        assert_eq!(info2.strategy, WorktreeStrategy::GitWorktree);
        assert_eq!(manager.list_active().len(), 1);

        cleanup(&workspace);
    }

    #[test]
    fn test_multiple_agents_get_separate_branches() {
        let workspace = temp_workspace();
        init_git_repo(&workspace);

        let manager = WorktreeManager::new();
        let run_id = Uuid::new_v4().to_string();
        let agent_a = Uuid::new_v4().to_string();
        let agent_b = Uuid::new_v4().to_string();
        let agent_c = Uuid::new_v4().to_string();

        let info_a = manager
            .create_worktree(&workspace, &run_id, &agent_a)
            .unwrap();
        let info_b = manager
            .create_worktree(&workspace, &run_id, &agent_b)
            .unwrap();
        let info_c = manager
            .create_worktree(&workspace, &run_id, &agent_c)
            .unwrap();

        assert_eq!(info_a.strategy, WorktreeStrategy::GitWorktree);
        assert_eq!(info_b.strategy, WorktreeStrategy::GitWorktree);
        assert_eq!(info_c.strategy, WorktreeStrategy::GitWorktree);

        let branches: Vec<&str> = [&info_a, &info_b, &info_c]
            .iter()
            .map(|i| i.branch.as_deref().unwrap())
            .collect();
        assert_ne!(branches[0], branches[1]);
        assert_ne!(branches[1], branches[2]);
        assert_ne!(branches[0], branches[2]);

        assert_ne!(info_a.path, info_b.path);
        assert_ne!(info_b.path, info_c.path);

        assert_eq!(manager.list_active().len(), 3);
        assert_eq!(manager.list_for_run(&run_id).len(), 3);

        cleanup(&workspace);
    }

    #[test]
    fn test_worktree_merge_fast_forward() {
        let workspace = temp_workspace();
        init_git_repo(&workspace);

        let manager = WorktreeManager::new();
        let run_id = Uuid::new_v4().to_string();
        let agent_id = Uuid::new_v4().to_string();

        let info = manager
            .create_worktree(&workspace, &run_id, &agent_id)
            .unwrap();
        assert_eq!(info.strategy, WorktreeStrategy::GitWorktree);

        std::fs::write(info.path.join("agent-output.txt"), "Hello from agent\n").unwrap();

        let result = manager.merge_worktree(&workspace, &agent_id).unwrap();
        assert!(result.success);
        assert!(result.conflicted_files.is_empty());
        assert!(workspace.join("agent-output.txt").exists());

        cleanup(&workspace);
    }

    #[test]
    fn test_worktree_merge_conflict_detection() {
        let workspace = temp_workspace();
        init_git_repo(&workspace);

        let manager = WorktreeManager::new();
        let run_id = Uuid::new_v4().to_string();
        let agent_a = Uuid::new_v4().to_string();
        let agent_b = Uuid::new_v4().to_string();

        let info_a = manager
            .create_worktree(&workspace, &run_id, &agent_a)
            .unwrap();
        let info_b = manager
            .create_worktree(&workspace, &run_id, &agent_b)
            .unwrap();

        std::fs::write(info_a.path.join("shared.txt"), "Agent A was here\n").unwrap();
        std::fs::write(info_b.path.join("shared.txt"), "Agent B was here\n").unwrap();

        let result_a = manager.merge_worktree(&workspace, &agent_a).unwrap();
        assert!(result_a.success, "first merge should succeed");

        let result_b = manager.merge_worktree(&workspace, &agent_b).unwrap();
        if !result_b.success {
            assert!(!result_b.conflicted_files.is_empty());
        }

        cleanup(&workspace);
    }

    #[test]
    fn test_worktree_cleanup() {
        let workspace = temp_workspace();
        init_git_repo(&workspace);

        let manager = WorktreeManager::new();
        let run_id = Uuid::new_v4().to_string();
        let agent_a = Uuid::new_v4().to_string();
        let agent_b = Uuid::new_v4().to_string();

        let info_a = manager
            .create_worktree(&workspace, &run_id, &agent_a)
            .unwrap();
        let info_b = manager
            .create_worktree(&workspace, &run_id, &agent_b)
            .unwrap();

        assert_eq!(manager.list_active().len(), 2);

        let cleaned = manager.cleanup_run(&workspace, &run_id).unwrap();
        assert_eq!(cleaned.len(), 2);
        assert_eq!(manager.list_active().len(), 0);
        assert!(!info_a.path.exists());
        assert!(!info_b.path.exists());

        cleanup(&workspace);
    }
}
