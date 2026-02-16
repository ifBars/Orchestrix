//! Apply-patch tool for structured file editing.
//!
//! Implements a custom patch format (inspired by Codex's apply-patch) that is
//! more LLM-friendly than unified diff. The format supports:
//!
//! - Adding new files (`*** Add File: <path>`)
//! - Deleting files (`*** Delete File: <path>`)
//! - Updating files with context-aware hunks (`*** Update File: <path>`)
//! - File moves via `*** Move to: <new_path>`
//! - Fuzzy matching with progressive leniency for resilient patching
//!
//! This operates directly on the filesystem (no git required).

mod parser;
mod seek;

use std::path::Path;

use crate::core::tool::ToolDescriptor;
use crate::policy::{PolicyDecision, PolicyEngine};
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

pub use parser::{parse_patch, Hunk, UpdateFileChunk};

/// Tool for applying structured patches to files.
pub struct FsPatchTool;

impl Tool for FsPatchTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "fs.patch".into(),
            description: concat!(
                "Apply a structured patch to add, delete, or update files. ",
                "Uses a simple LLM-friendly format with context-aware matching. ",
                "Does not require git. Preferred over fs.write for incremental edits."
            )
            .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "patch": {
                        "type": "string",
                        "description": concat!(
                            "The patch text in apply-patch format. Envelope: ",
                            "*** Begin Patch\\n[operations]\\n*** End Patch. ",
                            "Operations: *** Add File: <path> (lines prefixed with +), ",
                            "*** Delete File: <path>, ",
                            "*** Update File: <path> with @@ context markers and +/- lines. ",
                            "CRITICAL: Text after @@ must MATCH actual file content (used to find the change location). ",
                            "Use @@ alone (no context) if uncertain, or use fs.read to verify file content first. ",
                            "Context lines (prefixed with space) provide additional matching context."
                        )
                    }
                },
                "required": ["patch"]
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
        let patch_text = input
            .get("patch")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("patch required".into()))?;

        // Parse the patch
        let hunks = parse_patch(patch_text).map_err(|e| ToolError::InvalidInput(e.to_string()))?;

        if hunks.is_empty() {
            return Err(ToolError::InvalidInput(
                "patch contains no operations".into(),
            ));
        }

        // Check all paths against policy before making any changes
        for hunk in &hunks {
            let path = hunk.resolve_path(cwd);
            match policy.evaluate_path(&path) {
                PolicyDecision::Allow => {}
                PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
                PolicyDecision::NeedsApproval { scope, reason } => {
                    return Err(ToolError::ApprovalRequired { scope, reason })
                }
            }
            // Also check move targets
            if let Hunk::UpdateFile {
                move_path: Some(mp),
                ..
            } = hunk
            {
                let move_target = cwd.join(mp);
                match policy.evaluate_path(&move_target) {
                    PolicyDecision::Allow => {}
                    PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
                    PolicyDecision::NeedsApproval { scope, reason } => {
                        return Err(ToolError::ApprovalRequired { scope, reason })
                    }
                }
            }
        }

        // Apply the patch
        let mut added: Vec<String> = Vec::new();
        let mut modified: Vec<String> = Vec::new();
        let mut deleted: Vec<String> = Vec::new();
        let mut diffs: Vec<String> = Vec::new();

        for hunk in &hunks {
            match hunk {
                Hunk::AddFile { path, contents } => {
                    let full = cwd.join(path);
                    if let Some(parent) = full.parent() {
                        if !parent.as_os_str().is_empty() {
                            std::fs::create_dir_all(parent).map_err(|e| {
                                ToolError::Execution(format!(
                                    "failed to create directories for {}: {e}",
                                    path.display()
                                ))
                            })?;
                        }
                    }
                    std::fs::write(&full, contents).map_err(|e| {
                        ToolError::Execution(format!("failed to write {}: {e}", path.display()))
                    })?;
                    added.push(path.to_string_lossy().to_string());
                }
                Hunk::DeleteFile { path } => {
                    let full = cwd.join(path);
                    std::fs::remove_file(&full).map_err(|e| {
                        ToolError::Execution(format!("failed to delete {}: {e}", path.display()))
                    })?;
                    deleted.push(path.to_string_lossy().to_string());
                }
                Hunk::UpdateFile {
                    path,
                    move_path,
                    chunks,
                } => {
                    let full = cwd.join(path);
                    let original = std::fs::read_to_string(&full).map_err(|e| {
                        ToolError::Execution(format!("failed to read {}: {e}", path.display()))
                    })?;

                    let new_contents = apply_chunks(&original, &full, chunks)
                        .map_err(|e| ToolError::Execution(e))?;

                    // Generate unified diff for transparency / audit trail
                    let text_diff = similar::TextDiff::from_lines(&original, &new_contents);
                    let unified = text_diff.unified_diff().context_radius(3).to_string();
                    if !unified.is_empty() {
                        diffs.push(format!(
                            "--- {}\n+++ {}\n{}",
                            path.display(),
                            path.display(),
                            unified
                        ));
                    }

                    if let Some(dest) = move_path {
                        let dest_full = cwd.join(dest);
                        if let Some(parent) = dest_full.parent() {
                            if !parent.as_os_str().is_empty() {
                                std::fs::create_dir_all(parent).map_err(|e| {
                                    ToolError::Execution(format!(
                                        "failed to create directories for {}: {e}",
                                        dest.display()
                                    ))
                                })?;
                            }
                        }
                        std::fs::write(&dest_full, &new_contents).map_err(|e| {
                            ToolError::Execution(format!("failed to write {}: {e}", dest.display()))
                        })?;
                        std::fs::remove_file(&full).map_err(|e| {
                            ToolError::Execution(format!(
                                "failed to remove original {}: {e}",
                                path.display()
                            ))
                        })?;
                        modified.push(format!("{} -> {}", path.display(), dest.display()));
                    } else {
                        std::fs::write(&full, &new_contents).map_err(|e| {
                            ToolError::Execution(format!("failed to write {}: {e}", path.display()))
                        })?;
                        modified.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }

        let summary = build_summary(&added, &modified, &deleted);

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "summary": summary,
                "added": added,
                "modified": modified,
                "deleted": deleted,
                "diffs": diffs,
            }),
            error: None,
        })
    }
}

/// Apply update chunks to file contents, returning the new file content.
fn apply_chunks(original: &str, path: &Path, chunks: &[UpdateFileChunk]) -> Result<String, String> {
    let mut lines: Vec<String> = original.split('\n').map(String::from).collect();

    // Drop trailing empty element from final newline
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }

    let replacements = compute_replacements(&lines, path, chunks)?;
    let mut new_lines = apply_replacements(lines, &replacements);

    // Ensure file ends with newline
    if !new_lines.last().is_some_and(String::is_empty) {
        new_lines.push(String::new());
    }

    Ok(new_lines.join("\n"))
}

/// Compute replacement operations from chunks.
fn compute_replacements(
    original_lines: &[String],
    path: &Path,
    chunks: &[UpdateFileChunk],
) -> Result<Vec<(usize, usize, Vec<String>)>, String> {
    let mut replacements: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut line_index: usize = 0;

    for chunk in chunks {
        // If a chunk has a change_context, use seek to find it
        if let Some(ctx_line) = &chunk.change_context {
            if let Some(idx) = seek::seek_sequence(
                original_lines,
                std::slice::from_ref(ctx_line),
                line_index,
                false,
            ) {
                line_index = idx + 1;
            } else {
                return Err(format!(
                    "failed to find context '{}' in {}. The @@ context line must match actual file content. Use fs.read to verify, or use @@ alone (no context text).",
                    ctx_line,
                    path.display()
                ));
            }
        }

        if chunk.old_lines.is_empty() {
            // Pure addition at end of file
            let insertion_idx = if original_lines.last().is_some_and(String::is_empty) {
                original_lines.len() - 1
            } else {
                original_lines.len()
            };
            replacements.push((insertion_idx, 0, chunk.new_lines.clone()));
            continue;
        }

        // Try to locate old_lines in the file
        let mut pattern: &[String] = &chunk.old_lines;
        let mut found =
            seek::seek_sequence(original_lines, pattern, line_index, chunk.is_end_of_file);
        let mut new_slice: &[String] = &chunk.new_lines;

        // Retry without trailing empty line (represents final newline)
        if found.is_none() && pattern.last().is_some_and(String::is_empty) {
            pattern = &pattern[..pattern.len() - 1];
            if new_slice.last().is_some_and(String::is_empty) {
                new_slice = &new_slice[..new_slice.len() - 1];
            }
            found = seek::seek_sequence(original_lines, pattern, line_index, chunk.is_end_of_file);
        }

        if let Some(start_idx) = found {
            replacements.push((start_idx, pattern.len(), new_slice.to_vec()));
            line_index = start_idx + pattern.len();
        } else {
            return Err(format!(
                "failed to find expected lines in {}:\n{}",
                path.display(),
                chunk.old_lines.join("\n"),
            ));
        }
    }

    replacements.sort_by(|(a, _, _), (b, _, _)| a.cmp(b));
    Ok(replacements)
}

/// Apply replacements in reverse order to avoid index shifts.
fn apply_replacements(
    mut lines: Vec<String>,
    replacements: &[(usize, usize, Vec<String>)],
) -> Vec<String> {
    for (start_idx, old_len, new_segment) in replacements.iter().rev() {
        let start_idx = *start_idx;
        let old_len = *old_len;

        for _ in 0..old_len {
            if start_idx < lines.len() {
                lines.remove(start_idx);
            }
        }

        for (offset, new_line) in new_segment.iter().enumerate() {
            lines.insert(start_idx + offset, new_line.clone());
        }
    }
    lines
}

fn build_summary(added: &[String], modified: &[String], deleted: &[String]) -> String {
    let mut parts = Vec::new();
    if !added.is_empty() {
        parts.push(format!("Added: {}", added.join(", ")));
    }
    if !modified.is_empty() {
        parts.push(format!("Modified: {}", modified.join(", ")));
    }
    if !deleted.is_empty() {
        parts.push(format!("Deleted: {}", deleted.join(", ")));
    }
    if parts.is_empty() {
        "No changes applied".to_string()
    } else {
        parts.join("; ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::PolicyEngine;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn test_add_file() {
        let dir = temp_dir();
        let path = dir.path().join("new.txt");
        let patch = format!(
            "*** Begin Patch\n*** Add File: {}\n+hello\n+world\n*** End Patch",
            path.display()
        );
        let hunks = parse_patch(&patch).unwrap();
        assert_eq!(hunks.len(), 1);
        match &hunks[0] {
            Hunk::AddFile { contents, .. } => assert_eq!(contents, "hello\nworld\n"),
            _ => panic!("expected AddFile"),
        }
    }

    #[test]
    fn test_delete_file() {
        let patch = "*** Begin Patch\n*** Delete File: old.txt\n*** End Patch";
        let hunks = parse_patch(patch).unwrap();
        assert_eq!(hunks.len(), 1);
        assert!(matches!(&hunks[0], Hunk::DeleteFile { .. }));
    }

    #[test]
    fn test_update_file_via_tool() {
        let dir = temp_dir();
        let file = dir.path().join("test.rs");
        std::fs::write(&file, "fn hello() {\n    println!(\"hi\");\n}\n").unwrap();

        let patch = format!(
            "*** Begin Patch\n*** Update File: test.rs\n@@\n fn hello() {{\n-    println!(\"hi\");\n+    println!(\"hello world\");\n }}\n*** End Patch"
        );

        let tool = FsPatchTool;
        let policy = PolicyEngine::new(dir.path().to_path_buf());
        let result = tool
            .invoke(&policy, dir.path(), serde_json::json!({ "patch": patch }))
            .unwrap();

        assert!(result.ok);
        let content = std::fs::read_to_string(&file).unwrap();
        assert!(content.contains("hello world"));
        assert!(!content.contains("\"hi\""));
    }

    #[test]
    fn test_update_with_context_marker() {
        let dir = temp_dir();
        let file = dir.path().join("lib.rs");
        std::fs::write(
            &file,
            "mod a {\n    fn x() { 1 }\n}\n\nmod b {\n    fn y() { 2 }\n}\n",
        )
        .unwrap();

        let patch = format!(
            "*** Begin Patch\n*** Update File: lib.rs\n@@ mod b {{\n-    fn y() {{ 2 }}\n+    fn y() {{ 42 }}\n*** End Patch"
        );

        let tool = FsPatchTool;
        let policy = PolicyEngine::new(dir.path().to_path_buf());
        let result = tool
            .invoke(&policy, dir.path(), serde_json::json!({ "patch": patch }))
            .unwrap();

        assert!(result.ok);
        let content = std::fs::read_to_string(&file).unwrap();
        assert!(content.contains("fn y() { 42 }"));
        assert!(content.contains("fn x() { 1 }"));
    }

    #[test]
    fn test_move_file() {
        let dir = temp_dir();
        let src = dir.path().join("old.txt");
        std::fs::write(&src, "line\n").unwrap();

        let patch = format!(
            "*** Begin Patch\n*** Update File: old.txt\n*** Move to: new.txt\n@@\n-line\n+line2\n*** End Patch"
        );

        let tool = FsPatchTool;
        let policy = PolicyEngine::new(dir.path().to_path_buf());
        let result = tool
            .invoke(&policy, dir.path(), serde_json::json!({ "patch": patch }))
            .unwrap();

        assert!(result.ok);
        assert!(!src.exists());
        let dest = dir.path().join("new.txt");
        assert!(dest.exists());
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "line2\n");
    }

    #[test]
    fn test_multi_operation_patch() {
        let dir = temp_dir();
        let existing = dir.path().join("existing.txt");
        std::fs::write(&existing, "foo\nbar\nbaz\n").unwrap();
        let to_delete = dir.path().join("delete_me.txt");
        std::fs::write(&to_delete, "gone").unwrap();

        let patch = concat!(
            "*** Begin Patch\n",
            "*** Add File: new_file.txt\n",
            "+new content\n",
            "*** Update File: existing.txt\n",
            "@@\n",
            " foo\n",
            "-bar\n",
            "+BAR\n",
            " baz\n",
            "*** Delete File: delete_me.txt\n",
            "*** End Patch"
        );

        let tool = FsPatchTool;
        let policy = PolicyEngine::new(dir.path().to_path_buf());
        let result = tool
            .invoke(&policy, dir.path(), serde_json::json!({ "patch": patch }))
            .unwrap();

        assert!(result.ok);
        assert!(dir.path().join("new_file.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("new_file.txt")).unwrap(),
            "new content\n"
        );
        assert!(existing.exists());
        let content = std::fs::read_to_string(&existing).unwrap();
        assert!(content.contains("BAR"));
        assert!(!content.contains("\nbar\n"));
        assert!(!to_delete.exists());
    }

    #[test]
    fn test_policy_denies_outside_workspace() {
        let dir = temp_dir();
        let patch = "*** Begin Patch\n*** Add File: C:\\Windows\\evil.txt\n+bad\n*** End Patch";

        let tool = FsPatchTool;
        let policy = PolicyEngine::new(dir.path().to_path_buf());
        let result = tool.invoke(&policy, dir.path(), serde_json::json!({ "patch": patch }));

        assert!(result.is_err());
    }
}
