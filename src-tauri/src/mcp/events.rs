//! MCP event types for transparency and monitoring.
//!
//! These events are emitted during MCP operations to provide visibility
//! into server health, tool discovery, and tool execution.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Events emitted by the MCP client during operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum McpEvent {
    // Server lifecycle events
    /// A new MCP server was added.
    ServerAdded {
        server_id: String,
        server_name: String,
        transport: String,
    },

    /// An MCP server configuration was updated.
    ServerUpdated {
        server_id: String,
        server_name: String,
    },

    /// An MCP server was removed.
    ServerRemoved {
        server_id: String,
        server_name: String,
    },

    /// Server health status changed.
    ServerHealthChanged {
        server_id: String,
        server_name: String,
        health: String,
        previous_health: Option<String>,
    },

    /// Server encountered an error.
    ServerError {
        server_id: String,
        server_name: String,
        error: String,
    },

    // Tool discovery events
    /// Tool discovery started for a server.
    ToolDiscoveryStarted {
        server_id: String,
        server_name: String,
    },

    /// Tool discovery completed successfully.
    ToolDiscoveryCompleted {
        server_id: String,
        server_name: String,
        tool_count: usize,
        duration_ms: u64,
    },

    /// Tool discovery failed.
    ToolDiscoveryFailed {
        server_id: String,
        server_name: String,
        error: String,
    },

    /// Tools cache was refreshed.
    ToolsCacheRefreshed {
        total_tools: usize,
        server_count: usize,
    },

    // Tool execution events
    /// A tool call has started.
    ToolCallStarted {
        server_id: String,
        server_name: String,
        tool_name: String,
    },

    /// A tool call has completed.
    ToolCallCompleted {
        server_id: String,
        server_name: String,
        tool_name: String,
        duration_ms: u64,
        success: bool,
    },

    /// A tool call has failed.
    ToolCallFailed {
        server_id: String,
        server_name: String,
        tool_name: String,
        error: String,
    },

    /// A tool requires user approval.
    ToolApprovalRequired {
        server_id: String,
        server_name: String,
        tool_name: String,
    },

    /// Tool approval was granted.
    ToolApprovalGranted {
        server_id: String,
        tool_name: String,
    },

    /// Tool approval was denied.
    ToolApprovalDenied {
        server_id: String,
        tool_name: String,
        reason: String,
    },

    // Resource events
    /// Resource list retrieval started.
    ResourceListStarted {
        server_id: String,
        server_name: String,
    },

    /// Resource list retrieval completed.
    ResourceListCompleted {
        server_id: String,
        server_name: String,
        resource_count: usize,
        duration_ms: u64,
    },

    /// Resource list retrieval failed.
    ResourceListFailed {
        server_id: String,
        server_name: String,
        error: String,
    },

    /// Resource read started.
    ResourceReadStarted {
        server_id: String,
        server_name: String,
        uri: String,
    },

    /// Resource read completed.
    ResourceReadCompleted {
        server_id: String,
        server_name: String,
        uri: String,
        duration_ms: u64,
    },

    /// Resource read failed.
    ResourceReadFailed {
        server_id: String,
        server_name: String,
        uri: String,
        error: String,
    },

    /// Resource subscription started.
    ResourceSubscribed {
        server_id: String,
        server_name: String,
        uri: String,
    },

    /// Resource unsubscription completed.
    ResourceUnsubscribed {
        server_id: String,
        server_name: String,
        uri: String,
    },

    // Prompt events
    /// Prompt list retrieval started.
    PromptListStarted {
        server_id: String,
        server_name: String,
    },

    /// Prompt list retrieval completed.
    PromptListCompleted {
        server_id: String,
        server_name: String,
        prompt_count: usize,
        duration_ms: u64,
    },

    /// Prompt list retrieval failed.
    PromptListFailed {
        server_id: String,
        server_name: String,
        error: String,
    },

    /// Prompt get started.
    PromptGetStarted {
        server_id: String,
        server_name: String,
        prompt_name: String,
    },

    /// Prompt get completed.
    PromptGetCompleted {
        server_id: String,
        server_name: String,
        prompt_name: String,
        duration_ms: u64,
    },

    /// Prompt get failed.
    PromptGetFailed {
        server_id: String,
        server_name: String,
        prompt_name: String,
        error: String,
    },
}

impl McpEvent {
    /// Get the event category for routing.
    pub fn category(&self) -> &'static str {
        match self {
            McpEvent::ServerAdded { .. }
            | McpEvent::ServerUpdated { .. }
            | McpEvent::ServerRemoved { .. }
            | McpEvent::ServerHealthChanged { .. }
            | McpEvent::ServerError { .. } => "mcp.server",

            McpEvent::ToolDiscoveryStarted { .. }
            | McpEvent::ToolDiscoveryCompleted { .. }
            | McpEvent::ToolDiscoveryFailed { .. }
            | McpEvent::ToolsCacheRefreshed { .. } => "mcp.discovery",

            McpEvent::ToolCallStarted { .. }
            | McpEvent::ToolCallCompleted { .. }
            | McpEvent::ToolCallFailed { .. }
            | McpEvent::ToolApprovalRequired { .. }
            | McpEvent::ToolApprovalGranted { .. }
            | McpEvent::ToolApprovalDenied { .. } => "mcp.tool",

            McpEvent::ResourceListStarted { .. }
            | McpEvent::ResourceListCompleted { .. }
            | McpEvent::ResourceListFailed { .. }
            | McpEvent::ResourceReadStarted { .. }
            | McpEvent::ResourceReadCompleted { .. }
            | McpEvent::ResourceReadFailed { .. }
            | McpEvent::ResourceSubscribed { .. }
            | McpEvent::ResourceUnsubscribed { .. } => "mcp.resource",

            McpEvent::PromptListStarted { .. }
            | McpEvent::PromptListCompleted { .. }
            | McpEvent::PromptListFailed { .. }
            | McpEvent::PromptGetStarted { .. }
            | McpEvent::PromptGetCompleted { .. }
            | McpEvent::PromptGetFailed { .. } => "mcp.prompt",
        }
    }

    /// Get the event type name.
    pub fn event_type(&self) -> String {
        match self {
            McpEvent::ServerAdded { .. } => "server_added",
            McpEvent::ServerUpdated { .. } => "server_updated",
            McpEvent::ServerRemoved { .. } => "server_removed",
            McpEvent::ServerHealthChanged { .. } => "server_health_changed",
            McpEvent::ServerError { .. } => "server_error",
            McpEvent::ToolDiscoveryStarted { .. } => "tool_discovery_started",
            McpEvent::ToolDiscoveryCompleted { .. } => "tool_discovery_completed",
            McpEvent::ToolDiscoveryFailed { .. } => "tool_discovery_failed",
            McpEvent::ToolsCacheRefreshed { .. } => "tools_cache_refreshed",
            McpEvent::ToolCallStarted { .. } => "tool_call_started",
            McpEvent::ToolCallCompleted { .. } => "tool_call_completed",
            McpEvent::ToolCallFailed { .. } => "tool_call_failed",
            McpEvent::ToolApprovalRequired { .. } => "tool_approval_required",
            McpEvent::ToolApprovalGranted { .. } => "tool_approval_granted",
            McpEvent::ToolApprovalDenied { .. } => "tool_approval_denied",

            // Resource events
            McpEvent::ResourceListStarted { .. } => "resource_list_started",
            McpEvent::ResourceListCompleted { .. } => "resource_list_completed",
            McpEvent::ResourceListFailed { .. } => "resource_list_failed",
            McpEvent::ResourceReadStarted { .. } => "resource_read_started",
            McpEvent::ResourceReadCompleted { .. } => "resource_read_completed",
            McpEvent::ResourceReadFailed { .. } => "resource_read_failed",
            McpEvent::ResourceSubscribed { .. } => "resource_subscribed",
            McpEvent::ResourceUnsubscribed { .. } => "resource_unsubscribed",

            // Prompt events
            McpEvent::PromptListStarted { .. } => "prompt_list_started",
            McpEvent::PromptListCompleted { .. } => "prompt_list_completed",
            McpEvent::PromptListFailed { .. } => "prompt_list_failed",
            McpEvent::PromptGetStarted { .. } => "prompt_get_started",
            McpEvent::PromptGetCompleted { .. } => "prompt_get_completed",
            McpEvent::PromptGetFailed { .. } => "prompt_get_failed",
        }
        .to_string()
    }

    /// Convert to a JSON payload for the event bus.
    pub fn to_payload(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}))
    }
}

/// Converts MCP events to the application's event bus format.
pub struct McpEventAdapter {
    /// Callback to emit events to the application's event bus.
    emitter: Arc<dyn Fn(String, String, serde_json::Value) + Send + Sync>,
}

impl std::fmt::Debug for McpEventAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpEventAdapter")
            .field("emitter", &"<function>")
            .finish()
    }
}

impl Clone for McpEventAdapter {
    fn clone(&self) -> Self {
        Self {
            emitter: self.emitter.clone(),
        }
    }
}

use std::sync::Arc;

impl McpEventAdapter {
    /// Create a new event adapter with the given emitter callback.
    pub fn new<F>(emitter: F) -> Self
    where
        F: Fn(String, String, serde_json::Value) + Send + Sync + 'static,
    {
        Self {
            emitter: Arc::new(emitter),
        }
    }

    /// Emit an MCP event to the application event bus.
    pub fn emit(&self, event: McpEvent) {
        let category = event.category().to_string();
        let event_type = event.event_type();
        let payload = event.to_payload();

        (self.emitter)(category, event_type, payload);
    }
}

/// Statistics for MCP operations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpStatistics {
    /// Total number of tool calls made.
    pub total_tool_calls: u64,
    /// Number of successful tool calls.
    pub successful_tool_calls: u64,
    /// Number of failed tool calls.
    pub failed_tool_calls: u64,
    /// Total tool call duration in milliseconds.
    pub total_duration_ms: u64,
    /// Average response time in milliseconds.
    pub avg_response_time_ms: f64,
    /// Number of servers configured.
    pub server_count: usize,
    /// Number of servers currently healthy.
    pub healthy_server_count: usize,
    /// Total number of tools available.
    pub total_tools: usize,
}

impl McpStatistics {
    /// Update statistics with a completed tool call.
    pub fn record_tool_call(&mut self, success: bool, duration_ms: u64) {
        self.total_tool_calls += 1;
        if success {
            self.successful_tool_calls += 1;
        } else {
            self.failed_tool_calls += 1;
        }
        self.total_duration_ms += duration_ms;
        self.avg_response_time_ms = self.total_duration_ms as f64 / self.total_tool_calls as f64;
    }

    /// Get the success rate as a percentage.
    pub fn success_rate(&self) -> f64 {
        if self.total_tool_calls == 0 {
            return 100.0;
        }
        (self.successful_tool_calls as f64 / self.total_tool_calls as f64) * 100.0
    }
}
