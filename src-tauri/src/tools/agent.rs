//! Agent-related tools for todo management and mode switching.

use std::path::Path;

use crate::core::tool::ToolDescriptor;
use crate::policy::PolicyEngine;
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

/// Tool for managing agent todo lists.
pub struct AgentTodoTool;

impl Tool for AgentTodoTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "agent.todo".into(),
            description: concat!(
                "Manage the agent's local todo list. Actions: list, set, add, update, clear. ",
                "For 'update', pass a 'todos' array where position determines which todo to update. ",
                "Use 'list_id' to scope todos to a specific agent/run to avoid conflicts with parent/sub-agents."
            ).into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["list", "set", "add", "update", "clear"]},
                    "todos": {"type": "array", "items": {"type": "object"}, "description": "For 'set' or 'update' actions. For update, array position determines which todo to update."},
                    "item": {"type": "object", "description": "For 'add' action or 'update' with index"},
                    "index": {"type": "integer", "description": "Optional: specific index for update (legacy)"},
                    "list_id": {"type": "string", "description": "Optional: scope this todo list to a specific ID (e.g., agent/run identifier). Prevents conflicts between parent and sub-agent todos."}
                }
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
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("list");
        let list_id = input.get("list_id").and_then(|v| v.as_str());

        let state_dir = cwd.join(".orchestrix");
        std::fs::create_dir_all(&state_dir).map_err(|e| ToolError::Execution(e.to_string()))?;

        // Scope todo file to list_id if provided, otherwise use default
        let todo_path = if let Some(id) = list_id {
            // Sanitize list_id to be filesystem-safe
            let safe_id = id.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_");
            state_dir.join(format!("agent-todo-{}.json", safe_id))
        } else {
            state_dir.join("agent-todo.json")
        };

        let mut todos: Vec<serde_json::Value> = if todo_path.exists() {
            let raw = std::fs::read_to_string(&todo_path)
                .map_err(|e| ToolError::Execution(e.to_string()))?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            Vec::new()
        };

        match action {
            "set" => {
                let next = input
                    .get("todos")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| {
                        ToolError::InvalidInput("todos array is required for set".to_string())
                    })?;
                todos = next.clone();
            }
            "add" => {
                let item = input.get("item").ok_or_else(|| {
                    ToolError::InvalidInput("item is required for add".to_string())
                })?;
                todos.push(item.clone());
            }
            "update" => {
                if let Some(items) = input.get("todos").and_then(|v| v.as_array()) {
                    for (idx, item) in items.iter().enumerate() {
                        if idx < todos.len() {
                            todos[idx] = item.clone();
                        }
                    }
                } else if let Some(idx) = input.get("index").and_then(|v| v.as_u64()) {
                    let idx = idx as usize;
                    let item = input.get("item").ok_or_else(|| {
                        ToolError::InvalidInput("item is required when using index".to_string())
                    })?;
                    if idx >= todos.len() {
                        return Err(ToolError::InvalidInput("index out of range".to_string()));
                    }
                    todos[idx] = item.clone();
                } else {
                    return Err(ToolError::InvalidInput(
                        "todos array or index+item is required for update".to_string(),
                    ));
                }
            }
            "clear" => {
                todos.clear();
            }
            "list" => {}
            _ => return Err(ToolError::InvalidInput(format!("unknown action: {action}"))),
        }

        if action != "list" {
            std::fs::write(
                &todo_path,
                serde_json::to_string_pretty(&todos)
                    .map_err(|e| ToolError::Execution(e.to_string()))?,
            )
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        }

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({ "todos": todos }),
            error: None,
        })
    }
}

/// Tool for PLAN mode agents to request switching to BUILD mode.
pub struct RequestBuildModeTool;

impl Tool for RequestBuildModeTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "agent.request_build_mode".into(),
            description: "Request to switch from PLAN mode to BUILD mode. Use this when the plan is complete and ready for execution.".into(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        Err(ToolError::Execution(
            "agent.request_build_mode is orchestrator-managed and cannot be invoked directly"
                .to_string(),
        ))
    }
}

/// Tool for BUILD mode agents to request switching to PLAN mode.
pub struct RequestPlanModeTool;

impl Tool for RequestPlanModeTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "agent.request_plan_mode".into(),
            description: "Request to switch from BUILD mode to PLAN mode. Use this when you need to replan or create a new plan.".into(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        Err(ToolError::Execution(
            "agent.request_plan_mode is orchestrator-managed and cannot be invoked directly"
                .to_string(),
        ))
    }
}

/// Tool for creating artifacts (plan documents, etc.).
pub struct CreateArtifactTool;

impl Tool for CreateArtifactTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "agent.create_artifact".into(),
            description: "Create an artifact (e.g., a plan document). The content will be saved to the workspace.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "filename": {"type": "string", "description": "Name of the artifact file"},
                    "content": {"type": "string", "description": "Content of the artifact"},
                    "kind": {"type": "string", "description": "Type of artifact (e.g., 'plan', 'summary')"}
                },
                "required": ["filename", "content"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        Err(ToolError::Execution(
            "agent.create_artifact is orchestrator-managed and cannot be invoked directly"
                .to_string(),
        ))
    }
}

/// Tool for spawning sub-agents.
pub struct SubAgentSpawnTool;

impl Tool for SubAgentSpawnTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "subagent.spawn".into(),
            description: "Delegate a focused objective to a child sub-agent. Use this instead of implicit delegation actions.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "objective": {"type": "string", "description": "Focused delegated objective"},
                    "max_retries": {"type": "integer", "description": "Optional retries for delegated objective"}
                },
                "required": ["objective"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        Err(ToolError::Execution(
            "subagent.spawn is orchestrator-managed and cannot be invoked directly".to_string(),
        ))
    }
}
