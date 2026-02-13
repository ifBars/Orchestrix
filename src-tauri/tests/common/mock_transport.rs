// src-tauri/tests/common/mock_transport.rs
//! Mock transport implementation for MCP integration testing.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use orchestrix_lib::mcp::transport::{McpTransport, TransportError, TransportState};
use orchestrix_lib::mcp::types::*;
use orchestrix_lib::mcp::ServerCapabilities;

use super::mock_mcp_server::MockMcpServer;

/// A mock transport that connects to a MockMcpServer.
pub struct MockTransport {
    server: Arc<MockMcpServer>,
    state: Arc<Mutex<TransportState>>,
    protocol_version: Arc<Mutex<Option<String>>>,
    server_capabilities: Arc<Mutex<Option<ServerCapabilities>>>,
    should_fail_connection: Arc<Mutex<bool>>,
    connection_error_message: Arc<Mutex<String>>,
}

impl MockTransport {
    /// Create a new mock transport with the given server.
    pub fn new(server: MockMcpServer) -> Self {
        Self {
            server: Arc::new(server),
            state: Arc::new(Mutex::new(TransportState::Uninitialized)),
            protocol_version: Arc::new(Mutex::new(None)),
            server_capabilities: Arc::new(Mutex::new(None)),
            should_fail_connection: Arc::new(Mutex::new(false)),
            connection_error_message: Arc::new(Mutex::new("Connection failed".to_string())),
        }
    }

    /// Set whether the transport should fail connections.
    pub async fn set_should_fail_connection(&self, should_fail: bool, message: impl Into<String>) {
        *self.should_fail_connection.lock().await = should_fail;
        *self.connection_error_message.lock().await = message.into();
    }

    /// Create a boxed transport for use with McpClient.
    pub fn boxed(self) -> Box<dyn McpTransport> {
        Box::new(self)
    }
}

#[async_trait]
impl McpTransport for MockTransport {
    async fn initialize(&mut self) -> Result<ServerCapabilities, TransportError> {
        let mut state = self.state.lock().await;

        if *state == TransportState::Initialized {
            return Ok(self
                .server_capabilities
                .lock()
                .await
                .clone()
                .unwrap_or_default());
        }

        if *self.should_fail_connection.lock().await {
            *state = TransportState::Failed;
            return Err(TransportError::Connection(
                self.connection_error_message.lock().await.clone(),
            ));
        }

        let init_request = JsonRpcRequest::new(
            RequestId::Number(1),
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            })),
        );

        let response = self.server.handle_request(init_request).await;

        if let Some(error) = response.error {
            *state = TransportState::Failed;
            return Err(TransportError::Connection(error.message));
        }

        let result: serde_json::Value = response.result.unwrap_or_else(|| serde_json::json!({}));
        let capabilities: ServerCapabilities = serde_json::from_value(
            result
                .get("capabilities")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        )
        .map_err(|e| TransportError::InvalidResponse(e.to_string()))?;

        *self.protocol_version.lock().await = result
            .get("protocolVersion")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        *self.server_capabilities.lock().await = Some(capabilities.clone());

        let notification =
            JsonRpcRequest::notification("notifications/initialized", Some(serde_json::json!({})));
        let _ = self.server.handle_request(notification).await;

        *state = TransportState::Initialized;
        Ok(capabilities)
    }

    async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, TransportError> {
        if *self.should_fail_connection.lock().await {
            return Err(TransportError::Connection(
                self.connection_error_message.lock().await.clone(),
            ));
        }

        if method != "initialize" {
            let state = *self.state.lock().await;
            if state == TransportState::Uninitialized {
                return Err(TransportError::Connection(
                    "Transport not initialized".to_string(),
                ));
            }
            if state == TransportState::Failed {
                return Err(TransportError::Connection(
                    "Transport is in failed state".to_string(),
                ));
            }
        }

        let request = JsonRpcRequest::new(
            RequestId::Number(1),
            method,
            if params.is_object() {
                Some(params)
            } else {
                None
            },
        );

        let response = self.server.handle_request(request).await;

        if let Some(error) = response.error {
            if method == "initialize" {
                *self.state.lock().await = TransportState::Failed;
            }
            return Err(TransportError::Connection(error.message));
        }

        if method == "initialize" {
            if let Ok(result) = serde_json::from_value::<serde_json::Value>(
                response.result.clone().unwrap_or_default(),
            ) {
                if let Ok(caps) = serde_json::from_value::<ServerCapabilities>(
                    result.get("capabilities").cloned().unwrap_or_default(),
                ) {
                    *self.server_capabilities.lock().await = Some(caps);
                }
                if let Some(version) = result.get("protocolVersion").and_then(|v| v.as_str()) {
                    *self.protocol_version.lock().await = Some(version.to_string());
                }
            }
            *self.state.lock().await = TransportState::Initialized;
        }

        Ok(serde_json::to_value(&response).unwrap_or_else(|_| serde_json::json!({})))
    }

    async fn close(&self) -> Result<(), TransportError> {
        let mut state = self.state.lock().await;
        *state = TransportState::Uninitialized;
        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        let state = *self.state.lock().await;
        state == TransportState::Initialized
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
