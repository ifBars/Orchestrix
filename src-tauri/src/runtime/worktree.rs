use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Strategy used to create the sub-agent workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreeStrategy {
    /// Reused a previously-created worktree (recovery scenario).
    Existing,
    /// Created via `git worktree add -b <branch> <path> HEAD`.
    GitWorktree,
    /// Plain directory fallback (no `.git` in workspace root).
    IsolatedDir,
}

impl std::fmt::Display for WorktreeStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Existing => write!(f, "existing"),
            Self::GitWorktree => write!(f, "git-worktree"),
            Self::IsolatedDir => write!(f, "isolated-dir"),
        }
    }
}

/// Metadata for a single sub-agent worktree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    /// Absolute path to the worktree directory.
    pub path: PathBuf,
    /// The branch name created for this worktree (if git-backed).
    pub branch: Option<String>,
    /// Which strategy was used to create it.
    pub strategy: WorktreeStrategy,
    /// The run that owns this worktree.
    pub run_id: String,
    /// The sub-agent that owns this worktree.
    pub sub_agent_id: String,
    /// Base commit (HEAD at creation time).
    pub base_ref: Option<String>,
}

/// Result of merging a worktree branch back to the base branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    pub success: bool,
    pub strategy: MergeStrategy,
    pub message: String,
    /// Files that conflicted (empty if merge succeeded).
    pub conflicted_files: Vec<String>,
}

/// How the merge was performed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MergeStrategy {
    FastForward,
    ThreeWayMerge,
    Conflict,
    NoBranch,
    Skipped,
}

impl std::fmt::Display for MergeStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FastForward => write!(f, "fast-forward"),
            Self::ThreeWayMerge => write!(f, "three-way-merge"),
            Self::Conflict => write!(f, "conflict"),
            Self::NoBranch => write!(f, "no-branch"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

#[derive(Debug, Error)]
pub enum WorktreeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to create worktree: {0}")]
    Create(String),
    #[error("git command failed: {0}")]
    Git(String),
    #[error("worktree not found: {0}")]
    NotFound(String),
}

// ---------------------------------------------------------------------------
// WorktreeManager -- thread-safe registry of active worktrees
// ---------------------------------------------------------------------------

/// Centralized manager that tracks all active worktrees. This is held in the
/// Orchestrator and shared across sub-agent tasks via `Arc`.
///
/// Key responsibilities:
/// - Create branch-per-agent worktrees so agents never conflict
/// - Track which worktrees are active (prevents double-create on recovery)
/// - Merge completed worktree branches back to the base branch
/// - Clean up stale worktrees
pub struct WorktreeManager {
    /// Map from sub_agent_id -> WorktreeInfo for all active worktrees.
    active: Mutex<HashMap<String, WorktreeInfo>>,
}

impl WorktreeManager {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(HashMap::new()),
        }
    }

    /// Return a snapshot of all active worktrees.
    pub fn list_active(&self) -> Vec<WorktreeInfo> {
        let guard = self.active.lock().expect("worktree manager mutex poisoned");
        guard.values().cloned().collect()
    }

    /// Return a snapshot of active worktrees for a specific run.
    pub fn list_for_run(&self, run_id: &str) -> Vec<WorktreeInfo> {
        let guard = self.active.lock().expect("worktree manager mutex poisoned");
        guard
            .values()
            .filter(|w| w.run_id == run_id)
            .cloned()
            .collect()
    }

    /// Create or reclaim a worktree for a sub-agent.
    ///
    /// Each sub-agent gets its own *branch* derived from HEAD so agents
    /// working in parallel never write to the same git index. The branch name
    /// follows the pattern `orchestrix/<run_id_short>/<sub_agent_id_short>`.
    ///
    /// Layout on disk:
    /// ```text
    /// <workspace>/.orchestrix/worktrees/<run_id>/<sub_agent_id>/
    /// ```
    pub fn create_worktree(
        &self,
        workspace_root: &Path,
        run_id: &str,
        sub_agent_id: &str,
    ) -> Result<WorktreeInfo, WorktreeError> {
        // Check if already tracked (recovery / restart).
        {
            let guard = self.active.lock().expect("worktree manager mutex poisoned");
            if let Some(existing) = guard.get(sub_agent_id) {
                if existing.path.exists() {
                    return Ok(existing.clone());
                }
            }
        }

        let base_dir = workspace_root
            .join(".orchestrix")
            .join("worktrees")
            .join(run_id);
        std::fs::create_dir_all(&base_dir)?;

        let target = base_dir.join(sub_agent_id);

        // If the directory already exists on disk, reclaim it.
        if target.exists() {
            let info = WorktreeInfo {
                path: target,
                branch: detect_branch_name(workspace_root, sub_agent_id),
                strategy: WorktreeStrategy::Existing,
                run_id: run_id.to_string(),
                sub_agent_id: sub_agent_id.to_string(),
                base_ref: resolve_head(workspace_root),
            };
            let mut guard = self.active.lock().expect("worktree manager mutex poisoned");
            guard.insert(sub_agent_id.to_string(), info.clone());
            return Ok(info);
        }

        // Try git worktree with a dedicated branch.
        if workspace_root.join(".git").exists() {
            let branch_name = make_branch_name(run_id, sub_agent_id);
            let base_ref = resolve_head(workspace_root).unwrap_or_else(|| "HEAD".to_string());

            let result = create_git_worktree(workspace_root, &target, &branch_name, &base_ref);
            match result {
                Ok(()) => {
                    let info = WorktreeInfo {
                        path: target,
                        branch: Some(branch_name),
                        strategy: WorktreeStrategy::GitWorktree,
                        run_id: run_id.to_string(),
                        sub_agent_id: sub_agent_id.to_string(),
                        base_ref: Some(base_ref),
                    };
                    let mut guard = self.active.lock().expect("worktree manager mutex poisoned");
                    guard.insert(sub_agent_id.to_string(), info.clone());
                    return Ok(info);
                }
                Err(error) => {
                    tracing::warn!(
                        "git worktree creation failed, falling back to isolated dir: {error}"
                    );
                }
            }
        }

        // Fallback: plain isolated directory. Copy a workspace snapshot so
        // non-git sub-agents can still read/modify project files in isolation.
        std::fs::create_dir_all(&target)?;
        let copied = copy_workspace_snapshot_for_isolated_dir(workspace_root, &target)?;
        tracing::info!(
            "created isolated worktree snapshot for sub-agent {} ({} files)",
            sub_agent_id,
            copied
        );
        let info = WorktreeInfo {
            path: target,
            branch: None,
            strategy: WorktreeStrategy::IsolatedDir,
            run_id: run_id.to_string(),
            sub_agent_id: sub_agent_id.to_string(),
            base_ref: None,
        };
        let mut guard = self.active.lock().expect("worktree manager mutex poisoned");
        guard.insert(sub_agent_id.to_string(), info.clone());
        Ok(info)
    }

    /// Commit all staged + unstaged changes in the worktree, then merge the
    /// sub-agent's branch back into the original branch that HEAD pointed to
    /// at creation time.
    ///
    /// This is called after a sub-agent completes successfully. If the merge
    /// has conflicts, they are reported but the merge is aborted so the main
    /// branch stays clean.
    pub fn merge_worktree(
        &self,
        workspace_root: &Path,
        sub_agent_id: &str,
    ) -> Result<MergeResult, WorktreeError> {
        let info = {
            let guard = self.active.lock().expect("worktree manager mutex poisoned");
            guard
                .get(sub_agent_id)
                .cloned()
                .ok_or_else(|| WorktreeError::NotFound(sub_agent_id.to_string()))?
        };

        let Some(branch) = &info.branch else {
            let synced = sync_isolated_worktree_to_workspace(&info.path, workspace_root)?;
            return Ok(MergeResult {
                success: true,
                strategy: MergeStrategy::NoBranch,
                message: format!(
                    "non-git worktree synchronized {} file(s) to workspace",
                    synced
                ),
                conflicted_files: vec![],
            });
        };

        // 1. Auto-commit any outstanding changes in the worktree.
        auto_commit_worktree(&info.path, &info.sub_agent_id)?;

        // 2. Check if the branch has any commits beyond the base.
        let has_changes = branch_has_commits(workspace_root, branch, info.base_ref.as_deref());
        if !has_changes {
            // There may still be ignored/untracked files that were intentionally
            // produced by the sub-agent but not committed. Sync them by content.
            let synced = sync_isolated_worktree_to_workspace(&info.path, workspace_root)?;
            return Ok(MergeResult {
                success: true,
                strategy: MergeStrategy::Skipped,
                message: if synced > 0 {
                    format!(
                        "no new commits on agent branch; synchronized {} file(s)",
                        synced
                    )
                } else {
                    "no new commits on agent branch".to_string()
                },
                conflicted_files: vec![],
            });
        }

        // 3. Attempt merge in the main workspace.
        merge_branch_into_main(workspace_root, branch)
    }

    /// Remove a worktree from disk and from git's worktree list.
    /// Also removes the tracking entry.
    pub fn remove_worktree(
        &self,
        workspace_root: &Path,
        sub_agent_id: &str,
    ) -> Result<(), WorktreeError> {
        let info = {
            let mut guard = self.active.lock().expect("worktree manager mutex poisoned");
            guard.remove(sub_agent_id)
        };

        if let Some(info) = info {
            // Remove the git worktree registration.
            if info.strategy == WorktreeStrategy::GitWorktree {
                let _ = run_git_command(
                    workspace_root,
                    &[
                        "worktree",
                        "remove",
                        "--force",
                        &info.path.to_string_lossy(),
                    ],
                );
            }

            // Delete the branch if it exists.
            if let Some(branch) = &info.branch {
                let _ = run_git_command(workspace_root, &["branch", "-D", branch]);
            }

            // Remove the directory if it still exists.
            if info.path.exists() {
                let _ = std::fs::remove_dir_all(&info.path);
            }
        }

        Ok(())
    }

    /// Clean up all worktrees for a specific run.
    pub fn cleanup_run(
        &self,
        workspace_root: &Path,
        run_id: &str,
    ) -> Result<Vec<String>, WorktreeError> {
        let sub_agent_ids: Vec<String> = {
            let guard = self.active.lock().expect("worktree manager mutex poisoned");
            guard
                .values()
                .filter(|w| w.run_id == run_id)
                .map(|w| w.sub_agent_id.clone())
                .collect()
        };

        let mut cleaned = Vec::new();
        for id in &sub_agent_ids {
            if let Err(error) = self.remove_worktree(workspace_root, id) {
                tracing::warn!("failed to remove worktree for sub-agent {id}: {error}");
            } else {
                cleaned.push(id.clone());
            }
        }

        // Also remove the run-level directory.
        let run_dir = workspace_root
            .join(".orchestrix")
            .join("worktrees")
            .join(run_id);
        if run_dir.exists() {
            let _ = std::fs::remove_dir_all(&run_dir);
        }

        // Prune any stale worktree references git still has.
        let _ = run_git_command(workspace_root, &["worktree", "prune"]);

        Ok(cleaned)
    }

    /// Detect and clean up stale worktrees that have no running sub-agent.
    pub fn prune_stale(&self, workspace_root: &Path) -> Result<Vec<String>, WorktreeError> {
        let _ = run_git_command(workspace_root, &["worktree", "prune"]);

        // List all orchestrix branches that aren't tracked by any active worktree.
        let all_branches = list_orchestrix_branches(workspace_root);
        let active_branches: Vec<String> = {
            let guard = self.active.lock().expect("worktree manager mutex poisoned");
            guard.values().filter_map(|w| w.branch.clone()).collect()
        };

        let mut pruned = Vec::new();
        for branch in all_branches {
            if !active_branches.contains(&branch) {
                let _ = run_git_command(workspace_root, &["branch", "-D", &branch]);
                pruned.push(branch);
            }
        }

        Ok(pruned)
    }
}

// ---------------------------------------------------------------------------
// Git helpers
// ---------------------------------------------------------------------------

/// Deterministic branch name: `orchestrix/<run_short>/<agent_short>`.
fn make_branch_name(run_id: &str, sub_agent_id: &str) -> String {
    let run_short = &run_id[..run_id.len().min(8)];
    let agent_short = &sub_agent_id[..sub_agent_id.len().min(8)];
    format!("orchestrix/{run_short}/{agent_short}")
}

/// Resolve HEAD to a commit hash.
fn resolve_head(workspace_root: &Path) -> Option<String> {
    let output = run_git_command(workspace_root, &["rev-parse", "HEAD"]).ok()?;
    let hash = output.trim().to_string();
    if hash.is_empty() {
        None
    } else {
        Some(hash)
    }
}

/// Detect the current branch of a worktree directory.
fn detect_branch_name(workspace_root: &Path, _sub_agent_id: &str) -> Option<String> {
    // Try to find branches matching the orchestrix pattern.
    let output = run_git_command(workspace_root, &["branch", "--list", "orchestrix/*"]).ok()?;
    let branches: Vec<&str> = output
        .lines()
        .map(|l| l.trim().trim_start_matches("* "))
        .collect();
    // Return the first match containing the agent id fragment.
    let agent_short = &_sub_agent_id[.._sub_agent_id.len().min(8)];
    branches
        .into_iter()
        .find(|b| b.contains(agent_short))
        .map(|b| b.to_string())
}

/// Create a git worktree with a new branch.
fn create_git_worktree(
    workspace_root: &Path,
    target: &Path,
    branch_name: &str,
    base_ref: &str,
) -> Result<(), WorktreeError> {
    // First, make sure the branch doesn't already exist (stale from a previous run).
    let _ = run_git_command(workspace_root, &["branch", "-D", branch_name]);

    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .arg("worktree")
        .arg("add")
        .arg("-b")
        .arg(branch_name)
        .arg(target)
        .arg(base_ref)
        .output()
        .map_err(|e| WorktreeError::Git(format!("failed to run git worktree add: {e}")))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(WorktreeError::Create(format!(
            "git worktree add -b {branch_name} failed: {stderr}"
        )))
    }
}

/// Auto-commit all changes in a worktree directory.
fn auto_commit_worktree(worktree_path: &Path, sub_agent_id: &str) -> Result<(), WorktreeError> {
    // Stage all changes.
    let _ = run_git_command(worktree_path, &["add", "-A"]);

    // Check if there's anything to commit.
    let status = run_git_command(worktree_path, &["status", "--porcelain"]).unwrap_or_default();
    if status.trim().is_empty() {
        return Ok(());
    }

    let agent_short = &sub_agent_id[..sub_agent_id.len().min(8)];
    let message = format!("orchestrix: auto-commit from sub-agent {agent_short}");

    let output = Command::new("git")
        .arg("-C")
        .arg(worktree_path)
        .arg("commit")
        .arg("-m")
        .arg(&message)
        .arg("--allow-empty")
        .env("GIT_AUTHOR_NAME", "Orchestrix")
        .env("GIT_AUTHOR_EMAIL", "orchestrix@local")
        .env("GIT_COMMITTER_NAME", "Orchestrix")
        .env("GIT_COMMITTER_EMAIL", "orchestrix@local")
        .output()
        .map_err(|e| WorktreeError::Git(format!("auto-commit failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // "nothing to commit" is fine
        if !stderr.contains("nothing to commit") {
            tracing::warn!("auto-commit in worktree had issues: {stderr}");
        }
    }

    Ok(())
}

/// Check if a branch has any commits beyond a base ref.
fn branch_has_commits(workspace_root: &Path, branch: &str, base_ref: Option<&str>) -> bool {
    let base = base_ref.unwrap_or("HEAD");
    let range = format!("{base}..{branch}");
    let output = run_git_command(workspace_root, &["rev-list", "--count", &range]);
    match output {
        Ok(count_str) => {
            let count: u64 = count_str.trim().parse().unwrap_or(0);
            count > 0
        }
        Err(_) => false,
    }
}

/// Merge a branch back into the current branch of the main workspace.
fn merge_branch_into_main(
    workspace_root: &Path,
    branch: &str,
) -> Result<MergeResult, WorktreeError> {
    // Try fast-forward first.
    let ff_output = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .arg("merge")
        .arg("--ff-only")
        .arg(branch)
        .output()
        .map_err(|e| WorktreeError::Git(format!("merge --ff-only failed: {e}")))?;

    if ff_output.status.success() {
        return Ok(MergeResult {
            success: true,
            strategy: MergeStrategy::FastForward,
            message: format!("fast-forward merged {branch}"),
            conflicted_files: vec![],
        });
    }

    // Try a regular merge (no fast-forward possible, concurrent branches diverged).
    let merge_output = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .arg("merge")
        .arg("--no-edit")
        .arg(branch)
        .env("GIT_AUTHOR_NAME", "Orchestrix")
        .env("GIT_AUTHOR_EMAIL", "orchestrix@local")
        .env("GIT_COMMITTER_NAME", "Orchestrix")
        .env("GIT_COMMITTER_EMAIL", "orchestrix@local")
        .output()
        .map_err(|e| WorktreeError::Git(format!("merge failed: {e}")))?;

    if merge_output.status.success() {
        return Ok(MergeResult {
            success: true,
            strategy: MergeStrategy::ThreeWayMerge,
            message: format!("three-way merged {branch}"),
            conflicted_files: vec![],
        });
    }

    // Merge had conflicts. Collect the conflicted files, then abort.
    let conflicted = collect_conflict_files(workspace_root);

    // Abort the merge to leave the workspace clean.
    let _ = run_git_command(workspace_root, &["merge", "--abort"]);

    Ok(MergeResult {
        success: false,
        strategy: MergeStrategy::Conflict,
        message: format!(
            "merge of {branch} had {} conflict(s) -- aborted",
            conflicted.len()
        ),
        conflicted_files: conflicted,
    })
}

/// List files in a conflicted merge state.
fn collect_conflict_files(workspace_root: &Path) -> Vec<String> {
    let output = run_git_command(workspace_root, &["diff", "--name-only", "--diff-filter=U"]);
    match output {
        Ok(text) => text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect(),
        Err(_) => vec![],
    }
}

/// List all branches matching the `orchestrix/*` pattern.
fn list_orchestrix_branches(workspace_root: &Path) -> Vec<String> {
    let output = run_git_command(
        workspace_root,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads/orchestrix/",
        ],
    );
    match output {
        Ok(text) => text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect(),
        Err(_) => vec![],
    }
}

/// List all git worktrees registered in the repo.
pub fn list_git_worktrees(workspace_root: &Path) -> Result<Vec<GitWorktreeEntry>, WorktreeError> {
    let output = run_git_command(workspace_root, &["worktree", "list", "--porcelain"])
        .map_err(|e| WorktreeError::Git(e))?;

    let mut entries = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_head: Option<String> = None;
    let mut current_branch: Option<String> = None;
    let mut is_bare = false;

    for line in output.lines() {
        if line.starts_with("worktree ") {
            // Save the previous entry if any.
            if let Some(path) = current_path.take() {
                entries.push(GitWorktreeEntry {
                    path,
                    head: current_head.take(),
                    branch: current_branch.take(),
                    is_bare,
                });
                is_bare = false;
            }
            current_path = Some(line.trim_start_matches("worktree ").to_string());
        } else if line.starts_with("HEAD ") {
            current_head = Some(line.trim_start_matches("HEAD ").to_string());
        } else if line.starts_with("branch ") {
            current_branch = Some(line.trim_start_matches("branch ").to_string());
        } else if line == "bare" {
            is_bare = true;
        }
    }

    // Push the last entry.
    if let Some(path) = current_path {
        entries.push(GitWorktreeEntry {
            path,
            head: current_head,
            branch: current_branch,
            is_bare,
        });
    }

    Ok(entries)
}

/// A parsed git worktree entry from `git worktree list --porcelain`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitWorktreeEntry {
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub is_bare: bool,
}

/// Run a git command and return its stdout as a String.
fn run_git_command(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .map_err(|e| format!("failed to execute git: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn should_skip_snapshot_entry(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some(".git") | Some(".orchestrix")
    )
}

fn copy_workspace_snapshot_for_isolated_dir(
    workspace_root: &Path,
    isolated_root: &Path,
) -> Result<u64, WorktreeError> {
    copy_tree(workspace_root, isolated_root, true)
}

fn sync_isolated_worktree_to_workspace(
    isolated_root: &Path,
    workspace_root: &Path,
) -> Result<u64, WorktreeError> {
    copy_tree(isolated_root, workspace_root, false)
}

fn copy_tree(src_root: &Path, dst_root: &Path, copy_all: bool) -> Result<u64, WorktreeError> {
    let mut copied = 0_u64;
    copy_tree_inner(src_root, dst_root, copy_all, &mut copied)?;
    Ok(copied)
}

fn copy_tree_inner(
    src: &Path,
    dst: &Path,
    copy_all: bool,
    copied: &mut u64,
) -> Result<(), WorktreeError> {
    let entries = std::fs::read_dir(src)?;
    for entry in entries {
        let entry = entry?;
        let src_path = entry.path();
        if should_skip_snapshot_entry(&src_path) {
            continue;
        }

        let file_name = entry.file_name();
        let dst_path = dst.join(file_name);
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            std::fs::create_dir_all(&dst_path)?;
            copy_tree_inner(&src_path, &dst_path, copy_all, copied)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let needs_copy = if copy_all {
            true
        } else {
            !files_equal(&src_path, &dst_path)?
        };

        if needs_copy {
            if let Some(parent) = dst_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src_path, &dst_path)?;
            *copied += 1;
        }
    }
    Ok(())
}

fn files_equal(a: &Path, b: &Path) -> Result<bool, std::io::Error> {
    if !b.exists() {
        return Ok(false);
    }
    let a_meta = std::fs::metadata(a)?;
    let b_meta = std::fs::metadata(b)?;
    if a_meta.len() != b_meta.len() {
        return Ok(false);
    }

    let a_bytes = std::fs::read(a)?;
    let b_bytes = std::fs::read(b)?;
    Ok(a_bytes == b_bytes)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_name_format() {
        let name = make_branch_name(
            "abcdef12-3456-7890-abcd-ef1234567890",
            "01234567-abcd-0000-0000-000000000000",
        );
        assert_eq!(name, "orchestrix/abcdef12/01234567");
    }

    #[test]
    fn test_worktree_strategy_display() {
        assert_eq!(WorktreeStrategy::GitWorktree.to_string(), "git-worktree");
        assert_eq!(WorktreeStrategy::IsolatedDir.to_string(), "isolated-dir");
        assert_eq!(WorktreeStrategy::Existing.to_string(), "existing");
    }
}
