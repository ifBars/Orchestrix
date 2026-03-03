//! Filesystem tools for reading, writing, and listing files.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::core::tool::ToolDescriptor;
use crate::policy::{PolicyDecision, PolicyEngine};
use crate::tools::args::{schema_for_type, FsListArgs, FsReadArgs, FsWriteArgs};
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

/// Tool for reading file contents.
pub struct FsReadTool;

impl Tool for FsReadTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "fs.read".into(),
            description: "Read file contents. Supports reading specific lines with offset/limit parameters for large files. Returns content with line numbers by default.".into(),
            input_schema: schema_for_type::<FsReadArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: FsReadArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let offset = args.offset.unwrap_or(1).max(1);
        let limit = args.limit.unwrap_or(2000);
        let show_line_numbers = args.line_numbers.unwrap_or(true);

        let full = cwd.join(&args.path);

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
            input_schema: schema_for_type::<FsWriteArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: FsWriteArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let full = cwd.join(&args.path);

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
        std::fs::write(&full, &args.content).map_err(|e| ToolError::Execution(e.to_string()))?;

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
            input_schema: schema_for_type::<FsListArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: FsListArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let path = args.path.as_deref().unwrap_or(".");
        let recursive = args.recursive.unwrap_or(false);
        let max_depth = args.max_depth.unwrap_or(3) as usize;
        let limit = args.limit.unwrap_or(200).clamp(1, 2000) as usize;
        let files_only = args.files_only.unwrap_or(false);
        let dirs_only = args.dirs_only.unwrap_or(false);

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
