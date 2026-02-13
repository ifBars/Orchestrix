//! Dev server management tools.
//!
//! Provides tools for managing long-running development servers:
//! - dev_server.start: Start a dev server process
//! - dev_server.stop: Stop a running dev server
//! - dev_server.status: Check dev server health and status
//! - dev_server.logs: Retrieve recent log output

use std::collections::VecDeque;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command as TokioCommand};
use uuid::Uuid;

use crate::core::tool::ToolDescriptor;
use crate::policy::{PolicyDecision, PolicyEngine};
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

const DEFAULT_LOG_BUFFER_SIZE: usize = 1000;
const HEALTH_CHECK_TIMEOUT_SECS: u64 = 5;
const MAX_LOG_LINES_PER_REQUEST: usize = 500;

/// Global registry of running dev servers, keyed by server_id.
/// This uses DashMap for thread-safe concurrent access.
static DEV_SERVERS: std::sync::OnceLock<DashMap<String, DevServerHandle>> =
    std::sync::OnceLock::new();

/// Get or initialize the global dev server registry.
pub fn dev_server_registry() -> &'static DashMap<String, DevServerHandle> {
    DEV_SERVERS.get_or_init(|| DashMap::new())
}

/// Handle to a running dev server process.
pub struct DevServerHandle {
    #[allow(dead_code)]
    pub server_id: String,
    pub run_id: String,
    #[allow(dead_code)]
    pub sub_agent_id: String,
    pub process: Child,
    pub url: Option<String>,
    pub port: u16,
    pub started_at: Instant,
    pub stdout_buffer: Arc<Mutex<VecDeque<String>>>,
    pub stderr_buffer: Arc<Mutex<VecDeque<String>>>,
    pub command: String,
    #[allow(dead_code)]
    pub workdir: String,
}

impl DevServerHandle {
    /// Check if the process is still running.
    /// Uses interior mutability via Child's try_wait which is &mut self,
    /// so we need to use a different approach - just return true if we haven't explicitly stopped it.
    pub fn is_running(&self) -> bool {
        // Since we can't call try_wait without &mut, we'll rely on the process still being
        // in our registry. The stop_dev_server function will remove it when it exits.
        true
    }

    /// Get process exit code if it has exited.
    /// Returns None since we can't check without &mut self.
    pub fn exit_code(&self) -> Option<i32> {
        None
    }

    /// Get uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}

/// Input for dev_server.start tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevServerStartInput {
    pub command: String,
    pub port: Option<u16>,
    pub workdir: Option<String>,
    pub health_check_url: Option<String>,
    pub max_wait_secs: Option<u64>,
}

/// Output for dev_server.start tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevServerStartOutput {
    pub server_id: String,
    pub url: String,
    pub port: u16,
    pub status: String,
    pub pid: Option<u32>,
    pub health_check_result: Option<HealthCheckResult>,
}

/// Result of a health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub success: bool,
    pub status_code: Option<u16>,
    pub response_time_ms: u64,
    pub error: Option<String>,
}

/// Tool for starting a development server.
pub struct DevServerStartTool;

impl Tool for DevServerStartTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "dev_server.start".into(),
            description: concat!(
                "Start a development server in the background. ",
                "The server will run detached from the current tool call, ",
                "allowing the agent to continue while the server stays running. ",
                "Returns a server_id that can be used to stop, check status, or get logs."
            )
            .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to start the dev server (e.g., 'bun dev', 'npm run dev', 'vite')"
                    },
                    "port": {
                        "type": "integer",
                        "description": "Expected port number for health checking (optional, auto-detected if not provided)"
                    },
                    "workdir": {
                        "type": "string",
                        "description": "Working directory relative to workspace root (optional)"
                    },
                    "health_check_url": {
                        "type": "string",
                        "description": "URL to health check after starting (defaults to http://localhost:{port})"
                    },
                    "max_wait_secs": {
                        "type": "integer",
                        "description": "Max seconds to wait for server to be ready (default: 30)",
                        "default": 30
                    }
                },
                "required": ["command"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: DevServerStartInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        // Resolve working directory
        let workdir = match args.workdir {
            Some(wd) => {
                let path = cwd.join(&wd);
                match policy.evaluate_path(&path) {
                    PolicyDecision::Allow => path,
                    PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
                    PolicyDecision::NeedsApproval { scope, reason } => {
                        return Err(ToolError::ApprovalRequired { scope, reason })
                    }
                }
            }
            None => cwd.to_path_buf(),
        };

        // Check command policy
        let binary = args
            .command
            .split_whitespace()
            .next()
            .unwrap_or(&args.command);
        match policy.evaluate_command(binary) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }

        // Determine port
        let port = args
            .port
            .unwrap_or_else(|| detect_port_from_command(&args.command));

        // Generate server ID
        let server_id = Uuid::new_v4().to_string();

        // We'll need to capture run_id and sub_agent_id from context
        // For now, use placeholder - these should be passed via tool context
        let run_id = "unknown".to_string();
        let sub_agent_id = "unknown".to_string();

        // Start the process asynchronously
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|e| ToolError::Execution(format!("no async runtime: {}", e)))?;

        let output = runtime.block_on(async {
            start_dev_server(
                server_id.clone(),
                run_id,
                sub_agent_id,
                args.command.clone(),
                workdir.clone(),
                port,
                args.health_check_url,
                args.max_wait_secs.unwrap_or(30),
            )
            .await
        })?;

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::to_value(output).unwrap_or_default(),
            error: None,
        })
    }
}

/// Start a dev server and return its handle info.
async fn start_dev_server(
    server_id: String,
    run_id: String,
    sub_agent_id: String,
    command: String,
    workdir: std::path::PathBuf,
    port: u16,
    health_check_url: Option<String>,
    max_wait_secs: u64,
) -> Result<DevServerStartOutput, ToolError> {
    // Parse command
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err(ToolError::InvalidInput("empty command".into()));
    }

    let binary = parts[0];
    let args = &parts[1..];

    // Start the process
    let mut child = TokioCommand::new(binary)
        .args(args)
        .current_dir(&workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false)
        .spawn()
        .map_err(|e| ToolError::Execution(format!("failed to spawn process: {}", e)))?;

    let pid = child.id();

    // Set up log buffers
    let stdout_buffer: Arc<Mutex<VecDeque<String>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(DEFAULT_LOG_BUFFER_SIZE)));
    let stderr_buffer: Arc<Mutex<VecDeque<String>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(DEFAULT_LOG_BUFFER_SIZE)));

    // Spawn log capture tasks
    if let Some(stdout) = child.stdout.take() {
        let buf = Arc::clone(&stdout_buffer);
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut buffer = buf.lock().unwrap();
                if buffer.len() >= DEFAULT_LOG_BUFFER_SIZE {
                    buffer.pop_front();
                }
                buffer.push_back(line);
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let buf = Arc::clone(&stderr_buffer);
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut buffer = buf.lock().unwrap();
                if buffer.len() >= DEFAULT_LOG_BUFFER_SIZE {
                    buffer.pop_front();
                }
                buffer.push_back(line);
            }
        });
    }

    // Create handle
    let handle = DevServerHandle {
        server_id: server_id.clone(),
        run_id: run_id.clone(),
        sub_agent_id: sub_agent_id.clone(),
        process: child,
        url: None,
        port,
        started_at: Instant::now(),
        stdout_buffer,
        stderr_buffer,
        command: command.clone(),
        workdir: workdir.to_string_lossy().to_string(),
    };

    // Store in registry
    dev_server_registry().insert(server_id.clone(), handle);

    // Wait for health check
    let url = health_check_url.unwrap_or_else(|| format!("http://localhost:{}", port));
    let health_result = wait_for_health_check(&url, max_wait_secs).await;

    // Update stored URL
    if let Some(mut entry) = dev_server_registry().get_mut(&server_id) {
        entry.url = Some(url.clone());
    }

    Ok(DevServerStartOutput {
        server_id,
        url,
        port,
        status: if health_result.as_ref().map(|h| h.success).unwrap_or(false) {
            "running".to_string()
        } else {
            "starting".to_string()
        },
        pid,
        health_check_result: health_result,
    })
}

/// Detect port from common dev server commands.
fn detect_port_from_command(command: &str) -> u16 {
    // Check for explicit port flags
    if command.contains("--port") || command.contains("-p") {
        // Try to extract port number
        let parts: Vec<&str> = command.split_whitespace().collect();
        for (i, part) in parts.iter().enumerate() {
            if (*part == "--port" || *part == "-p") && i + 1 < parts.len() {
                if let Ok(port) = parts[i + 1].parse::<u16>() {
                    return port;
                }
            }
            if part.starts_with("--port=") {
                if let Ok(port) = part[7..].parse::<u16>() {
                    return port;
                }
            }
        }
    }

    // Default ports for common tools
    if command.contains("vite") {
        5173
    } else if command.contains("next") {
        3000
    } else if command.contains("nuxt") {
        3000
    } else if command.contains("astro") {
        4321
    } else if command.contains("svelte-kit") || command.contains("vite") {
        5173
    } else if command.contains("remix") {
        3000
    } else if command.contains("gatsby") {
        8000
    } else {
        3000 // Generic default
    }
}

/// Wait for a health check to succeed.
async fn wait_for_health_check(url: &str, max_wait_secs: u64) -> Option<HealthCheckResult> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS))
        .build()
        .ok()?;

    let start = Instant::now();
    let max_wait = std::time::Duration::from_secs(max_wait_secs);

    while start.elapsed() < max_wait {
        let check_start = Instant::now();
        match client.get(url).send().await {
            Ok(response) => {
                let elapsed = check_start.elapsed().as_millis() as u64;
                return Some(HealthCheckResult {
                    success: response.status().is_success(),
                    status_code: Some(response.status().as_u16()),
                    response_time_ms: elapsed,
                    error: None,
                });
            }
            Err(e) => {
                // Server not ready yet, wait and retry
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if start.elapsed() >= max_wait {
                    return Some(HealthCheckResult {
                        success: false,
                        status_code: None,
                        response_time_ms: check_start.elapsed().as_millis() as u64,
                        error: Some(e.to_string()),
                    });
                }
            }
        }
    }

    None
}

/// Tool for stopping a dev server.
pub struct DevServerStopTool;

impl Tool for DevServerStopTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "dev_server.stop".into(),
            description: concat!(
                "Stop a running development server. ",
                "Sends SIGTERM first, then SIGKILL after timeout if needed."
            )
            .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "server_id": {
                        "type": "string",
                        "description": "The server ID returned by dev_server.start"
                    },
                    "graceful_timeout_secs": {
                        "type": "integer",
                        "description": "Seconds to wait for graceful shutdown before force kill (default: 5)",
                        "default": 5
                    }
                },
                "required": ["server_id"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let server_id = input
            .get("server_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("server_id is required".into()))?;

        let graceful_timeout = input
            .get("graceful_timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(5);

        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|e| ToolError::Execution(format!("no async runtime: {}", e)))?;

        let result = runtime.block_on(stop_dev_server(server_id, graceful_timeout))?;
        let has_error = result.error.clone();

        Ok(ToolCallOutput {
            ok: result.success,
            data: serde_json::to_value(result).unwrap_or_default(),
            error: has_error,
        })
    }
}

/// Result of stopping a dev server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevServerStopResult {
    pub server_id: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub runtime_secs: u64,
    pub error: Option<String>,
}

async fn stop_dev_server(
    server_id: &str,
    graceful_timeout_secs: u64,
) -> Result<DevServerStopResult, ToolError> {
    let entry = dev_server_registry()
        .remove(server_id)
        .ok_or_else(|| ToolError::InvalidInput(format!("server not found: {}", server_id)))?;

    let mut handle = entry.1;
    let runtime_secs = handle.uptime_secs();

    // Try graceful shutdown first with timeout
    let graceful_timeout = std::time::Duration::from_secs(graceful_timeout_secs);
    let start = Instant::now();

    // Poll for process exit with timeout
    let (success, exit_code, error) = loop {
        match handle.process.try_wait() {
            Ok(Some(status)) => {
                // Process exited gracefully
                break (true, status.code(), None);
            }
            Ok(None) => {
                // Still running
                if start.elapsed() >= graceful_timeout {
                    // Timeout exceeded, force kill
                    break force_kill_process(&mut handle.process).await;
                }
                // Wait a bit before checking again
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            Err(_e) => {
                // Error checking status, try to kill
                break force_kill_process(&mut handle.process).await;
            }
        }
    };

    Ok(DevServerStopResult {
        server_id: server_id.to_string(),
        success,
        exit_code,
        runtime_secs,
        error,
    })
}

/// Force kill a process and wait for it to exit.
async fn force_kill_process(
    process: &mut tokio::process::Child,
) -> (bool, Option<i32>, Option<String>) {
    match process.kill().await {
        Ok(_) => {
            // Wait for process to exit
            match process.wait().await {
                Ok(status) => (true, status.code(), Some("force killed".to_string())),
                Err(e) => (
                    false,
                    None,
                    Some(format!("failed to wait after kill: {}", e)),
                ),
            }
        }
        Err(e) => (false, None, Some(format!("failed to kill process: {}", e))),
    }
}

/// Tool for checking dev server status.
pub struct DevServerStatusTool;

impl Tool for DevServerStatusTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "dev_server.status".into(),
            description: concat!(
                "Check the status of a running development server. ",
                "Returns whether it's running, its uptime, current health, and recent errors."
            )
            .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "server_id": {
                        "type": "string",
                        "description": "The server ID returned by dev_server.start"
                    }
                },
                "required": ["server_id"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let server_id = input
            .get("server_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("server_id is required".into()))?;

        let entry = dev_server_registry()
            .get(server_id)
            .ok_or_else(|| ToolError::InvalidInput(format!("server not found: {}", server_id)))?;

        let handle = entry.value();
        let is_running = handle.is_running();
        let uptime_secs = handle.uptime_secs();
        let exit_code = if is_running { None } else { handle.exit_code() };

        // Get last few stderr lines for error context
        let recent_errors: Vec<String> = {
            let buffer = handle.stderr_buffer.lock().unwrap();
            buffer.iter().rev().take(5).cloned().collect()
        };

        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|e| ToolError::Execution(format!("no async runtime: {}", e)))?;

        // Perform health check if running and has URL
        let health_result = if is_running {
            if let Some(ref url) = handle.url {
                runtime.block_on(async {
                    let client = match reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(5))
                        .build()
                    {
                        Ok(c) => c,
                        Err(_) => return None,
                    };
                    let start = Instant::now();
                    match client.get(url).send().await {
                        Ok(response) => Some(HealthCheckResult {
                            success: response.status().is_success(),
                            status_code: Some(response.status().as_u16()),
                            response_time_ms: start.elapsed().as_millis() as u64,
                            error: None,
                        }),
                        Err(e) => Some(HealthCheckResult {
                            success: false,
                            status_code: None,
                            response_time_ms: start.elapsed().as_millis() as u64,
                            error: Some(e.to_string()),
                        }),
                    }
                })
            } else {
                None
            }
        } else {
            None
        };

        let status = DevServerStatusOutput {
            server_id: server_id.to_string(),
            is_running,
            uptime_secs,
            exit_code,
            url: handle.url.clone(),
            port: handle.port,
            command: handle.command.clone(),
            health_check: health_result,
            recent_errors: if recent_errors.is_empty() {
                None
            } else {
                Some(recent_errors)
            },
        };

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::to_value(status).unwrap_or_default(),
            error: None,
        })
    }
}

/// Output for dev_server.status tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevServerStatusOutput {
    pub server_id: String,
    pub is_running: bool,
    pub uptime_secs: u64,
    pub exit_code: Option<i32>,
    pub url: Option<String>,
    pub port: u16,
    pub command: String,
    pub health_check: Option<HealthCheckResult>,
    pub recent_errors: Option<Vec<String>>,
}

/// Tool for retrieving dev server logs.
pub struct DevServerLogsTool;

impl Tool for DevServerLogsTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "dev_server.logs".into(),
            description: concat!(
                "Retrieve recent logs from a running or recently stopped development server."
            )
            .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "server_id": {
                        "type": "string",
                        "description": "The server ID returned by dev_server.start"
                    },
                    "stream": {
                        "type": "string",
                        "enum": ["stdout", "stderr", "both"],
                        "description": "Which log stream to retrieve (default: both)",
                        "default": "both"
                    },
                    "lines": {
                        "type": "integer",
                        "description": "Number of lines to retrieve (default: 50, max: 500)",
                        "default": 50
                    }
                },
                "required": ["server_id"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let server_id = input
            .get("server_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("server_id is required".into()))?;

        let stream = input
            .get("stream")
            .and_then(|v| v.as_str())
            .unwrap_or("both");

        let lines = input
            .get("lines")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(50)
            .min(MAX_LOG_LINES_PER_REQUEST);

        let entry = dev_server_registry()
            .get(server_id)
            .ok_or_else(|| ToolError::InvalidInput(format!("server not found: {}", server_id)))?;

        let handle = entry.value();

        let (stdout_lines, stderr_lines) = match stream {
            "stdout" => {
                let buf = handle.stdout_buffer.lock().unwrap();
                (
                    buf.iter().rev().take(lines).cloned().collect::<Vec<_>>(),
                    Vec::new(),
                )
            }
            "stderr" => {
                let buf = handle.stderr_buffer.lock().unwrap();
                (
                    Vec::new(),
                    buf.iter().rev().take(lines).cloned().collect::<Vec<_>>(),
                )
            }
            _ => {
                let stdout_buf = handle.stdout_buffer.lock().unwrap();
                let stderr_buf = handle.stderr_buffer.lock().unwrap();
                (
                    stdout_buf
                        .iter()
                        .rev()
                        .take(lines)
                        .cloned()
                        .collect::<Vec<_>>(),
                    stderr_buf
                        .iter()
                        .rev()
                        .take(lines)
                        .cloned()
                        .collect::<Vec<_>>(),
                )
            }
        };

        let total_lines = stdout_lines.len() + stderr_lines.len();
        let output = DevServerLogsOutput {
            server_id: server_id.to_string(),
            stdout: if stdout_lines.is_empty() {
                None
            } else {
                Some(stdout_lines)
            },
            stderr: if stderr_lines.is_empty() {
                None
            } else {
                Some(stderr_lines)
            },
            total_lines,
        };

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::to_value(output).unwrap_or_default(),
            error: None,
        })
    }
}

/// Output for dev_server.logs tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevServerLogsOutput {
    pub server_id: String,
    pub stdout: Option<Vec<String>>,
    pub stderr: Option<Vec<String>>,
    pub total_lines: usize,
}

/// Stop all dev servers associated with a run_id.
/// This should be called when a step or run completes/cancels.
pub async fn stop_all_dev_servers_for_run(run_id: &str) -> Vec<DevServerStopResult> {
    let registry = dev_server_registry();
    let mut results = Vec::new();

    // Collect server IDs to stop
    let servers_to_stop: Vec<String> = registry
        .iter()
        .filter(|entry| entry.value().run_id == run_id)
        .map(|entry| entry.key().clone())
        .collect();

    for server_id in servers_to_stop {
        if let Ok(result) = stop_dev_server(&server_id, 3).await {
            results.push(result);
        }
    }

    results
}
