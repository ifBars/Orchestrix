//! Worker helper functions.

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
