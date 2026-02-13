//! JSON-RPC 2.0 client implementation for MCP.
//!
//! This module provides a robust JSON-RPC client that works with the transport layer
//! to provide type-safe request/response handling, proper error types, timeout support,
//! and concurrent request management.
//!
//! # Example
//! ```rust,ignore
//! use orchestrix_lib::mcp::jsonrpc::{JsonRpcClient, RequestIdGenerator};
//! use orchestrix_lib::mcp::transport::{McpTransport, StdioTransport};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = StdioTransport::new(
//!     "mcp-server".to_string(),
//!     vec![],
//!     std::collections::HashMap::new(),
//!     None,
//!     Default::default(),
//! )?;
//!
//! let mut client = JsonRpcClient::new(transport);
//!
//! // Make a request and wait for response
//! let result: serde_json::Value = client.call("tools/list", serde_json::json!({})).await?;
//!
//! // Send a notification (fire-and-forget)
//! client.notify("notifications/initialized", serde_json::json!({})).await?;
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio::time::timeout;

use super::transport::McpTransport;
use super::types::{JsonRpcError as RpcError, JsonRpcResponse, RequestId};

/// Default timeout for JSON-RPC requests.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Thread-safe request ID generator using atomic counter.
///
/// Provides sequential numeric IDs that are safe to use across multiple threads.
/// IDs start at 1 and increment monotonically.
#[derive(Debug, Clone)]
pub struct RequestIdGenerator {
    counter: Arc<AtomicU64>,
    prefix: Option<String>,
}

impl RequestIdGenerator {
    /// Create a new request ID generator with numeric IDs starting at 1.
    pub fn new() -> Self {
        Self {
            counter: Arc::new(AtomicU64::new(1)),
            prefix: None,
        }
    }

    /// Create a new request ID generator with a string prefix.
    ///
    /// Generated IDs will be in the format "{prefix}-{counter}".
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            counter: Arc::new(AtomicU64::new(1)),
            prefix: Some(prefix.into()),
        }
    }

    /// Generate the next request ID.
    ///
    /// If a prefix was set, returns a string ID in the format "{prefix}-{counter}".
    /// Otherwise, returns a numeric ID.
    pub fn next_id(&self) -> RequestId {
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        
        match &self.prefix {
            Some(prefix) => RequestId::String(format!("{}-{}", prefix, counter)),
            None => RequestId::Number(counter as i64),
        }
    }

    /// Get the current counter value without incrementing.
    pub fn current(&self) -> u64 {
        self.counter.load(Ordering::SeqCst)
    }

    /// Reset the counter to 1.
    pub fn reset(&self) {
        self.counter.store(1, Ordering::SeqCst);
    }
}

impl Default for RequestIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during JSON-RPC client communication.
#[derive(Error, Debug, Clone)]
pub enum RpcClientError {
    /// Transport-level error (connection issues, I/O errors).
    #[error("Transport error: {0}")]
    Transport(String),

    /// Request timed out.
    #[error("Request timed out after {duration:?}")]
    Timeout {
        /// The duration that was waited.
        duration: Duration,
    },

    /// Failed to parse the response.
    #[error("Parse error: {message}")]
    Parse {
        /// Description of what failed to parse.
        message: String,
    },

    /// Server returned a JSON-RPC error.
    #[error("Server error {code}: {message}")]
    Server {
        /// JSON-RPC error code.
        code: i32,
        /// Error message.
        message: String,
        /// Optional additional error data.
        data: Option<serde_json::Value>,
    },

    /// Request was cancelled.
    #[error("Request cancelled")]
    Cancelled,

    /// Client is closed.
    #[error("Client is closed")]
    Closed,
}

impl RpcClientError {
    /// Create a transport error from any error type.
    pub fn transport<E: fmt::Display>(err: E) -> Self {
        RpcClientError::Transport(err.to_string())
    }

    /// Create a parse error.
    pub fn parse<E: fmt::Display>(err: E) -> Self {
        RpcClientError::Parse {
            message: err.to_string(),
        }
    }

    /// Check if this error is a timeout.
    pub fn is_timeout(&self) -> bool {
        matches!(self, RpcClientError::Timeout { .. })
    }

    /// Check if this error is a cancellation.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, RpcClientError::Cancelled)
    }

    /// Check if this error indicates the client is closed.
    pub fn is_closed(&self) -> bool {
        matches!(self, RpcClientError::Closed)
    }

    /// Get the server error code if this is a server error.
    pub fn server_code(&self) -> Option<i32> {
        match self {
            RpcClientError::Server { code, .. } => Some(*code),
            _ => None,
        }
    }

    /// Check if this is a server error with a specific code.
    pub fn has_code(&self, code: i32) -> bool {
        self.server_code() == Some(code)
    }
}

impl From<RpcError> for RpcClientError {
    fn from(err: RpcError) -> Self {
        RpcClientError::Server {
            code: err.code,
            message: err.message,
            data: err.data,
        }
    }
}

/// Result type alias for JSON-RPC operations.
pub type JsonRpcResult<T> = Result<T, RpcClientError>;

/// JSON-RPC 2.0 client for MCP communication.
///
/// Provides a high-level interface for making JSON-RPC requests over any
/// transport implementation. Handles request serialization, response parsing,
/// error handling, and timeouts.
///
/// # Thread Safety
///
/// The client is thread-safe and can be shared across tasks. It uses interior
/// mutability for the transport, allowing concurrent requests.
pub struct JsonRpcClient {
    transport: Box<dyn McpTransport>,
    id_generator: RequestIdGenerator,
    timeout: Duration,
    closed: bool,
}

impl JsonRpcClient {
    /// Create a new JSON-RPC client with the given transport.
    ///
    /// Uses the default request timeout of 30 seconds.
    ///
    /// # Example
    /// ```rust,ignore
    /// let client = JsonRpcClient::new(transport);
    /// ```
    pub fn new(transport: Box<dyn McpTransport>) -> Self {
        Self {
            transport,
            id_generator: RequestIdGenerator::new(),
            timeout: DEFAULT_REQUEST_TIMEOUT,
            closed: false,
        }
    }

    /// Create a new JSON-RPC client with a custom timeout.
    ///
    /// # Example
    /// ```rust,ignore
    /// let client = JsonRpcClient::with_timeout(transport, Duration::from_secs(60));
    /// ```
    pub fn with_timeout(transport: Box<dyn McpTransport>, timeout: Duration) -> Self {
        Self {
            transport,
            id_generator: RequestIdGenerator::new(),
            timeout,
            closed: false,
        }
    }

    /// Create a new JSON-RPC client with a custom ID generator.
    ///
    /// Useful when you need specific ID formats (e.g., prefixed IDs).
    ///
    /// # Example
    /// ```rust,ignore
    /// let id_gen = RequestIdGenerator::with_prefix("req");
    /// let client = JsonRpcClient::with_id_generator(transport, id_gen);
    /// ```
    pub fn with_id_generator(
        transport: Box<dyn McpTransport>,
        id_generator: RequestIdGenerator,
    ) -> Self {
        Self {
            transport,
            id_generator,
            timeout: DEFAULT_REQUEST_TIMEOUT,
            closed: false,
        }
    }

    /// Create a new client with full customization.
    pub fn with_options(
        transport: Box<dyn McpTransport>,
        id_generator: RequestIdGenerator,
        timeout: Duration,
    ) -> Self {
        Self {
            transport,
            id_generator,
            timeout,
            closed: false,
        }
    }

    /// Get the current timeout setting.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Set a new timeout for future requests.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Get a reference to the ID generator.
    pub fn id_generator(&self) -> &RequestIdGenerator {
        &self.id_generator
    }

    /// Check if the client has been closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Make a JSON-RPC request and wait for a response.
    ///
    /// # Type Parameters
    ///
    /// - `T`: The type of the request parameters. Must implement `Serialize`.
    /// - `R`: The type of the response result. Must implement `DeserializeOwned`.
    ///
    /// # Arguments
    ///
    /// - `method`: The JSON-RPC method name to call.
    /// - `params`: The parameters to pass to the method.
    ///
    /// # Returns
    ///
    /// Returns `Ok(R)` on success, or `Err(RpcClientError)` on failure.
    ///
    /// # Errors
    ///
    /// Can return:
    /// - `RpcClientError::Transport` - If there's a transport-level error
    /// - `RpcClientError::Timeout` - If the request times out
    /// - `RpcClientError::Parse` - If the response cannot be parsed
    /// - `RpcClientError::Server` - If the server returns an error
    /// - `RpcClientError::Closed` - If the client has been closed
    ///
    /// # Example
    /// ```rust,ignore
    /// #[derive(Serialize)]
    /// struct ListToolsParams {
    ///     cursor: Option<String>,
    /// }
    ///
    /// #[derive(Deserialize)]
    /// struct ListToolsResult {
    ///     tools: Vec<Tool>,
    /// }
    ///
    /// let params = ListToolsParams { cursor: None };
    /// let result: ListToolsResult = client.call("tools/list", params).await?;
    /// ```
    pub async fn call<T, R>(&self, method: &str, params: T) -> JsonRpcResult<R>
    where
        T: Serialize,
        R: DeserializeOwned,
    {
        if self.closed {
            return Err(RpcClientError::Closed);
        }

        // Serialize parameters
        let params_value = serde_json::to_value(params)
            .map_err(|e| RpcClientError::parse(format!("Failed to serialize params: {}", e)))?;

        // Make the request with timeout
        let result = timeout(self.timeout, self.transport.request(method, params_value))
            .await
            .map_err(|_| RpcClientError::Timeout { duration: self.timeout })?;

        // Handle transport result
        let response_value = result.map_err(RpcClientError::transport)?;

        // Parse the response
        let response: JsonRpcResponse = serde_json::from_value(response_value)
            .map_err(|e| RpcClientError::parse(format!("Failed to parse response: {}", e)))?;

        // Check for server error
        if let Some(error) = response.error {
            return Err(RpcClientError::Server {
                code: error.code,
                message: error.message,
                data: error.data,
            });
        }

        // Extract and deserialize result
        let result_value = response.result.ok_or_else(|| {
            RpcClientError::parse("Response missing result field")
        })?;

        serde_json::from_value(result_value)
            .map_err(|e| RpcClientError::parse(format!("Failed to deserialize result: {}", e)))
    }

    /// Make a JSON-RPC request with raw JSON parameters.
    ///
    /// This is a convenience method for when you already have the parameters
    /// as a `serde_json::Value`.
    ///
    /// # Arguments
    ///
    /// - `method`: The JSON-RPC method name to call.
    /// - `params`: The parameters as a JSON value.
    ///
    /// # Returns
    ///
    /// Returns `Ok(R)` on success, or `Err(RpcClientError)` on failure.
    pub async fn call_raw<R>(&self, method: &str, params: serde_json::Value) -> JsonRpcResult<R>
    where
        R: DeserializeOwned,
    {
        if self.closed {
            return Err(RpcClientError::Closed);
        }

        // Make the request with timeout
        let result = timeout(self.timeout, self.transport.request(method, params))
            .await
            .map_err(|_| RpcClientError::Timeout { duration: self.timeout })?;

        // Handle transport result
        let response_value = result.map_err(RpcClientError::transport)?;

        // Parse the response
        let response: JsonRpcResponse = serde_json::from_value(response_value)
            .map_err(|e| RpcClientError::parse(format!("Failed to parse response: {}", e)))?;

        // Check for server error
        if let Some(error) = response.error {
            return Err(RpcClientError::Server {
                code: error.code,
                message: error.message,
                data: error.data,
            });
        }

        // Extract and deserialize result
        let result_value = response.result.ok_or_else(|| {
            RpcClientError::parse("Response missing result field")
        })?;

        serde_json::from_value(result_value)
            .map_err(|e| RpcClientError::parse(format!("Failed to deserialize result: {}", e)))
    }

    /// Send a JSON-RPC notification (fire-and-forget).
    ///
    /// Notifications do not expect a response and are typically used for
    /// one-way communication like status updates or signals.
    ///
    /// # Arguments
    ///
    /// - `method`: The JSON-RPC method name.
    /// - `params`: The notification parameters.
    ///
    /// # Errors
    ///
    /// Can return:
    /// - `RpcClientError::Transport` - If there's a transport-level error
    /// - `RpcClientError::Timeout` - If the notification times out
    /// - `RpcClientError::Closed` - If the client has been closed
    ///
    /// # Example
    /// ```rust,ignore
    /// client.notify("notifications/initialized", serde_json::json!({})).await?;
    /// ```
    pub async fn notify<T>(&self, method: &str, params: T) -> JsonRpcResult<()>
    where
        T: Serialize,
    {
        if self.closed {
            return Err(RpcClientError::Closed);
        }

        // Serialize parameters
        let params_value = serde_json::to_value(params)
            .map_err(|e| RpcClientError::parse(format!("Failed to serialize params: {}", e)))?;

        // Send the notification with timeout
        // We use the same request method since transports typically handle
        // notifications the same way but without waiting for a response
        timeout(self.timeout, self.transport.request(method, params_value))
            .await
            .map_err(|_| RpcClientError::Timeout { duration: self.timeout })?
            .map_err(RpcClientError::transport)?;

        Ok(())
    }

    /// Send a notification with raw JSON parameters.
    ///
    /// # Arguments
    ///
    /// - `method`: The JSON-RPC method name.
    /// - `params`: The parameters as a JSON value.
    pub async fn notify_raw(&self, method: &str, params: serde_json::Value) -> JsonRpcResult<()> {
        if self.closed {
            return Err(RpcClientError::Closed);
        }

        timeout(self.timeout, self.transport.request(method, params))
            .await
            .map_err(|_| RpcClientError::Timeout { duration: self.timeout })?
            .map_err(RpcClientError::transport)?;

        Ok(())
    }

    /// Close the client and its transport.
    ///
    /// After closing, all subsequent requests will return `RpcClientError::Closed`.
    pub async fn close(&mut self) -> JsonRpcResult<()> {
        if self.closed {
            return Ok(());
        }

        self.transport
            .close()
            .await
            .map_err(RpcClientError::transport)?;

        self.closed = true;
        Ok(())
    }

    /// Check if the underlying transport is healthy.
    pub async fn is_healthy(&self) -> bool {
        if self.closed {
            return false;
        }

        self.transport.is_healthy().await
    }
}

impl fmt::Debug for JsonRpcClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsonRpcClient")
            .field("timeout", &self.timeout)
            .field("closed", &self.closed)
            .field("id_generator", &self.id_generator)
            .finish_non_exhaustive()
    }
}

/// Builder for constructing JSON-RPC clients with custom options.
///
/// # Example
/// ```rust,ignore
/// let client = JsonRpcClientBuilder::new(transport)
///     .timeout(Duration::from_secs(60))
///     .id_generator(RequestIdGenerator::with_prefix("mcp"))
///     .build();
/// ```
pub struct JsonRpcClientBuilder {
    transport: Box<dyn McpTransport>,
    id_generator: Option<RequestIdGenerator>,
    timeout: Option<Duration>,
}

impl JsonRpcClientBuilder {
    /// Create a new builder with the given transport.
    pub fn new(transport: Box<dyn McpTransport>) -> Self {
        Self {
            transport,
            id_generator: None,
            timeout: None,
        }
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the ID generator.
    pub fn id_generator(mut self, generator: RequestIdGenerator) -> Self {
        self.id_generator = Some(generator);
        self
    }

    /// Build the client with the configured options.
    pub fn build(self) -> JsonRpcClient {
        JsonRpcClient::with_options(
            self.transport,
            self.id_generator.unwrap_or_default(),
            self.timeout.unwrap_or(DEFAULT_REQUEST_TIMEOUT),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_generator_numeric() {
        let gen = RequestIdGenerator::new();
        
        let id1 = gen.next_id();
        let id2 = gen.next_id();
        let id3 = gen.next_id();
        
        assert_eq!(id1, RequestId::Number(1));
        assert_eq!(id2, RequestId::Number(2));
        assert_eq!(id3, RequestId::Number(3));
    }

    #[test]
    fn test_request_id_generator_prefixed() {
        let gen = RequestIdGenerator::with_prefix("req");
        
        let id1 = gen.next_id();
        let id2 = gen.next_id();
        
        assert_eq!(id1, RequestId::String("req-1".to_string()));
        assert_eq!(id2, RequestId::String("req-2".to_string()));
    }

    #[test]
    fn test_request_id_generator_reset() {
        let gen = RequestIdGenerator::new();
        
        let _ = gen.next_id();
        let _ = gen.next_id();
        assert_eq!(gen.current(), 3);
        
        gen.reset();
        assert_eq!(gen.current(), 1);
        
        let id = gen.next_id();
        assert_eq!(id, RequestId::Number(1));
    }

    #[test]
    fn test_request_id_generator_thread_safety() {
        use std::thread;
        
        let gen = RequestIdGenerator::new();
        let mut handles = vec![];
        
        for _ in 0..10 {
            let gen_clone = gen.clone();
            let handle = thread::spawn(move || {
                let mut ids = vec![];
                for _ in 0..100 {
                    ids.push(gen_clone.next_id());
                }
                ids
            });
            handles.push(handle);
        }
        
        let all_ids: Vec<_> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();
        
        // Check all IDs are unique
        let unique_count = all_ids.len();
        let set_count = all_ids.into_iter().collect::<std::collections::HashSet<_>>().len();
        assert_eq!(unique_count, set_count);
    }

    #[test]
    fn test_json_rpc_error_variants() {
        let transport_err = RpcClientError::transport("connection refused");
        assert!(matches!(transport_err, RpcClientError::Transport(_)));
        
        let timeout_err = RpcClientError::Timeout { duration: Duration::from_secs(30) };
        assert!(timeout_err.is_timeout());
        
        let server_err = RpcClientError::Server {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        };
        assert_eq!(server_err.server_code(), Some(-32601));
        assert!(server_err.has_code(-32601));
    }

    #[test]
    fn test_json_rpc_error_from_rpc_error() {
        let rpc_err = RpcError {
            code: -32602,
            message: "Invalid params".to_string(),
            data: Some(serde_json::json!({"field": "name"})),
        };
        
        let err: RpcClientError = rpc_err.into();
        match err {
            RpcClientError::Server { code, message, data } => {
                assert_eq!(code, -32602);
                assert_eq!(message, "Invalid params");
                assert!(data.is_some());
            }
            _ => panic!("Expected Server error variant"),
        }
    }

    #[test]
    fn test_client_builder_exists() {
        // This test verifies the builder type and methods exist
        // We can't actually create a transport without a real process
        
        // Just verify the methods exist by taking their function pointers
        let _: fn(Box<dyn McpTransport>) -> JsonRpcClientBuilder = JsonRpcClientBuilder::new;
        let _: fn(JsonRpcClientBuilder, Duration) -> JsonRpcClientBuilder = JsonRpcClientBuilder::timeout;
        let _: fn(JsonRpcClientBuilder, RequestIdGenerator) -> JsonRpcClientBuilder = JsonRpcClientBuilder::id_generator;
        let _: fn(JsonRpcClientBuilder) -> JsonRpcClient = JsonRpcClientBuilder::build;
    }
}
