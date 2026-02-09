//! PolicyEngine unit tests

#[cfg(test)]
mod tests {
    use crate::policy::PolicyEngine;
    use crate::runtime::worktree::WorktreeManager;
    use crate::tests::{cleanup, init_git_repo, temp_workspace};
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn test_policy_scoped_to_worktree() {
        let workspace = temp_workspace();
        init_git_repo(&workspace);

        let manager = WorktreeManager::new();
        let run_id = Uuid::new_v4().to_string();
        let agent_id = Uuid::new_v4().to_string();

        let info = manager
            .create_worktree(&workspace, &run_id, &agent_id)
            .unwrap();
        let policy = PolicyEngine::new(info.path.clone());

        let inside = info.path.join("some-file.txt");
        match policy.evaluate_path(&inside) {
            crate::policy::PolicyDecision::Allow => {}
            crate::policy::PolicyDecision::Deny(reason) => panic!("should be allowed: {reason}"),
            crate::policy::PolicyDecision::NeedsApproval { reason, .. } => {
                panic!("should not require approval for inside path: {reason}")
            }
        }

        let truly_outside = PathBuf::from(r"C:\Windows\System32\notepad.exe");
        match policy.evaluate_path(&truly_outside) {
            crate::policy::PolicyDecision::Allow => {
                panic!("system path should not be auto-allowed")
            }
            crate::policy::PolicyDecision::NeedsApproval { .. } => {}
            crate::policy::PolicyDecision::Deny(reason) => {
                panic!("outside path should require approval, got hard deny: {reason}")
            }
        }

        cleanup(&workspace);
    }
}
