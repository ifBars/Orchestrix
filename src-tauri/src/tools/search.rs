//! Search tools for finding content in files.

use std::path::Path;
use std::process::Command;

use crate::core::tool::ToolDescriptor;
use crate::policy::{PolicyDecision, PolicyEngine};
use crate::tools::args::{schema_for_type, SearchRgArgs};
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

/// Tool for searching with ripgrep.
///
/// Supports both raw text output and structured JSON output with per-match
/// file, line number, and match text. Additional options for case sensitivity,
/// file type filtering, context lines, and result limits.
pub struct SearchRgTool;

impl Tool for SearchRgTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "search.rg".into(),
            description: concat!(
                "Search file contents with ripgrep. Returns matching lines with file paths and line numbers. ",
                "Supports regex patterns, case sensitivity, file type filtering, context lines, and result limits. ",
                "Use 'json_output: true' for structured match data."
            ).into(),
            input_schema: schema_for_type::<SearchRgArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: SearchRgArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let path = args.path.as_deref().unwrap_or(".");
        let json_output = args.json_output.unwrap_or(false);
        let file_type = args.file_type.as_deref();
        let context_lines = args.context_lines;
        let max_results = args.max_results;
        let files_with_matches = args.files_with_matches.unwrap_or(false);

        let full = cwd.join(path);

        match policy.evaluate_path(&full) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }

        let mut cmd = Command::new("rg");

        if json_output {
            cmd.arg("--json");
        } else {
            cmd.arg("--line-number");
            cmd.arg("--with-filename");
        }

        match args.case_sensitive {
            Some(true) => {
                cmd.arg("--case-sensitive");
            }
            Some(false) => {
                cmd.arg("--ignore-case");
            }
            None => {
                cmd.arg("--smart-case");
            }
        }

        if args.fixed_strings.unwrap_or(false) {
            cmd.arg("--fixed-strings");
        }

        if let Some(ft) = file_type {
            cmd.arg("-t").arg(ft);
        }

        if let Some(ctx) = context_lines {
            cmd.arg("-C").arg(ctx.to_string());
        }

        if let Some(max) = max_results {
            cmd.arg("-m").arg(max.to_string());
        }

        if files_with_matches {
            cmd.arg("--files-with-matches");
        }

        cmd.arg(&args.pattern).arg(&full);

        let output = cmd
            .output()
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        let stdout_raw = String::from_utf8_lossy(&output.stdout);
        let stderr_raw = String::from_utf8_lossy(&output.stderr);

        if json_output {
            let matches = parse_rg_json(&stdout_raw, cwd);
            Ok(ToolCallOutput {
                ok: output.status.success() || output.status.code() == Some(1),
                data: serde_json::json!({
                    "matches": matches,
                    "match_count": matches.len(),
                    "stderr": if stderr_raw.is_empty() { None } else { Some(&*stderr_raw) },
                }),
                error: None,
            })
        } else {
            Ok(ToolCallOutput {
                ok: output.status.success() || output.status.code() == Some(1),
                data: serde_json::json!({
                    "stdout": &*stdout_raw,
                    "stderr": &*stderr_raw,
                }),
                error: None,
            })
        }
    }
}

fn parse_rg_json(output: &str, cwd: &Path) -> Vec<serde_json::Value> {
    let mut matches = Vec::new();

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }
        let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        let msg_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if msg_type != "match" {
            continue;
        }

        let data = match obj.get("data") {
            Some(d) => d,
            None => continue,
        };

        let file_path = data
            .get("path")
            .and_then(|p| p.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let relative_path = if let Ok(rel) = Path::new(file_path).strip_prefix(cwd) {
            rel.to_string_lossy().replace('\\', "/")
        } else {
            file_path.replace('\\', "/")
        };

        let line_number = data.get("line_number").and_then(|v| v.as_u64());

        let line_text = data
            .get("lines")
            .and_then(|l| l.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim_end();

        matches.push(serde_json::json!({
            "path": relative_path,
            "line": line_number,
            "text": line_text,
        }));
    }

    matches
}
