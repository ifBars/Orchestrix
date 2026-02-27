//! Auto-memory tools for durable user/project preferences.

use std::path::Path;

use crate::core::preferences_memory;
use crate::core::tool::ToolDescriptor;
use crate::policy::PolicyEngine;
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

pub struct MemoryListTool;
pub struct MemoryReadTool;
pub struct MemoryUpsertTool;
pub struct MemoryDeleteTool;
pub struct MemoryCompactTool;

impl Tool for MemoryListTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "memory.list".into(),
            description: "List durable auto-memory preferences for this project.".into(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let preferences =
            preferences_memory::list_preferences(cwd).map_err(ToolError::Execution)?;
        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "preferences": preferences,
                "count": preferences.len()
            }),
            error: None,
        })
    }
}

impl Tool for MemoryReadTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "memory.read".into(),
            description:
                "Read auto-memory startup context (same concise MEMORY.md content loaded at run start)."
                    .into(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let content =
            preferences_memory::startup_memory_context(cwd).map_err(ToolError::Execution)?;
        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({ "content": content }),
            error: None,
        })
    }
}

impl Tool for MemoryUpsertTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "memory.upsert".into(),
            description: "Store or update a durable preference in auto memory.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string"},
                    "value": {"type": "string"},
                    "category": {"type": "string"}
                },
                "required": ["key", "value"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let key = input
            .get("key")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| ToolError::InvalidInput("key is required".to_string()))?;
        let value = input
            .get("value")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| ToolError::InvalidInput("value is required".to_string()))?;
        let category = input
            .get("category")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty());

        let entry = preferences_memory::upsert_preference(cwd, key, value, category)
            .map_err(ToolError::Execution)?;
        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({ "stored": true, "entry": entry }),
            error: None,
        })
    }
}

impl Tool for MemoryDeleteTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "memory.delete".into(),
            description: "Delete a durable preference from auto memory by key.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string"}
                },
                "required": ["key"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let key = input
            .get("key")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| ToolError::InvalidInput("key is required".to_string()))?;

        let deleted =
            preferences_memory::delete_preference(cwd, key).map_err(ToolError::Execution)?;
        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({ "deleted": deleted, "key": key }),
            error: None,
        })
    }
}

impl Tool for MemoryCompactTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "memory.compact".into(),
            description: "Compact auto memory by trimming older entries and keeping the freshest preferences.".into(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let removed = preferences_memory::compact_preferences(cwd).map_err(ToolError::Execution)?;
        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({ "compacted": true, "removed": removed }),
            error: None,
        })
    }
}
