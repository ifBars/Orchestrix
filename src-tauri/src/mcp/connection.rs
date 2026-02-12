//! MCP connection management with health monitoring and pooling.
//!
//! This module provides:
//! - Connection pooling for MCP transports
//! - Health monitoring and automatic reconnection
//! - Runtime statistics tracking

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::time::interval;

use super::transport::{McpTransport, TransportConfig};
use super::{McpServerConfig, McpTransportType, ServerHealth};

/// A pooled MCP connection with metadata.
struct PooledConnection {
    transport: Box<dyn McpTransport>,
    created_at: Instant,
    last_used: Instant,
    use_count: u64,
}

/// Connection pool for a single MCP server.
struct ConnectionPool {
    config: McpServerConfig,
    connections: Vec<PooledConnection>,
    max_size: usize,
}

impl ConnectionPool {
    fn new(config: McpServerConfig) -> Self {
        let max_size = config.pool_size;
        Self {
            config,
            connections: Vec::with_capacity(max_size),
            max_size,
        }
    }
    
    /// Get a connection from the pool or create a new one.
    async fn acquire(&mut self,
    ) -> Result<&mut PooledConnection, String> {
        // Check health of all connections sequentially
        let mut healthy_idx: Option<usize> = None;
        for (idx, conn) in self.connections.iter().enumerate() {
            if conn.transport.is_healthy().await {
                healthy_idx = Some(idx);
                break;
            }
        }
        
        // Use existing healthy connection
        if let Some(idx) = healthy_idx {
            let conn = &mut self.connections[idx];
            conn.last_used = Instant::now();
            conn.use_count += 1;
            return Ok(conn);
        }
        
        // Create a new connection if under limit
        if self.connections.len() < self.max_size {
            let transport = create_transport(&self.config).await?;
            let conn = PooledConnection {
                transport,
                created_at: Instant::now(),
                last_used: Instant::now(),
                use_count: 1,
            };
            self.connections.push(conn);
            let idx = self.connections.len() - 1;
            return Ok(&mut self.connections[idx]);
        }
        
        // Pool is exhausted, try to reuse the least recently used connection
        if let Some(idx) = self.connections.iter()
            .enumerate()
            .min_by_key(|(_, c)| c.last_used)
            .map(|(idx, _)| idx)
        {
            // Close and recreate this connection
            let _ = self.connections[idx].transport.close().await;
            let transport = create_transport(&self.config).await?;
            self.connections[idx] = PooledConnection {
                transport,
                created_at: Instant::now(),
                last_used: Instant::now(),
                use_count: 1,
            };
            return Ok(&mut self.connections[idx]);
        }
        
        Err("Connection pool exhausted and cannot create new connection".to_string())
    }
    
    /// Return health status of the pool.
    async fn health_status(&self) -> ServerHealth {
        if self.connections.is_empty() {
            return ServerHealth::Connecting;
        }
        
        let mut healthy_count = 0;
        for conn in &self.connections {
            if conn.transport.is_healthy().await {
                healthy_count += 1;
            }
        }
        
        if healthy_count > 0 {
            ServerHealth::Healthy
        } else {
            ServerHealth::Unhealthy {
                reason: "All connections in pool are unhealthy".to_string(),
                since: chrono::Utc::now().to_rfc3339(),
            }
        }
    }
    
    /// Close all connections in the pool.
    async fn close_all(&mut self) {
        for conn in &mut self.connections {
            let _ = conn.transport.close().await;
        }
        self.connections.clear();
    }
}

/// Global connection manager for all MCP servers.
pub struct ConnectionManager {
    pools: Arc<Mutex<HashMap<String, ConnectionPool>>>,
    health_status: Arc<Mutex<HashMap<String, ServerHealth>>>,
}

impl ConnectionManager {
    /// Create a new connection manager.
    pub fn new() -> Self {
        Self {
            pools: Arc::new(Mutex::new(HashMap::new())),
            health_status: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Initialize connection pool for a server.
    pub async fn initialize_pool(
        &self,
        server_config: &McpServerConfig,
    ) -> Result<(), String> {
        if !server_config.enabled {
            self.health_status.lock().await.insert(
                server_config.id.clone(),
                ServerHealth::Disabled,
            );
            return Ok(());
        }
        
        let pool = ConnectionPool::new(server_config.clone());
        
        {
            let mut pools = self.pools.lock().await;
            pools.insert(server_config.id.clone(), pool);
        }
        
        self.health_status.lock().await.insert(
            server_config.id.clone(),
            ServerHealth::Connecting,
        );
        
        Ok(())
    }
    
    /// Get a transport for the specified server.
    pub async fn get_transport(
        &self,
        server_id: &str,
    ) -> Result<Box<dyn McpTransport>, String> {
        let pools = self.pools.lock().await;
        let pool = pools.get(server_id)
            .ok_or_else(|| format!("No connection pool for server: {}", server_id))?;
        
        // For simplicity, we'll create a one-off transport
        // In production, you'd want proper connection pooling
        let config = pool.config.clone();
        drop(pools);
        
        create_transport(&config).await
    }
    
    /// Execute a request on a server's connection.
    pub async fn execute_request(
        &self,
        server_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let mut pools = self.pools.lock().await;
        let pool = pools.get_mut(server_id)
            .ok_or_else(|| format!("No connection pool for server: {}", server_id))?;
        
        let conn = pool.acquire().await?;
        
        conn.transport.request(method, params).await
    }
    
    /// Close all connections for a server.
    pub async fn close_connection(&self, server_id: &str) {
        let mut pools = self.pools.lock().await;
        if let Some(pool) = pools.remove(server_id) {
            let mut pool = pool;
            pool.close_all().await;
        }
        
        let mut health = self.health_status.lock().await;
        health.remove(server_id);
    }
    
    /// Get health status for a server.
    pub async fn get_health(&self, server_id: &str) -> Option<ServerHealth> {
        self.health_status.lock().await.get(server_id).cloned()
    }
    
    /// Update health status for a server.
    async fn update_health(&self, server_id: &str, health: ServerHealth) {
        self.health_status.lock().await.insert(server_id.to_string(), health);
    }
    
    /// Start health monitoring background task.
    pub async fn start_health_monitoring(&self, check_interval: Duration) {
        let pools = self.pools.clone();
        let health_status = self.health_status.clone();
        
        tokio::spawn(async move {
            let mut ticker = interval(check_interval);
            
            loop {
                ticker.tick().await;
                
                let server_ids: Vec<String> = {
                    let pools_guard = pools.lock().await;
                    pools_guard.keys().cloned().collect()
                };
                
                for server_id in server_ids {
                    let health = {
                        let pools_guard = pools.lock().await;
                        if let Some(pool) = pools_guard.get(&server_id) {
                            pool.health_status().await
                        } else {
                            continue;
                        }
                    };
                    
                    let mut health_guard = health_status.lock().await;
                    health_guard.insert(server_id, health);
                }
            }
        });
    }
    
    /// Get all health statuses.
    pub async fn get_all_health(&self) -> HashMap<String, ServerHealth> {
        self.health_status.lock().await.clone()
    }
}

/// Create a transport for the given server configuration.
async fn create_transport(
    config: &McpServerConfig,
) -> Result<Box<dyn McpTransport>, String> {
    let transport_config = TransportConfig {
        timeout: Duration::from_secs(config.timeout_secs),
        auth: config.auth.clone(),
    };
    
    match config.transport {
        McpTransportType::Stdio => {
            let command = config.command.as_ref()
                .ok_or_else(|| "Stdio transport requires a command".to_string())?;
            super::transport::StdioTransport::new(
                command.clone(),
                config.args.clone(),
                config.env.clone(),
                config.working_dir.clone(),
                transport_config,
            )
        }
        McpTransportType::Http => {
            let url = config.url.as_ref()
                .ok_or_else(|| "HTTP transport requires a URL".to_string())?;
            super::transport::HttpTransport::new(url.clone(), transport_config).await
        }
        McpTransportType::Sse => {
            let url = config.url.as_ref()
                .ok_or_else(|| "SSE transport requires a URL".to_string())?;
            super::transport::SseTransport::new(url.clone(), transport_config).await
        }
    }
}
