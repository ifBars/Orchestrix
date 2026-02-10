//! Test helpers and utilities for integration tests.
//!
//! This module provides common utilities used across all test files.

use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

#[cfg(test)]
mod integration;

#[cfg(test)]
mod agentic;

#[cfg(test)]
mod events;

#[cfg(test)]
mod tools;

#[cfg(test)]
mod providers;

/// Mutex to ensure skills tests run serially and don't interfere with each other
/// via the ORCHESTRIX_SKILLS_PATH environment variable.
pub static SKILLS_TEST_MUTEX: Mutex<()> = Mutex::new(());

/// Load the API key from the well-known path.
pub fn load_api_key() -> String {
    let path = r"C:\Users\ghost\Desktop\Coding\minimax-key.txt";
    std::fs::read_to_string(path)
        .expect("API key file not found")
        .trim()
        .to_string()
}

/// Create a temp directory for test workspaces.
pub fn temp_workspace() -> PathBuf {
    let dir = std::env::temp_dir()
        .join("orchestrix-test")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

/// Initialize a git repo in the given directory.
pub fn init_git_repo(dir: &std::path::Path) {
    let output = std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("git init failed");
    assert!(
        output.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Configure git user for commits
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "test@orchestrix.local"])
        .current_dir(dir)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Orchestrix Test"])
        .current_dir(dir)
        .output();

    // Create initial commit so HEAD exists
    std::fs::write(dir.join("README.md"), "# Test Repo\n").expect("write failed");
    let _ = std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .output();
    let _ = std::process::Command::new("git")
        .args(["commit", "-m", "initial commit"])
        .current_dir(dir)
        .output();
}

/// Cleanup a temp workspace.
pub fn cleanup(dir: &std::path::Path) {
    let _ = std::process::Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(dir)
        .output();
    let _ = std::fs::remove_dir_all(dir);
}

/// Helper: set ORCHESTRIX_SKILLS_PATH to a temp file so tests don't
/// interfere with real custom skills or each other.
/// Returns (path, _guard) where guard holds the mutex to ensure serial execution.
pub fn isolated_skills_path() -> (PathBuf, std::sync::MutexGuard<'static, ()>) {
    let guard = SKILLS_TEST_MUTEX.lock().unwrap();
    let dir = std::env::temp_dir()
        .join("orchestrix-test-skills")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).expect("create skills test dir");
    let path = dir.join("custom-skills-v1.json");
    std::env::set_var("ORCHESTRIX_SKILLS_PATH", &path);
    (path, guard)
}

/// Cleanup ORCHESTRIX_SKILLS_PATH env and the temp dir.
pub fn cleanup_skills_env(path: &std::path::Path) {
    std::env::remove_var("ORCHESTRIX_SKILLS_PATH");
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}
