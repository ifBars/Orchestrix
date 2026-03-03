use std::path::Path;

use crate::core::agent_presets::{self, AgentMode, AgentPreset, ToolPermission};
use crate::core::preferences_memory;
use crate::core::tool::ToolDescriptor;
use crate::policy::{PolicyDecision, PolicyEngine};
use crate::runtime::questions::{UserQuestionOption, UserQuestionRequest};
use crate::tools::args::{
    schema_for_type, AgentAskUserArgs, AgentCompleteArgs, AgentCreatePresetArgs, AgentTaskArgs,
    CreateArtifactArgs, MemoryUpsertArgs, SubAgentSpawnArgs,
};
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

/// Tool for asking the user targeted preference questions.
pub struct AgentAskUserTool;

impl Tool for AgentAskUserTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "agent.ask_user".into(),
            description: "Ask the user a clarification/preference question and pause execution until answered.".into(),
            input_schema: schema_for_type::<AgentAskUserArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: AgentAskUserArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let question_text = args.question.trim();
        if question_text.is_empty() {
            return Err(ToolError::InvalidInput("question is required".to_string()));
        }

        if args.options.is_empty() {
            return Err(ToolError::InvalidInput(
                "options array cannot be empty".to_string(),
            ));
        }

        let options: Vec<UserQuestionOption> = args
            .options
            .into_iter()
            .map(|opt| {
                let id = opt.id.trim().to_string();
                if id.is_empty() {
                    return Err(ToolError::InvalidInput(
                        "each option requires non-empty id".to_string(),
                    ));
                }
                let label = opt.label.trim().to_string();
                if label.is_empty() {
                    return Err(ToolError::InvalidInput(
                        "each option requires non-empty label".to_string(),
                    ));
                }
                Ok(UserQuestionOption {
                    id,
                    label,
                    description: opt.description,
                })
            })
            .collect::<Result<Vec<UserQuestionOption>, ToolError>>()?;

        Err(ToolError::UserQuestionRequired {
            question: UserQuestionRequest {
                id: String::new(),
                task_id: String::new(),
                run_id: String::new(),
                sub_agent_id: String::new(),
                tool_call_id: String::new(),
                question: question_text.to_string(),
                options,
                multiple: args.multiple.unwrap_or(false),
                allow_custom: args.allow_custom.unwrap_or(true),
                created_at: String::new(),
            },
        })
    }
}

/// Compatibility tool for persisting preferences into auto memory.
pub struct AgentMemoryUpsertTool;

impl Tool for AgentMemoryUpsertTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "agent.memory_upsert".into(),
            description: "Store or update a durable user preference in auto memory (MEMORY.md) for future runs."
                .into(),
            input_schema: schema_for_type::<MemoryUpsertArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: MemoryUpsertArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let key = args.key.trim();
        if key.is_empty() {
            return Err(ToolError::InvalidInput("key is required".to_string()));
        }
        let value = args.value.trim();
        if value.is_empty() {
            return Err(ToolError::InvalidInput("value is required".to_string()));
        }
        let category = args
            .category
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());

        let entry = preferences_memory::upsert_preference(cwd, key, value, category)
            .map_err(ToolError::Execution)?;

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "stored": true,
                "entry": entry,
            }),
            error: None,
        })
    }
}

/// Tool for managing agent task lists.
pub struct AgentTaskTool;

impl Tool for AgentTaskTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "agent.task".into(),
            description: concat!(
                "Manage the agent's task list and coordinate sub-agents. Actions: list, set, add, update, clear. ",
                "For 'update', pass a 'tasks' array where position determines which task to update. ",
                "Use 'list_id' to scope tasks to a specific agent/run. Tasks help communicate dependencies and share updates across agents."
            ).into(),
            input_schema: schema_for_type::<AgentTaskArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: AgentTaskArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let action = args.action.as_deref().unwrap_or("list");
        let list_id = args.list_id.as_deref();

        let state_dir = cwd.join(".orchestrix");
        std::fs::create_dir_all(&state_dir).map_err(|e| ToolError::Execution(e.to_string()))?;

        // Scope task file to list_id if provided, otherwise use default
        let task_path = if let Some(id) = list_id {
            // Sanitize list_id to be filesystem-safe
            let safe_id = id.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_");
            state_dir.join(format!("agent-task-{}.json", safe_id))
        } else {
            state_dir.join("agent-task.json")
        };

        let mut tasks: Vec<serde_json::Value> = if task_path.exists() {
            let raw = std::fs::read_to_string(&task_path)
                .map_err(|e| ToolError::Execution(e.to_string()))?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            Vec::new()
        };

        match action {
            "set" => {
                let next = args.tasks.clone().ok_or_else(|| {
                    ToolError::InvalidInput("tasks array is required for set".to_string())
                })?;
                tasks = next;
            }
            "add" => {
                let item = args.item.as_ref().ok_or_else(|| {
                    ToolError::InvalidInput("item is required for add".to_string())
                })?;
                tasks.push(item.clone());
            }
            "update" => {
                if let Some(items) = args.tasks.as_ref() {
                    for (idx, item) in items.iter().enumerate() {
                        if idx < tasks.len() {
                            tasks[idx] = item.clone();
                        }
                    }
                } else if let Some(idx) = args.index {
                    let idx = idx as usize;
                    let item = args.item.as_ref().ok_or_else(|| {
                        ToolError::InvalidInput("item is required when using index".to_string())
                    })?;
                    if idx >= tasks.len() {
                        return Err(ToolError::InvalidInput("index out of range".to_string()));
                    }
                    tasks[idx] = item.clone();
                } else {
                    return Err(ToolError::InvalidInput(
                        "tasks array or index+item is required for update".to_string(),
                    ));
                }
            }
            "clear" => {
                tasks.clear();
            }
            "list" => {}
            _ => return Err(ToolError::InvalidInput(format!("unknown action: {action}"))),
        }

        if action != "list" {
            std::fs::write(
                &task_path,
                serde_json::to_string_pretty(&tasks)
                    .map_err(|e| ToolError::Execution(e.to_string()))?,
            )
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        }

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({ "tasks": tasks }),
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
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let reason = input
            .get("reason")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("requested by agent");
        let ready_to_build = input
            .get("ready_to_build")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "requested": true,
                "target_mode": "build",
                "reason": reason,
                "ready_to_build": ready_to_build,
            }),
            error: None,
        })
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
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let reason = input
            .get("reason")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("requested by agent");
        let needs_revision = input
            .get("needs_revision")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "requested": true,
                "target_mode": "plan",
                "reason": reason,
                "needs_revision": needs_revision,
            }),
            error: None,
        })
    }
}

/// Tool for creating artifacts (plan documents, etc.).
pub struct CreateArtifactTool;

impl Tool for CreateArtifactTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "agent.create_artifact".into(),
            description: "Create an artifact (e.g., a plan document). The content will be saved to the workspace.".into(),
            input_schema: schema_for_type::<CreateArtifactArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: CreateArtifactArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let filename = args.filename.trim();
        if filename.is_empty() {
            return Err(ToolError::InvalidInput("filename is required".to_string()));
        }
        let kind = args.kind.as_deref().unwrap_or("note");

        let relative = std::path::Path::new(filename);
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir
                        | std::path::Component::Prefix(_)
                        | std::path::Component::RootDir
                )
            })
        {
            return Err(ToolError::InvalidInput(
                "filename must be a safe relative path".to_string(),
            ));
        }

        let artifacts_root = cwd.join(".orchestrix").join("artifacts");
        let artifact_path = artifacts_root.join(relative);

        match policy.evaluate_path(&artifact_path) {
            PolicyDecision::Allow => {}
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason });
            }
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
        }

        if let Some(parent) = artifact_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::Execution(e.to_string()))?;
        }
        std::fs::write(&artifact_path, &args.content)
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "path": artifact_path.to_string_lossy().to_string(),
                "filename": filename,
                "kind": kind,
                "bytes": args.content.as_bytes().len(),
            }),
            error: None,
        })
    }
}

/// Tool for spawning sub-agents.
pub struct SubAgentSpawnTool;

impl Tool for SubAgentSpawnTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "subagent.spawn".into(),
            description: "Delegate a focused objective to a child sub-agent. Use this instead of implicit delegation actions.".into(),
            input_schema: schema_for_type::<SubAgentSpawnArgs>(),
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

/// Tool for explicitly marking delegated work complete.
pub struct AgentCompleteTool;

impl Tool for AgentCompleteTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "agent.complete".into(),
            description: "Mark the current delegated objective as complete and stop further tool calls for this agent turn loop.".into(),
            input_schema: schema_for_type::<AgentCompleteArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: AgentCompleteArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let summary = args.summary.trim();
        if summary.is_empty() {
            return Err(ToolError::InvalidInput(
                "summary is required for agent.complete".to_string(),
            ));
        }

        let outputs = args.outputs.unwrap_or_default();
        let confidence = args
            .confidence
            .as_deref()
            .filter(|v| matches!(*v, "low" | "medium" | "high"))
            .unwrap_or("medium");

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "completed": true,
                "summary": summary,
                "outputs": outputs,
                "confidence": confidence,
            }),
            error: None,
        })
    }
}

/// Tool for creating agent presets.
pub struct AgentCreatePresetTool;

impl Tool for AgentCreatePresetTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "agent.create_preset".into(),
            description: "Create a new agent preset that can be used with @agent:id mentions or for subagent delegation. The preset will be saved to the workspace and can be referenced in prompts or delegated to as a subagent.".into(),
            input_schema: schema_for_type::<AgentCreatePresetArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: AgentCreatePresetArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        // Extract required fields
        let id = args.id.trim();
        if id.is_empty() {
            return Err(ToolError::InvalidInput(
                "id is required and must be non-empty".to_string(),
            ));
        }

        let name = args.name.trim();
        if name.is_empty() {
            return Err(ToolError::InvalidInput(
                "name is required and must be non-empty".to_string(),
            ));
        }

        let prompt = args.prompt.trim();
        if prompt.is_empty() {
            return Err(ToolError::InvalidInput(
                "prompt is required and must be non-empty".to_string(),
            ));
        }

        // Validate ID format (kebab-case, alphanumeric, hyphens, underscores)
        let valid_id = id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid_id {
            return Err(ToolError::InvalidInput(format!(
                "invalid agent ID '{}': must be alphanumeric with hyphens/underscores only",
                id
            )));
        }

        // Parse mode
        let mode_str = args.mode.as_deref().map(|m| m.to_ascii_lowercase());

        let mode = match mode_str.as_deref() {
            Some("primary") => AgentMode::Primary,
            Some("subagent") => AgentMode::Subagent,
            Some(m) => {
                return Err(ToolError::InvalidInput(format!(
                    "invalid mode '{}': must be 'primary' or 'subagent'",
                    m
                )))
            }
            None => AgentMode::Subagent,
        };
        let mode_clone = mode.clone();

        // Optional fields
        let description = args.description.as_deref().unwrap_or("").to_string();

        let model = args
            .model
            .as_deref()
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string());

        let temperature = args
            .temperature
            .map(|t| t as f32)
            .filter(|&t| t >= 0.0 && t <= 2.0);

        let steps = args.steps.filter(|&s| s <= 1000);

        let tags = args
            .tags
            .unwrap_or_default()
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect();

        // Parse tool permissions
        let tools = args.tools.map(|obj| {
            let mut tools_map = std::collections::HashMap::new();

            if let Some(write) = obj.write {
                tools_map.insert("write".to_string(), ToolPermission::Bool(write));
            }
            if let Some(edit) = obj.edit {
                tools_map.insert("edit".to_string(), ToolPermission::Bool(edit));
            }
            if let Some(bash) = obj.bash {
                tools_map.insert("bash".to_string(), ToolPermission::Bool(bash));
            }

            tools_map
        });

        // Build the preset
        let preset = AgentPreset {
            id: id.to_string(),
            name: name.to_string(),
            description,
            mode: mode_clone,
            model,
            temperature,
            steps,
            tools,
            permission: None,
            prompt: prompt.to_string(),
            tags,
            file_path: String::new(), // Will be set by write_agent_preset
            source: "workspace".to_string(),
            enabled: true,
            validation_issues: vec![],
        };

        // Write to file
        let file_path = agent_presets::write_agent_preset(cwd, id, &preset)
            .map_err(|e| ToolError::Execution(format!("failed to write agent preset: {}", e)))?;

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "id": id,
                "name": name,
                "mode": match mode {
                    AgentMode::Primary => "primary",
                    AgentMode::Subagent => "subagent",
                },
                "file_path": file_path,
                "mention_token": format!("@agent:{}", id),
                "message": format!(
                    "Agent preset '{}' created successfully. You can reference it with @agent:{} in prompts or delegate to it via subagent.spawn.",
                    name, id
                ),
            }),
            error: None,
        })
    }
}
