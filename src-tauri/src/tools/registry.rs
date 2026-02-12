//! Tool registry for dynamic tool discovery and invocation.
//!
//! The ToolRegistry manages all available tools including:
//! - Built-in tools (filesystem, git, commands, etc.)
//! - MCP tools loaded from external servers
//!
//! Tools are accessed by name and invoked with policy-checked permissions.

use std::collections::HashMap;
use std::path::Path;

use crate::core::mcp::{call_mcp_tool_by_server_and_name, load_mcp_tools_cache};
use crate::core::tool::ToolDescriptor;
use crate::policy::PolicyEngine;
use crate::tools::agent::{
    AgentCompleteTool, AgentTodoTool, CreateArtifactTool, RequestBuildModeTool,
    RequestPlanModeTool, SubAgentSpawnTool,
};
use crate::tools::cmd::CommandExecTool;
use crate::tools::fs::{FsListTool, FsReadTool, FsWriteTool};
use crate::tools::git::{GitApplyPatchTool, GitCommitTool, GitDiffTool, GitLogTool, GitStatusTool};
use crate::tools::search::SearchRgTool;
use crate::tools::skills::{SkillsListTool, SkillsLoadTool, SkillsRemoveTool};
use crate::tools::types::{Tool, ToolCallInput, ToolCallOutput, ToolError};

/// Registry of all available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Creates a new registry with all built-in tools registered.
    pub fn default() -> Self {
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();

        // Filesystem tools
        tools.insert("fs.read".to_string(), Box::new(FsReadTool));
        tools.insert("fs.write".to_string(), Box::new(FsWriteTool));
        tools.insert("fs.list".to_string(), Box::new(FsListTool));

        // Search tools
        tools.insert("search.rg".to_string(), Box::new(SearchRgTool));

        // Command execution
        tools.insert("cmd.exec".to_string(), Box::new(CommandExecTool));

        // Git tools
        tools.insert("git.status".to_string(), Box::new(GitStatusTool));
        tools.insert("git.diff".to_string(), Box::new(GitDiffTool));
        tools.insert("git.apply_patch".to_string(), Box::new(GitApplyPatchTool));
        tools.insert("git.commit".to_string(), Box::new(GitCommitTool));
        tools.insert("git.log".to_string(), Box::new(GitLogTool));

        // Skills tools
        tools.insert("skills.list".to_string(), Box::new(SkillsListTool));
        tools.insert("skills.load".to_string(), Box::new(SkillsLoadTool));
        tools.insert("skills.remove".to_string(), Box::new(SkillsRemoveTool));

        // Agent tools
        tools.insert("agent.todo".to_string(), Box::new(AgentTodoTool));
        tools.insert("agent.complete".to_string(), Box::new(AgentCompleteTool));
        tools.insert("subagent.spawn".to_string(), Box::new(SubAgentSpawnTool));
        tools.insert(
            "agent.request_build_mode".to_string(),
            Box::new(RequestBuildModeTool),
        );
        tools.insert(
            "agent.request_plan_mode".to_string(),
            Box::new(RequestPlanModeTool),
        );
        tools.insert(
            "agent.create_artifact".to_string(),
            Box::new(CreateArtifactTool),
        );

        Self { tools }
    }

    /// Get tools available for PLAN mode.
    ///
    /// Only includes read-only tools and plan-specific agent tools:
    /// fs.read, fs.list, search.rg, git.*, skills.*, agent.todo,
    /// agent.create_artifact, agent.request_build_mode
    pub fn list_for_plan_mode(&self) -> Vec<ToolDescriptor> {
        let allowed_tools: std::collections::HashSet<&str> = [
            "fs.read",
            "fs.list",
            "search.rg",
            "git.status",
            "git.diff",
            "git.log",
            "skills.list",
            "skills.load",
            "agent.todo",
            "agent.create_artifact",
            "agent.request_build_mode",
        ]
        .iter()
        .cloned()
        .collect();

        self.list()
            .into_iter()
            .filter(|t| allowed_tools.contains(t.name.as_str()))
            .collect()
    }

    /// Get tools available for BUILD mode.
    ///
    /// Includes all tools except request_build_mode and create_artifact.
    pub fn list_for_build_mode(&self) -> Vec<ToolDescriptor> {
        self.list()
            .into_iter()
            .filter(|t| t.name != "agent.request_build_mode" && t.name != "agent.create_artifact")
            .collect()
    }

    /// Generate a detailed tool reference string for PLAN mode.
    pub fn tool_reference_for_plan_mode(&self) -> String {
        let mut tools: Vec<_> = self.list_for_plan_mode();
        tools.sort_by(|a, b| a.name.cmp(&b.name));

        let mut out = String::new();
        for tool in &tools {
            out.push_str(&format!("### {}\n", tool.name));
            out.push_str(&format!("{}\n", tool.description));
            out.push_str(&format!(
                "Input schema: {}\n\n",
                serde_json::to_string(&tool.input_schema).unwrap_or_else(|_| "{}".to_string())
            ));
        }
        out
    }

    /// Generate a detailed tool reference string for BUILD mode.
    pub fn tool_reference_for_build_mode(&self) -> String {
        let mut tools: Vec<_> = self.list_for_build_mode();
        tools.sort_by(|a, b| a.name.cmp(&b.name));

        let mut out = String::new();
        for tool in &tools {
            out.push_str(&format!("### {}\n", tool.name));
            out.push_str(&format!("{}\n", tool.description));
            out.push_str(&format!(
                "Input schema: {}\n\n",
                serde_json::to_string(&tool.input_schema).unwrap_or_else(|_| "{}".to_string())
            ));
        }
        out
    }

    /// List all available tools including MCP tools.
    pub fn list(&self) -> Vec<ToolDescriptor> {
        let mut descriptors: Vec<ToolDescriptor> =
            self.tools.values().map(|t| t.descriptor()).collect();

        // Add MCP tools from cache
        let mcp_descriptors = load_mcp_tools_cache()
            .into_iter()
            .map(|entry| ToolDescriptor {
                name: format!("mcp.{}.{}", entry.server_id, entry.tool_name),
                description: format!("MCP ({}) - {}", entry.server_name, entry.description),
                input_schema: entry.input_schema,
                output_schema: None,
            })
            .collect::<Vec<_>>();

        descriptors.extend(mcp_descriptors);
        descriptors
    }

    /// Generate a detailed tool reference string for inclusion in LLM prompts.
    #[allow(dead_code)]
    pub fn tool_reference_for_prompt(&self) -> String {
        let mut tools: Vec<_> = self.tools.values().map(|t| t.descriptor()).collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));

        let mut out = String::new();
        for tool in &tools {
            out.push_str(&format!("### {}\n", tool.name));
            out.push_str(&format!("{}\n", tool.description));
            out.push_str(&format!(
                "Input schema: {}\n\n",
                serde_json::to_string(&tool.input_schema).unwrap_or_else(|_| "{}".to_string())
            ));
        }
        out
    }

    /// Invoke a tool by name with the given arguments.
    pub fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        call: ToolCallInput,
    ) -> Result<ToolCallOutput, ToolError> {
        // Try built-in tools first
        if let Some(tool) = self.tools.get(&call.name) {
            return tool.invoke(policy, cwd, call.args);
        }

        // Try MCP tools
        if let Some((server_id, tool_name)) = parse_mcp_tool_name(&call.name) {
            let result = call_mcp_tool_by_server_and_name(&server_id, &tool_name, call.args)
                .map_err(ToolError::Execution)?;
            return Ok(ToolCallOutput {
                ok: true,
                data: result,
                error: None,
            });
        }

        Err(ToolError::InvalidInput(format!(
            "unknown tool: {}",
            call.name
        )))
    }
}

/// Parse an MCP tool name in the format "mcp.{server_id}.{tool_name}".
fn parse_mcp_tool_name(raw: &str) -> Option<(String, String)> {
    if !raw.starts_with("mcp.") {
        return None;
    }

    let without_prefix = &raw[4..];
    let (server_id, tool_name) = without_prefix.split_once('.')?;
    if server_id.trim().is_empty() || tool_name.trim().is_empty() {
        return None;
    }

    Some((server_id.to_string(), tool_name.to_string()))
}
