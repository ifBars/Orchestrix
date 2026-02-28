//! Worker helper functions.

// ---------------------------------------------------------------------------
// Agent Mode
// ---------------------------------------------------------------------------

/// Execution mode for the agent (PLAN or BUILD).
/// Mode-specific restrictions are enforced at tool execution time to preserve
/// prompt cache (tools are always included in requests, filtered at execution).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentMode {
    /// Plan mode - read-only exploration and planning
    Plan,
    /// Build mode - execution and implementation
    #[default]
    Build,
}

impl AgentMode {
    /// Parse mode from context JSON.
    pub fn from_context(context_json: Option<&str>) -> Self {
        let mode_str = context_json.and_then(|ctx| {
            serde_json::from_str::<serde_json::Value>(ctx)
                .ok()
                .and_then(|v| v.get("mode").and_then(|m| m.as_str().map(String::from)))
        });

        match mode_str.as_deref() {
            Some("plan") => AgentMode::Plan,
            Some("build") => AgentMode::Build,
            _ => AgentMode::Build,
        }
    }
}

/// Tools that are write/execute tools and should be blocked in PLAN mode.
/// These are always included in API requests for cache purposes, but rejected
/// at execution time if the agent is in PLAN mode.
const WRITE_TOOLS_IN_PLAN_MODE: &[&str] = &[
    "fs.write",
    "fs.patch",
    "cmd.exec",
    "subagent.spawn",
    "git.commit",
    "git.apply_patch",
    "dev_server.start",
    "dev_server.stop",
    "skills.install",
    "skills.remove",
    "memory.upsert",
    "memory.delete",
    "memory.compact",
];

/// Tools that are plan-mode only and should be blocked in BUILD mode.
/// (Currently none, as build mode should have access to everything)
const PLAN_MODE_ONLY_TOOLS: &[&str] = &[
    // Currently empty - plan mode tools are available in build mode too
];

/// Check if a tool is allowed in the given mode.
/// Returns false if the tool should be blocked.
pub fn is_tool_allowed_in_mode(tool_name: &str, mode: AgentMode) -> bool {
    match mode {
        AgentMode::Plan => {
            // Block write/execute tools in plan mode
            !WRITE_TOOLS_IN_PLAN_MODE.contains(&tool_name)
        }
        AgentMode::Build => {
            // Allow all tools in build mode (plan-only tools available too)
            !PLAN_MODE_ONLY_TOOLS.contains(&tool_name)
        }
    }
}

/// Filter available tools based on the current mode.
/// This is used to restrict tools at execution time while keeping the full
/// tool list in API requests for cache-friendly execution.
pub fn filter_tools_for_mode(tools: &[String], mode: AgentMode) -> Vec<String> {
    tools
        .iter()
        .filter(|name| is_tool_allowed_in_mode(name, mode))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Sub-agent Contract
// ---------------------------------------------------------------------------

/// Parse the sub-agent contract from context JSON.
pub fn parse_sub_agent_contract(context_json: Option<&str>) -> SubAgentContract {
    context_json
        .and_then(|ctx| {
            serde_json::from_str::<serde_json::Value>(ctx)
                .ok()
                .and_then(|v| {
                    v.get("contract")
                        .and_then(|c| serde_json::from_value::<SubAgentContract>(c.clone()).ok())
                })
        })
        .unwrap_or_default()
}

/// Sub-agent contract definition.
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct SubAgentContract {
    #[serde(default)]
    pub permissions: SubAgentPermissions,
    #[serde(default)]
    pub execution: SubAgentExecution,
}

impl Default for SubAgentContract {
    fn default() -> Self {
        Self {
            permissions: SubAgentPermissions::default(),
            execution: SubAgentExecution::default(),
        }
    }
}

/// Permissions for a sub-agent.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SubAgentPermissions {
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default = "default_can_spawn")]
    pub can_spawn_children: bool,
    #[serde(default = "default_max_depth")]
    pub max_delegation_depth: u32,
}

impl Default for SubAgentPermissions {
    fn default() -> Self {
        Self {
            allowed_tools: Vec::new(),
            can_spawn_children: default_can_spawn(),
            max_delegation_depth: default_max_depth(),
        }
    }
}

/// Execution settings for a sub-agent.
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct SubAgentExecution {
    #[serde(default = "default_timeout")]
    pub attempt_timeout_ms: u64,
    #[serde(default)]
    pub close_on_completion: bool,
}

impl Default for SubAgentExecution {
    fn default() -> Self {
        Self {
            attempt_timeout_ms: default_timeout(),
            close_on_completion: false,
        }
    }
}

fn default_can_spawn() -> bool {
    true
}

fn default_max_depth() -> u32 {
    3
}

fn default_timeout() -> u64 {
    300_000 // 5 minutes
}

/// Count open todos in the latest todo observation.
///
/// Returns Some(count) if the last observation is a successful agent.todo call
/// with pending/in_progress items, None otherwise.
pub fn open_todos_in_latest_todo_observation(observations: &[serde_json::Value]) -> Option<usize> {
    let last = observations.last()?;
    let tool_name = last.get("tool_name")?.as_str()?;
    if tool_name != "agent.todo" {
        return None;
    }

    let status = last.get("status")?.as_str()?;
    if status != "succeeded" {
        return None;
    }

    let todos = last.get("output")?.get("todos")?.as_array()?;
    let open = todos
        .iter()
        .filter(|todo| {
            let state = todo
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("pending");
            state != "completed" && state != "cancelled"
        })
        .count();

    Some(open)
}
