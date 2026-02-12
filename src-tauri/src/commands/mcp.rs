//! MCP Tauri commands.
//!
//! These commands provide comprehensive MCP server management including:
//! - Local (stdio) and remote (HTTP/SSE) server support
//! - Authentication (OAuth, API keys, custom headers)
//! - Tool filtering and approval policies
//! - Health monitoring and connection management

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::mcp::{
    FilterMode, GlobalApprovalPolicy, McpAuthConfig, McpServerConfig, McpTransportType,
    ServerHealth, ToolApprovalPolicy, ToolFilter, migrate_legacy_config,
};
use crate::AppError;

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Input for creating/updating an MCP server.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateMcpServerInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(default = "default_stdio")]
    pub transport: McpTransportType,
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    // Stdio fields
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub working_dir: Option<String>,
    
    // HTTP/SSE fields
    pub url: Option<String>,
    pub auth: Option<McpAuthInput>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
    
    // Filtering and approval
    pub tool_filter: Option<ToolFilterInput>,
    pub approval_policy: Option<ToolApprovalPolicyInput>,
}

fn default_stdio() -> McpTransportType { McpTransportType::Stdio }
fn default_true() -> bool { true }
fn default_timeout() -> u64 { 30 }
fn default_pool_size() -> usize { 5 }

/// Authentication input.
#[derive(Debug, Clone, Deserialize)]
pub struct McpAuthInput {
    pub oauth_token: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub api_key: Option<String>,
    pub api_key_header: Option<String>,
}

/// Tool filter input.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolFilterInput {
    pub mode: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub allow_all_read_only: bool,
    #[serde(default)]
    pub block_all_modifying: bool,
}

/// Tool approval policy input.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolApprovalPolicyInput {
    pub global_policy: String,
    #[serde(default)]
    pub tool_overrides: Vec<ToolOverrideInput>,
    #[serde(default = "default_true")]
    pub read_only_never_requires_approval: bool,
    #[serde(default)]
    pub modifying_always_requires_approval: bool,
}

/// Tool override input.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolOverrideInput {
    pub pattern: String,
    pub requires_approval: bool,
    #[serde(default)]
    pub is_glob: bool,
}

// ---------------------------------------------------------------------------
// View types
// ---------------------------------------------------------------------------

/// Server configuration view for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct McpServerView {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
    pub timeout_secs: u64,
    pub pool_size: usize,
    pub tool_count: usize,
    pub health: Option<McpServerHealthView>,
}

/// Server health view.
#[derive(Debug, Clone, Serialize)]
pub struct McpServerHealthView {
    pub status: String,
    pub last_check: Option<String>,
    pub connected_at: Option<String>,
    pub error_count: u64,
}

/// Tool view.
#[derive(Debug, Clone, Serialize)]
pub struct McpToolView {
    pub server_id: String,
    pub server_name: String,
    pub tool_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub read_only_hint: Option<bool>,
    pub requires_approval: bool,
}

/// Tool call result view.
#[derive(Debug, Clone, Serialize)]
pub struct McpToolCallResult {
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// List all configured MCP servers.
#[tauri::command]
pub async fn list_mcp_servers(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<McpServerView>, AppError> {
    let manager = &state.mcp_manager;
    let servers = manager.list_servers().await;
    let tools = manager.load_tools_cache().await;
    
    let mut views = Vec::new();
    for server in servers {
        let tool_count = tools.iter()
            .filter(|t| t.server_id == server.id)
            .count();
        
        let health = manager.get_server_runtime_info(&server.id).await
            .map(|info| McpServerHealthView {
                status: format!("{:?}", info.health).to_lowercase(),
                last_check: info.last_health_check,
                connected_at: info.connected_at,
                error_count: info.error_count,
            });
        
        views.push(McpServerView {
            id: server.id,
            name: server.name,
            transport: server.transport.to_string(),
            enabled: server.enabled,
            command: server.command,
            args: server.args,
            url: server.url,
            timeout_secs: server.timeout_secs,
            pool_size: server.pool_size,
            tool_count,
            health,
        });
    }
    
    Ok(views)
}

/// Get a specific MCP server configuration.
#[tauri::command]
pub async fn get_mcp_server(
    state: tauri::State<'_, crate::AppState>,
    server_id: String,
) -> Result<Option<McpServerView>, AppError> {
    let manager = &state.mcp_manager;
    
    let server = match manager.get_server(&server_id).await {
        Some(s) => s,
        None => return Ok(None),
    };
    
    let tools = manager.load_tools_cache().await;
    let tool_count = tools.iter()
        .filter(|t| t.server_id == server.id)
        .count();
    
    let health = manager.get_server_runtime_info(&server.id).await
        .map(|info| McpServerHealthView {
            status: format!("{:?}", info.health).to_lowercase(),
            last_check: info.last_health_check,
            connected_at: info.connected_at,
            error_count: info.error_count,
        });
    
    Ok(Some(McpServerView {
        id: server.id,
        name: server.name,
        transport: server.transport.to_string(),
        enabled: server.enabled,
        command: server.command,
        args: server.args,
        url: server.url,
        timeout_secs: server.timeout_secs,
        pool_size: server.pool_size,
        tool_count,
        health,
    }))
}

/// Create or update an MCP server configuration.
#[tauri::command]
pub async fn upsert_mcp_server(
    state: tauri::State<'_, crate::AppState>,
    input: CreateMcpServerInput,
) -> Result<McpServerView, AppError> {
    let manager = &state.mcp_manager;
    
    // Validate input
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::Other("name is required".to_string()));
    }
    
    // Generate or use provided ID
    let id = input.id
        .as_deref()
        .map(sanitize_id)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| sanitize_id(name));
    
    if id.is_empty() {
        return Err(AppError::Other("could not derive valid id".to_string()));
    }
    
    // Convert auth input
    let auth = input.auth.map(|a| McpAuthConfig {
        oauth_token: a.oauth_token,
        headers: a.headers,
        api_key: a.api_key,
        api_key_header: a.api_key_header,
    }).unwrap_or_default();
    
    // Convert tool filter input
    let tool_filter = input.tool_filter.map(|f| ToolFilter {
        mode: match f.mode.as_str() {
            "include" => FilterMode::Include,
            "exclude" => FilterMode::Exclude,
            _ => FilterMode::Include,
        },
        tools: f.tools,
        allow_all_read_only: f.allow_all_read_only,
        block_all_modifying: f.block_all_modifying,
    }).unwrap_or_default();
    
    // Convert approval policy input
    let approval_policy = input.approval_policy.map(|p| ToolApprovalPolicy {
        global_policy: match p.global_policy.as_str() {
            "always" => GlobalApprovalPolicy::Always,
            "never" => GlobalApprovalPolicy::Never,
            _ => GlobalApprovalPolicy::ByTool,
        },
        tool_overrides: p.tool_overrides.into_iter().map(|o| {
            crate::mcp::filtering::ToolOverride {
                pattern: o.pattern,
                requires_approval: o.requires_approval,
                is_glob: o.is_glob,
            }
        }).collect(),
        read_only_never_requires_approval: p.read_only_never_requires_approval,
        modifying_always_requires_approval: p.modifying_always_requires_approval,
    }).unwrap_or_default();
    
    // Capture values before they are moved
    let transport_type = input.transport.clone();
    let enabled = input.enabled;
    let command = input.command.clone();
    let args = input.args.clone();
    let url = input.url.clone();
    let timeout_secs = input.timeout_secs;
    let pool_size = input.pool_size;
    
    let server = McpServerConfig {
        id: id.clone(),
        name: name.to_string(),
        transport: transport_type,
        enabled,
        command,
        args,
        env: input.env,
        working_dir: input.working_dir,
        url,
        auth,
        timeout_secs,
        pool_size,
        health_check_interval_secs: 60,
        tool_filter,
        approval_policy,
    };
    
    server.validate()
        .map_err(|e| AppError::Other(format!("validation failed: {}", e)))?;
    
    manager.upsert_server(server).await
        .map_err(|e| AppError::Other(format!("failed to save server: {}", e)))?;
    
    // Refresh tools cache in background
    let manager_clone = manager.clone();
    tokio::spawn(async move {
        let _ = manager_clone.refresh_tools_cache().await;
    });
    
    // Return the created server
    let tools = manager.load_tools_cache().await;
    let tool_count = tools.iter()
        .filter(|t| t.server_id == id)
        .count();
    
    Ok(McpServerView {
        id,
        name: name.to_string(),
        transport: match transport_type {
            McpTransportType::Stdio => "stdio".to_string(),
            McpTransportType::Http => "http".to_string(),
            McpTransportType::Sse => "sse".to_string(),
        },
        enabled,
        command: input.command.clone(),
        args: input.args.clone(),
        url: input.url.clone(),
        timeout_secs,
        pool_size,
        tool_count,
        health: None,
    })
}

/// Remove an MCP server configuration.
#[tauri::command]
pub async fn remove_mcp_server(
    state: tauri::State<'_, crate::AppState>,
    server_id: String,
) -> Result<bool, AppError> {
    let manager = &state.mcp_manager;
    
    manager.remove_server(&server_id).await
        .map_err(|e| AppError::Other(format!("failed to remove server: {}", e)))
}

/// Refresh the MCP tools cache from all enabled servers.
#[tauri::command]
pub async fn refresh_mcp_tools_cache(
    state: tauri::State<'_, crate::AppState>,
) -> Result<McpToolsCacheView, AppError> {
    let manager = &state.mcp_manager;
    
    let cache = manager.refresh_tools_cache().await
        .map_err(|e| AppError::Other(format!("failed to refresh tools: {}", e)))?;
    
    Ok(McpToolsCacheView {
        total_tools: cache.tools.len(),
        server_count: cache.server_count,
        updated_at: cache.updated_at,
    })
}

/// Tools cache view.
#[derive(Debug, Clone, Serialize)]
pub struct McpToolsCacheView {
    pub total_tools: usize,
    pub server_count: usize,
    pub updated_at: String,
}

/// List cached MCP tools.
#[tauri::command]
pub async fn list_mcp_tools(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<McpToolView>, AppError> {
    let manager = &state.mcp_manager;
    let tools = manager.load_tools_cache().await;
    
    let views = tools.into_iter().map(|t| McpToolView {
        server_id: t.server_id,
        server_name: t.server_name,
        tool_name: t.tool_name,
        description: t.description,
        input_schema: t.input_schema,
        read_only_hint: t.read_only_hint,
        requires_approval: t.requires_approval,
    }).collect();
    
    Ok(views)
}

/// Call an MCP tool.
#[tauri::command]
pub async fn call_mcp_tool(
    state: tauri::State<'_, crate::AppState>,
    server_id: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> Result<McpToolCallResult, AppError> {
    let manager = &state.mcp_manager;
    let start = std::time::Instant::now();
    
    match manager.call_tool(&server_id, &tool_name, arguments).await {
        Ok(result) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            Ok(McpToolCallResult {
                success: true,
                result: Some(result),
                error: None,
                duration_ms,
            })
        }
        Err(e) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            Ok(McpToolCallResult {
                success: false,
                result: None,
                error: Some(e),
                duration_ms,
            })
        }
    }
}

/// Test connection to an MCP server.
#[tauri::command]
pub async fn test_mcp_server_connection(
    state: tauri::State<'_, crate::AppState>,
    server_id: String,
) -> Result<McpConnectionTestResult, AppError> {
    let manager = &state.mcp_manager;
    
    let server = manager.get_server(&server_id).await
        .ok_or_else(|| AppError::Other(format!("server not found: {}", server_id)))?;
    
    if !server.enabled {
        return Ok(McpConnectionTestResult {
            success: false,
            error: Some("server is disabled".to_string()),
            latency_ms: None,
            tool_count: None,
        });
    }
    
    let start = std::time::Instant::now();
    
    // Try to refresh tools which will test the connection
    match manager.refresh_tools_cache().await {
        Ok(cache) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            let tool_count = cache.tools.iter()
                .filter(|t| t.server_id == server_id)
                .count();
            
            Ok(McpConnectionTestResult {
                success: true,
                error: None,
                latency_ms: Some(latency_ms),
                tool_count: Some(tool_count),
            })
        }
        Err(e) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            Ok(McpConnectionTestResult {
                success: false,
                error: Some(e),
                latency_ms: Some(latency_ms),
                tool_count: None,
            })
        }
    }
}

/// Connection test result.
#[derive(Debug, Clone, Serialize)]
pub struct McpConnectionTestResult {
    pub success: bool,
    pub error: Option<String>,
    pub latency_ms: Option<u64>,
    pub tool_count: Option<usize>,
}

/// Get MCP statistics.
#[tauri::command]
pub async fn get_mcp_statistics(
    state: tauri::State<'_, crate::AppState>,
) -> Result<McpStatisticsView, AppError> {
    let manager = &state.mcp_manager;
    let servers = manager.list_servers().await;
    let tools = manager.load_tools_cache().await;
    let runtime_info = manager.get_runtime_info().await;
    
    let healthy_count = runtime_info.iter()
        .filter(|info| matches!(info.health, ServerHealth::Healthy))
        .count();
    
    // TODO: Track actual statistics in the manager
    Ok(McpStatisticsView {
        server_count: servers.len(),
        healthy_server_count: healthy_count,
        total_tools: tools.len(),
    })
}

/// Statistics view.
#[derive(Debug, Clone, Serialize)]
pub struct McpStatisticsView {
    pub server_count: usize,
    pub healthy_server_count: usize,
    pub total_tools: usize,
}

/// Migrate legacy MCP configuration.
#[tauri::command]
pub async fn migrate_mcp_config() -> Result<bool, AppError> {
    match migrate_legacy_config().await {
        Ok(()) => Ok(true),
        Err(e) => Err(AppError::Other(format!("migration failed: {}", e))),
    }
}

// Legacy commands for backward compatibility
// These are deprecated and will be removed in a future version

#[derive(Debug, Clone, Deserialize)]
pub struct LegacyMcpServerInput {
    pub id: Option<String>,
    pub name: String,
    pub command: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub enabled: Option<bool>,
}

#[tauri::command]
pub fn list_mcp_server_configs() -> Vec<crate::core::mcp::McpServerConfig> {
    crate::core::mcp::list_mcp_servers()
}

#[tauri::command]
pub fn upsert_mcp_server_config(input: LegacyMcpServerInput) -> Result<crate::core::mcp::McpServerConfig, AppError> {
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

    let server = crate::core::mcp::McpServerConfig {
        id,
        name: name.to_string(),
        command: command.to_string(),
        args: input.args.unwrap_or_default(),
        env: input.env.unwrap_or_default(),
        enabled: input.enabled.unwrap_or(true),
    };

    crate::core::mcp::upsert_mcp_server(server.clone()).map_err(AppError::Other)?;
    let _ = crate::core::mcp::refresh_mcp_tools_cache();
    Ok(server)
}

#[tauri::command]
pub fn remove_mcp_server_config(server_id: String) -> Result<(), AppError> {
    crate::core::mcp::remove_mcp_server(server_id.trim()).map_err(AppError::Other)?;
    Ok(())
}

#[tauri::command]
pub fn refresh_mcp_tools() -> Result<Vec<crate::core::mcp::McpToolEntry>, AppError> {
    crate::core::mcp::refresh_mcp_tools_cache().map_err(AppError::Other)
}

#[tauri::command]
pub fn list_cached_mcp_tools() -> Vec<crate::core::mcp::McpToolEntry> {
    crate::core::mcp::load_mcp_tools_cache()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
