use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolEntry {
    pub server_id: String,
    pub server_name: String,
    pub tool_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolsCache {
    pub tools: Vec<McpToolEntry>,
    pub updated_at: String,
}

pub fn list_mcp_servers() -> Vec<McpServerConfig> {
    let path = mcp_servers_path();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<McpServerConfig>>(&raw).unwrap_or_default()
}

pub fn save_mcp_servers(servers: &[McpServerConfig]) -> Result<(), String> {
    let path = mcp_servers_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed creating mcp dir: {e}"))?;
    }
    let body = serde_json::to_string_pretty(servers)
        .map_err(|e| format!("failed serializing mcp servers: {e}"))?;
    std::fs::write(path, body).map_err(|e| format!("failed writing mcp servers: {e}"))
}

pub fn upsert_mcp_server(server: McpServerConfig) -> Result<(), String> {
    let mut servers = list_mcp_servers();
    servers.retain(|s| s.id != server.id);
    servers.push(server);
    servers.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    save_mcp_servers(&servers)
}

pub fn remove_mcp_server(server_id: &str) -> Result<bool, String> {
    let mut servers = list_mcp_servers();
    let before = servers.len();
    servers.retain(|s| s.id != server_id);
    if before == servers.len() {
        return Ok(false);
    }
    save_mcp_servers(&servers)?;
    refresh_mcp_tools_cache().ok();
    Ok(true)
}

pub fn load_mcp_tools_cache() -> Vec<McpToolEntry> {
    let path = mcp_tools_cache_path();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<McpToolsCache>(&raw) else {
        return Vec::new();
    };
    parsed.tools
}

pub fn refresh_mcp_tools_cache() -> Result<Vec<McpToolEntry>, String> {
    let servers = list_mcp_servers();
    let mut tools = Vec::new();

    for server in servers.into_iter().filter(|s| s.enabled) {
        if let Ok(server_tools) = list_server_tools(&server) {
            tools.extend(server_tools.into_iter().map(|t| McpToolEntry {
                server_id: server.id.clone(),
                server_name: server.name.clone(),
                tool_name: t.name,
                description: t.description,
                input_schema: t.input_schema,
            }));
        }
    }

    let cache = McpToolsCache {
        tools: tools.clone(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    let path = mcp_tools_cache_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed creating mcp dir: {e}"))?;
    }
    let body = serde_json::to_string_pretty(&cache)
        .map_err(|e| format!("failed serializing mcp tools cache: {e}"))?;
    std::fs::write(path, body).map_err(|e| format!("failed writing mcp tools cache: {e}"))?;

    Ok(tools)
}

pub fn call_mcp_tool_by_server_and_name(
    server_id: &str,
    tool_name: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let server = list_mcp_servers()
        .into_iter()
        .find(|s| s.id == server_id && s.enabled)
        .ok_or_else(|| format!("mcp server not found or disabled: {server_id}"))?;
    call_server_tool(&server, tool_name, arguments)
}

#[derive(Debug, Clone, Deserialize)]
struct McpToolDescriptor {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    input_schema: serde_json::Value,
}

fn list_server_tools(server: &McpServerConfig) -> Result<Vec<McpToolDescriptor>, String> {
    let mut session = McpSession::start(server)?;
    session.initialize()?;
    let resp = session.request("tools/list", serde_json::json!({}))?;
    let tools = resp
        .get("tools")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "invalid tools/list response".to_string())?;
    let mut out = Vec::new();
    for tool in tools {
        if let Ok(parsed) = serde_json::from_value::<McpToolDescriptor>(tool.clone()) {
            out.push(parsed);
        }
    }
    Ok(out)
}

fn call_server_tool(
    server: &McpServerConfig,
    tool_name: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut session = McpSession::start(server)?;
    session.initialize()?;
    let resp = session.request(
        "tools/call",
        serde_json::json!({
            "name": tool_name,
            "arguments": arguments,
        }),
    )?;
    Ok(resp)
}

struct McpSession {
    child: std::process::Child,
    reader: BufReader<std::process::ChildStdout>,
    writer: std::process::ChildStdin,
    next_id: i64,
}

impl McpSession {
    fn start(server: &McpServerConfig) -> Result<Self, String> {
        let mut cmd = Command::new(&server.command);
        cmd.args(&server.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (k, v) in &server.env {
            cmd.env(k, v);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("failed spawning MCP server {}: {e}", server.name))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "missing stdout pipe".to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "missing stdin pipe".to_string())?;

        Ok(Self {
            child,
            reader: BufReader::new(stdout),
            writer: stdin,
            next_id: 1,
        })
    }

    fn initialize(&mut self) -> Result<(), String> {
        let _ = self.request(
            "initialize",
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "orchestrix", "version": "0.1.0"}
            }),
        )?;

        self.notify("notifications/initialized", serde_json::json!({}))?;
        Ok(())
    }

    fn request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = self.next_id;
        self.next_id += 1;

        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_message(&payload)?;

        loop {
            let msg = self.read_message()?;
            if msg.get("id").and_then(|v| v.as_i64()) != Some(id) {
                continue;
            }
            if let Some(err) = msg.get("error") {
                return Err(format!("MCP error calling {method}: {err}"));
            }
            return Ok(msg
                .get("result")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})));
        }
    }

    fn notify(&mut self, method: &str, params: serde_json::Value) -> Result<(), String> {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&payload)
    }

    fn write_message(&mut self, payload: &serde_json::Value) -> Result<(), String> {
        let body = serde_json::to_vec(payload).map_err(|e| format!("json encode failed: {e}"))?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        self.writer
            .write_all(header.as_bytes())
            .map_err(|e| format!("write header failed: {e}"))?;
        self.writer
            .write_all(&body)
            .map_err(|e| format!("write body failed: {e}"))?;
        self.writer
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    fn read_message(&mut self) -> Result<serde_json::Value, String> {
        let mut content_length: usize = 0;

        loop {
            let mut line = String::new();
            self.reader
                .read_line(&mut line)
                .map_err(|e| format!("read header failed: {e}"))?;
            if line.is_empty() {
                return Err("unexpected EOF while reading MCP headers".to_string());
            }

            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }

            if let Some((key, value)) = trimmed.split_once(':') {
                if key.eq_ignore_ascii_case("Content-Length") {
                    content_length = value
                        .trim()
                        .parse::<usize>()
                        .map_err(|e| format!("invalid content-length: {e}"))?;
                }
            }
        }

        if content_length == 0 {
            return Err("missing content-length in MCP response".to_string());
        }

        let mut buf = vec![0u8; content_length];
        self.reader
            .read_exact(&mut buf)
            .map_err(|e| format!("read body failed: {e}"))?;

        serde_json::from_slice::<serde_json::Value>(&buf)
            .map_err(|e| format!("invalid MCP JSON response: {e}"))
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn mcp_servers_path() -> PathBuf {
    data_dir().join("mcp-servers-v1.json")
}

fn mcp_tools_cache_path() -> PathBuf {
    data_dir().join("mcp-tools-cache-v1.json")
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
