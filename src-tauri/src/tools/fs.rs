//! Filesystem tools for reading, writing, and listing files.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::core::tool::ToolDescriptor;
use crate::policy::{PolicyDecision, PolicyEngine};
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

/// Tool for reading file contents.
pub struct FsReadTool;

impl Tool for FsReadTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "fs.read".into(),
            description: "Read file contents. Supports reading specific lines with offset/limit parameters for large files. Returns content with line numbers by default.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path to the file"},
                    "offset": {"type": "integer", "description": "Start reading from this line number (1-indexed). Default: 1."},
                    "limit": {"type": "integer", "description": "Maximum number of lines to read. Default: 2000."},
                    "line_numbers": {"type": "boolean", "description": "If true, prefix each line with its number (e.g. '1: content'). Default: true."}
                },
                "required": ["path"]
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
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("path required".into()))?;

        let offset = input
            .get("offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
            .max(1); // Ensure 1-based indexing

        let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(2000);

        let show_line_numbers = input
            .get("line_numbers")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let full = cwd.join(path);

        match policy.evaluate_path(&full) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }

        let file = File::open(&full)
            .map_err(|e| ToolError::Execution(format!("failed to open file: {}", e)))?;
        let reader = BufReader::new(file);

        let lines: Vec<String> = reader
            .lines()
            .skip((offset - 1) as usize)
            .take(limit as usize)
            .enumerate()
            .map(|(idx, line_res)| {
                let line_num = offset + (idx as u64);
                // Handle potentially invalid UTF-8 gracefully-ish
                let line_content =
                    line_res.unwrap_or_else(|_| "<binary or read error>".to_string());
                if show_line_numbers {
                    format!("{}: {}", line_num, line_content)
                } else {
                    line_content
                }
            })
            .collect();

        let content = lines.join("\n");

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "path": full,
                "content": content,
                "offset": offset,
                "limit": limit
            }),
            error: None,
        })
    }
}

/// Tool for writing file contents.
pub struct FsWriteTool;

impl Tool for FsWriteTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "fs.write".into(),
            description: "Write file contents".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"]
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
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("path required".into()))?;
        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("content required".into()))?;
        let full = cwd.join(path);

        // Check policy BEFORE creating directories to avoid OS errors on denied paths
        match policy.evaluate_path(&full) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }

        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::Execution(e.to_string()))?;
        }
        std::fs::write(&full, content).map_err(|e| ToolError::Execution(e.to_string()))?;

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({"path": full}),
            error: None,
        })
    }
}

/// Tool for listing directory contents.
pub struct FsListTool;

impl Tool for FsListTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "fs.list".into(),
            description: "List directory contents without shell commands. Supports recursion, depth limit, and entry limit.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Directory path relative to workspace root (default: .)"},
                    "recursive": {"type": "boolean", "description": "If true, walk subdirectories recursively"},
                    "max_depth": {"type": "integer", "minimum": 0, "description": "Max depth when recursive=true (0 means only the target directory)"},
                    "limit": {"type": "integer", "minimum": 1, "description": "Max number of entries to return (default: 200)"},
                    "files_only": {"type": "boolean", "description": "If true, only include files"},
                    "dirs_only": {"type": "boolean", "description": "If true, only include directories"}
                }
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
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let recursive = input
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let max_depth = input.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(200)
            .clamp(1, 2000) as usize;
        let files_only = input
            .get("files_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let dirs_only = input
            .get("dirs_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if files_only && dirs_only {
            return Err(ToolError::InvalidInput(
                "files_only and dirs_only cannot both be true".to_string(),
            ));
        }

        let full = cwd.join(path);
        match policy.evaluate_path(&full) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }

        if !full.exists() {
            return Err(ToolError::Execution(format!(
                "directory does not exist: {}",
                full.to_string_lossy()
            )));
        }
        if !full.is_dir() {
            return Err(ToolError::Execution(format!(
                "path is not a directory: {}",
                full.to_string_lossy()
            )));
        }

        let mut entries: Vec<serde_json::Value> = Vec::new();
        let mut stack: Vec<(PathBuf, usize)> = vec![(full.clone(), 0)];
        let mut truncated = false;

        while let Some((dir, depth)) = stack.pop() {
            let read_dir =
                std::fs::read_dir(&dir).map_err(|e| ToolError::Execution(e.to_string()))?;

            for item in read_dir {
                let item = item.map_err(|e| ToolError::Execution(e.to_string()))?;
                let item_path = item.path();

                let metadata = item
                    .metadata()
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                let is_dir = metadata.is_dir();
                let is_file = metadata.is_file();

                if (files_only && !is_file) || (dirs_only && !is_dir) {
                    if recursive && is_dir && depth < max_depth {
                        stack.push((item_path, depth + 1));
                    }
                    continue;
                }

                let rel_path = item_path
                    .strip_prefix(cwd)
                    .unwrap_or(&item_path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let modified_unix = metadata
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());

                entries.push(serde_json::json!({
                    "name": item.file_name().to_string_lossy(),
                    "path": rel_path,
                    "is_dir": is_dir,
                    "is_file": is_file,
                    "size": metadata.len(),
                    "modified_unix": modified_unix,
                    "depth": depth,
                }));

                if entries.len() >= limit {
                    truncated = true;
                    break;
                }

                if recursive && is_dir && depth < max_depth {
                    stack.push((item_path, depth + 1));
                }
            }

            if truncated {
                break;
            }
        }

        entries.sort_by(|a, b| {
            let a_path = a.get("path").and_then(|v| v.as_str()).unwrap_or_default();
            let b_path = b.get("path").and_then(|v| v.as_str()).unwrap_or_default();
            a_path.cmp(b_path)
        });

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "path": full,
                "recursive": recursive,
                "max_depth": max_depth,
                "limit": limit,
                "count": entries.len(),
                "truncated": truncated,
                "entries": entries,
            }),
            error: None,
        })
    }
}
