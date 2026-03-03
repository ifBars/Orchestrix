//! Git tools for repository operations.

use std::path::Path;
use std::process::Command;

use crate::core::tool::ToolDescriptor;
use crate::policy::{PolicyDecision, PolicyEngine};
use crate::tools::args::{
    schema_for_type, GitApplyPatchArgs, GitCommitArgs, GitDiffArgs, GitLogArgs,
};
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

fn run_git(policy: &PolicyEngine, cwd: &Path, args: &[&str]) -> Result<ToolCallOutput, ToolError> {
    match policy.evaluate_path(cwd) {
        PolicyDecision::Allow => {}
        PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
        PolicyDecision::NeedsApproval { scope, reason } => {
            return Err(ToolError::ApprovalRequired { scope, reason })
        }
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .map_err(|e| ToolError::Execution(format!("git failed: {e}")))?;

    Ok(ToolCallOutput {
        ok: output.status.success(),
        data: serde_json::json!({
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
        }),
        error: if output.status.success() {
            None
        } else {
            Some(String::from_utf8_lossy(&output.stderr).to_string())
        },
    })
}

/// Tool for running git status.
pub struct GitStatusTool;

impl Tool for GitStatusTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "git.status".into(),
            description: "Run git status --short. Shows modified, added, and deleted files in the current worktree.".into(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        _: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        run_git(policy, cwd, &["status", "--short"])
    }
}

/// Tool for running git diff.
pub struct GitDiffTool;

impl Tool for GitDiffTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "git.diff".into(),
            description: "Run git diff. Shows unstaged changes in the current worktree. Pass {\"staged\": true} to see staged changes.".into(),
            input_schema: schema_for_type::<GitDiffArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: GitDiffArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        if args.staged.unwrap_or(false) {
            run_git(policy, cwd, &["diff", "--cached"])
        } else {
            run_git(policy, cwd, &["diff"])
        }
    }
}

/// Tool for applying patches via git apply.
pub struct GitApplyPatchTool;

impl Tool for GitApplyPatchTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "git.apply_patch".into(),
            description: "Apply patch via git apply".into(),
            input_schema: schema_for_type::<GitApplyPatchArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: GitApplyPatchArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let patch_path = cwd.join(".orchestrix").join("patch.diff");
        if let Some(parent) = patch_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::Execution(e.to_string()))?;
        }
        std::fs::write(&patch_path, &args.patch)
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        run_git(policy, cwd, &["apply", &patch_path.to_string_lossy()])
    }
}

/// Tool for committing changes.
pub struct GitCommitTool;

impl Tool for GitCommitTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "git.commit".into(),
            description: "Stage all changes and commit in the current worktree. This is useful inside agent worktrees to checkpoint progress.".into(),
            input_schema: schema_for_type::<GitCommitArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: GitCommitArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        run_git(policy, cwd, &["add", "-A"])?;

        let output = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .arg("commit")
            .arg("-m")
            .arg(&args.message)
            .env("GIT_AUTHOR_NAME", "Orchestrix")
            .env("GIT_AUTHOR_EMAIL", "orchestrix@local")
            .env("GIT_COMMITTER_NAME", "Orchestrix")
            .env("GIT_COMMITTER_EMAIL", "orchestrix@local")
            .output()
            .map_err(|e| ToolError::Execution(format!("git commit failed: {e}")))?;

        Ok(ToolCallOutput {
            ok: output.status.success(),
            data: serde_json::json!({
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr),
                "code": output.status.code(),
            }),
            error: if output.status.success() {
                None
            } else {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            },
        })
    }
}

/// Tool for viewing git log.
pub struct GitLogTool;

impl Tool for GitLogTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "git.log".into(),
            description: "Show recent git log entries. Defaults to 10 entries in oneline format."
                .into(),
            input_schema: schema_for_type::<GitLogArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: GitLogArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let count = args.count.unwrap_or(10);
        let count_str = format!("-{count}");
        run_git(policy, cwd, &["log", "--oneline", &count_str])
    }
}
