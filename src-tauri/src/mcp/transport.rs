//! MCP transport implementations.
//!
//! This module provides transport layer abstractions for different MCP server types:
//! - StdioTransport: For local MCP servers via stdio
//! - HttpTransport: For remote MCP servers via HTTP
//! - SseTransport: For remote MCP servers via Server-Sent Events

#![allow(dead_code)]

use std::collections::HashMap;
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

use super::McpAuthConfig;

/// Configuration for MCP transports.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Request timeout.
    pub timeout: Duration,
    /// Authentication configuration.
    pub auth: McpAuthConfig,
}

/// Trait for MCP transports.
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Send a JSON-RPC request and return the response.
    async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
    
    /// Close the transport connection.
    async fn close(&self) -> Result<(), String>;
    
    /// Check if the transport is healthy.
    async fn is_healthy(&self) -> bool;
}

/// Stdio transport for local MCP servers.
pub struct StdioTransport {
    process: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<tokio::process::ChildStdin>>,
    stdout: Arc<Mutex<BufReader<tokio::process::ChildStdout>>>,
    next_id: Arc<AtomicI64>,
    initialized: Arc<Mutex<bool>>,
}

impl StdioTransport {
    /// Create a new stdio transport.
    pub fn new(
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        working_dir: Option<String>,
        _config: TransportConfig,
    ) -> Result<Box<dyn McpTransport>, String> {
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
        
        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to spawn MCP process '{}': {}", command, e))?;
        
        let stdout = child.stdout.take()
            .ok_or_else(|| "Failed to capture stdout".to_string())?;
        let stdin = child.stdin.take()
            .ok_or_else(|| "Failed to capture stdin".to_string())?;
        
        let transport = Self {
            process: Arc::new(Mutex::new(child)),
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: Arc::new(Mutex::new(BufReader::new(stdout))),
            next_id: Arc::new(AtomicI64::new(1)),
            initialized: Arc::new(Mutex::new(false)),
        };
        
        Ok(Box::new(transport))
    }
    
    /// Perform MCP initialization handshake.
    async fn initialize(&self) -> Result<(), String> {
        let mut initialized = self.initialized.lock().await;
        if *initialized {
            return Ok(());
        }
        
        let protocol_versions = ["2024-11-05", "2024-10-07"];
        let mut last_error: Option<String> = None;

        for protocol_version in protocol_versions {
            let init_result = self
                .request_internal(
                    "initialize",
                    serde_json::json!({
                        "protocolVersion": protocol_version,
                        "capabilities": {},
                        "clientInfo": {
                            "name": "orchestrix",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }),
                )
                .await;

            match init_result {
                Ok(_) => {
                    self.notify("notifications/initialized", serde_json::json!({})).await?;
                    *initialized = true;
                    return Ok(());
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }

        Err(format!(
            "failed to initialize MCP server with supported protocol versions: {}",
            last_error.unwrap_or_else(|| "unknown initialization error".to_string())
        ))
    }
    
    /// Send a JSON-RPC request without initialization check.
    async fn request_internal(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        
        self.write_message(&request).await?;
        
        // Read responses until we find one with matching ID
        loop {
            let response = self.read_message().await?;
            
            // Check if this is the response we're waiting for
            if let Some(response_id) = response.get("id").and_then(|v| v.as_i64()) {
                if response_id == id {
                    if let Some(error) = response.get("error") {
                        return Err(format!("MCP error: {}", error));
                    }
                    return Ok(response.get("result").cloned().unwrap_or_else(|| serde_json::json!({})));
                }
            }
            
            // This might be a notification, skip it
        }
    }
    
    /// Send a JSON-RPC notification.
    async fn notify(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), String> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        
        self.write_message(&notification).await
    }
    
    /// Write a message to the process stdin.
    async fn write_message(
        &self,
        message: &serde_json::Value,
    ) -> Result<(), String> {
        let body = serde_json::to_vec(message)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;
        
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(header.as_bytes()).await
            .map_err(|e| format!("Failed to write header: {}", e))?;
        stdin.write_all(&body).await
            .map_err(|e| format!("Failed to write body: {}", e))?;
        stdin.flush().await
            .map_err(|e| format!("Failed to flush: {}", e))?;
        
        Ok(())
    }
    
    /// Read a message from the process stdout.
    async fn read_message(&self) -> Result<serde_json::Value, String> {
        let mut stdout = self.stdout.lock().await;
        let mut content_length: Option<usize> = None;
        
        // Read headers
        loop {
            let mut line = String::new();
            match stdout.read_line(&mut line).await {
                Ok(0) => return Err("Unexpected EOF while reading headers".to_string()),
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        // End of headers
                        break;
                    }
                    
                    if let Some((key, value)) = trimmed.split_once(':') {
                        if key.eq_ignore_ascii_case("Content-Length") {
                            content_length = Some(value.trim().parse()
                                .map_err(|e| format!("Invalid Content-Length: {}", e))?);
                        }
                    }
                }
                Err(e) => return Err(format!("Failed to read header: {}", e)),
            }
        }
        
        let length = content_length.ok_or_else(|| "Missing Content-Length header".to_string())?;
        
        // Read body
        let mut buffer = vec![0u8; length];
        // Read from the BufReader behind the MutexGuard
        use tokio::io::AsyncReadExt;
        AsyncReadExt::read_exact(&mut *stdout, &mut buffer).await
            .map_err(|e| format!("Failed to read body: {}", e))?;
        
        serde_json::from_slice(&buffer)
            .map_err(|e| format!("Failed to parse JSON: {}", e))
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Ensure initialized
        self.initialize().await?;
        
        self.request_internal(method, params).await
    }
    
    async fn close(&self) -> Result<(), String> {
        let mut process = self.process.lock().await;
        let _ = process.start_kill();
        let _ = process.wait().await;
        Ok(())
    }
    
    async fn is_healthy(&self) -> bool {
        let mut process = self.process.lock().await;
        process.try_wait().ok().flatten().is_none()
    }
}

/// HTTP transport for remote MCP servers.
pub struct HttpTransport {
    client: reqwest::Client,
    base_url: String,
    headers: reqwest::header::HeaderMap,
    next_id: Arc<AtomicI64>,
}

impl HttpTransport {
    /// Create a new HTTP transport.
    pub async fn new(
        base_url: String,
        config: TransportConfig,
    ) -> Result<Box<dyn McpTransport>, String> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        
        let mut headers = reqwest::header::HeaderMap::new();
        
        // Add OAuth token if present
        if let Some(token) = &config.auth.oauth_token {
            let auth_value = format!("Bearer {}", token);
            headers.insert(
                reqwest::header::AUTHORIZATION,
                auth_value.parse().map_err(|e| format!("Invalid auth header: {}", e))?,
            );
        }
        
        // Add API key if present
        if let Some(api_key) = &config.auth.api_key {
            let header_name = config.auth.api_key_header.as_deref().unwrap_or("X-API-Key");
            let header = reqwest::header::HeaderName::from_bytes(header_name.as_bytes())
                .map_err(|e| format!("Invalid API key header name: {}", e))?;
            headers.insert(header, api_key.parse().map_err(|e| format!("Invalid API key: {}", e))?);
        }
        
        // Add custom headers
        for (key, value) in &config.auth.headers {
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|e| format!("Invalid header name '{}': {}", key, e))?;
            headers.insert(header_name, value.parse().map_err(|e| format!("Invalid header value: {}", e))?);
        }
        
        let transport = Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            headers,
            next_id: Arc::new(AtomicI64::new(1)),
        };
        
        Ok(Box::new(transport))
    }
}

#[async_trait]
impl McpTransport for HttpTransport {
    async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        
        let url = format!("{}/rpc", self.base_url);
        
        let response = self.client
            .post(&url)
            .headers(self.headers.clone())
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("HTTP error {}: {}", status, body));
        }
        
        let response_body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        
        if let Some(error) = response_body.get("error") {
            return Err(format!("MCP error: {}", error));
        }
        
        Ok(response_body.get("result").cloned().unwrap_or_else(|| serde_json::json!({})))
    }
    
    async fn close(&self) -> Result<(), String> {
        // HTTP transport doesn't maintain persistent connections
        Ok(())
    }
    
    async fn is_healthy(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).headers(self.headers.clone()).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}

/// SSE transport for remote MCP servers with streaming support.
pub struct SseTransport {
    client: reqwest::Client,
    base_url: String,
    headers: reqwest::header::HeaderMap,
    next_id: Arc<AtomicI64>,
    event_stream: Arc<Mutex<Option<Pin<Box<dyn Stream<Item = Result<SseEvent, String>> + Send>>>>>,
}

/// SSE event structure.
#[derive(Debug, Clone)]
struct SseEvent {
    event_type: String,
    data: String,
    id: Option<String>,
}

impl SseTransport {
    /// Create a new SSE transport.
    pub async fn new(
        base_url: String,
        config: TransportConfig,
    ) -> Result<Box<dyn McpTransport>, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300)) // Long timeout for SSE (5 minutes)
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
                auth_value.parse().map_err(|e| format!("Invalid auth header: {}", e))?,
            );
        }
        
        // Add API key if present
        if let Some(api_key) = &config.auth.api_key {
            let header_name = config.auth.api_key_header.as_deref().unwrap_or("X-API-Key");
            let header = reqwest::header::HeaderName::from_bytes(header_name.as_bytes())
                .map_err(|e| format!("Invalid API key header name: {}", e))?;
            headers.insert(header, api_key.parse().map_err(|e| format!("Invalid API key: {}", e))?);
        }
        
        // Add custom headers
        for (key, value) in &config.auth.headers {
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|e| format!("Invalid header name '{}': {}", key, e))?;
            headers.insert(header_name, value.parse().map_err(|e| format!("Invalid header value: {}", e))?);
        }
        
        let transport = Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            headers,
            next_id: Arc::new(AtomicI64::new(1)),
            event_stream: Arc::new(Mutex::new(None)),
        };
        
        Ok(Box::new(transport))
    }
    
    /// Parse SSE stream into events.
    fn parse_sse_stream(
        &self,
        response: reqwest::Response,
    ) -> Pin<Box<dyn Stream<Item = Result<SseEvent, String>> + Send>> {
        let stream = response.bytes_stream();
        
        Box::pin(stream.then(|chunk| async move {
            match chunk {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    // Parse SSE format
                    // This is a simplified parser - real implementation would be more robust
                    Ok(SseEvent {
                        event_type: "message".to_string(),
                        data: text.to_string(),
                        id: None,
                    })
                }
                Err(e) => Err(format!("Stream error: {}", e)),
            }
        }))
    }
}

#[async_trait]
impl McpTransport for SseTransport {
    async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        
        let url = format!("{}/rpc", self.base_url);
        
        let response = self.client
            .post(&url)
            .headers(self.headers.clone())
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("HTTP error {}: {}", status, body));
        }
        
        let response_body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        
        if let Some(error) = response_body.get("error") {
            return Err(format!("MCP error: {}", error));
        }
        
        Ok(response_body.get("result").cloned().unwrap_or_else(|| serde_json::json!({})))
    }
    
    async fn close(&self) -> Result<(), String> {
        // Close SSE connection
        Ok(())
    }
    
    async fn is_healthy(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).headers(self.headers.clone()).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}
