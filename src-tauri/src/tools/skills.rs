//! Skills management tools.

use std::path::Path;

use crate::core::skills::{
    add_custom_skill, import_context7_skill, import_vercel_skill, list_all_skills,
    remove_custom_skill, NewCustomSkill,
};
use crate::core::tool::ToolDescriptor;
use crate::policy::PolicyEngine;
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

/// Tool for listing all available skills.
pub struct SkillsListTool;

impl Tool for SkillsListTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "skills.list".into(),
            description: "List all available skills (builtin + custom + imported).".into(),
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
        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({"skills": list_all_skills()}),
            error: None,
        })
    }
}

/// Tool for loading/importing skills.
pub struct SkillsLoadTool;

impl Tool for SkillsLoadTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "skills.load".into(),
            description: concat!(
                "Load/import a skill into the local custom catalog. ",
                "First call skills.list to see available skills. ",
                "Modes: 'context7' requires library_id; 'vercel' requires skill_name; ",
                "'custom' requires title, install_command, and url."
            )
            .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["mode"],
                "properties": {
                    "mode": {"type": "string", "enum": ["custom", "context7", "vercel"],
                             "description": "Import mode. Use 'context7' for Context7 libraries, 'vercel' for Vercel agent-skills, 'custom' for manually-defined skills."},
                    "id": {"type": "string", "description": "Optional custom ID"},
                    "title": {"type": "string", "description": "Required for custom mode"},
                    "description": {"type": "string", "description": "Skill description (custom mode)"},
                    "install_command": {"type": "string", "description": "Required for custom mode. How to install the skill."},
                    "url": {"type": "string", "description": "Required for custom mode. URL for the skill."},
                    "source": {"type": "string", "description": "Optional source label (custom mode)"},
                    "tags": {"type": "array", "items": {"type": "string"}, "description": "Optional tags"},
                    "library_id": {"type": "string", "description": "Required for context7 mode. The Context7 library ID."},
                    "skill_name": {"type": "string", "description": "Required for vercel mode. The skill name in vercel-labs/agent-skills."}
                }
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let mode = input
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("custom")
            .to_ascii_lowercase();

        let loaded = match mode.as_str() {
            "context7" => {
                let library_id = input
                    .get("library_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidInput(
                            "library_id is required for context7 mode".to_string(),
                        )
                    })?;
                import_context7_skill(library_id, input.get("title").and_then(|v| v.as_str()))
                    .map_err(ToolError::Execution)?
            }
            "vercel" => {
                let skill_name = input
                    .get("skill_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidInput(
                            "skill_name is required for vercel mode".to_string(),
                        )
                    })?;
                import_vercel_skill(skill_name).map_err(ToolError::Execution)?
            }
            _ => {
                let title = input.get("title").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidInput("title is required for custom mode".to_string())
                })?;
                let description = input
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let install_command = input
                    .get("install_command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidInput(
                            "install_command is required for custom mode".to_string(),
                        )
                    })?;
                let url = input.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidInput("url is required for custom mode".to_string())
                })?;

                let tags = input.get("tags").and_then(|v| v.as_array()).map(|items| {
                    items
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                });

                add_custom_skill(NewCustomSkill {
                    id: input
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|v| v.to_string()),
                    title: title.to_string(),
                    description: description.to_string(),
                    install_command: install_command.to_string(),
                    url: url.to_string(),
                    source: input
                        .get("source")
                        .and_then(|v| v.as_str())
                        .map(|v| v.to_string()),
                    tags,
                })
                .map_err(ToolError::Execution)?
            }
        };

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({"skill": loaded}),
            error: None,
        })
    }
}

/// Tool for removing custom skills.
pub struct SkillsRemoveTool;

impl Tool for SkillsRemoveTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "skills.remove".into(),
            description: "Remove a custom skill from the local catalog.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "skill_id": {"type": "string", "description": "ID of the skill to remove"}
                },
                "required": ["skill_id"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let id = input
            .get("skill_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("skill_id is required".to_string()))?;

        let removed = remove_custom_skill(id).map_err(ToolError::Execution)?;

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({"removed": removed}),
            error: None,
        })
    }
}
