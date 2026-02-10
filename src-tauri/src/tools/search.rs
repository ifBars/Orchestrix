//! Search tools for finding content in files.

use std::path::Path;
use std::process::Command;

use crate::core::tool::ToolDescriptor;
use crate::policy::{PolicyDecision, PolicyEngine};
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

/// Tool for searching with ripgrep.
pub struct SearchRgTool;

impl Tool for SearchRgTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "search.rg".into(),
            description: "Search with ripgrep".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string"}
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
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("pattern required".into()))?;
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let full = cwd.join(path);

        match policy.evaluate_path(&full) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }

        let output = Command::new("rg")
            .arg(pattern)
            .arg(&full)
            .output()
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        Ok(ToolCallOutput {
            ok: output.status.success(),
            data: serde_json::json!({
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr),
            }),
            error: None,
        })
    }
}
