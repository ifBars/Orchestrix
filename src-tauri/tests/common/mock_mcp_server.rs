// src-tauri/tests/common/mock_mcp_server.rs
//! Mock MCP server implementation for integration testing.
//!
//! This module provides a mock MCP server that implements the full MCP protocol
//! for testing without external dependencies.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use orchestrix_lib::mcp::types::error_codes::*;
use orchestrix_lib::mcp::types::*;

/// A mock MCP server for testing.
pub struct MockMcpServer {
    pub tools: std::sync::Mutex<Vec<Tool>>,
    pub resources: std::sync::Mutex<Vec<Resource>>,
    pub prompts: std::sync::Mutex<Vec<Prompt>>,
    request_log: std::sync::Mutex<Vec<JsonRpcRequest>>,
    next_id: AtomicU64,
    should_fail_requests: std::sync::Mutex<bool>,
    failure_message: std::sync::Mutex<String>,
    response_delay_ms: std::sync::Mutex<u64>,
}

impl MockMcpServer {
    /// Create a new empty mock server.
    pub fn new() -> Self {
        Self {
            tools: std::sync::Mutex::new(Vec::new()),
            resources: std::sync::Mutex::new(Vec::new()),
            prompts: std::sync::Mutex::new(Vec::new()),
            request_log: std::sync::Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
            should_fail_requests: std::sync::Mutex::new(false),
            failure_message: std::sync::Mutex::new("Request failed".to_string()),
            response_delay_ms: std::sync::Mutex::new(0),
        }
    }

    /// Create a mock server with sample tools.
    pub fn with_sample_tools() -> Self {
        let server = Self::new();
        let tools = vec![
            Tool {
                name: "read_file".to_string(),
                description: Some("Read a file".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    },
                    "required": ["path"]
                }),
                output_schema: None,
                annotations: Some(ToolAnnotations {
                    read_only_hint: Some(true),
                    open_world_hint: Some(false),
                    destructive_hint: Some(false),
                    idempotent_hint: Some(true),
                }),
            },
            Tool {
                name: "write_file".to_string(),
                description: Some("Write to a file".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["path", "content"]
                }),
                output_schema: None,
                annotations: Some(ToolAnnotations {
                    read_only_hint: Some(false),
                    open_world_hint: Some(false),
                    destructive_hint: Some(true),
                    idempotent_hint: Some(false),
                }),
            },
        ];
        *server.tools.lock().unwrap() = tools;
        server
    }

    /// Create a mock server with sample resources.
    pub fn with_sample_resources() -> Self {
        let server = Self::new();
        let resources = vec![
            Resource {
                uri: "file:///example.txt".to_string(),
                name: "example.txt".to_string(),
                description: Some("An example text file".to_string()),
                mime_type: Some("text/plain".to_string()),
                size: Some(1024),
            },
            Resource {
                uri: "file:///data.json".to_string(),
                name: "data.json".to_string(),
                description: Some("JSON data file".to_string()),
                mime_type: Some("application/json".to_string()),
                size: Some(2048),
            },
        ];
        *server.resources.lock().unwrap() = resources;
        server
    }

    /// Create a mock server with sample prompts.
    pub fn with_sample_prompts() -> Self {
        let server = Self::new();
        let prompts = vec![
            Prompt {
                name: "greeting".to_string(),
                description: Some("A greeting prompt".to_string()),
                arguments: Some(vec![PromptArgument {
                    name: "name".to_string(),
                    description: Some("Name to greet".to_string()),
                    required: Some(true),
                }]),
            },
            Prompt {
                name: "code_review".to_string(),
                description: Some("Code review prompt".to_string()),
                arguments: Some(vec![PromptArgument {
                    name: "language".to_string(),
                    description: Some("Programming language".to_string()),
                    required: Some(false),
                }]),
            },
        ];
        *server.prompts.lock().unwrap() = prompts;
        server
    }

    /// Set whether requests should fail.
    pub fn set_should_fail(&self, should_fail: bool, message: impl Into<String>) {
        *self.should_fail_requests.lock().unwrap() = should_fail;
        *self.failure_message.lock().unwrap() = message.into();
    }

    /// Set response delay for testing timeouts.
    pub fn set_response_delay(&self, delay_ms: u64) {
        *self.response_delay_ms.lock().unwrap() = delay_ms;
    }

    /// Get the request log.
    pub fn get_request_log(&self) -> Vec<JsonRpcRequest> {
        self.request_log.lock().unwrap().clone()
    }

    /// Clear the request log.
    pub fn clear_log(&self) {
        self.request_log.lock().unwrap().clear();
    }

    /// Add a tool to the server.
    pub fn add_tool(&self, tool: Tool) {
        self.tools.lock().unwrap().push(tool);
    }

    /// Add a resource to the server.
    pub fn add_resource(&self, resource: Resource) {
        self.resources.lock().unwrap().push(resource);
    }

    /// Add a prompt to the server.
    pub fn add_prompt(&self, prompt: Prompt) {
        self.prompts.lock().unwrap().push(prompt);
    }

    /// Handle a JSON-RPC request.
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        // Log the request
        self.request_log.lock().unwrap().push(request.clone());

        // Check if should fail
        if *self.should_fail_requests.lock().unwrap() {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone().unwrap_or(RequestId::Number(0)),
                result: None,
                error: Some(JsonRpcError {
                    code: INTERNAL_ERROR,
                    message: self.failure_message.lock().unwrap().clone(),
                    data: None,
                }),
            };
        }

        // Handle response delay
        let delay = *self.response_delay_ms.lock().unwrap();
        if delay > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }

        // Route to appropriate handler
        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params),
            "tools/list" => self.handle_tools_list(request.params),
            "tools/call" => self.handle_tools_call(request.params),
            "resources/list" => self.handle_resources_list(request.params),
            "resources/read" => self.handle_resources_read(request.params),
            "resources/subscribe" => self.handle_resources_subscribe(request.params),
            "resources/unsubscribe" => self.handle_resources_unsubscribe(request.params),
            "prompts/list" => self.handle_prompts_list(request.params),
            "prompts/get" => self.handle_prompts_get(request.params),
            "notifications/initialized" => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone().unwrap_or(RequestId::Number(0)),
                    result: Some(serde_json::json!({})),
                    error: None,
                }
            }
            _ => Err(JsonRpcError {
                code: METHOD_NOT_FOUND,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        };

        match result {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone().unwrap_or(RequestId::Number(0)),
                result: Some(result),
                error: None,
            },
            Err(error) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone().unwrap_or(RequestId::Number(0)),
                result: None,
                error: Some(error),
            },
        }
    }

    fn handle_initialize(
        &self,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, JsonRpcError> {
        Ok(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": ServerCapabilities {
                experimental: None,
                logging: None,
                prompts: Some(PromptsCapability { list_changed: Some(true) }),
                resources: Some(ResourcesCapability {
                    subscribe: Some(true),
                    list_changed: Some(true),
                }),
                tools: Some(ToolsCapability { list_changed: Some(true) }),
            },
            "serverInfo": Implementation {
                name: "mock-mcp-server".to_string(),
                version: "1.0.0".to_string(),
            },
        }))
    }

    fn handle_tools_list(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let tools = self.tools.lock().unwrap().clone();

        // Handle cursor/pagination if provided
        let _cursor = params
            .as_ref()
            .and_then(|p| p.get("cursor"))
            .and_then(|c| c.as_str());

        Ok(serde_json::json!(ListToolsResult {
            tools,
            next_cursor: None,
        }))
    }

    fn handle_tools_call(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let params = params.ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing params".to_string(),
            data: None,
        })?;

        let name = params
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| JsonRpcError {
                code: INVALID_PARAMS,
                message: "Missing tool name".to_string(),
                data: None,
            })?;

        // Check if tool exists
        let tools = self.tools.lock().unwrap();
        if !tools.iter().any(|t| t.name == name) {
            return Err(JsonRpcError {
                code: INVALID_PARAMS,
                message: format!("Tool not found: {}", name),
                data: None,
            });
        }

        // Simulate tool execution
        let arguments = params.get("arguments").cloned();

        let content = vec![Content::Text(TextContent {
            text: format!("Executed tool '{}' with args: {:?}", name, arguments),
        })];

        Ok(serde_json::json!(CallToolResult {
            content,
            is_error: Some(false),
            structured_content: None,
        }))
    }

    fn handle_resources_list(
        &self,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let resources = self.resources.lock().unwrap().clone();

        Ok(serde_json::json!(ListResourcesResult {
            resources,
            next_cursor: None,
        }))
    }

    fn handle_resources_read(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let params = params.ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing params".to_string(),
            data: None,
        })?;

        let uri = params
            .get("uri")
            .and_then(|u| u.as_str())
            .ok_or_else(|| JsonRpcError {
                code: INVALID_PARAMS,
                message: "Missing resource URI".to_string(),
                data: None,
            })?;

        // Check if resource exists
        let resources = self.resources.lock().unwrap();
        let resource = resources
            .iter()
            .find(|r| r.uri == uri)
            .ok_or_else(|| JsonRpcError {
                code: INVALID_PARAMS,
                message: format!("Resource not found: {}", uri),
                data: None,
            })?;

        let content = ResourceContent {
            uri: resource.uri.clone(),
            mime_type: resource.mime_type.clone(),
            text: Some(format!("Content of {}", resource.uri)),
            blob: None,
        };

        Ok(serde_json::json!(ReadResourceResult {
            contents: vec![content],
        }))
    }

    fn handle_resources_subscribe(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let params = params.ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing params".to_string(),
            data: None,
        })?;

        let uri = params
            .get("uri")
            .and_then(|u| u.as_str())
            .ok_or_else(|| JsonRpcError {
                code: INVALID_PARAMS,
                message: "Missing resource URI".to_string(),
                data: None,
            })?;

        // In a real server, this would set up a subscription
        Ok(serde_json::json!(EmptyResult {}))
    }

    fn handle_resources_unsubscribe(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let params = params.ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing params".to_string(),
            data: None,
        })?;

        let uri = params
            .get("uri")
            .and_then(|u| u.as_str())
            .ok_or_else(|| JsonRpcError {
                code: INVALID_PARAMS,
                message: "Missing resource URI".to_string(),
                data: None,
            })?;

        // In a real server, this would remove a subscription
        Ok(serde_json::json!(EmptyResult {}))
    }

    fn handle_prompts_list(
        &self,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let prompts = self.prompts.lock().unwrap().clone();

        Ok(serde_json::json!(ListPromptsResult {
            prompts,
            next_cursor: None,
        }))
    }

    fn handle_prompts_get(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let params = params.ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing params".to_string(),
            data: None,
        })?;

        let name = params
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| JsonRpcError {
                code: INVALID_PARAMS,
                message: "Missing prompt name".to_string(),
                data: None,
            })?;

        // Check if prompt exists
        let prompts = self.prompts.lock().unwrap();
        let prompt = prompts
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| JsonRpcError {
                code: INVALID_PARAMS,
                message: format!("Prompt not found: {}", name),
                data: None,
            })?;

        let arguments: Option<HashMap<String, serde_json::Value>> = params
            .get("arguments")
            .and_then(|a| serde_json::from_value(a.clone()).ok());

        let content = if let Some(args) = arguments {
            format!("Prompt '{}' with args: {:?}", name, args)
        } else {
            format!("Prompt '{}'", name)
        };

        let messages = vec![PromptMessage {
            role: Role::Assistant,
            content: Content::Text(TextContent { text: content }),
        }];

        Ok(serde_json::json!(GetPromptResult {
            description: prompt.description.clone(),
            messages,
        }))
    }
}

impl Default for MockMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MockMcpServer {
    fn clone(&self) -> Self {
        Self {
            tools: std::sync::Mutex::new(self.tools.lock().unwrap().clone()),
            resources: std::sync::Mutex::new(self.resources.lock().unwrap().clone()),
            prompts: std::sync::Mutex::new(self.prompts.lock().unwrap().clone()),
            request_log: std::sync::Mutex::new(self.request_log.lock().unwrap().clone()),
            next_id: AtomicU64::new(self.next_id.load(Ordering::SeqCst)),
            should_fail_requests: std::sync::Mutex::new(*self.should_fail_requests.lock().unwrap()),
            failure_message: std::sync::Mutex::new(self.failure_message.lock().unwrap().clone()),
            response_delay_ms: std::sync::Mutex::new(*self.response_delay_ms.lock().unwrap()),
        }
    }
}
