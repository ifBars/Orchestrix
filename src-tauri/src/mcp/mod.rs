//! Enhanced MCP (Model Context Protocol) client implementation.
//!
//! This module provides comprehensive MCP client support including:
//! - Multiple transport types: stdio (local), HTTP (remote), SSE (streaming remote)
//! - Connection pooling and health monitoring
//! - Authentication support (OAuth tokens, headers)
//! - Tool filtering and approval workflows
//! - Event emission for transparency

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

pub mod transport;
pub mod connection;
pub mod filtering;
pub mod events;

use transport::{McpTransport, TransportConfig};
use connection::ConnectionManager;
pub use filtering::{FilterMode, GlobalApprovalPolicy, ToolApprovalPolicy, ToolFilter};

/// Type of MCP server transport.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportType {
    /// Local process via stdio (traditional MCP).
    #[default]
    Stdio,
    /// Remote HTTP endpoint.
    Http,
    /// Remote Server-Sent Events endpoint.
    Sse,
}

impl fmt::Display for McpTransportType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpTransportType::Stdio => write!(f, "stdio"),
            McpTransportType::Http => write!(f, "http"),
            McpTransportType::Sse => write!(f, "sse"),
        }
    }
}

/// Authentication configuration for remote MCP servers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpAuthConfig {
    /// OAuth access token for authentication.
    pub oauth_token: Option<String>,
    /// Custom HTTP headers to include in requests.
    pub headers: HashMap<String, String>,
    /// API key (for simple key-based auth).
    pub api_key: Option<String>,
    /// API key header name (default: "X-API-Key").
    pub api_key_header: Option<String>,
}

/// Configuration for a single MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Unique identifier for the server.
    pub id: String,
    /// Display name for the server.
    pub name: String,
    /// Transport type for this server.
    #[serde(default)]
    pub transport: McpTransportType,
    /// Whether the server is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    // Stdio-specific fields
    /// Command to execute (for stdio transport).
    pub command: Option<String>,
    /// Arguments for the command (for stdio transport).
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables (for stdio transport).
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Working directory for the command (for stdio transport).
    pub working_dir: Option<String>,
    
    // HTTP/SSE-specific fields
    /// URL endpoint for HTTP/SSE transport.
    pub url: Option<String>,
    /// Authentication configuration for remote servers.
    #[serde(default)]
    pub auth: McpAuthConfig,
    /// Request timeout in seconds (default: 30).
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Connection pool size (default: 5).
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
    /// Health check interval in seconds (default: 60).
    #[serde(default = "default_health_interval")]
    pub health_check_interval_secs: u64,
    
    // Tool filtering
    /// Tool filter configuration.
    #[serde(default)]
    pub tool_filter: ToolFilter,
    /// Tool approval policy.
    #[serde(default)]
    pub approval_policy: ToolApprovalPolicy,
}

fn default_true() -> bool { true }
fn default_timeout() -> u64 { 30 }
fn default_pool_size() -> usize { 5 }
fn default_health_interval() -> u64 { 60 }

impl McpServerConfig {
    /// Create a new stdio-based server configuration.
    #[allow(dead_code)]
    pub fn new_stdio(id: String, name: String, command: String, args: Vec<String>) -> Self {
        Self {
            id,
            name,
            transport: McpTransportType::Stdio,
            enabled: true,
            command: Some(command),
            args,
            env: HashMap::new(),
            working_dir: None,
            url: None,
            auth: McpAuthConfig::default(),
            timeout_secs: default_timeout(),
            pool_size: default_pool_size(),
            health_check_interval_secs: default_health_interval(),
            tool_filter: ToolFilter::default(),
            approval_policy: ToolApprovalPolicy::default(),
        }
    }
    
    /// Create a new HTTP-based server configuration.
    #[allow(dead_code)]
    pub fn new_http(id: String, name: String, url: String) -> Self {
        Self {
            id,
            name,
            transport: McpTransportType::Http,
            enabled: true,
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            working_dir: None,
            url: Some(url),
            auth: McpAuthConfig::default(),
            timeout_secs: default_timeout(),
            pool_size: default_pool_size(),
            health_check_interval_secs: default_health_interval(),
            tool_filter: ToolFilter::default(),
            approval_policy: ToolApprovalPolicy::default(),
        }
    }
    
    /// Create a new SSE-based server configuration.
    #[allow(dead_code)]
    pub fn new_sse(id: String, name: String, url: String) -> Self {
        Self {
            id,
            name,
            transport: McpTransportType::Sse,
            enabled: true,
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            working_dir: None,
            url: Some(url),
            auth: McpAuthConfig::default(),
            timeout_secs: default_timeout(),
            pool_size: 1, // SSE typically doesn't need pooling
            health_check_interval_secs: default_health_interval(),
            tool_filter: ToolFilter::default(),
            approval_policy: ToolApprovalPolicy::default(),
        }
    }
    
    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("Server ID cannot be empty".to_string());
        }
        if self.name.trim().is_empty() {
            return Err("Server name cannot be empty".to_string());
        }
        
        match self.transport {
            McpTransportType::Stdio => {
                if self.command.is_none() || self.command.as_ref().unwrap().trim().is_empty() {
                    return Err("Stdio transport requires a command".to_string());
                }
            }
            McpTransportType::Http | McpTransportType::Sse => {
                if self.url.is_none() || self.url.as_ref().unwrap().trim().is_empty() {
                    return Err(format!("{} transport requires a URL", self.transport));
                }
                let url = self.url.as_ref().unwrap();
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err(format!("Invalid URL scheme: {}", url));
                }
            }
        }
        
        Ok(())
    }
}

/// Health status of an MCP server connection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServerHealth {
    /// Server is healthy and responsive.
    Healthy,
    /// Server is currently connecting.
    Connecting,
    /// Server connection failed.
    Unhealthy { reason: String, since: String },
    /// Server is disabled.
    Disabled,
}

/// Runtime information about an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerRuntimeInfo {
    pub server_id: String,
    pub health: ServerHealth,
    pub last_health_check: Option<String>,
    pub connected_at: Option<String>,
    pub tool_count: usize,
    pub avg_response_time_ms: Option<u64>,
    pub error_count: u64,
}

/// Entry for a cached MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolEntry {
    pub server_id: String,
    pub server_name: String,
    pub tool_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    /// Whether the tool modifies data (read-only hint).
    #[serde(default)]
    pub read_only_hint: Option<bool>,
    /// Whether this tool requires approval based on policy.
    #[serde(default)]
    pub requires_approval: bool,
}

/// Cache of discovered MCP tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolsCache {
    pub tools: Vec<McpToolEntry>,
    pub updated_at: String,
    pub server_count: usize,
}

/// Global MCP client manager.
pub struct McpClientManager {
    connection_manager: Arc<ConnectionManager>,
    config: Arc<RwLock<HashMap<String, McpServerConfig>>>,
    runtime_info: Arc<RwLock<HashMap<String, McpServerRuntimeInfo>>>,
    event_emitter: Option<Box<dyn Fn(events::McpEvent) + Send + Sync>>,
}

impl McpClientManager {
    /// Create a new MCP client manager.
    pub async fn new() -> Result<Self, String> {
        let config = Self::load_config().await?;
        let config_arc = Arc::new(RwLock::new(config));
        let connection_manager = Arc::new(ConnectionManager::new());
        
        Ok(Self {
            connection_manager,
            config: config_arc,
            runtime_info: Arc::new(RwLock::new(HashMap::new())),
            event_emitter: None,
        })
    }
    
    /// Set an event emitter callback.
    pub fn set_event_emitter<F>(&mut self, emitter: F)
    where
        F: Fn(events::McpEvent) + Send + Sync + 'static,
    {
        self.event_emitter = Some(Box::new(emitter));
    }
    
    /// Emit an event if emitter is configured.
    fn emit_event(&self, event: events::McpEvent) {
        if let Some(emitter) = &self.event_emitter {
            emitter(event);
        }
    }
    
    /// Load server configurations from disk.
    async fn load_config() -> Result<HashMap<String, McpServerConfig>, String> {
        let path = mcp_servers_path();
        let Ok(raw) = tokio::fs::read_to_string(&path).await else {
            return Ok(HashMap::new());
        };
        
        let servers: Vec<McpServerConfig> = serde_json::from_str(&raw)
            .map_err(|e| format!("Failed to parse MCP config: {}", e))?;
        
        let mut map = HashMap::new();
        for server in servers {
            map.insert(server.id.clone(), server);
        }
        
        Ok(map)
    }
    
    /// Save server configurations to disk.
    async fn save_config(&self) -> Result<(), String> {
        let path = mcp_servers_path();
        let config = self.config.read().await;
        let servers: Vec<&McpServerConfig> = config.values().collect();
        
        let body = serde_json::to_string_pretty(&servers)
            .map_err(|e| format!("Failed to serialize MCP config: {}", e))?;
        
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| format!("Failed to create config dir: {}", e))?;
        }
        
        tokio::fs::write(&path, body).await
            .map_err(|e| format!("Failed to write MCP config: {}", e))?;
        
        Ok(())
    }
    
    /// List all configured servers.
    pub async fn list_servers(&self) -> Vec<McpServerConfig> {
        let config = self.config.read().await;
        let mut servers: Vec<McpServerConfig> = config.values().cloned().collect();
        servers.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
        servers
    }
    
    /// Get a specific server configuration.
    pub async fn get_server(&self, server_id: &str) -> Option<McpServerConfig> {
        self.config.read().await.get(server_id).cloned()
    }
    
    /// Upsert a server configuration.
    pub async fn upsert_server(&self, server: McpServerConfig) -> Result<(), String> {
        server.validate()?;
        
        let is_new = !self.config.read().await.contains_key(&server.id);
        
        {
            let mut config = self.config.write().await;
            config.insert(server.id.clone(), server.clone());
        }
        
        self.save_config().await?;
        
        self.emit_event(if is_new {
            events::McpEvent::ServerAdded { 
                server_id: server.id,
                server_name: server.name,
                transport: server.transport.to_string(),
            }
        } else {
            events::McpEvent::ServerUpdated { 
                server_id: server.id,
                server_name: server.name,
            }
        });
        
        Ok(())
    }
    
    /// Remove a server configuration.
    pub async fn remove_server(&self, server_id: &str) -> Result<bool, String> {
        let server = {
            let mut config = self.config.write().await;
            config.remove(server_id)
        };
        
        if server.is_none() {
            return Ok(false);
        }
        
        let server = server.unwrap();
        
        // Close any active connections
        self.connection_manager.close_connection(server_id).await;
        
        self.save_config().await?;
        
        self.emit_event(events::McpEvent::ServerRemoved { 
            server_id: server_id.to_string(),
            server_name: server.name,
        });
        
        Ok(true)
    }
    
    /// Refresh the tools cache from all enabled servers.
    pub async fn refresh_tools_cache(&self) -> Result<McpToolsCache, String> {
        let servers = self.list_servers().await;
        let mut all_tools = Vec::new();
        let mut server_count = 0;
        
        for server in servers.into_iter().filter(|s| s.enabled) {
            match self.discover_server_tools(&server).await {
                Ok(tools) => {
                    server_count += 1;
                    all_tools.extend(tools);
                }
                Err(e) => {
                    eprintln!("Failed to discover tools from server {}: {}", server.name, e);
                    self.emit_event(events::McpEvent::ServerError {
                        server_id: server.id.clone(),
                        server_name: server.name.clone(),
                        error: format!("Tool discovery failed: {}", e),
                    });
                }
            }
        }
        
        let cache = McpToolsCache {
            tools: all_tools.clone(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            server_count,
        };
        
        // Save cache to disk
        let cache_path = mcp_tools_cache_path();
        let body = serde_json::to_string_pretty(&cache)
            .map_err(|e| format!("Failed to serialize tools cache: {}", e))?;
        
        if let Some(parent) = cache_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| format!("Failed to create cache dir: {}", e))?;
        }
        
        tokio::fs::write(&cache_path, body).await
            .map_err(|e| format!("Failed to write tools cache: {}", e))?;
        
        self.emit_event(events::McpEvent::ToolsCacheRefreshed {
            total_tools: all_tools.len(),
            server_count,
        });
        
        Ok(cache)
    }
    
    /// Discover tools from a specific server.
    async fn discover_server_tools(&self, server: &McpServerConfig) -> Result<Vec<McpToolEntry>, String> {
        let start = Instant::now();
        
        self.emit_event(events::McpEvent::ToolDiscoveryStarted {
            server_id: server.id.clone(),
            server_name: server.name.clone(),
        });
        
        let transport = self.create_transport(server).await?;
        let response = transport.request("tools/list", serde_json::json!({})).await?;
        
        let tools_array = response
            .get("tools")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Invalid tools/list response: missing 'tools' array".to_string())?;
        
        let mut entries = Vec::new();
        for tool in tools_array {
            let name = tool
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            
            let description = tool
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            
            let input_schema = tool
                .get("inputSchema")
                .or_else(|| tool.get("input_schema"))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            
            let read_only_hint = tool
                .get("readOnlyHint")
                .or_else(|| tool.get("read_only_hint"))
                .and_then(|v| v.as_bool());
            
            // Check if tool requires approval based on policy
            let requires_approval = server.approval_policy.requires_approval(&name, read_only_hint);
            
            // Check if tool passes the filter
            if !server.tool_filter.allows(&name) {
                continue;
            }
            
            entries.push(McpToolEntry {
                server_id: server.id.clone(),
                server_name: server.name.clone(),
                tool_name: name,
                description,
                input_schema,
                read_only_hint,
                requires_approval,
            });
        }
        
        let duration = start.elapsed().as_millis() as u64;
        
        self.emit_event(events::McpEvent::ToolDiscoveryCompleted {
            server_id: server.id.clone(),
            server_name: server.name.clone(),
            tool_count: entries.len(),
            duration_ms: duration,
        });
        
        Ok(entries)
    }
    
    /// Load cached tools from disk.
    pub async fn load_tools_cache(&self) -> Vec<McpToolEntry> {
        let path = mcp_tools_cache_path();
        let Ok(raw) = tokio::fs::read_to_string(&path).await else {
            return Vec::new();
        };
        
        let Ok(parsed) = serde_json::from_str::<McpToolsCache>(&raw) else {
            return Vec::new();
        };
        
        parsed.tools
    }
    
    /// Create a transport for a server configuration.
    async fn create_transport(&self, server: &McpServerConfig) -> Result<Box<dyn McpTransport>, String> {
        let config = TransportConfig {
            timeout: Duration::from_secs(server.timeout_secs),
            auth: server.auth.clone(),
        };
        
        match server.transport {
            McpTransportType::Stdio => {
                let command = server.command.as_ref()
                    .ok_or_else(|| "Stdio transport requires a command".to_string())?;
                transport::StdioTransport::new(
                    command.clone(),
                    server.args.clone(),
                    server.env.clone(),
                    server.working_dir.clone(),
                    config,
                )
            }
            McpTransportType::Http => {
                let url = server.url.as_ref()
                    .ok_or_else(|| "HTTP transport requires a URL".to_string())?;
                transport::HttpTransport::new(url.clone(), config).await
            }
            McpTransportType::Sse => {
                let url = server.url.as_ref()
                    .ok_or_else(|| "SSE transport requires a URL".to_string())?;
                transport::SseTransport::new(url.clone(), config).await
            }
        }
    }
    
    /// Call a tool on a specific server.
    pub async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let server = self.get_server(server_id).await
            .ok_or_else(|| format!("Server not found: {}", server_id))?;
        
        if !server.enabled {
            return Err(format!("Server is disabled: {}", server_id));
        }
        
        // Check if tool requires approval
        let tools = self.load_tools_cache().await;
        let tool_entry = tools.iter()
            .find(|t| t.server_id == server_id && t.tool_name == tool_name);
        
        if let Some(entry) = tool_entry {
            if entry.requires_approval {
                self.emit_event(events::McpEvent::ToolApprovalRequired {
                    server_id: server_id.to_string(),
                    server_name: server.name.clone(),
                    tool_name: tool_name.to_string(),
                });
            }
        }
        
        self.emit_event(events::McpEvent::ToolCallStarted {
            server_id: server_id.to_string(),
            server_name: server.name.clone(),
            tool_name: tool_name.to_string(),
        });
        
        let start = Instant::now();
        
        let transport = self.create_transport(&server).await?;
        let response = transport.request(
            "tools/call",
            serde_json::json!({
                "name": tool_name,
                "arguments": arguments,
            }),
        ).await;
        
        let duration = start.elapsed().as_millis() as u64;
        
        match response {
            Ok(result) => {
                self.emit_event(events::McpEvent::ToolCallCompleted {
                    server_id: server_id.to_string(),
                    server_name: server.name.clone(),
                    tool_name: tool_name.to_string(),
                    duration_ms: duration,
                    success: true,
                });
                Ok(result)
            }
            Err(e) => {
                self.emit_event(events::McpEvent::ToolCallCompleted {
                    server_id: server_id.to_string(),
                    server_name: server.name.clone(),
                    tool_name: tool_name.to_string(),
                    duration_ms: duration,
                    success: false,
                });
                self.emit_event(events::McpEvent::ToolCallFailed {
                    server_id: server_id.to_string(),
                    server_name: server.name.clone(),
                    tool_name: tool_name.to_string(),
                    error: e.clone(),
                });
                Err(e)
            }
        }
    }
    
    /// Get runtime information for all servers.
    pub async fn get_runtime_info(&self) -> Vec<McpServerRuntimeInfo> {
        self.runtime_info.read().await.values().cloned().collect()
    }
    
    /// Get runtime information for a specific server.
    pub async fn get_server_runtime_info(&self, server_id: &str) -> Option<McpServerRuntimeInfo> {
        self.runtime_info.read().await.get(server_id).cloned()
    }
    
    /// Initialize and start health monitoring for all enabled servers.
    pub async fn initialize(&self) -> Result<(), String> {
        // Load initial runtime info
        let servers = self.list_servers().await;
        
        for server in servers {
            let info = McpServerRuntimeInfo {
                server_id: server.id.clone(),
                health: if server.enabled { 
                    ServerHealth::Connecting 
                } else { 
                    ServerHealth::Disabled 
                },
                last_health_check: None,
                connected_at: None,
                tool_count: 0,
                avg_response_time_ms: None,
                error_count: 0,
            };
            
            self.runtime_info.write().await.insert(server.id.clone(), info);
        }
        
        // Initial tool cache refresh
        self.refresh_tools_cache().await?;
        
        Ok(())
    }
}

fn mcp_servers_path() -> PathBuf {
    data_dir().join("mcp-servers-v2.json")
}

fn mcp_tools_cache_path() -> PathBuf {
    data_dir().join("mcp-tools-cache-v2.json")
}

fn data_dir() -> PathBuf {
    if let Ok(path) = std::env::var("ORCHESTRIX_DATA_DIR") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            return PathBuf::from(app_data).join("Orchestrix");
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".orchestrix");
    }

    if let Ok(home) = std::env::var("USERPROFILE") {
        return PathBuf::from(home).join(".orchestrix");
    }

    PathBuf::from(".orchestrix")
}

/// Legacy config migration function
pub async fn migrate_legacy_config() -> Result<(), String> {
    let legacy_path = data_dir().join("mcp-servers-v1.json");
    let new_path = data_dir().join("mcp-servers-v2.json");
    
    // Check if new config already exists
    if tokio::fs::try_exists(&new_path).await.unwrap_or(false) {
        return Ok(());
    }
    
    // Check if legacy config exists
    if !tokio::fs::try_exists(&legacy_path).await.unwrap_or(false) {
        return Ok(());
    }
    
    let raw = tokio::fs::read_to_string(&legacy_path).await
        .map_err(|e| format!("Failed to read legacy config: {}", e))?;
    
    #[derive(Debug, Clone, Deserialize)]
    struct LegacyConfig {
        id: String,
        name: String,
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        enabled: bool,
    }
    
    let legacy_servers: Vec<LegacyConfig> = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse legacy config: {}", e))?;
    
    let new_servers: Vec<McpServerConfig> = legacy_servers.into_iter()
        .map(|legacy| McpServerConfig {
            id: legacy.id,
            name: legacy.name,
            transport: McpTransportType::Stdio,
            enabled: legacy.enabled,
            command: Some(legacy.command),
            args: legacy.args,
            env: legacy.env,
            working_dir: None,
            url: None,
            auth: McpAuthConfig::default(),
            timeout_secs: default_timeout(),
            pool_size: default_pool_size(),
            health_check_interval_secs: default_health_interval(),
            tool_filter: ToolFilter::default(),
            approval_policy: ToolApprovalPolicy::default(),
        })
        .collect();
    
    let body = serde_json::to_string_pretty(&new_servers)
        .map_err(|e| format!("Failed to serialize migrated config: {}", e))?;
    
    tokio::fs::write(&new_path, body).await
        .map_err(|e| format!("Failed to write migrated config: {}", e))?;
    
    // Rename legacy file as backup
    let backup_path = data_dir().join("mcp-servers-v1.json.backup");
    let _ = tokio::fs::rename(&legacy_path, &backup_path).await;
    
    Ok(())
}
