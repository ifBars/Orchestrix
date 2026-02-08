use serde::{Deserialize, Serialize};

/// MCP-compatible tool descriptor.
/// Maps 1:1 with the MCP tool definition shape so future MCP servers
/// can register tools through the same interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
}
