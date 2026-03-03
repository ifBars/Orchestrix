//! Skills management tools.
//!
//! New workflow:
//! - skills.list_installed() - list workspace skills with lightweight metadata
//! - skills.search(query) - search available skills from remote sources
//! - skills.load(skill_id) - load full skill content into context
//! - skills.unload(skill_id) - remove skill from active context
//!
//! The skills are NOT auto-loaded. AI must discover and load them explicitly.

use std::path::Path;

use crate::core::skills::search_agent_skills;
use crate::core::tool::ToolDescriptor;
use crate::core::workspace_skills::{scan_workspace_skills, WorkspaceSkill};
use crate::policy::PolicyEngine;
use crate::tools::args::{
    schema_for_type, SkillsInstallArgs, SkillsListInstalledArgs, SkillsLoadArgs, SkillsRemoveArgs,
    SkillsSearchArgs,
};
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

/// Lightweight skill info for listing (without full content)
#[derive(serde::Serialize)]
struct SkillSummary {
    id: String,
    name: String,
    description: String,
    tags: Vec<String>,
    source: String,
    is_builtin: bool,
    is_loaded: bool,
}

/// Tool for listing installed workspace skills with lightweight metadata.
pub struct SkillsListInstalledTool;

impl Tool for SkillsListInstalledTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "skills.list_installed".into(),
            description: "List all installed workspace skills with lightweight metadata. Use this to discover available skills before loading one.".into(),
            input_schema: schema_for_type::<SkillsListInstalledArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: SkillsListInstalledArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let source_filter = args.source.as_deref().unwrap_or("all");

        let skills = scan_workspace_skills(cwd);

        let filtered: Vec<SkillSummary> = skills
            .into_iter()
            .filter(|s| match source_filter {
                "builtin" => s.source == "builtin",
                "workspace" => s.source == "workspace",
                "global" => s.source == "global",
                _ => true,
            })
            .map(|s| SkillSummary {
                id: s.id,
                name: s.name,
                description: s.description,
                tags: s.tags,
                source: s.source,
                is_builtin: s.is_builtin,
                is_loaded: false, // Will be tracked in conversation context
            })
            .collect();

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "skills": filtered,
                "count": filtered.len(),
                "hint": "Use skills.load with a skill_id to load its full content into context"
            }),
            error: None,
        })
    }
}

/// Tool for searching skills from remote sources.
pub struct SkillsSearchTool;

impl Tool for SkillsSearchTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "skills.search".into(),
            description: "Search for skills from remote sources (e.g., vercel-labs/agent-skills). Returns ranked results with confidence and suggestions.".into(),
            input_schema: schema_for_type::<SkillsSearchArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: SkillsSearchArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let limit = args.limit.unwrap_or(10) as usize;

        let query = args.query.clone();
        let results = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { search_agent_skills(&query, limit).await })
        });

        match results {
            Ok(items) => {
                let formatted: Vec<serde_json::Value> = items
                    .into_iter()
                    .map(|item| {
                        serde_json::json!({
                            "skill_id": item.skill_name,
                            "name": item.title,
                            "description": item.description,
                            "source": item.source,
                            "installs": item.installs,
                            "url": item.url,
                            "install_command": item.install_command,
                            "suggested_action": "Use skills.install to install this skill, then load it"
                        })
                    })
                    .collect();

                Ok(ToolCallOutput {
                    ok: true,
                    data: serde_json::json!({
                        "results": formatted,
                        "count": formatted.len(),
                        "query": query,
                        "hint": "Found skills. Use skills.install to install, then skills.load to activate"
                    }),
                    error: None,
                })
            }
            Err(e) => Ok(ToolCallOutput {
                ok: false,
                data: serde_json::json!({"results": [], "error": e}),
                error: Some(e),
            }),
        }
    }
}

/// Tool for loading a skill's full content into context.
pub struct SkillsLoadTool;

impl Tool for SkillsLoadTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "skills.load".into(),
            description: "Load a skill's full content into the current context. The skill content will be returned for you to incorporate into your instructions. Always use skills.list_installed or skills.search first to discover available skills.".into(),
            input_schema: schema_for_type::<SkillsLoadArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: SkillsLoadArgs = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        let skill_id = args.skill_id.as_deref();
        let name = args.name.as_deref();
        let query = args.query.as_deref();

        if skill_id.is_none() && name.is_none() && query.is_none() {
            return Err(ToolError::InvalidInput(
                "skill_id, name, or query is required".to_string(),
            ));
        }

        let skills = scan_workspace_skills(cwd);

        // Resolve the skill
        let resolved = if let Some(q) = query {
            // Search by query
            let q_lower = q.to_lowercase();
            let mut candidates: Vec<&WorkspaceSkill> = skills
                .iter()
                .filter(|s| {
                    s.name.to_lowercase().contains(&q_lower)
                        || s.description.to_lowercase().contains(&q_lower)
                        || s.tags.iter().any(|t| t.to_lowercase().contains(&q_lower))
                        || s.id.to_lowercase().contains(&q_lower)
                })
                .collect();

            if candidates.len() == 1 {
                Some(candidates.remove(0))
            } else if candidates.is_empty() {
                // Try fuzzy search on IDs
                skills
                    .iter()
                    .find(|s| s.id.to_lowercase().contains(&q_lower.replace(' ', "-")))
            } else {
                // Multiple matches - return disambiguation
                let suggestions: Vec<_> = candidates
                    .iter()
                    .map(|s| serde_json::json!({"id": s.id, "name": s.name}))
                    .collect();
                return Ok(ToolCallOutput {
                    ok: true,
                    data: serde_json::json!({
                        "status": "multiple_matches",
                        "message": format!("Multiple skills match '{}'. Specify one of:", q),
                        "suggestions": suggestions
                    }),
                    error: None,
                });
            }
        } else if let Some(id) = skill_id {
            skills.iter().find(|s| s.id == id)
        } else if let Some(n) = name {
            let n_lower = n.to_lowercase();
            skills.iter().find(|s| {
                s.id.to_lowercase() == n_lower || s.name.to_lowercase().contains(&n_lower)
            })
        } else {
            None
        };

        match resolved {
            Some(skill) => Ok(ToolCallOutput {
                ok: true,
                data: serde_json::json!({
                    "status": "loaded",
                    "skill_id": skill.id,
                    "name": skill.name,
                    "description": skill.description,
                    "content": skill.content,
                    "tags": skill.tags,
                    "message": format!("Skill '{}' loaded. Follow its instructions for relevant tasks.", skill.name)
                }),
                error: None,
            }),
            None => {
                // Provide helpful error with suggestions
                Ok(ToolCallOutput {
                    ok: false,
                    data: serde_json::json!({
                        "status": "not_found",
                        "message": "Skill not found. Use skills.list_installed() to see available skills or skills.search() to find more."
                    }),
                    error: Some("Skill not found".to_string()),
                })
            }
        }
    }
}

/// Tool for installing a skill from remote sources.
pub struct SkillsInstallTool;

impl Tool for SkillsInstallTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "skills.install".into(),
            description: "Install a skill from remote sources (vercel-labs/agent-skills). After installing, use skills.load to activate it.".into(),
            input_schema: schema_for_type::<SkillsInstallArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        // This tool would need to call into the existing install logic
        // For now, we return a message to use the UI or existing install mechanism
        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "message": "Please use the Skills Search UI to install new skills, or provide more details for programmatic installation.",
                "hint": "The UI provides a better experience for discovering and installing skills."
            }),
            error: None,
        })
    }
}

/// Tool for removing workspace skills.
pub struct SkillsRemoveTool;

impl Tool for SkillsRemoveTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "skills.remove".into(),
            description: "Remove a workspace skill. Cannot remove built-in skills.".into(),
            input_schema: schema_for_type::<SkillsRemoveArgs>(),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        // This would need proper implementation with workspace path
        // For now, return a message
        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "message": "Please use the Settings UI to remove workspace skills.",
                "hint": "Navigate to Settings > Skills to manage installed skills."
            }),
            error: None,
        })
    }
}
