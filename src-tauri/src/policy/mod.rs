use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allow,
    Deny(String),
    NeedsApproval { scope: String, reason: String },
}

#[derive(Debug, Clone)]
pub struct PolicyEngine {
    workspace_root: PathBuf,
    approved_scopes: Arc<Mutex<HashSet<String>>>,
}

impl PolicyEngine {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            approved_scopes: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn with_approved_scopes(
        workspace_root: PathBuf,
        approved_scopes: Arc<Mutex<HashSet<String>>>,
    ) -> Self {
        Self {
            workspace_root,
            approved_scopes,
        }
    }

    pub fn allow_scope(&self, scope: &str) {
        if let Ok(mut guard) = self.approved_scopes.lock() {
            guard.insert(normalize_path_text(scope));
        }
    }

    fn is_scope_allowed(&self, candidate: &Path) -> bool {
        let candidate_str = normalize_path_text(candidate.to_string_lossy().as_ref());
        let Ok(guard) = self.approved_scopes.lock() else {
            return false;
        };
        guard.iter().any(|allowed| {
            candidate_str == *allowed
                || candidate_str.starts_with(&format!("{allowed}/"))
                || candidate_str.starts_with(&format!("{allowed}\\"))
        })
    }

    pub fn evaluate_path(&self, candidate: &Path) -> PolicyDecision {
        let root = match self.workspace_root.canonicalize() {
            Ok(v) => v,
            Err(e) => return PolicyDecision::Deny(format!("workspace root invalid: {e}")),
        };

        // Try canonical path first (works for existing files)
        if let Ok(canonical) = candidate.canonicalize() {
            if canonical.starts_with(&root) {
                return PolicyDecision::Allow;
            }
        }

        // For paths that don't exist yet (new files), walk up to find an existing ancestor
        // and check if it's inside the workspace
        let mut ancestor = candidate.to_path_buf();
        loop {
            if ancestor.exists() {
                if let Ok(canonical_ancestor) = ancestor.canonicalize() {
                    if canonical_ancestor.starts_with(&root) {
                        return PolicyDecision::Allow;
                    }
                }
                break;
            }
            if !ancestor.pop() {
                break;
            }
        }

        // Last resort: normalize path strings (strip Windows \\?\ prefix) and compare
        let root_str = root.to_string_lossy().replace("\\\\?\\", "");
        let candidate_str = candidate.to_string_lossy().replace("\\\\?\\", "");
        if candidate_str.starts_with(&root_str) {
            return PolicyDecision::Allow;
        }

        if self.is_scope_allowed(candidate) {
            return PolicyDecision::Allow;
        }

        let scope = normalize_path_text(candidate.to_string_lossy().as_ref());
        PolicyDecision::NeedsApproval {
            scope: scope.clone(),
            reason: format!("path outside workspace: {scope}"),
        }
    }

    pub fn evaluate_command(&self, cmd: &str) -> PolicyDecision {
        // Extract the binary name from compound commands (e.g. "mkdir -p foo" -> "mkdir")
        let binary = cmd.split_whitespace().next().unwrap_or(cmd);
        // Also strip any path prefix (e.g. "/usr/bin/git" -> "git")
        let binary = binary.rsplit('/').next().unwrap_or(binary);
        let binary = binary.rsplit('\\').next().unwrap_or(binary);
        // Strip .exe suffix on Windows
        let binary = binary.strip_suffix(".exe").unwrap_or(binary);

        const ALLOWLIST: &[&str] = &[
            // Version control
            "git",
            // Search
            "rg",
            // Rust toolchain
            "cargo",
            "rustc",
            "rustup",
            // JavaScript / Node toolchain
            "bun",
            "bunx",
            "node",
            "npx",
            "npm",
            "deno",
            // Python
            "python",
            "python3",
            "pip",
            "pip3",
            "uv",
            // File operations (needed for project scaffolding)
            "mkdir",
            "cp",
            "mv",
            "rm",
            "ls",
            "cat",
            "touch",
            "cd",
            // Windows equivalents
            "cmd",
            "powershell",
            "pwsh",
            "xcopy",
            "robocopy",
            "dir",
            "del",
            "copy",
            "move",
            "type",
            // Common dev tools
            "echo",
            "tar",
            "unzip",
            "zip",
            "curl",
            "wget",
            "make",
            "cmake",
            "docker",
            "docker-compose",
            // Testing / linting
            "jest",
            "vitest",
            "eslint",
            "prettier",
            "tsc",
        ];

        if ALLOWLIST.contains(&binary) {
            PolicyDecision::Allow
        } else {
            PolicyDecision::Deny(format!("command not allowed: {cmd}"))
        }
    }
}

fn normalize_path_text(raw: &str) -> String {
    raw.replace("\\\\?\\", "")
}
