use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::core::mcp::{
    call_mcp_tool_by_server_and_name, list_mcp_servers, load_mcp_tools_cache,
    refresh_mcp_tools_cache, remove_mcp_server, upsert_mcp_server, McpServerConfig, McpToolEntry,
};
use crate::AppError;

#[derive(Debug, Clone, Deserialize)]
pub struct McpServerInput {
    pub id: Option<String>,
    pub name: String,
    pub command: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpToolCallView {
    pub result: serde_json::Value,
}

#[tauri::command]
pub fn list_mcp_server_configs() -> Vec<McpServerConfig> {
    list_mcp_servers()
}

#[tauri::command]
pub fn upsert_mcp_server_config(input: McpServerInput) -> Result<McpServerConfig, AppError> {
    let name = input.name.trim();
    let command = input.command.trim();
    if name.is_empty() {
        return Err(AppError::Other("name is required".to_string()));
    }
    if command.is_empty() {
        return Err(AppError::Other("command is required".to_string()));
    }

    let id = input
        .id
        .as_deref()
        .map(sanitize_id)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| sanitize_id(name));

    if id.is_empty() {
        return Err(AppError::Other("could not derive valid id".to_string()));
    }

    let server = McpServerConfig {
        id,
        name: name.to_string(),
        command: command.to_string(),
        args: input.args.unwrap_or_default(),
        env: input.env.unwrap_or_default(),
        enabled: input.enabled.unwrap_or(true),
    };

    upsert_mcp_server(server.clone()).map_err(AppError::Other)?;
    let _ = refresh_mcp_tools_cache();
    Ok(server)
}

#[tauri::command]
pub fn remove_mcp_server_config(server_id: String) -> Result<(), AppError> {
    remove_mcp_server(server_id.trim()).map_err(AppError::Other)?;
    Ok(())
}

#[tauri::command]
pub fn refresh_mcp_tools() -> Result<Vec<McpToolEntry>, AppError> {
    refresh_mcp_tools_cache().map_err(AppError::Other)
}

#[tauri::command]
pub fn list_cached_mcp_tools() -> Vec<McpToolEntry> {
    load_mcp_tools_cache()
}

#[tauri::command]
pub fn call_mcp_tool(
    server_id: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> Result<McpToolCallView, AppError> {
    let result = call_mcp_tool_by_server_and_name(server_id.trim(), tool_name.trim(), arguments)
        .map_err(AppError::Other)?;
    Ok(McpToolCallView { result })
}

fn sanitize_id(raw: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in raw.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
