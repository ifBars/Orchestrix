//! MCP tool filtering and approval policies.
//!
//! This module provides:
//! - Tool name filtering (allowlist/blocklist)
//! - Read-only vs read-write classification
//! - Approval policies based on tool characteristics

use serde::{Deserialize, Serialize};

/// Filter configuration for MCP tools.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolFilter {
    /// Filter mode: include (allowlist) or exclude (blocklist).
    #[serde(default)]
    pub mode: FilterMode,
    /// List of tool names to filter.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Allow all read-only tools regardless of list.
    #[serde(default)]
    pub allow_all_read_only: bool,
    /// Block all tools that modify data regardless of list.
    #[serde(default)]
    pub block_all_modifying: bool,
}

/// Filter mode: include means allowlist, exclude means blocklist.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode {
    /// Allow only tools in the list (allowlist).
    #[default]
    Include,
    /// Block tools in the list (blocklist).
    Exclude,
}

impl ToolFilter {
    /// Check if a tool passes the filter.
    pub fn allows(&self, tool_name: &str) -> bool {
        // First check block-all-modifying
        if self.block_all_modifying {
            // We don't have read_only_hint here, so this is checked at approval time
            // For now, we'll pass and let approval policy handle it
        }

        let in_list = self.tools.iter().any(|t| t == tool_name);

        match self.mode {
            FilterMode::Include => {
                // Allowlist: tool must be in list (or list is empty = allow all)
                self.tools.is_empty() || in_list
            }
            FilterMode::Exclude => {
                // Blocklist: tool must NOT be in list
                !in_list
            }
        }
    }

    /// Check if a tool passes the filter with read-only hint.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn allows_with_hint(&self, tool_name: &str, read_only_hint: Option<bool>) -> bool {
        // Check allow-all-read-only
        if self.allow_all_read_only {
            if let Some(true) = read_only_hint {
                return true;
            }
        }

        // Check block-all-modifying
        if self.block_all_modifying {
            if let Some(false) = read_only_hint {
                return false;
            }
        }

        self.allows(tool_name)
    }
}

/// Policy for when tools require approval.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolApprovalPolicy {
    /// Global approval setting.
    #[serde(default)]
    pub global_policy: GlobalApprovalPolicy,
    /// Tool-specific overrides.
    #[serde(default)]
    pub tool_overrides: Vec<ToolOverride>,
    /// Read-only tools never require approval.
    #[serde(default = "default_true")]
    pub read_only_never_requires_approval: bool,
    /// Tools that modify data always require approval.
    #[serde(default)]
    pub modifying_always_requires_approval: bool,
}

fn default_true() -> bool {
    true
}

/// Global approval policy.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GlobalApprovalPolicy {
    /// All tools require approval.
    Always,
    /// No tools require approval.
    Never,
    /// Use tool-specific settings (default).
    #[default]
    ByTool,
}

/// Tool-specific approval override.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOverride {
    /// Tool name pattern (exact match or glob).
    pub pattern: String,
    /// Whether this tool requires approval.
    pub requires_approval: bool,
    /// Whether this is a glob pattern.
    #[serde(default)]
    pub is_glob: bool,
}

impl ToolApprovalPolicy {
    /// Check if a tool requires approval.
    pub fn requires_approval(&self, tool_name: &str, read_only_hint: Option<bool>) -> bool {
        // Check global policy first
        match self.global_policy {
            GlobalApprovalPolicy::Always => return true,
            GlobalApprovalPolicy::Never => return false,
            GlobalApprovalPolicy::ByTool => {}
        }

        // Check read-only hint
        if self.read_only_never_requires_approval {
            if let Some(true) = read_only_hint {
                return false;
            }
        }

        // Check modifying hint
        if self.modifying_always_requires_approval {
            if let Some(false) = read_only_hint {
                return true;
            }
        }

        // Check tool-specific overrides (in order, last match wins)
        for override_ in &self.tool_overrides {
            let matches = if override_.is_glob {
                // Simple glob matching
                Self::glob_match(&override_.pattern, tool_name)
            } else {
                override_.pattern == tool_name
            };

            if matches {
                return override_.requires_approval;
            }
        }

        // Default: no approval required
        false
    }

    /// Simple glob matching (supports * and ?).
    fn glob_match(pattern: &str, text: &str) -> bool {
        let mut pattern_chars = pattern.chars().peekable();
        let mut text_chars = text.chars().peekable();

        while let Some(p) = pattern_chars.next() {
            match p {
                '*' => {
                    // Match any sequence
                    if pattern_chars.peek().is_none() {
                        return true; // * at end matches everything
                    }
                    // Try to match the rest
                    let rest: String = pattern_chars.collect();
                    for i in 0..=text.len() {
                        if Self::glob_match(&rest, &text[i..]) {
                            return true;
                        }
                    }
                    return false;
                }
                '?' => {
                    // Match any single character
                    if text_chars.next().is_none() {
                        return false;
                    }
                }
                c => {
                    // Exact match
                    if text_chars.next() != Some(c) {
                        return false;
                    }
                }
            }
        }

        text_chars.next().is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_include_mode() {
        let filter = ToolFilter {
            mode: FilterMode::Include,
            tools: vec!["read_file".to_string(), "list_dir".to_string()],
            allow_all_read_only: false,
            block_all_modifying: false,
        };

        assert!(filter.allows("read_file"));
        assert!(filter.allows("list_dir"));
        assert!(!filter.allows("write_file"));
        assert!(!filter.allows("delete_file"));
    }

    #[test]
    fn test_filter_exclude_mode() {
        let filter = ToolFilter {
            mode: FilterMode::Exclude,
            tools: vec!["dangerous_tool".to_string()],
            allow_all_read_only: false,
            block_all_modifying: false,
        };

        assert!(filter.allows("read_file"));
        assert!(filter.allows("write_file"));
        assert!(!filter.allows("dangerous_tool"));
    }

    #[test]
    fn test_filter_empty_list() {
        let filter = ToolFilter {
            mode: FilterMode::Include,
            tools: vec![],
            allow_all_read_only: false,
            block_all_modifying: false,
        };

        // Empty include list means allow all
        assert!(filter.allows("any_tool"));
    }

    #[test]
    fn test_approval_policy_global() {
        let policy = ToolApprovalPolicy {
            global_policy: GlobalApprovalPolicy::Always,
            tool_overrides: vec![],
            read_only_never_requires_approval: false,
            modifying_always_requires_approval: false,
        };

        assert!(policy.requires_approval("any_tool", None));
        assert!(policy.requires_approval("read_only", Some(true)));
    }

    #[test]
    fn test_approval_policy_read_only_hint() {
        let policy = ToolApprovalPolicy {
            global_policy: GlobalApprovalPolicy::ByTool,
            tool_overrides: vec![],
            read_only_never_requires_approval: true,
            modifying_always_requires_approval: false,
        };

        assert!(!policy.requires_approval("read_tool", Some(true)));
        assert!(!policy.requires_approval("write_tool", None));
        assert!(!policy.requires_approval("write_tool", Some(false)));
    }

    #[test]
    fn test_approval_policy_tool_override() {
        let policy = ToolApprovalPolicy {
            global_policy: GlobalApprovalPolicy::ByTool,
            tool_overrides: vec![
                ToolOverride {
                    pattern: "write_*".to_string(),
                    requires_approval: true,
                    is_glob: true,
                },
                ToolOverride {
                    pattern: "read_file".to_string(),
                    requires_approval: false,
                    is_glob: false,
                },
            ],
            read_only_never_requires_approval: false,
            modifying_always_requires_approval: false,
        };

        assert!(policy.requires_approval("write_file", None));
        assert!(policy.requires_approval("write_data", None));
        assert!(!policy.requires_approval("read_file", None));
        assert!(!policy.requires_approval("other_tool", None));
    }

    #[test]
    fn test_glob_match() {
        assert!(ToolApprovalPolicy::glob_match("*", "anything"));
        assert!(ToolApprovalPolicy::glob_match("read_*", "read_file"));
        assert!(ToolApprovalPolicy::glob_match("read_*", "read_directory"));
        assert!(!ToolApprovalPolicy::glob_match("read_*", "write_file"));
        assert!(ToolApprovalPolicy::glob_match("?at", "cat"));
        assert!(ToolApprovalPolicy::glob_match("?at", "bat"));
        assert!(!ToolApprovalPolicy::glob_match("?at", "cats"));
        assert!(ToolApprovalPolicy::glob_match(
            "read_*.json",
            "read_config.json"
        ));
        assert!(!ToolApprovalPolicy::glob_match(
            "read_*.json",
            "read_config.txt"
        ));
    }
}
