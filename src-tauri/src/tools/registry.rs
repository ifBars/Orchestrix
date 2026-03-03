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
    AgentAskUserTool, AgentCompleteTool, AgentCreatePresetTool, AgentMemoryUpsertTool,
    AgentTaskTool, CreateArtifactTool, RequestBuildModeTool, RequestPlanModeTool,
    SubAgentSpawnTool,
};
use crate::tools::canvas::{CanvasApplyOpsTool, CanvasReadStateTool};
use crate::tools::cmd::CommandExecTool;
use crate::tools::dev_server::{
    DevServerLogsTool, DevServerStartTool, DevServerStatusTool, DevServerStopTool,
};
use crate::tools::file_search::SearchFilesTool;
use crate::tools::fs::{FsListTool, FsReadTool, FsWriteTool};
use crate::tools::git::{GitApplyPatchTool, GitCommitTool, GitDiffTool, GitLogTool, GitStatusTool};
use crate::tools::memory::{
    MemoryCompactTool, MemoryDeleteTool, MemoryListTool, MemoryReadTool, MemoryUpsertTool,
};
use crate::tools::patch::FsPatchTool;
use crate::tools::search::SearchRgTool;
use crate::tools::semantic_search::SearchEmbeddingsTool;
use crate::tools::skills::{
    SkillsInstallTool, SkillsListInstalledTool, SkillsLoadTool, SkillsRemoveTool, SkillsSearchTool,
};
use crate::tools::types::{Tool, ToolCallInput, ToolCallOutput, ToolError};
use crate::tools::web_snapshot::WebSnapshotTool;

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
        tools.insert("fs.patch".to_string(), Box::new(FsPatchTool));

        // Search tools
        tools.insert("search.rg".to_string(), Box::new(SearchRgTool));
        tools.insert("search.files".to_string(), Box::new(SearchFilesTool));
        tools.insert(
            "search.embeddings".to_string(),
            Box::new(SearchEmbeddingsTool),
        );

        // Command execution
        tools.insert("cmd.exec".to_string(), Box::new(CommandExecTool));

        // Canvas tools
        tools.insert(
            "diagram.read_graph".to_string(),
            Box::new(CanvasReadStateTool),
        );
        tools.insert(
            "diagram.apply_ops".to_string(),
            Box::new(CanvasApplyOpsTool),
        );

        // Git tools
        tools.insert("git.status".to_string(), Box::new(GitStatusTool));
        tools.insert("git.diff".to_string(), Box::new(GitDiffTool));
        tools.insert("git.apply_patch".to_string(), Box::new(GitApplyPatchTool));
        tools.insert("git.commit".to_string(), Box::new(GitCommitTool));
        tools.insert("git.log".to_string(), Box::new(GitLogTool));

        // Skills tools
        tools.insert(
            "skills.list_installed".to_string(),
            Box::new(SkillsListInstalledTool),
        );
        tools.insert("skills.search".to_string(), Box::new(SkillsSearchTool));
        tools.insert("skills.load".to_string(), Box::new(SkillsLoadTool));
        tools.insert("skills.install".to_string(), Box::new(SkillsInstallTool));
        tools.insert("skills.remove".to_string(), Box::new(SkillsRemoveTool));

        // Memory tools
        tools.insert("memory.list".to_string(), Box::new(MemoryListTool));
        tools.insert("memory.read".to_string(), Box::new(MemoryReadTool));
        tools.insert("memory.upsert".to_string(), Box::new(MemoryUpsertTool));
        tools.insert("memory.delete".to_string(), Box::new(MemoryDeleteTool));
        tools.insert("memory.compact".to_string(), Box::new(MemoryCompactTool));

        // Agent tools
        tools.insert("agent.task".to_string(), Box::new(AgentTaskTool));
        tools.insert("agent.ask_user".to_string(), Box::new(AgentAskUserTool));
        tools.insert(
            "agent.memory_upsert".to_string(),
            Box::new(AgentMemoryUpsertTool),
        );
        tools.insert("agent.complete".to_string(), Box::new(AgentCompleteTool));
        tools.insert(
            "agent.create_preset".to_string(),
            Box::new(AgentCreatePresetTool),
        );
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

        // Dev server tools
        tools.insert("dev_server.start".to_string(), Box::new(DevServerStartTool));
        tools.insert("dev_server.stop".to_string(), Box::new(DevServerStopTool));
        tools.insert(
            "dev_server.status".to_string(),
            Box::new(DevServerStatusTool),
        );
        tools.insert("dev_server.logs".to_string(), Box::new(DevServerLogsTool));

        // Web snapshot tool
        tools.insert("web.snapshot".to_string(), Box::new(WebSnapshotTool));

        Self { tools }
    }

    /// List ALL available tools (unified list for cache-safe execution).
    ///
    /// This returns all tools regardless of mode. Mode-specific restrictions
    /// are enforced at execution time, not at prompt construction time.
    /// This ensures maximum cache reuse across plan/build mode transitions.
    pub fn list_all(&self, include_embeddings: bool) -> Vec<ToolDescriptor> {
        self.list()
            .into_iter()
            .filter(|t| {
                if !include_embeddings && t.name == "search.embeddings" {
                    return false;
                }
                true
            })
            .collect()
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
