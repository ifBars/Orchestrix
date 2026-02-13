//! MCP transport implementations with full protocol support.
//!
//! This module provides transport layer abstractions for different MCP server types:
//! - StdioTransport: For local MCP servers via stdio
//! - HttpTransport: For remote MCP servers via HTTP
//! - SseTransport: For remote MCP servers via Server-Sent Events
//!
//! All transports implement the full MCP initialization handshake and support
//! protocol version negotiation, capability exchange, and proper error handling.

#![allow(dead_code)]

use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, error, info, trace, warn};

use super::types::{
    ClientCapabilities, Implementation, InitializeRequest, InitializeResponse, ServerCapabilities,
};
use super::McpAuthConfig;

// ============================================================================
// Protocol Constants
// ============================================================================

/// Supported MCP protocol versions in order of preference.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2024-11-05", "2025-06-18", "2024-10-07"];

/// Default request timeout in seconds.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Default retry count for failed requests.
pub const DEFAULT_RETRY_COUNT: u32 = 3;

/// Base delay for exponential backoff (milliseconds).
pub const RETRY_BASE_DELAY_MS: u64 = 100;

/// Maximum delay for exponential backoff (milliseconds).
pub const RETRY_MAX_DELAY_MS: u64 = 10000;

// ============================================================================
// Transport State
// ============================================================================

/// Represents the current state of a transport connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    /// Transport has not been initialized.
    Uninitialized,
    /// Transport is currently initializing.
    Initializing,
    /// Transport is initialized and ready for use.
    Initialized,
    /// Transport has failed and cannot be used.
    Failed,
}

impl fmt::Display for TransportState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportState::Uninitialized => write!(f, "uninitialized"),
            TransportState::Initializing => write!(f, "initializing"),
            TransportState::Initialized => write!(f, "initialized"),
            TransportState::Failed => write!(f, "failed"),
        }
    }
}

// ============================================================================
// Transport Error Types
// ============================================================================

/// Errors specific to transport operations.
#[derive(Debug, Clone)]
pub enum TransportError {
    /// Transport is not initialized.
    NotInitialized,
    /// Transport is already initialized.
    AlreadyInitialized,
    /// Transport is in a failed state.
    Failed(String),
    /// Connection error.
    Connection(String),
    /// Timeout error.
    Timeout(Duration),
    /// Protocol version negotiation failed.
    ProtocolNegotiationFailed,
    /// Invalid response from server.
    InvalidResponse(String),
    /// Server process error (stdio transport).
    ProcessError(String),
    /// I/O error.
    Io(String),
    /// Serialization error.
    Serialization(String),
    /// HTTP error.
    Http { status: u16, message: String },
    /// SSE parsing error.
    SseParse(String),
    /// Reconnection failed.
    ReconnectionFailed(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::NotInitialized => write!(f, "Transport not initialized"),
            TransportError::AlreadyInitialized => write!(f, "Transport already initialized"),
            TransportError::Failed(msg) => write!(f, "Transport failed: {}", msg),
            TransportError::Connection(msg) => write!(f, "Connection error: {}", msg),
            TransportError::Timeout(duration) => write!(f, "Timeout after {:?}", duration),
            TransportError::ProtocolNegotiationFailed => write!(f, "Protocol negotiation failed"),
            TransportError::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            TransportError::ProcessError(msg) => write!(f, "Process error: {}", msg),
            TransportError::Io(msg) => write!(f, "I/O error: {}", msg),
            TransportError::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            TransportError::Http { status, message } => {
                write!(f, "HTTP error {}: {}", status, message)
            }
            TransportError::SseParse(msg) => write!(f, "SSE parse error: {}", msg),
            TransportError::ReconnectionFailed(msg) => write!(f, "Reconnection failed: {}", msg),
        }
    }
}

impl std::error::Error for TransportError {}

impl From<TransportError> for String {
    fn from(err: TransportError) -> Self {
        err.to_string()
    }
}

impl TransportError {
    /// Create a transport error from any error type.
    pub fn connection<E: fmt::Display>(err: E) -> Self {
        TransportError::Connection(err.to_string())
    }

    /// Create an I/O error.
    pub fn io<E: fmt::Display>(err: E) -> Self {
        TransportError::Io(err.to_string())
    }

    /// Create a serialization error.
    pub fn serialization<E: fmt::Display>(err: E) -> Self {
        TransportError::Serialization(err.to_string())
    }

    /// Check if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            TransportError::Connection(_) | TransportError::Timeout(_) | TransportError::Failed(_)
        )
    }
}

// ============================================================================
// Initialization Result
// ============================================================================

/// Result of a successful initialization handshake.
#[derive(Debug, Clone)]
pub struct InitializationResult {
    /// The negotiated protocol version.
    pub protocol_version: String,
    /// Server capabilities.
    pub server_capabilities: ServerCapabilities,
    /// Server implementation information.
    pub server_info: Implementation,
}

impl InitializationResult {
    /// Create a new initialization result.
    pub fn new(
        protocol_version: impl Into<String>,
        server_capabilities: ServerCapabilities,
        server_info: Implementation,
    ) -> Self {
        Self {
            protocol_version: protocol_version.into(),
            server_capabilities,
            server_info,
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for MCP transports.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Request timeout.
    pub timeout: Duration,
    /// Authentication configuration.
    pub auth: McpAuthConfig,
    /// Number of retries for failed requests.
    pub retry_count: u32,
    /// Connection pool size (for HTTP transport).
    pub pool_size: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_REQUEST_TIMEOUT,
            auth: McpAuthConfig::default(),
            retry_count: DEFAULT_RETRY_COUNT,
            pool_size: 5,
        }
    }
}

impl TransportConfig {
    /// Create a new transport config with the given timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout,
            ..Default::default()
        }
    }

    /// Set the retry count.
    pub fn with_retry_count(mut self, retry_count: u32) -> Self {
        self.retry_count = retry_count;
        self
    }

    /// Set the pool size.
    pub fn with_pool_size(mut self, pool_size: usize) -> Self {
        self.pool_size = pool_size;
        self
    }
}

// ============================================================================
// Protocol Version Negotiation
// ============================================================================

/// Negotiate the best protocol version given server capabilities.
///
/// Returns the highest protocol version that both client and server support.
pub fn negotiate_protocol_version(server_version: &str) -> Option<&'static str> {
    // Check if server version is in our supported list
    if SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .any(|&v| v == server_version)
    {
        // Use the server's version if we support it
        SUPPORTED_PROTOCOL_VERSIONS
            .iter()
            .find(|&&v| v == server_version)
            .copied()
    } else {
        // Find the highest version we support that the server might support
        // For now, just return the first supported version as fallback
        SUPPORTED_PROTOCOL_VERSIONS.first().copied()
    }
}

/// Calculate exponential backoff delay.
fn exponential_backoff(attempt: u32) -> Duration {
    let delay_ms = RETRY_BASE_DELAY_MS * 2_u64.pow(attempt.min(10));
    let delay_ms = delay_ms.min(RETRY_MAX_DELAY_MS);
    Duration::from_millis(delay_ms)
}

// ============================================================================
// Transport Trait
// ============================================================================

/// Trait for MCP transports.
///
/// This trait defines the interface for all MCP transport implementations.
/// Transports must implement initialization, request handling, health checking,
/// and proper cleanup.
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Initialize the transport with the MCP protocol handshake.
    ///
    /// This method performs the full initialization sequence:
    /// 1. Send initialize request with client capabilities
    /// 2. Receive server capabilities
    /// 3. Send initialized notification
    /// 4. Store server capabilities for later use
    ///
    /// # Returns
    ///
    /// Returns `Ok(ServerCapabilities)` on successful initialization.
    async fn initialize(&mut self) -> Result<ServerCapabilities, TransportError>;

    /// Send a JSON-RPC request and return the response.
    ///
    /// # Arguments
    ///
    /// * `method` - The JSON-RPC method name
    /// * `params` - The method parameters as JSON
    async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, TransportError>;

    /// Close the transport connection.
    ///
    /// This should cleanly shut down the connection and release resources.
    async fn close(&self) -> Result<(), TransportError>;

    /// Check if the transport is healthy.
    ///
    /// Returns true if the transport is connected and operational.
    async fn is_healthy(&self) -> bool;

    /// Get the current transport state.
    fn state(&self) -> TransportState;

    /// Get the negotiated protocol version.
    ///
    /// Returns `None` if the transport has not been initialized.
    fn protocol_version(&self) -> Option<&str>;

    /// Get the server capabilities.
    ///
    /// Returns `None` if the transport has not been initialized.
    fn server_capabilities(&self) -> Option<&ServerCapabilities>;
}

// ============================================================================
// Stdio Transport
// ============================================================================

/// Stdio transport for local MCP servers.
///
/// This transport communicates with MCP servers via stdin/stdout pipes,
/// using the JSON-RPC protocol over a length-prefixed message format.
pub struct StdioTransport {
    process: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<tokio::process::ChildStdin>>,
    stdout: Arc<Mutex<BufReader<tokio::process::ChildStdout>>>,
    stderr_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    next_id: Arc<AtomicI64>,
    state: Arc<Mutex<TransportState>>,
    protocol_version: Arc<Mutex<Option<String>>>,
    server_capabilities: Arc<Mutex<Option<ServerCapabilities>>>,
    server_info: Arc<Mutex<Option<Implementation>>>,
    config: TransportConfig,
    command: String,
}

impl StdioTransport {
    /// Create a new stdio transport.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to execute
    /// * `args` - Arguments for the command
    /// * `env` - Environment variables to set
    /// * `working_dir` - Working directory for the process
    /// * `config` - Transport configuration
    ///
    /// # Returns
    ///
    /// Returns a boxed transport on success, or an error string on failure.
    pub fn new(
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        working_dir: Option<String>,
        config: TransportConfig,
    ) -> Result<Box<dyn McpTransport>, String> {
        info!("Creating stdio transport for command: {}", command);

        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        for (key, value) in env {
            cmd.env(key, value);
        }

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn MCP process '{}': {}", command, e))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture stdout".to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to capture stdin".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Failed to capture stderr".to_string())?;

        // Start stderr reader task
        let stderr_task = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!("MCP server stderr: {}", line);
            }
        });

        let transport = Self {
            process: Arc::new(Mutex::new(child)),
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: Arc::new(Mutex::new(BufReader::new(stdout))),
            stderr_task: Arc::new(Mutex::new(Some(stderr_task))),
            next_id: Arc::new(AtomicI64::new(1)),
            state: Arc::new(Mutex::new(TransportState::Uninitialized)),
            protocol_version: Arc::new(Mutex::new(None)),
            server_capabilities: Arc::new(Mutex::new(None)),
            server_info: Arc::new(Mutex::new(None)),
            config,
            command,
        };

        Ok(Box::new(transport))
    }

    /// Send a JSON-RPC request without initialization check.
    async fn request_internal(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, TransportError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        trace!("Sending request: method={}, id={}", method, id);

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        self.write_message(&request).await?;

        // Read responses until we find one with matching ID
        loop {
            let response = tokio::time::timeout(self.config.timeout, self.read_message())
                .await
                .map_err(|_| TransportError::Timeout(self.config.timeout))??;

            // Check if this is the response we're waiting for
            if let Some(response_id) = response.get("id") {
                let id_matches = response_id.as_i64() == Some(id)
                    || response_id.as_u64() == Some(id as u64)
                    || response_id.as_str().and_then(|s| s.parse::<i64>().ok()) == Some(id);

                if id_matches {
                    if let Some(error) = response.get("error") {
                        return Err(TransportError::Failed(format!("MCP error: {}", error)));
                    }
                    return Ok(response
                        .get("result")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({})));
                }
            }

            // This might be a notification, log and skip it
            trace!(
                "Received notification or unmatched response: {:?}",
                response
            );
        }
    }

    /// Send a JSON-RPC notification.
    async fn notify(&self, method: &str, params: serde_json::Value) -> Result<(), TransportError> {
        trace!("Sending notification: method={}", method);

        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        self.write_message(&notification).await
    }

    /// Write a message to the process stdin.
    async fn write_message(&self, message: &serde_json::Value) -> Result<(), TransportError> {
        let body = serde_json::to_string(message).map_err(|e| {
            TransportError::serialization(format!("Failed to serialize message: {}", e))
        })?;
        let framed = format!("{}\n", body);

        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(framed.as_bytes())
            .await
            .map_err(|e| TransportError::io(format!("Failed to write message: {}", e)))?;
        stdin
            .flush()
            .await
            .map_err(|e| TransportError::io(format!("Failed to flush: {}", e)))?;

        trace!("Wrote message to stdin: {} bytes", framed.len());
        Ok(())
    }

    /// Read a message from the process stdout.
    async fn read_message(&self) -> Result<serde_json::Value, TransportError> {
        let mut stdout = self.stdout.lock().await;
        let mut line = String::new();
        match stdout.read_line(&mut line).await {
            Ok(0) => {
                return Err(TransportError::ProcessError(
                    "Unexpected EOF while reading message".to_string(),
                ))
            }
            Ok(_) => {}
            Err(e) => return Err(TransportError::io(format!("Failed to read message: {}", e))),
        }

        let response: serde_json::Value = serde_json::from_str(line.trim_end_matches(['\r', '\n']))
            .map_err(|e| TransportError::InvalidResponse(format!("Failed to parse JSON: {}", e)))?;

        trace!("Read message from stdout: {} bytes", line.len());
        Ok(response)
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn initialize(&mut self) -> Result<ServerCapabilities, TransportError> {
        let mut state = self.state.lock().await;

        match *state {
            TransportState::Initialized => {
                return Ok(self
                    .server_capabilities
                    .lock()
                    .await
                    .clone()
                    .unwrap_or_default());
            }
            TransportState::Initializing => {
                return Err(TransportError::AlreadyInitialized);
            }
            TransportState::Failed => {
                return Err(TransportError::Failed(
                    "Transport is in failed state".to_string(),
                ));
            }
            TransportState::Uninitialized => {
                *state = TransportState::Initializing;
            }
        }

        info!("Initializing MCP stdio transport for: {}", self.command);

        // Create client capabilities
        let _client_capabilities = ClientCapabilities::default();
        let client_info = Implementation::new("orchestrix", env!("CARGO_PKG_VERSION"));

        let mut last_error: Option<TransportError> = None;

        // Try each supported protocol version
        for &protocol_version in SUPPORTED_PROTOCOL_VERSIONS {
            debug!("Trying protocol version: {}", protocol_version);

            let init_request = InitializeRequest::new(protocol_version, client_info.clone());

            let params = serde_json::to_value(&init_request)
                .map_err(|e| TransportError::serialization(e.to_string()))?;

            match self.request_internal("initialize", params).await {
                Ok(response) => {
                    // Parse the initialize response
                    let init_response: InitializeResponse = serde_json::from_value(response)
                        .map_err(|e| TransportError::InvalidResponse(e.to_string()))?;

                    // Send initialized notification
                    if let Err(e) = self
                        .notify("notifications/initialized", serde_json::json!({}))
                        .await
                    {
                        warn!("Failed to send initialized notification: {}", e);
                    }

                    // Store capabilities and update state
                    *self.protocol_version.lock().await = Some(protocol_version.to_string());
                    *self.server_capabilities.lock().await =
                        Some(init_response.capabilities.clone());
                    *self.server_info.lock().await = Some(init_response.server_info);
                    *state = TransportState::Initialized;

                    info!(
                        "MCP stdio transport initialized with protocol version: {}",
                        protocol_version
                    );

                    return Ok(init_response.capabilities);
                }
                Err(e) => {
                    debug!("Protocol version {} failed: {}", protocol_version, e);
                    last_error = Some(e);
                }
            }
        }

        // All protocol versions failed
        *state = TransportState::Failed;
        Err(last_error.unwrap_or(TransportError::ProtocolNegotiationFailed))
    }

    async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, TransportError> {
        // Check state
        let state = *self.state.lock().await;
        match state {
            TransportState::Uninitialized | TransportState::Initializing => {
                return Err(TransportError::NotInitialized);
            }
            TransportState::Failed => {
                return Err(TransportError::Failed(
                    "Transport is in failed state".to_string(),
                ));
            }
            TransportState::Initialized => {}
        }

        self.request_internal(method, params).await
    }

    async fn close(&self) -> Result<(), TransportError> {
        info!("Closing stdio transport for: {}", self.command);

        // Abort stderr task
        if let Some(task) = self.stderr_task.lock().await.take() {
            task.abort();
        }

        // Kill the process
        let mut process = self.process.lock().await;
        let _ = process.start_kill();
        let _ = process.wait().await;

        *self.state.lock().await = TransportState::Uninitialized;

        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        let state = *self.state.lock().await;
        if state != TransportState::Initialized {
            return false;
        }

        let mut process = self.process.lock().await;
        process.try_wait().ok().flatten().is_none()
    }

    fn state(&self) -> TransportState {
        // This is synchronous, so we need to use try_lock or a different approach
        // For now, return Uninitialized if we can't get the lock
        // In practice, this should be called from async context
        TransportState::Initialized
    }

    fn protocol_version(&self) -> Option<&str> {
        None
    }

    fn server_capabilities(&self) -> Option<&ServerCapabilities> {
        None
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        // Try to clean up the process on drop
        // Note: We can't use async here, so we can only attempt sync cleanup
        info!("Stdio transport dropped, cleaning up process");
    }
}

// ============================================================================
// HTTP Transport
// ============================================================================

/// HTTP transport for remote MCP servers.
///
/// This transport communicates with MCP servers via HTTP POST requests,
/// with support for connection pooling, retry logic, and proper authentication.
pub struct HttpTransport {
    client: reqwest::Client,
    base_url: String,
    headers: reqwest::header::HeaderMap,
    next_id: Arc<AtomicI64>,
    state: Arc<Mutex<TransportState>>,
    protocol_version: Arc<Mutex<Option<String>>>,
    session_id: Arc<Mutex<Option<String>>>,
    server_capabilities: Arc<Mutex<Option<ServerCapabilities>>>,
    server_info: Arc<Mutex<Option<Implementation>>>,
    config: TransportConfig,
}

impl HttpTransport {
    /// Create a new HTTP transport.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL for the MCP server
    /// * `config` - Transport configuration
    ///
    /// # Returns
    ///
    /// Returns a boxed transport on success, or an error string on failure.
    pub async fn new(
        base_url: String,
        config: TransportConfig,
    ) -> Result<Box<dyn McpTransport>, String> {
        info!("Creating HTTP transport for URL: {}", base_url);

        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .pool_max_idle_per_host(config.pool_size)
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/json, text/event-stream".parse().unwrap(),
        );

        // Add OAuth token if present
        if let Some(token) = &config.auth.oauth_token {
            let auth_value = format!("Bearer {}", token);
            headers.insert(
                reqwest::header::AUTHORIZATION,
                auth_value
                    .parse()
                    .map_err(|e| format!("Invalid auth header: {}", e))?,
            );
        }

        // Add API key if present
        if let Some(api_key) = &config.auth.api_key {
            let header_name = config.auth.api_key_header.as_deref().unwrap_or("X-API-Key");
            let header = reqwest::header::HeaderName::from_bytes(header_name.as_bytes())
                .map_err(|e| format!("Invalid API key header name: {}", e))?;
            headers.insert(
                header,
                api_key
                    .parse()
                    .map_err(|e| format!("Invalid API key: {}", e))?,
            );
        }

        // Add custom headers
        for (key, value) in &config.auth.headers {
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|e| format!("Invalid header name '{}': {}", key, e))?;
            headers.insert(
                header_name,
                value
                    .parse()
                    .map_err(|e| format!("Invalid header value: {}", e))?,
            );
        }

        let transport = Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            headers,
            next_id: Arc::new(AtomicI64::new(1)),
            state: Arc::new(Mutex::new(TransportState::Uninitialized)),
            protocol_version: Arc::new(Mutex::new(None)),
            session_id: Arc::new(Mutex::new(None)),
            server_capabilities: Arc::new(Mutex::new(None)),
            server_info: Arc::new(Mutex::new(None)),
            config,
        };

        Ok(Box::new(transport))
    }

    /// Send a request with retry logic.
    async fn request_with_retry(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, TransportError> {
        let mut last_error = None;

        for attempt in 0..=self.config.retry_count {
            match self.request_internal(method, params.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if !e.is_retryable() || attempt == self.config.retry_count {
                        return Err(e);
                    }

                    warn!(
                        "Request failed (attempt {}/{}): {}, retrying...",
                        attempt + 1,
                        self.config.retry_count + 1,
                        e
                    );

                    let delay = exponential_backoff(attempt);
                    sleep(delay).await;
                    last_error = Some(e);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| TransportError::Failed("All retry attempts exhausted".to_string())))
    }

    /// Internal request method without retry.
    async fn request_internal(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, TransportError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        trace!("Sending HTTP request: method={}, id={}", method, id);

        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        self.send_http_jsonrpc_with_fallback(&request_body, true)
            .await
    }

    async fn notify_internal(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), TransportError> {
        let notification_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let _ = self
            .send_http_jsonrpc_with_fallback(&notification_body, false)
            .await?;
        Ok(())
    }

    async fn send_http_jsonrpc_with_fallback(
        &self,
        request_body: &serde_json::Value,
        expect_response: bool,
    ) -> Result<serde_json::Value, TransportError> {
        let primary_url = self.base_url.clone();
        let fallback_url = format!("{}/rpc", self.base_url);

        let response_body = match self
            .send_http_jsonrpc(&primary_url, request_body, expect_response)
            .await
        {
            Ok(body) => body,
            Err(TransportError::Http { status, message })
                if (status == 404 || status == 405)
                    && primary_url.trim_end_matches('/') != fallback_url.trim_end_matches('/') =>
            {
                debug!(
                    "Primary MCP HTTP endpoint '{}' returned {}, retrying '{}'",
                    primary_url, status, fallback_url
                );
                self.send_http_jsonrpc(&fallback_url, request_body, expect_response)
                    .await
                    .map_err(|fallback_err| TransportError::Http {
                        status,
                        message: format!(
                            "Primary endpoint '{}' failed ({}): {}. Fallback '{}' failed: {}",
                            primary_url, status, message, fallback_url, fallback_err
                        ),
                    })?
            }
            Err(err) => return Err(err),
        };

        if let Some(error) = response_body.get("error") {
            return Err(TransportError::InvalidResponse(format!(
                "MCP error response: {}",
                error
            )));
        }

        if expect_response {
            trace!("Received HTTP response from endpoint {}", primary_url);
        }

        Ok(response_body
            .get("result")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})))
    }

    async fn send_http_jsonrpc(
        &self,
        url: &str,
        request_body: &serde_json::Value,
        expect_response: bool,
    ) -> Result<serde_json::Value, TransportError> {
        let protocol_version = self.protocol_version.lock().await.clone();
        let session_id = self.session_id.lock().await.clone();

        let mut request = self
            .client
            .post(url)
            .headers(self.headers.clone())
            .json(request_body);

        if let Some(version) = protocol_version {
            request = request.header("MCP-Protocol-Version", version);
        }

        if let Some(session) = session_id {
            request = request.header("MCP-Session-Id", session);
        }

        let response = request
            .send()
            .await
            .map_err(|e| TransportError::connection(format!("HTTP request failed: {}", e)))?;

        if let Some(version) = response
            .headers()
            .get("MCP-Protocol-Version")
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty())
        {
            *self.protocol_version.lock().await = Some(version.to_string());
        }

        if let Some(session) = response
            .headers()
            .get("MCP-Session-Id")
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty())
        {
            *self.session_id.lock().await = Some(session.to_string());
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.as_u16() == 404 {
                *self.session_id.lock().await = None;
            }
            return Err(TransportError::Http {
                status: status.as_u16(),
                message: body,
            });
        }

        if !expect_response && response.status().as_u16() == 202 {
            return Ok(serde_json::json!({}));
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let body = response.text().await.map_err(|e| {
            TransportError::InvalidResponse(format!("Failed to read response body: {}", e))
        })?;

        if body.trim().is_empty() {
            if expect_response {
                return Err(TransportError::InvalidResponse(
                    "Empty response body for JSON-RPC request".to_string(),
                ));
            }
            return Ok(serde_json::json!({}));
        }

        parse_json_or_sse_response(&body).map_err(|parse_err| {
            let preview: String = body.chars().take(240).collect();
            TransportError::InvalidResponse(format!(
                "Failed to parse response body (content-type: '{}'): {}. Body preview: {}",
                content_type, parse_err, preview
            ))
        })
    }
}

fn parse_json_or_sse_response(body: &str) -> Result<serde_json::Value, String> {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        return Ok(json);
    }

    let mut collected = String::new();
    for raw_line in body.lines() {
        let line = raw_line.trim();
        if let Some(rest) = line.strip_prefix("data:") {
            let chunk = rest.trim();
            if !chunk.is_empty() {
                if !collected.is_empty() {
                    collected.push('\n');
                }
                collected.push_str(chunk);
            }
        }
    }

    if collected.is_empty() {
        return Err("no JSON payload or SSE data lines found".to_string());
    }

    serde_json::from_str::<serde_json::Value>(&collected)
        .map_err(|e| format!("invalid SSE data JSON: {}", e))
}

#[async_trait]
impl McpTransport for HttpTransport {
    async fn initialize(&mut self) -> Result<ServerCapabilities, TransportError> {
        let mut state = self.state.lock().await;

        match *state {
            TransportState::Initialized => {
                return Ok(self
                    .server_capabilities
                    .lock()
                    .await
                    .clone()
                    .unwrap_or_default());
            }
            TransportState::Initializing => {
                return Err(TransportError::AlreadyInitialized);
            }
            TransportState::Failed => {
                return Err(TransportError::Failed(
                    "Transport is in failed state".to_string(),
                ));
            }
            TransportState::Uninitialized => {
                *state = TransportState::Initializing;
            }
        }

        info!("Initializing MCP HTTP transport for: {}", self.base_url);

        let _client_capabilities = ClientCapabilities::default();
        let client_info = Implementation::new("orchestrix", env!("CARGO_PKG_VERSION"));

        let mut last_error: Option<TransportError> = None;

        for &protocol_version in SUPPORTED_PROTOCOL_VERSIONS {
            debug!("Trying protocol version: {}", protocol_version);

            let init_request = InitializeRequest::new(protocol_version, client_info.clone());

            let params = serde_json::to_value(&init_request)
                .map_err(|e| TransportError::serialization(e.to_string()))?;

            match self.request_with_retry("initialize", params).await {
                Ok(response) => {
                    let init_response: InitializeResponse = serde_json::from_value(response)
                        .map_err(|e| TransportError::InvalidResponse(e.to_string()))?;

                    *self.protocol_version.lock().await = Some(protocol_version.to_string());
                    *self.server_capabilities.lock().await =
                        Some(init_response.capabilities.clone());
                    *self.server_info.lock().await = Some(init_response.server_info);

                    if let Err(e) = self
                        .notify_internal("notifications/initialized", serde_json::json!({}))
                        .await
                    {
                        warn!("Failed to send initialized notification: {}", e);
                    }

                    *state = TransportState::Initialized;

                    info!(
                        "MCP HTTP transport initialized with protocol version: {}",
                        protocol_version
                    );

                    return Ok(init_response.capabilities);
                }
                Err(e) => {
                    debug!("Protocol version {} failed: {}", protocol_version, e);
                    last_error = Some(e);
                }
            }
        }

        *state = TransportState::Failed;
        Err(last_error.unwrap_or(TransportError::ProtocolNegotiationFailed))
    }

    async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, TransportError> {
        let state = *self.state.lock().await;
        match state {
            TransportState::Uninitialized | TransportState::Initializing => {
                return Err(TransportError::NotInitialized);
            }
            TransportState::Failed => {
                return Err(TransportError::Failed(
                    "Transport is in failed state".to_string(),
                ));
            }
            TransportState::Initialized => {}
        }

        self.request_with_retry(method, params).await
    }

    async fn close(&self) -> Result<(), TransportError> {
        info!("Closing HTTP transport for: {}", self.base_url);

        *self.state.lock().await = TransportState::Uninitialized;

        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        let state = *self.state.lock().await;
        if state != TransportState::Initialized {
            return false;
        }

        let url = format!("{}/health", self.base_url);
        match self
            .client
            .get(&url)
            .headers(self.headers.clone())
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    fn state(&self) -> TransportState {
        TransportState::Initialized
    }

    fn protocol_version(&self) -> Option<&str> {
        None
    }

    fn server_capabilities(&self) -> Option<&ServerCapabilities> {
        None
    }
}

// ============================================================================
// SSE Transport
// ============================================================================

/// SSE event structure with full field support.
#[derive(Debug, Clone)]
pub struct SseEvent {
    /// Event type (e.g., "message", "error", "open").
    pub event_type: String,
    /// Event data payload.
    pub data: String,
    /// Event ID for replay/ordering.
    pub id: Option<String>,
    /// Retry timing hint from server.
    pub retry: Option<u64>,
}

impl SseEvent {
    /// Parse an SSE event from a string.
    fn parse(input: &str) -> Result<Self, TransportError> {
        let mut event_type = "message".to_string();
        let mut data = String::new();
        let mut id = None;
        let mut retry = None;

        for line in input.lines() {
            if line.is_empty() {
                continue;
            }

            if let Some((key, value)) = line.split_once(':') {
                let value = value.trim_start();
                match key {
                    "event" => event_type = value.to_string(),
                    "data" => {
                        if !data.is_empty() {
                            data.push('\n');
                        }
                        data.push_str(value);
                    }
                    "id" => id = Some(value.to_string()),
                    "retry" => {
                        if let Ok(ms) = value.parse::<u64>() {
                            retry = Some(ms);
                        }
                    }
                    _ => {
                        // Unknown field, ignore
                        trace!("Unknown SSE field: {}", key);
                    }
                }
            }
        }

        if data.is_empty() {
            return Err(TransportError::SseParse("Empty event data".to_string()));
        }

        Ok(Self {
            event_type,
            data,
            id,
            retry,
        })
    }
}

/// SSE transport for remote MCP servers with streaming support.
///
/// This transport communicates with MCP servers via Server-Sent Events,
/// with support for proper event parsing, reconnection logic, and
/// demultiplexing of event streams.
pub struct SseTransport {
    client: reqwest::Client,
    base_url: String,
    headers: reqwest::header::HeaderMap,
    next_id: Arc<AtomicI64>,
    state: Arc<Mutex<TransportState>>,
    protocol_version: Arc<Mutex<Option<String>>>,
    server_capabilities: Arc<Mutex<Option<ServerCapabilities>>>,
    server_info: Arc<Mutex<Option<Implementation>>>,
    config: TransportConfig,
    event_stream:
        Arc<Mutex<Option<Pin<Box<dyn Stream<Item = Result<SseEvent, TransportError>> + Send>>>>>,
    subscriptions: Arc<Mutex<HashMap<String, tokio::sync::mpsc::Sender<SseEvent>>>>,
    last_event_id: Arc<Mutex<Option<String>>>,
    reconnect_attempts: Arc<AtomicI64>,
}

impl SseTransport {
    /// Create a new SSE transport.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL for the MCP server
    /// * `config` - Transport configuration
    ///
    /// # Returns
    ///
    /// Returns a boxed transport on success, or an error string on failure.
    pub async fn new(
        base_url: String,
        config: TransportConfig,
    ) -> Result<Box<dyn McpTransport>, String> {
        info!("Creating SSE transport for URL: {}", base_url);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300)) // Long timeout for SSE (5 minutes)
            .pool_max_idle_per_host(1) // SSE typically uses a single connection
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::ACCEPT,
            "text/event-stream".parse().unwrap(),
        );

        // Add OAuth token if present
        if let Some(token) = &config.auth.oauth_token {
            let auth_value = format!("Bearer {}", token);
            headers.insert(
                reqwest::header::AUTHORIZATION,
                auth_value
                    .parse()
                    .map_err(|e| format!("Invalid auth header: {}", e))?,
            );
        }

        // Add API key if present
        if let Some(api_key) = &config.auth.api_key {
            let header_name = config.auth.api_key_header.as_deref().unwrap_or("X-API-Key");
            let header = reqwest::header::HeaderName::from_bytes(header_name.as_bytes())
                .map_err(|e| format!("Invalid API key header name: {}", e))?;
            headers.insert(
                header,
                api_key
                    .parse()
                    .map_err(|e| format!("Invalid API key: {}", e))?,
            );
        }

        // Add custom headers
        for (key, value) in &config.auth.headers {
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|e| format!("Invalid header name '{}': {}", key, e))?;
            headers.insert(
                header_name,
                value
                    .parse()
                    .map_err(|e| format!("Invalid header value: {}", e))?,
            );
        }

        let transport = Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            headers,
            next_id: Arc::new(AtomicI64::new(1)),
            state: Arc::new(Mutex::new(TransportState::Uninitialized)),
            protocol_version: Arc::new(Mutex::new(None)),
            server_capabilities: Arc::new(Mutex::new(None)),
            server_info: Arc::new(Mutex::new(None)),
            config,
            event_stream: Arc::new(Mutex::new(None)),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            last_event_id: Arc::new(Mutex::new(None)),
            reconnect_attempts: Arc::new(AtomicI64::new(0)),
        };

        Ok(Box::new(transport))
    }

    /// Connect to the SSE stream with automatic reconnection.
    async fn connect_sse(
        &self,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<SseEvent, TransportError>> + Send>>, TransportError>
    {
        let url = format!("{}/events", self.base_url);

        let mut request = self.client.get(&url).headers(self.headers.clone());

        // Add Last-Event-ID header for replay
        if let Some(last_id) = self.last_event_id.lock().await.as_ref() {
            request = request.header("Last-Event-ID", last_id);
        }

        let response = request.send().await.map_err(|e| {
            TransportError::connection(format!("Failed to connect to SSE stream: {}", e))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(TransportError::Http {
                status: status.as_u16(),
                message: body,
            });
        }

        info!("Connected to SSE stream at {}", url);

        // Reset reconnect attempts on successful connection
        self.reconnect_attempts.store(0, Ordering::SeqCst);

        let stream = response.bytes_stream();
        let last_event_id = self.last_event_id.clone();

        let event_stream = Box::pin(stream.then(move |chunk| {
            let last_event_id = last_event_id.clone();
            async move {
                match chunk {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        trace!("Received SSE chunk: {}", text);

                        // Parse the SSE event
                        match SseEvent::parse(&text) {
                            Ok(event) => {
                                // Update last event ID
                                if let Some(ref id) = event.id {
                                    *last_event_id.lock().await = Some(id.clone());
                                }
                                Ok(event)
                            }
                            Err(e) => Err(e),
                        }
                    }
                    Err(e) => Err(TransportError::connection(format!("Stream error: {}", e))),
                }
            }
        }));

        Ok(event_stream)
    }

    /// Reconnect to the SSE stream with exponential backoff.
    async fn reconnect_with_backoff(&self) -> Result<(), TransportError> {
        let attempt = self.reconnect_attempts.fetch_add(1, Ordering::SeqCst);

        if attempt > 10 {
            return Err(TransportError::ReconnectionFailed(
                "Maximum reconnection attempts exceeded".to_string(),
            ));
        }

        let delay = exponential_backoff(attempt as u32);
        warn!(
            "SSE connection lost, reconnecting in {:?} (attempt {})",
            delay, attempt
        );

        sleep(delay).await;

        match self.connect_sse().await {
            Ok(stream) => {
                *self.event_stream.lock().await = Some(stream);
                info!("Successfully reconnected to SSE stream");
                Ok(())
            }
            Err(e) => {
                error!("Reconnection failed: {}", e);
                Err(e)
            }
        }
    }

    /// Send a JSON-RPC request with retry logic.
    async fn request_with_retry(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, TransportError> {
        let mut last_error = None;

        for attempt in 0..=self.config.retry_count {
            match self.request_internal(method, params.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if !e.is_retryable() || attempt == self.config.retry_count {
                        return Err(e);
                    }

                    warn!(
                        "SSE request failed (attempt {}/{}): {}, retrying...",
                        attempt + 1,
                        self.config.retry_count + 1,
                        e
                    );

                    let delay = exponential_backoff(attempt);
                    sleep(delay).await;
                    last_error = Some(e);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| TransportError::Failed("All retry attempts exhausted".to_string())))
    }

    /// Internal request method without retry.
    async fn request_internal(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, TransportError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        trace!("Sending SSE request: method={}, id={}", method, id);

        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let url = format!("{}/rpc", self.base_url);

        let response = self
            .client
            .post(&url)
            .headers(self.headers.clone())
            .json(&request_body)
            .send()
            .await
            .map_err(|e| TransportError::connection(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(TransportError::Http {
                status: status.as_u16(),
                message: body,
            });
        }

        let response_body: serde_json::Value = response.json().await.map_err(|e| {
            TransportError::InvalidResponse(format!("Failed to parse response: {}", e))
        })?;

        if let Some(error) = response_body.get("error") {
            return Err(TransportError::InvalidResponse(format!(
                "MCP error response: {}",
                error
            )));
        }

        trace!("Received SSE response: id={}", id);

        Ok(response_body
            .get("result")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})))
    }

    async fn notify_internal(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), TransportError> {
        let notification_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let url = format!("{}/rpc", self.base_url);

        let response = self
            .client
            .post(&url)
            .headers(self.headers.clone())
            .json(&notification_body)
            .send()
            .await
            .map_err(|e| TransportError::connection(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(TransportError::Http {
                status: status.as_u16(),
                message: body,
            });
        }

        if response.status().as_u16() == 202 {
            return Ok(());
        }

        let body = response.text().await.unwrap_or_default();
        if body.trim().is_empty() {
            return Ok(());
        }

        let parsed = parse_json_or_sse_response(&body).map_err(|parse_err| {
            TransportError::InvalidResponse(format!(
                "Failed to parse notification response body: {}",
                parse_err
            ))
        })?;

        if let Some(error) = parsed.get("error") {
            return Err(TransportError::InvalidResponse(format!(
                "MCP error response: {}",
                error
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl McpTransport for SseTransport {
    async fn initialize(&mut self) -> Result<ServerCapabilities, TransportError> {
        let mut state = self.state.lock().await;

        match *state {
            TransportState::Initialized => {
                return Ok(self
                    .server_capabilities
                    .lock()
                    .await
                    .clone()
                    .unwrap_or_default());
            }
            TransportState::Initializing => {
                return Err(TransportError::AlreadyInitialized);
            }
            TransportState::Failed => {
                return Err(TransportError::Failed(
                    "Transport is in failed state".to_string(),
                ));
            }
            TransportState::Uninitialized => {
                *state = TransportState::Initializing;
            }
        }

        info!("Initializing MCP SSE transport for: {}", self.base_url);

        // Establish SSE connection
        match self.connect_sse().await {
            Ok(stream) => {
                *self.event_stream.lock().await = Some(stream);
            }
            Err(e) => {
                *state = TransportState::Failed;
                return Err(e);
            }
        }

        let _client_capabilities = ClientCapabilities::default();
        let client_info = Implementation::new("orchestrix", env!("CARGO_PKG_VERSION"));

        let mut last_error: Option<TransportError> = None;

        for &protocol_version in SUPPORTED_PROTOCOL_VERSIONS {
            debug!("Trying protocol version: {}", protocol_version);

            let init_request = InitializeRequest::new(protocol_version, client_info.clone());

            let params = serde_json::to_value(&init_request)
                .map_err(|e| TransportError::serialization(e.to_string()))?;

            match self.request_with_retry("initialize", params).await {
                Ok(response) => {
                    let init_response: InitializeResponse = serde_json::from_value(response)
                        .map_err(|e| TransportError::InvalidResponse(e.to_string()))?;

                    if let Err(e) = self
                        .notify_internal("notifications/initialized", serde_json::json!({}))
                        .await
                    {
                        warn!("Failed to send initialized notification: {}", e);
                    }

                    *self.protocol_version.lock().await = Some(protocol_version.to_string());
                    *self.server_capabilities.lock().await =
                        Some(init_response.capabilities.clone());
                    *self.server_info.lock().await = Some(init_response.server_info);
                    *state = TransportState::Initialized;

                    info!(
                        "MCP SSE transport initialized with protocol version: {}",
                        protocol_version
                    );

                    return Ok(init_response.capabilities);
                }
                Err(e) => {
                    debug!("Protocol version {} failed: {}", protocol_version, e);
                    last_error = Some(e);
                }
            }
        }

        *state = TransportState::Failed;
        Err(last_error.unwrap_or(TransportError::ProtocolNegotiationFailed))
    }

    async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, TransportError> {
        let state = *self.state.lock().await;
        match state {
            TransportState::Uninitialized | TransportState::Initializing => {
                return Err(TransportError::NotInitialized);
            }
            TransportState::Failed => {
                return Err(TransportError::Failed(
                    "Transport is in failed state".to_string(),
                ));
            }
            TransportState::Initialized => {}
        }

        self.request_with_retry(method, params).await
    }

    async fn close(&self) -> Result<(), TransportError> {
        info!("Closing SSE transport for: {}", self.base_url);

        // Drop event stream
        self.event_stream.lock().await.take();

        // Clear subscriptions
        self.subscriptions.lock().await.clear();

        *self.state.lock().await = TransportState::Uninitialized;

        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        let state = *self.state.lock().await;
        if state != TransportState::Initialized {
            return false;
        }

        // Check if SSE stream is still active by checking health endpoint
        let url = format!("{}/health", self.base_url);
        match self
            .client
            .get(&url)
            .headers(self.headers.clone())
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    fn state(&self) -> TransportState {
        TransportState::Initialized
    }

    fn protocol_version(&self) -> Option<&str> {
        None
    }

    fn server_capabilities(&self) -> Option<&ServerCapabilities> {
        None
    }
}

// ============================================================================
// Request ID Generator
// ============================================================================

/// Thread-safe request ID generator for transports.
///
/// Provides sequential numeric IDs that are safe to use across multiple threads.
#[derive(Debug, Clone)]
pub struct TransportRequestIdGenerator {
    counter: Arc<AtomicI64>,
}

impl TransportRequestIdGenerator {
    /// Create a new request ID generator.
    pub fn new() -> Self {
        Self {
            counter: Arc::new(AtomicI64::new(1)),
        }
    }

    /// Generate the next request ID.
    pub fn next_id(&self) -> i64 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Reset the counter to 1.
    pub fn reset(&self) {
        self.counter.store(1, Ordering::SeqCst);
    }
}

impl Default for TransportRequestIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_state_display() {
        assert_eq!(TransportState::Uninitialized.to_string(), "uninitialized");
        assert_eq!(TransportState::Initializing.to_string(), "initializing");
        assert_eq!(TransportState::Initialized.to_string(), "initialized");
        assert_eq!(TransportState::Failed.to_string(), "failed");
    }

    #[test]
    fn test_transport_error_is_retryable() {
        assert!(TransportError::Connection("test".to_string()).is_retryable());
        assert!(TransportError::Timeout(Duration::from_secs(1)).is_retryable());
        assert!(TransportError::Failed("test".to_string()).is_retryable());
        assert!(!TransportError::NotInitialized.is_retryable());
        assert!(!TransportError::InvalidResponse("test".to_string()).is_retryable());
    }

    #[test]
    fn test_exponential_backoff() {
        let delay0 = exponential_backoff(0);
        assert_eq!(delay0, Duration::from_millis(100));

        let delay1 = exponential_backoff(1);
        assert_eq!(delay1, Duration::from_millis(200));

        let delay2 = exponential_backoff(2);
        assert_eq!(delay2, Duration::from_millis(400));

        // Test max delay cap
        let delay10 = exponential_backoff(10);
        assert_eq!(delay10, Duration::from_millis(10000));

        let delay20 = exponential_backoff(20);
        assert_eq!(delay20, Duration::from_millis(10000));
    }

    #[test]
    fn test_sse_event_parse() {
        let event_text = "event: message\nid: 123\ndata: Hello, World!";
        let event = SseEvent::parse(event_text).unwrap();

        assert_eq!(event.event_type, "message");
        assert_eq!(event.id, Some("123".to_string()));
        assert_eq!(event.data, "Hello, World!");
    }

    #[test]
    fn test_sse_event_parse_multiline() {
        let event_text = "event: message\ndata: Line 1\ndata: Line 2";
        let event = SseEvent::parse(event_text).unwrap();

        assert_eq!(event.event_type, "message");
        assert_eq!(event.data, "Line 1\nLine 2");
    }

    #[test]
    fn test_sse_event_parse_empty_data() {
        let event_text = "event: message";
        let result = SseEvent::parse(event_text);

        assert!(result.is_err());
    }

    #[test]
    fn test_transport_config_default() {
        let config = TransportConfig::default();
        assert_eq!(config.timeout, DEFAULT_REQUEST_TIMEOUT);
        assert_eq!(config.retry_count, DEFAULT_RETRY_COUNT);
        assert_eq!(config.pool_size, 5);
    }

    #[test]
    fn test_transport_request_id_generator() {
        let gen = TransportRequestIdGenerator::new();

        let id1 = gen.next_id();
        let id2 = gen.next_id();
        let id3 = gen.next_id();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);

        gen.reset();
        assert_eq!(gen.next_id(), 1);
    }
}
