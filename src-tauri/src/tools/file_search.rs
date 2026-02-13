//! Fuzzy file search tool.
//!
//! Uses `ignore` for .gitignore-aware directory walking and `nucleo-matcher`
//! for fuzzy scoring. This provides fast filename discovery without shelling
//! out to external processes.

use std::path::Path;

use ignore::WalkBuilder;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::core::tool::ToolDescriptor;
use crate::policy::{PolicyDecision, PolicyEngine};
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

/// Tool for fuzzy file name search.
pub struct SearchFilesTool;

impl Tool for SearchFilesTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "search.files".into(),
            description: concat!(
                "Fuzzy search for files by name in the workspace. ",
                "Respects .gitignore. Returns top matches ranked by relevance score. ",
                "Use this to quickly find files when you know part of the name."
            )
            .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Fuzzy search pattern (partial file name, e.g. 'mod.rs', 'component', 'config')"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search in (relative to workspace root, default: '.')"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 20, max: 100)"
                    }
                },
                "required": ["pattern"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let pattern_text = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("pattern required".into()))?;

        if pattern_text.trim().is_empty() {
            return Err(ToolError::InvalidInput("pattern must not be empty".into()));
        }

        let search_path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(20)
            .clamp(1, 100) as usize;

        let full_path = cwd.join(search_path);
        match policy.evaluate_path(&full_path) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }

        if !full_path.exists() || !full_path.is_dir() {
            return Err(ToolError::Execution(format!(
                "search directory does not exist: {}",
                full_path.display()
            )));
        }

        // Collect files via ignore-aware walker
        let mut file_paths: Vec<String> = Vec::new();
        let walker = WalkBuilder::new(&full_path)
            .hidden(false)
            .follow_links(true)
            .require_git(false)
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if entry.file_type().map_or(true, |ft| ft.is_dir()) {
                continue;
            }
            if let Ok(relative) = entry.path().strip_prefix(cwd) {
                let rel_str = relative.to_string_lossy().replace('\\', "/");
                file_paths.push(rel_str);
            }
        }

        // Fuzzy match and score
        let pattern = Pattern::new(
            pattern_text,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let config = Config::DEFAULT.match_paths();
        let mut matcher = Matcher::new(config);
        let mut utf32_buf = Vec::new();

        let mut scored: Vec<(u32, &str)> = file_paths
            .iter()
            .filter_map(|path| {
                let haystack = Utf32Str::new(path, &mut utf32_buf);
                let score = pattern.score(haystack, &mut matcher)?;
                Some((score, path.as_str()))
            })
            .collect();

        // Sort by descending score, then ascending path
        scored.sort_by(|a, b| match b.0.cmp(&a.0) {
            std::cmp::Ordering::Equal => a.1.cmp(b.1),
            other => other,
        });

        let total = scored.len();
        scored.truncate(limit);

        let matches: Vec<serde_json::Value> = scored
            .iter()
            .map(|(score, path)| {
                serde_json::json!({
                    "path": path,
                    "score": score,
                })
            })
            .collect();

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "query": pattern_text,
                "total_matches": total,
                "shown": matches.len(),
                "truncated": total > limit,
                "matches": matches,
            }),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::PolicyEngine;

    #[test]
    fn test_search_files_finds_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/utils")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
        std::fs::write(dir.path().join("src/utils/helper.rs"), "").unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();

        let tool = SearchFilesTool;
        let policy = PolicyEngine::new(dir.path().to_path_buf());
        let result = tool
            .invoke(&policy, dir.path(), serde_json::json!({"pattern": "main"}))
            .unwrap();

        assert!(result.ok);
        let matches = result.data.get("matches").unwrap().as_array().unwrap();
        assert!(!matches.is_empty());
        let first_path = matches[0].get("path").unwrap().as_str().unwrap();
        assert!(first_path.contains("main.rs"));
    }

    #[test]
    fn test_search_files_respects_limit() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..10 {
            std::fs::write(dir.path().join(format!("file{i}.txt")), "").unwrap();
        }

        let tool = SearchFilesTool;
        let policy = PolicyEngine::new(dir.path().to_path_buf());
        let result = tool
            .invoke(
                &policy,
                dir.path(),
                serde_json::json!({"pattern": "file", "limit": 3}),
            )
            .unwrap();

        assert!(result.ok);
        let matches = result.data.get("matches").unwrap().as_array().unwrap();
        assert!(matches.len() <= 3);
        assert!(result.data.get("truncated").unwrap().as_bool().unwrap());
    }

    #[test]
    fn test_search_files_empty_pattern_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let tool = SearchFilesTool;
        let policy = PolicyEngine::new(dir.path().to_path_buf());
        let result = tool.invoke(&policy, dir.path(), serde_json::json!({"pattern": ""}));
        assert!(result.is_err());
    }

    #[test]
    fn test_search_files_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/app.rs"), "").unwrap();
        std::fs::write(dir.path().join("README.md"), "").unwrap();

        let tool = SearchFilesTool;
        let policy = PolicyEngine::new(dir.path().to_path_buf());
        let result = tool
            .invoke(
                &policy,
                dir.path(),
                serde_json::json!({"pattern": "app", "path": "src"}),
            )
            .unwrap();

        assert!(result.ok);
        let matches = result.data.get("matches").unwrap().as_array().unwrap();
        assert!(!matches.is_empty());
    }
}
