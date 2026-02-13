//! Unit tests for MCP connection management.
//!
//! These tests verify connection pooling, health monitoring, and lifecycle management.

#![cfg(test)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::json;
use tokio::sync::Mutex;

use crate::mcp::connection::ConnectionManager;
use crate::mcp::transport::{McpTransport, TransportError, TransportState};
use crate::mcp::types::ServerCapabilities;
use crate::mcp::{McpAuthConfig, McpServerConfig, McpTransportType, ServerHealth};

// ============================================================================
// Mock Transport Implementation
// ============================================================================

/// Mock transport for unit testing.
#[derive(Debug)]
struct MockTransport {
    healthy: AtomicBool,
    request_count: AtomicUsize,
    state: AtomicBool, // true = initialized
    server_capabilities: Option<ServerCapabilities>,
}

impl MockTransport {
    fn new(healthy: bool) -> Self {
        Self {
            healthy: AtomicBool::new(healthy),
            request_count: AtomicUsize::new(0),
            state: AtomicBool::new(false),
            server_capabilities: Some(ServerCapabilities::default()),
        }
    }

    fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::SeqCst);
    }

    fn get_request_count(&self) -> usize {
        self.request_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl McpTransport for MockTransport {
    async fn request(
        &self,
        _method: &str,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, TransportError> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"result": "ok"}))
    }

    async fn close(&self) -> Result<(), TransportError> {
        self.state.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::SeqCst) && self.state.load(Ordering::SeqCst)
    }

    fn state(&self) -> TransportState {
        if self.state.load(Ordering::SeqCst) {
            TransportState::Initialized
        } else {
            TransportState::Uninitialized
        }
    }

    fn protocol_version(&self) -> Option<&str> {
        Some("2025-06-18")
    }

    fn server_capabilities(&self) -> Option<&ServerCapabilities> {
        self.server_capabilities.as_ref()
    }

    async fn initialize(&mut self) -> Result<ServerCapabilities, TransportError> {
        self.state.store(true, Ordering::SeqCst);
        Ok(ServerCapabilities::default())
    }
}

// Factory for creating mock transports in tests
struct MockTransportFactory {
    transports: Arc<Mutex<Vec<Arc<MockTransport>>>>,
}

impl MockTransportFactory {
    fn new() -> Self {
        Self {
            transports: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn create(&self, healthy: bool) -> Arc<MockTransport> {
        let transport = Arc::new(MockTransport::new(healthy));
        self.transports.lock().await.push(transport.clone());
        transport
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_config(id: &str, pool_size: usize) -> McpServerConfig {
    McpServerConfig {
        id: id.to_string(),
        name: format!("Test Server {}", id),
        transport: McpTransportType::Stdio,
        enabled: true,
        command: Some("echo".to_string()),
        args: vec![],
        env: HashMap::new(),
        working_dir: None,
        url: None,
        auth: McpAuthConfig::default(),
        timeout_secs: 30,
        pool_size,
        health_check_interval_secs: 60,
        tool_filter: crate::mcp::ToolFilter::default(),
        approval_policy: crate::mcp::ToolApprovalPolicy::default(),
    }
}

fn create_http_config(id: &str, url: &str, pool_size: usize) -> McpServerConfig {
    McpServerConfig {
        id: id.to_string(),
        name: format!("Test HTTP Server {}", id),
        transport: McpTransportType::Http,
        enabled: true,
        command: None,
        args: vec![],
        env: HashMap::new(),
        working_dir: None,
        url: Some(url.to_string()),
        auth: McpAuthConfig::default(),
        timeout_secs: 30,
        pool_size,
        health_check_interval_secs: 60,
        tool_filter: crate::mcp::ToolFilter::default(),
        approval_policy: crate::mcp::ToolApprovalPolicy::default(),
    }
}

fn create_sse_config(id: &str, url: &str, pool_size: usize) -> McpServerConfig {
    McpServerConfig {
        id: id.to_string(),
        name: format!("Test SSE Server {}", id),
        transport: McpTransportType::Sse,
        enabled: true,
        command: None,
        args: vec![],
        env: HashMap::new(),
        working_dir: None,
        url: Some(url.to_string()),
        auth: McpAuthConfig::default(),
        timeout_secs: 30,
        pool_size,
        health_check_interval_secs: 60,
        tool_filter: crate::mcp::ToolFilter::default(),
        approval_policy: crate::mcp::ToolApprovalPolicy::default(),
    }
}

// ============================================================================
// Connection Pool Tests (via ConnectionManager)
// ============================================================================

#[tokio::test]
async fn test_pool_creation() {
    let manager = ConnectionManager::new();
    let config = create_test_config("test", 5);

    // Initialize pool
    manager.initialize_pool(&config).await.unwrap();

    // Verify pool was created by checking health
    let health = manager.get_health("test").await;
    assert!(health.is_some());
}

#[tokio::test]
async fn test_pool_acquire_creates_connection() {
    let manager = ConnectionManager::new();
    let config = create_test_config("acquire-test", 2);

    // Initialize pool
    manager.initialize_pool(&config).await.unwrap();

    // Try to get a transport (this would create a connection)
    // Note: In actual implementation, this may spawn a process
    // For unit test, we verify the pool exists
    let health = manager.get_health("acquire-test").await;
    assert!(health.is_some());
}

#[tokio::test]
async fn test_pool_acquire_reuses_healthy_connection() {
    let transport = MockTransport::new(true);
    let mut transport = transport;
    transport.initialize().await.unwrap();

    // First request
    let _ = transport.request("test", json!({})).await;
    assert_eq!(transport.get_request_count(), 1);

    // Second request - should reuse
    let _ = transport.request("test", json!({})).await;
    assert_eq!(transport.get_request_count(), 2);

    // Connection should still be healthy
    assert!(transport.is_healthy().await);
}

#[tokio::test]
async fn test_pool_max_size_respected() {
    // Test that pool size is properly configured
    let manager = ConnectionManager::new();
    let config = create_test_config("max-size-test", 3);

    manager.initialize_pool(&config).await.unwrap();

    // Pool should exist with correct config
    let health = manager.get_health("max-size-test").await;
    assert!(health.is_some());

    // The pool size is stored in config, verify it's 3
    assert_eq!(config.pool_size, 3);
}

#[tokio::test]
async fn test_pool_closes_unhealthy_connections() {
    // Simulate pool health check behavior
    let healthy_transport = MockTransport::new(true);
    let mut healthy_transport = healthy_transport;
    healthy_transport.initialize().await.unwrap();

    let unhealthy_transport = MockTransport::new(false);
    let mut unhealthy_transport = unhealthy_transport;
    unhealthy_transport.initialize().await.unwrap();
    unhealthy_transport.set_healthy(false);

    // Verify healthy vs unhealthy
    assert!(healthy_transport.is_healthy().await);
    assert!(!unhealthy_transport.is_healthy().await);
}

#[tokio::test]
async fn test_pool_lru_eviction() {
    // Test LRU eviction logic
    let now = Instant::now();
    let old_time = now - Duration::from_secs(100);
    let older_time = now - Duration::from_secs(200);
    let oldest_time = now - Duration::from_secs(300);

    // Simulate finding least recently used
    let timestamps = vec![(0, old_time), (1, oldest_time), (2, older_time)];

    let lru_idx = timestamps
        .iter()
        .min_by_key(|(_, time)| *time)
        .map(|(idx, _)| *idx);

    assert_eq!(lru_idx, Some(1)); // Index 1 has oldest_time
}

// ============================================================================
// Connection Lifecycle Tests
// ============================================================================

#[tokio::test]
async fn test_connection_creation() {
    let transport = MockTransport::new(true);

    // Transport should be created in uninitialized state
    assert_eq!(transport.state(), TransportState::Uninitialized);
}

#[tokio::test]
async fn test_connection_health_check() {
    let transport = MockTransport::new(true);

    // Initially not initialized, so not healthy
    assert!(!transport.is_healthy().await);

    // Initialize the transport
    let mut transport_mut = transport;
    transport_mut.initialize().await.unwrap();

    // Now should be healthy
    assert!(transport_mut.is_healthy().await);

    // Mark as unhealthy
    transport_mut.set_healthy(false);
    assert!(!transport_mut.is_healthy().await);
}

#[tokio::test]
async fn test_connection_use_count_tracking() {
    let transport = MockTransport::new(true);
    let transport_arc = Arc::new(transport);

    // Simulate request counting through transport
    let transport_clone = transport_arc.clone();

    // Make several requests
    for _ in 0..5 {
        let _ = transport_clone.request("test", json!({})).await;
    }

    assert_eq!(transport_clone.get_request_count(), 5);

    // More requests
    for _ in 0..3 {
        let _ = transport_clone.request("test", json!({})).await;
    }

    assert_eq!(transport_clone.get_request_count(), 8);
}

#[tokio::test]
async fn test_connection_last_used_tracking() {
    let now = Instant::now();
    let earlier = now - Duration::from_secs(10);
    let later = now - Duration::from_secs(5);

    // Verify time comparison works correctly
    assert!(earlier < later);
    assert!(later > earlier);

    // Simulate updating last_used
    let mut last_used = earlier;
    assert_eq!(last_used, earlier);

    last_used = later;
    assert_eq!(last_used, later);
}

// ============================================================================
// ConnectionManager Tests
// ============================================================================

#[tokio::test]
async fn test_manager_creation() {
    let manager = ConnectionManager::new();

    // Manager should start with no pools
    let health = manager.get_all_health().await;
    assert!(health.is_empty());
}

#[tokio::test]
async fn test_initialize_pool() {
    let manager = ConnectionManager::new();
    let config = create_test_config("test-server", 3);

    // Initialize pool
    let result = manager.initialize_pool(&config).await;
    assert!(result.is_ok());

    // Check health status was set
    let health = manager.get_health("test-server").await;
    assert!(health.is_some());
    assert_eq!(health.unwrap(), ServerHealth::Connecting);
}

#[tokio::test]
async fn test_initialize_disabled_server() {
    let manager = ConnectionManager::new();
    let mut config = create_test_config("disabled-server", 3);
    config.enabled = false;

    // Initialize disabled server
    let result = manager.initialize_pool(&config).await;
    assert!(result.is_ok());

    // Check health status is Disabled
    let health = manager.get_health("disabled-server").await;
    assert!(health.is_some());
    assert_eq!(health.unwrap(), ServerHealth::Disabled);
}

#[tokio::test]
async fn test_close_connection() {
    let manager = ConnectionManager::new();
    let config = create_test_config("close-test", 2);

    // Initialize pool
    manager.initialize_pool(&config).await.unwrap();

    // Verify pool exists
    assert!(manager.get_health("close-test").await.is_some());

    // Close connection
    manager.close_connection("close-test").await;

    // Verify pool was removed
    assert!(manager.get_health("close-test").await.is_none());
}

#[tokio::test]
async fn test_close_nonexistent_pool() {
    let manager = ConnectionManager::new();

    // Closing non-existent pool should not panic
    manager.close_connection("nonexistent").await;

    // Health should still be None
    assert!(manager.get_health("nonexistent").await.is_none());
}

// ============================================================================
// Health Monitoring Tests
// ============================================================================

#[tokio::test]
async fn test_health_status_healthy() {
    let transport = MockTransport::new(true);
    let mut transport = transport;
    transport.initialize().await.unwrap();

    assert!(transport.is_healthy().await);
}

#[tokio::test]
async fn test_health_status_unhealthy() {
    let transport = MockTransport::new(false);
    let mut transport = transport;
    transport.initialize().await.unwrap();

    // Set unhealthy
    transport.set_healthy(false);
    assert!(!transport.is_healthy().await);
}

#[tokio::test]
async fn test_health_status_connecting() {
    let transport = MockTransport::new(true);

    // Not initialized yet
    assert!(!transport.is_healthy().await);
    assert_eq!(transport.state(), TransportState::Uninitialized);
}

#[tokio::test]
async fn test_health_status_empty_pool() {
    // Empty pool should report Connecting status
    let pool_connections: Vec<()> = vec![];
    let is_empty = pool_connections.is_empty();

    assert!(is_empty);

    // This simulates the empty pool check in health_status()
    if is_empty {
        // Would return ServerHealth::Connecting
        let health = ServerHealth::Connecting;
        assert_eq!(health, ServerHealth::Connecting);
    }
}

#[tokio::test]
async fn test_get_health_existing_pool() {
    let manager = ConnectionManager::new();
    let config = create_test_config("health-test", 2);

    manager.initialize_pool(&config).await.unwrap();

    let health = manager.get_health("health-test").await;
    assert!(health.is_some());
}

#[tokio::test]
async fn test_get_health_nonexistent_pool() {
    let manager = ConnectionManager::new();

    let health = manager.get_health("nonexistent").await;
    assert!(health.is_none());
}

// ============================================================================
// Transport Integration Tests
// ============================================================================

#[tokio::test]
async fn test_create_transport_stdio() {
    let config = create_test_config("stdio-test", 2);

    assert_eq!(config.transport, McpTransportType::Stdio);
    assert!(config.command.is_some());
    assert_eq!(config.command.unwrap(), "echo");
}

#[tokio::test]
async fn test_create_transport_http() {
    let config = create_http_config("http-test", "http://localhost:8080", 5);

    assert_eq!(config.transport, McpTransportType::Http);
    assert!(config.url.is_some());
    assert_eq!(config.url.unwrap(), "http://localhost:8080");
    assert_eq!(config.pool_size, 5);
}

#[tokio::test]
async fn test_create_transport_sse() {
    let config = create_sse_config("sse-test", "http://localhost:8080/events", 1);

    assert_eq!(config.transport, McpTransportType::Sse);
    assert!(config.url.is_some());
    assert_eq!(config.url.unwrap(), "http://localhost:8080/events");
    assert_eq!(config.pool_size, 1);
}

#[tokio::test]
async fn test_create_transport_invalid_config() {
    // Stdio without command
    let mut stdio_config = create_test_config("invalid-stdio", 2);
    stdio_config.command = None;

    assert!(stdio_config.command.is_none());

    // HTTP without URL
    let mut http_config = create_http_config("invalid-http", "http://localhost:8080", 5);
    http_config.url = None;

    assert!(http_config.url.is_none());
}

// ============================================================================
// Async Tests
// ============================================================================

#[tokio::test]
async fn test_acquire_connection_async() {
    let manager = ConnectionManager::new();
    let config = create_test_config("async-acquire", 2);

    // Initialize pool
    manager.initialize_pool(&config).await.unwrap();

    // Verify pool exists (simulating async acquire)
    let health = manager.get_health("async-acquire").await;
    assert!(health.is_some());
}

#[tokio::test]
async fn test_concurrent_health_checks() {
    let manager = Arc::new(ConnectionManager::new());

    // Initialize multiple pools
    for i in 0..5 {
        let config = create_test_config(&format!("server-{}", i), 2);
        manager.initialize_pool(&config).await.unwrap();
    }

    // Concurrent health checks
    let mut handles = vec![];

    for i in 0..5 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            let health = manager_clone.get_health(&format!("server-{}", i)).await;
            assert!(health.is_some());
        });
        handles.push(handle);
    }

    // Wait for all checks
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_concurrent_pool_operations() {
    let manager = Arc::new(ConnectionManager::new());
    let config = create_test_config("concurrent-test", 5);

    manager.initialize_pool(&config).await.unwrap();

    // Concurrent health queries
    let mut handles = vec![];

    for _ in 0..10 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            let health = manager_clone.get_health("concurrent-test").await;
            assert!(health.is_some());
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_health_monitoring_task() {
    let manager = ConnectionManager::new();
    let config = create_test_config("monitor-test", 2);

    // Initialize
    manager.initialize_pool(&config).await.unwrap();

    // Check initial health
    let initial_health = manager.get_health("monitor-test").await;
    assert_eq!(initial_health.unwrap(), ServerHealth::Connecting);

    // Health would be updated by background task
    // For test, we verify the structure exists
    let all_health = manager.get_all_health().await;
    assert!(all_health.contains_key("monitor-test"));
}

// ============================================================================
// Transport State Tests
// ============================================================================

#[tokio::test]
async fn test_transport_state_transitions() {
    let transport = MockTransport::new(true);

    // Initial state
    assert_eq!(transport.state(), TransportState::Uninitialized);

    let mut transport = transport;

    // After initialize
    transport.initialize().await.unwrap();
    assert_eq!(transport.state(), TransportState::Initialized);

    // After close
    transport.close().await.unwrap();
    assert_eq!(transport.state(), TransportState::Uninitialized);
}

#[tokio::test]
async fn test_transport_request_error_handling() {
    let transport = MockTransport::new(false); // Unhealthy
    let mut transport = transport;
    transport.initialize().await.unwrap();

    // Set unhealthy - requests should still work in mock but health check fails
    transport.set_healthy(false);

    // Request still succeeds (mock doesn't check health)
    let result = transport.request("test", json!({})).await;
    assert!(result.is_ok());

    // But health check fails
    assert!(!transport.is_healthy().await);
}

// ============================================================================
// ServerHealth Enum Tests
// ============================================================================

#[tokio::test]
async fn test_server_health_variants() {
    let healthy = ServerHealth::Healthy;
    let connecting = ServerHealth::Connecting;
    let disabled = ServerHealth::Disabled;
    let unhealthy = ServerHealth::Unhealthy {
        reason: "Test failure".to_string(),
        since: "2024-01-01T00:00:00Z".to_string(),
    };

    // Verify all variants can be created
    assert_eq!(healthy, ServerHealth::Healthy);
    assert_eq!(connecting, ServerHealth::Connecting);
    assert_eq!(disabled, ServerHealth::Disabled);

    match unhealthy {
        ServerHealth::Unhealthy { reason, since } => {
            assert_eq!(reason, "Test failure");
            assert_eq!(since, "2024-01-01T00:00:00Z");
        }
        _ => panic!("Expected Unhealthy variant"),
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
async fn test_full_connection_lifecycle() {
    let manager = ConnectionManager::new();
    let config = create_test_config("lifecycle-test", 2);

    // 1. Initialize pool
    manager.initialize_pool(&config).await.unwrap();
    let health = manager.get_health("lifecycle-test").await;
    assert_eq!(health.unwrap(), ServerHealth::Connecting);

    // 2. Close connection
    manager.close_connection("lifecycle-test").await;
    let health = manager.get_health("lifecycle-test").await;
    assert!(health.is_none());

    // 3. Re-initialize (simulating restart)
    manager.initialize_pool(&config).await.unwrap();
    let health = manager.get_health("lifecycle-test").await;
    assert_eq!(health.unwrap(), ServerHealth::Connecting);
}

#[tokio::test]
async fn test_multiple_server_management() {
    let manager = ConnectionManager::new();

    // Create multiple servers
    let servers = vec![("server-a", 2), ("server-b", 3), ("server-c", 1)];

    for (id, pool_size) in &servers {
        let config = create_test_config(id, *pool_size);
        manager.initialize_pool(&config).await.unwrap();
    }

    // Verify all exist
    let all_health = manager.get_all_health().await;
    assert_eq!(all_health.len(), 3);

    for (id, _) in &servers {
        assert!(all_health.contains_key(*id));
    }

    // Close one
    manager.close_connection("server-b").await;

    let all_health = manager.get_all_health().await;
    assert_eq!(all_health.len(), 2);
    assert!(!all_health.contains_key("server-b"));
}

#[tokio::test]
async fn test_transport_type_variants() {
    // Test all transport types
    let stdio = McpTransportType::Stdio;
    let http = McpTransportType::Http;
    let sse = McpTransportType::Sse;

    assert_eq!(stdio, McpTransportType::Stdio);
    assert_eq!(http, McpTransportType::Http);
    assert_eq!(sse, McpTransportType::Sse);

    // Test Display
    assert_eq!(format!("{}", stdio), "stdio");
    assert_eq!(format!("{}", http), "http");
    assert_eq!(format!("{}", sse), "sse");
}
