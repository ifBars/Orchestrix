// src-tauri/tests/common/mod.rs
//! Common test utilities for MCP integration tests.

pub mod mock_mcp_server;
pub mod mock_transport;

pub use mock_mcp_server::MockMcpServer;
pub use mock_transport::MockTransport;
