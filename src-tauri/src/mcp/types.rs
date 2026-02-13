//! MCP (Model Context Protocol) protocol types.
//!
//! This module contains all type definitions for the MCP specification 2025-06-18.
//! Types follow the JSON-RPC 2.0 specification and MCP protocol standards.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// JSON-RPC Base Types
// ============================================================================

/// JSON-RPC version constant.
pub const JSON_RPC_VERSION: &str = "2.0";

/// JSON-RPC error codes as defined by the MCP specification.
pub mod error_codes {
    /// Parse error (-32700): Invalid JSON was received by the server.
    pub const PARSE_ERROR: i32 = -32700;
    /// Invalid Request (-32600): The JSON sent is not a valid Request object.
    pub const INVALID_REQUEST: i32 = -32600;
    /// Method not found (-32601): The method does not exist / is not available.
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// Invalid params (-32602): Invalid method parameter(s).
    pub const INVALID_PARAMS: i32 = -32602;
    /// Internal error (-32603): Internal JSON-RPC error.
    pub const INTERNAL_ERROR: i32 = -32603;
    /// Server error (-32000 to -32099): Reserved for implementation-defined server-errors.
    pub const SERVER_ERROR_START: i32 = -32000;
    /// Server error end range.
    pub const SERVER_ERROR_END: i32 = -32099;
}

/// A JSON-RPC request object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcRequest {
    /// JSON-RPC version (must be "2.0").
    pub jsonrpc: String,
    /// Request identifier (can be null for notifications).
    pub id: Option<RequestId>,
    /// Method name to invoke.
    pub method: String,
    /// Method parameters (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request.
    pub fn new(
        id: RequestId,
        method: impl Into<String>,
        params: Option<serde_json::Value>,
    ) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id: Some(id),
            method: method.into(),
            params,
        }
    }

    /// Create a new notification request (no id).
    pub fn notification(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id: None,
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC response object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcResponse {
    /// JSON-RPC version (must be "2.0").
    pub jsonrpc: String,
    /// Request identifier matching the request.
    pub id: RequestId,
    /// Result of the method call (if successful).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error object (if the call failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Create a successful response.
    pub fn success(id: RequestId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: RequestId, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// A JSON-RPC notification (request without id).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcNotification {
    /// JSON-RPC version (must be "2.0").
    pub jsonrpc: String,
    /// Method name to invoke.
    pub method: String,
    /// Method parameters (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
    /// Create a new notification.
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcError {
    /// Error code (integer).
    pub code: i32,
    /// Error message (short description).
    pub message: String,
    /// Additional error data (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Create a new JSON-RPC error.
    pub fn new(code: i32, message: impl Into<String>, data: Option<serde_json::Value>) -> Self {
        Self {
            code,
            message: message.into(),
            data,
        }
    }

    /// Create a parse error.
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::new(error_codes::PARSE_ERROR, message, None)
    }

    /// Create an invalid request error.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(error_codes::INVALID_REQUEST, message, None)
    }

    /// Create a method not found error.
    pub fn method_not_found(method: impl AsRef<str>) -> Self {
        Self::new(
            error_codes::METHOD_NOT_FOUND,
            format!("Method not found: {}", method.as_ref()),
            None,
        )
    }

    /// Create an invalid params error.
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(error_codes::INVALID_PARAMS, message, None)
    }

    /// Create an internal error.
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(error_codes::INTERNAL_ERROR, message, None)
    }
}

/// Request identifier type (string or integer).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    /// String identifier.
    String(String),
    /// Integer identifier.
    Number(i64),
}

impl From<String> for RequestId {
    fn from(s: String) -> Self {
        RequestId::String(s)
    }
}

impl From<&str> for RequestId {
    fn from(s: &str) -> Self {
        RequestId::String(s.to_string())
    }
}

impl From<i64> for RequestId {
    fn from(n: i64) -> Self {
        RequestId::Number(n)
    }
}

impl From<i32> for RequestId {
    fn from(n: i32) -> Self {
        RequestId::Number(n as i64)
    }
}

// ============================================================================
// Initialize Types
// ============================================================================

/// Initialize request sent by client to server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    /// Protocol version supported by the client.
    pub protocol_version: String,
    /// Client capabilities.
    pub capabilities: ClientCapabilities,
    /// Information about the client implementation.
    pub client_info: Implementation,
}

impl InitializeRequest {
    /// Create a new initialize request with default capabilities.
    pub fn new(protocol_version: impl Into<String>, client_info: Implementation) -> Self {
        Self {
            protocol_version: protocol_version.into(),
            capabilities: ClientCapabilities::default(),
            client_info,
        }
    }
}

/// Initialize response sent by server to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    /// Protocol version supported by the server.
    pub protocol_version: String,
    /// Server capabilities.
    pub capabilities: ServerCapabilities,
    /// Information about the server implementation.
    pub server_info: Implementation,
}

/// Client capabilities during initialization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    /// Experimental, non-standard capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
    /// Whether the client supports the roots capability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<RootsCapability>,
    /// Whether the client supports the sampling capability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<serde_json::Value>,
}

/// Roots capability configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootsCapability {
    /// Whether the client supports notifications for root changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Server capabilities during initialization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    /// Experimental, non-standard capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
    /// Logging capability configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<serde_json::Value>,
    /// Prompts capability configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
    /// Resources capability configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    /// Tools capability configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
}

/// Prompts capability configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptsCapability {
    /// Whether the server supports notifications for prompt list changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Resources capability configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesCapability {
    /// Whether the server supports subscribing to resource updates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
    /// Whether the server supports notifications for resource list changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Tools capability configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    /// Whether the server supports notifications for tool list changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Implementation information (name and version).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Implementation {
    /// Implementation name.
    pub name: String,
    /// Implementation version.
    pub version: String,
}

impl Implementation {
    /// Create new implementation info.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }
}

// ============================================================================
// Tool Types
// ============================================================================

/// A tool that can be called by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    /// Tool name (unique identifier).
    pub name: String,
    /// Human-readable description of the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
    /// JSON Schema for the tool's output (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
    /// Additional tool annotations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
}

/// Annotations for tool behavior hints.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    /// Hint indicating whether the tool only reads data (does not modify).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    /// Hint indicating whether the tool may interact with external systems.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
    /// Hint indicating whether the tool performs destructive operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    /// Hint indicating whether the tool produces the same output given the same input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,
}

/// Request to list available tools.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsRequest {
    /// Pagination cursor for fetching the next page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Result of listing tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    /// List of available tools.
    pub tools: Vec<Tool>,
    /// Cursor for fetching the next page (if more results available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Request to call a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolRequest {
    /// Name of the tool to call.
    pub name: String,
    /// Arguments for the tool call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Result of a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    /// Content returned by the tool (text, images, or embedded resources).
    pub content: Vec<Content>,
    /// Whether the tool call resulted in an error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    /// Structured content data (optional, for complex responses).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<serde_json::Value>,
}

// ============================================================================
// Resource Types
// ============================================================================

/// A resource that can be read by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    /// Unique URI for the resource.
    pub uri: String,
    /// Human-readable name for the resource.
    pub name: String,
    /// Description of the resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type of the resource content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Size of the resource in bytes (if known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// A resource template for dynamic resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplate {
    /// URI template (RFC 6570) for matching resource URIs.
    pub uri_template: String,
    /// Human-readable name for resources matching this template.
    pub name: String,
    /// Description of resources matching this template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type of resources matching this template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Request to list available resources.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesRequest {
    /// Pagination cursor for fetching the next page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Result of listing resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesResult {
    /// List of available resources.
    pub resources: Vec<Resource>,
    /// Cursor for fetching the next page (if more results available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Request to read a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceRequest {
    /// URI of the resource to read.
    pub uri: String,
}

/// Result of reading a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceResult {
    /// Contents of the resource (may be multiple parts).
    pub contents: Vec<ResourceContent>,
}

/// Content of a resource (text or binary).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContent {
    /// URI of the resource.
    pub uri: String,
    /// MIME type of the content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Text content (for text resources).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Binary content as base64 (for binary resources).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

// ============================================================================
// Prompt Types
// ============================================================================

/// A prompt template that can be used by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    /// Prompt name (unique identifier).
    pub name: String,
    /// Human-readable description of the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Arguments that can be passed to customize the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

/// An argument that can be passed to a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptArgument {
    /// Argument name.
    pub name: String,
    /// Human-readable description of the argument.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this argument is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// Request to list available prompts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsRequest {
    /// Pagination cursor for fetching the next page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Result of listing prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsResult {
    /// List of available prompts.
    pub prompts: Vec<Prompt>,
    /// Cursor for fetching the next page (if more results available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Request to get a specific prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptRequest {
    /// Name of the prompt to get.
    pub name: String,
    /// Arguments to pass to the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<HashMap<String, String>>,
}

/// Result of getting a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptResult {
    /// Description of the prompt (may differ from the template).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Messages in the prompt conversation.
    pub messages: Vec<PromptMessage>,
}

/// A message in a prompt conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMessage {
    /// Role of the message sender (user or assistant).
    pub role: Role,
    /// Content of the message.
    pub content: Content,
}

/// Role of a message sender.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// User message.
    User,
    /// Assistant message.
    Assistant,
}

// ============================================================================
// Content Types
// ============================================================================

/// Content types that can be returned by tools or used in prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Content {
    /// Text content.
    Text(TextContent),
    /// Image content.
    Image(ImageContent),
    /// Embedded resource content.
    Resource(EmbeddedResource),
}

/// Text content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextContent {
    /// The text content.
    pub text: String,
}

/// Image content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageContent {
    /// Base64-encoded image data.
    pub data: String,
    /// MIME type of the image (e.g., "image/png", "image/jpeg").
    pub mime_type: String,
}

/// Embedded resource content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedResource {
    /// The embedded resource.
    pub resource: ResourceContent,
}

// ============================================================================
// Pagination Types
// ============================================================================

/// Trait for paginated requests.
pub trait PaginatedRequest {
    /// Get the pagination cursor.
    fn cursor(&self) -> Option<&str>;
    /// Set the pagination cursor.
    fn set_cursor(&mut self, cursor: Option<String>);
}

/// Trait for paginated results.
pub trait PaginatedResult {
    /// Get the next cursor for pagination.
    fn next_cursor(&self) -> Option<&str>;
    /// Check if there are more results.
    fn has_more(&self) -> bool;
}

// ============================================================================
// Notification Types
// ============================================================================

/// Progress notification parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressNotification {
    /// Progress token from the initial request.
    pub progress_token: String,
    /// Current progress value.
    pub progress: f64,
    /// Total progress value (if known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    /// Optional message describing the current progress.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Logging message notification parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggingMessageNotification {
    /// Logging level.
    pub level: LoggingLevel,
    /// Log message.
    pub message: String,
    /// Additional logger name or context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logger: Option<String>,
    /// Additional structured data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Logging levels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoggingLevel {
    /// Debug level (verbose).
    Debug,
    /// Info level (standard).
    Info,
    /// Warning level (potential issues).
    Warning,
    /// Error level (failures).
    Error,
}

/// Resource updated notification parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUpdatedNotification {
    /// URI of the resource that was updated.
    pub uri: String,
}

/// Resource list changed notification.
/// This notification has no additional parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceListChangedNotification {}

/// Tool list changed notification.
/// This notification has no additional parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolListChangedNotification {}

/// Prompt list changed notification.
/// This notification has no additional parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptListChangedNotification {}

// ============================================================================
// Additional Request/Result Types
// ============================================================================

/// Request to subscribe to resource updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeRequest {
    /// URI of the resource to subscribe to.
    pub uri: String,
}

/// Request to unsubscribe from resource updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeRequest {
    /// URI of the resource to unsubscribe from.
    pub uri: String,
}

/// Empty result type for operations that return no data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyResult {}

impl Default for EmptyResult {
    fn default() -> Self {
        Self {}
    }
}

/// Request to set logging level.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetLevelRequest {
    /// Logging level to set.
    pub level: LoggingLevel,
}

// ============================================================================
// Utility Implementations
// ============================================================================

impl Default for Role {
    fn default() -> Self {
        Role::User
    }
}

impl Default for LoggingLevel {
    fn default() -> Self {
        LoggingLevel::Info
    }
}

impl From<TextContent> for Content {
    fn from(content: TextContent) -> Self {
        Content::Text(content)
    }
}

impl From<ImageContent> for Content {
    fn from(content: ImageContent) -> Self {
        Content::Image(content)
    }
}

impl From<EmbeddedResource> for Content {
    fn from(content: EmbeddedResource) -> Self {
        Content::Resource(content)
    }
}

impl TextContent {
    /// Create new text content.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl ImageContent {
    /// Create new image content.
    pub fn new(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }
}

impl EmbeddedResource {
    /// Create new embedded resource content.
    pub fn new(resource: ResourceContent) -> Self {
        Self { resource }
    }
}

impl ResourceContent {
    /// Create text resource content.
    pub fn text(
        uri: impl Into<String>,
        mime_type: Option<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            uri: uri.into(),
            mime_type,
            text: Some(text.into()),
            blob: None,
        }
    }

    /// Create binary resource content.
    pub fn blob(
        uri: impl Into<String>,
        mime_type: Option<String>,
        blob: impl Into<String>,
    ) -> Self {
        Self {
            uri: uri.into(),
            mime_type,
            text: None,
            blob: Some(blob.into()),
        }
    }

    /// Check if this content is text.
    pub fn is_text(&self) -> bool {
        self.text.is_some()
    }

    /// Check if this content is binary.
    pub fn is_blob(&self) -> bool {
        self.blob.is_some()
    }
}

// Safety: All types in this module are Send + Sync because they contain only
// standard types (String, Option, Vec, HashMap, serde_json::Value) which are
// all Send + Sync.

#[cfg(test)]
mod tests {
    use serde_json::json;
    use std::collections::HashMap;

    use super::*;

    // ============================================================================
    // JSON-RPC Types Tests
    // ============================================================================

    #[test]
    fn test_json_rpc_request_serialization() {
        let request = JsonRpcRequest::new(
            RequestId::Number(1),
            "tools/list",
            Some(json!({ "cursor": "abc123" })),
        );

        let json = serde_json::to_value(&request).unwrap();

        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["method"], "tools/list");
        assert_eq!(json["params"]["cursor"], "abc123");
    }

    #[test]
    fn test_json_rpc_response_serialization() {
        let response = JsonRpcResponse::success(RequestId::Number(1), json!({ "tools": [] }));

        let json = serde_json::to_value(&response).unwrap();

        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["result"]["tools"], json!([]));
        assert!(json["error"].is_null());
    }

    #[test]
    fn test_json_rpc_notification_serialization() {
        let notification =
            JsonRpcNotification::new("notifications/progress", Some(json!({ "progress": 50 })));

        let json = serde_json::to_value(&notification).unwrap();

        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["method"], "notifications/progress");
        assert_eq!(json["params"]["progress"], 50);
        assert!(json.get("id").is_none());
    }

    #[test]
    fn test_json_rpc_error_serialization() {
        let error = JsonRpcError::new(
            error_codes::METHOD_NOT_FOUND,
            "Method not found",
            Some(json!({ "method": "unknown/method" })),
        );

        let json = serde_json::to_value(&error).unwrap();

        assert_eq!(json["code"], error_codes::METHOD_NOT_FOUND);
        assert_eq!(json["message"], "Method not found");
        assert_eq!(json["data"]["method"], "unknown/method");
    }

    #[test]
    fn test_request_id_variants() {
        // Test numeric ID
        let numeric_id: RequestId = 42i64.into();
        let json = serde_json::to_value(&numeric_id).unwrap();
        assert_eq!(json, 42);

        let deserialized: RequestId = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, numeric_id);

        // Test string ID
        let string_id: RequestId = "req-123".into();
        let json = serde_json::to_value(&string_id).unwrap();
        assert_eq!(json, "req-123");

        let deserialized: RequestId = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, string_id);

        // Test from String
        let string_id2: RequestId = String::from("req-456").into();
        assert!(matches!(string_id2, RequestId::String(_)));

        // Test from i32
        let int_id: RequestId = 100i32.into();
        assert!(matches!(int_id, RequestId::Number(100)));
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(error_codes::PARSE_ERROR, -32700);
        assert_eq!(error_codes::INVALID_REQUEST, -32600);
        assert_eq!(error_codes::METHOD_NOT_FOUND, -32601);
        assert_eq!(error_codes::INVALID_PARAMS, -32602);
        assert_eq!(error_codes::INTERNAL_ERROR, -32603);
        assert_eq!(error_codes::SERVER_ERROR_START, -32000);
        assert_eq!(error_codes::SERVER_ERROR_END, -32099);
    }

    // ============================================================================
    // Initialize Types Tests
    // ============================================================================

    #[test]
    fn test_initialize_request_serialization() {
        let client_info = Implementation::new("TestClient", "1.0.0");
        let request = InitializeRequest::new("2024-11-05", client_info);

        let json = serde_json::to_value(&request).unwrap();

        assert_eq!(json["protocolVersion"], "2024-11-05");
        assert_eq!(json["clientInfo"]["name"], "TestClient");
        assert_eq!(json["clientInfo"]["version"], "1.0.0");
        assert!(json["capabilities"].is_object());
    }

    #[test]
    fn test_initialize_response_serialization() {
        let response = InitializeResponse {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                experimental: None,
                logging: Some(json!({})),
                prompts: Some(PromptsCapability {
                    list_changed: Some(true),
                }),
                resources: Some(ResourcesCapability {
                    subscribe: Some(true),
                    list_changed: Some(true),
                }),
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
                }),
            },
            server_info: Implementation::new("TestServer", "2.0.0"),
        };

        let json = serde_json::to_value(&response).unwrap();

        assert_eq!(json["protocolVersion"], "2024-11-05");
        assert_eq!(json["serverInfo"]["name"], "TestServer");
        assert_eq!(json["capabilities"]["tools"]["listChanged"], true);
        assert_eq!(json["capabilities"]["resources"]["subscribe"], true);
    }

    #[test]
    fn test_client_capabilities_serialization() {
        let caps = ClientCapabilities {
            experimental: Some(HashMap::from([("custom".to_string(), json!(true))])),
            roots: Some(RootsCapability {
                list_changed: Some(true),
            }),
            sampling: Some(json!({})),
        };

        let json = serde_json::to_value(&caps).unwrap();

        assert_eq!(json["experimental"]["custom"], true);
        assert_eq!(json["roots"]["listChanged"], true);
        assert!(json["sampling"].is_object());

        // Test default
        let default_caps = ClientCapabilities::default();
        let json = serde_json::to_value(&default_caps).unwrap();
        assert!(json.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_server_capabilities_serialization() {
        let caps = ServerCapabilities::default();
        let json = serde_json::to_value(&caps).unwrap();
        // Default should have no fields (all None with skip_serializing_if)
        assert!(json.as_object().unwrap().is_empty());

        // With some fields set
        let caps = ServerCapabilities {
            experimental: None,
            logging: Some(json!({})),
            prompts: Some(PromptsCapability {
                list_changed: Some(true),
            }),
            resources: None,
            tools: None,
        };
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["prompts"]["listChanged"], true);
        assert!(json.get("resources").is_none());
    }

    #[test]
    fn test_implementation_serialization() {
        let impl_info = Implementation::new("MyClient", "1.2.3");

        let json = serde_json::to_value(&impl_info).unwrap();
        assert_eq!(json["name"], "MyClient");
        assert_eq!(json["version"], "1.2.3");

        let deserialized: Implementation = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.name, "MyClient");
        assert_eq!(deserialized.version, "1.2.3");
    }

    // ============================================================================
    // Tool Types Tests
    // ============================================================================

    #[test]
    fn test_tool_serialization() {
        let tool = Tool {
            name: "read_file".to_string(),
            description: Some("Read a file's contents".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
            output_schema: Some(json!({ "type": "string" })),
            annotations: Some(ToolAnnotations {
                read_only_hint: Some(true),
                open_world_hint: Some(false),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
            }),
        };

        let json = serde_json::to_value(&tool).unwrap();

        assert_eq!(json["name"], "read_file");
        assert_eq!(json["description"], "Read a file's contents");
        assert_eq!(json["inputSchema"]["type"], "object");
        assert_eq!(json["outputSchema"]["type"], "string");

        // Roundtrip
        let deserialized: Tool = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.name, tool.name);
        assert_eq!(deserialized.description, tool.description);
    }

    #[test]
    fn test_tool_annotations_serialization() {
        let annotations = ToolAnnotations {
            read_only_hint: Some(true),
            open_world_hint: Some(false),
            destructive_hint: None,
            idempotent_hint: Some(true),
        };

        let json = serde_json::to_value(&annotations).unwrap();

        assert_eq!(json["readOnlyHint"], true);
        assert_eq!(json["openWorldHint"], false);
        assert!(json.get("destructiveHint").is_none()); // skip_serializing_if
        assert_eq!(json["idempotentHint"], true);

        // Test default (all None)
        let default = ToolAnnotations::default();
        let json = serde_json::to_value(&default).unwrap();
        assert!(json.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_list_tools_request_serialization() {
        let request = ListToolsRequest {
            cursor: Some("page2".to_string()),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["cursor"], "page2");

        // Without cursor
        let request = ListToolsRequest::default();
        let json = serde_json::to_value(&request).unwrap();
        assert!(json.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_list_tools_result_serialization() {
        let result = ListToolsResult {
            tools: vec![Tool {
                name: "tool1".to_string(),
                description: None,
                input_schema: json!({}),
                output_schema: None,
                annotations: None,
            }],
            next_cursor: Some("page3".to_string()),
        };

        let json = serde_json::to_value(&result).unwrap();

        assert_eq!(json["tools"].as_array().unwrap().len(), 1);
        assert_eq!(json["tools"][0]["name"], "tool1");
        assert_eq!(json["nextCursor"], "page3");
    }

    #[test]
    fn test_call_tool_request_serialization() {
        let request = CallToolRequest {
            name: "write_file".to_string(),
            arguments: Some(
                [
                    ("path".to_string(), json!("/tmp/test.txt")),
                    ("content".to_string(), json!("hello")),
                ]
                .into_iter()
                .collect(),
            ),
        };

        let json = serde_json::to_value(&request).unwrap();

        assert_eq!(json["name"], "write_file");
        assert_eq!(json["arguments"]["path"], "/tmp/test.txt");
        assert_eq!(json["arguments"]["content"], "hello");
    }

    #[test]
    fn test_call_tool_result_serialization() {
        let result = CallToolResult {
            content: vec![Content::Text(TextContent {
                text: "Success!".to_string(),
            })],
            is_error: Some(false),
            structured_content: Some(json!({ "bytes_written": 100 })),
        };

        let json = serde_json::to_value(&result).unwrap();

        assert_eq!(json["content"][0]["type"], "text");
        assert_eq!(json["content"][0]["text"], "Success!");
        assert_eq!(json["isError"], false);
        assert_eq!(json["structuredContent"]["bytes_written"], 100);
    }

    // ============================================================================
    // Resource Types Tests
    // ============================================================================

    #[test]
    fn test_resource_serialization() {
        let resource = Resource {
            uri: "file:///home/user/doc.txt".to_string(),
            name: "Document".to_string(),
            description: Some("A text document".to_string()),
            mime_type: Some("text/plain".to_string()),
            size: Some(1024),
        };

        let json = serde_json::to_value(&resource).unwrap();

        assert_eq!(json["uri"], "file:///home/user/doc.txt");
        assert_eq!(json["name"], "Document");
        assert_eq!(json["description"], "A text document");
        assert_eq!(json["mimeType"], "text/plain");
        assert_eq!(json["size"], 1024);

        // Roundtrip
        let deserialized: Resource = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.uri, resource.uri);
        assert_eq!(deserialized.mime_type, resource.mime_type);
    }

    #[test]
    fn test_resource_template_serialization() {
        let template = ResourceTemplate {
            uri_template: "file:///{path}".to_string(),
            name: "File Resource".to_string(),
            description: Some("A file resource template".to_string()),
            mime_type: Some("application/octet-stream".to_string()),
        };

        let json = serde_json::to_value(&template).unwrap();

        assert_eq!(json["uriTemplate"], "file:///{path}");
        assert_eq!(json["name"], "File Resource");
        assert_eq!(json["description"], "A file resource template");
        assert_eq!(json["mimeType"], "application/octet-stream");
    }

    #[test]
    fn test_list_resources_request_serialization() {
        let request = ListResourcesRequest {
            cursor: Some("next_page".to_string()),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["cursor"], "next_page");

        // Test default (no cursor)
        let default = ListResourcesRequest::default();
        let json = serde_json::to_value(&default).unwrap();
        assert!(json.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_list_resources_result_serialization() {
        let result = ListResourcesResult {
            resources: vec![
                Resource {
                    uri: "file:///a.txt".to_string(),
                    name: "A".to_string(),
                    description: None,
                    mime_type: None,
                    size: None,
                },
                Resource {
                    uri: "file:///b.txt".to_string(),
                    name: "B".to_string(),
                    description: None,
                    mime_type: None,
                    size: None,
                },
            ],
            next_cursor: None,
        };

        let json = serde_json::to_value(&result).unwrap();

        assert_eq!(json["resources"].as_array().unwrap().len(), 2);
        assert!(json.get("nextCursor").is_none()); // skip_serializing_if
    }

    #[test]
    fn test_read_resource_request_serialization() {
        let request = ReadResourceRequest {
            uri: "file:///test.txt".to_string(),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["uri"], "file:///test.txt");

        // Roundtrip
        let deserialized: ReadResourceRequest = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.uri, "file:///test.txt");
    }

    #[test]
    fn test_read_resource_result_serialization() {
        let result = ReadResourceResult {
            contents: vec![ResourceContent::text(
                "file:///test.txt",
                Some("text/plain".to_string()),
                "Hello, World!",
            )],
        };

        let json = serde_json::to_value(&result).unwrap();

        assert_eq!(json["contents"].as_array().unwrap().len(), 1);
        assert_eq!(json["contents"][0]["uri"], "file:///test.txt");
        assert_eq!(json["contents"][0]["text"], "Hello, World!");
    }

    #[test]
    fn test_resource_content_variants() {
        // Text content
        let text_content =
            ResourceContent::text("file:///doc.txt", Some("text/plain".to_string()), "content");
        let json = serde_json::to_value(&text_content).unwrap();
        assert_eq!(json["text"], "content");
        assert!(json.get("blob").is_none());
        assert!(text_content.is_text());
        assert!(!text_content.is_blob());

        // Blob content
        let blob_content = ResourceContent::blob(
            "file:///image.png",
            Some("image/png".to_string()),
            "base64data",
        );
        let json = serde_json::to_value(&blob_content).unwrap();
        assert_eq!(json["blob"], "base64data");
        assert!(json.get("text").is_none());
        assert!(blob_content.is_blob());
        assert!(!blob_content.is_text());
    }

    // ============================================================================
    // Prompt Types Tests
    // ============================================================================

    #[test]
    fn test_prompt_serialization() {
        let prompt = Prompt {
            name: "code_review".to_string(),
            description: Some("Review code for issues".to_string()),
            arguments: Some(vec![PromptArgument {
                name: "code".to_string(),
                description: Some("The code to review".to_string()),
                required: Some(true),
            }]),
        };

        let json = serde_json::to_value(&prompt).unwrap();

        assert_eq!(json["name"], "code_review");
        assert_eq!(json["description"], "Review code for issues");
        assert_eq!(json["arguments"][0]["name"], "code");
        assert_eq!(json["arguments"][0]["required"], true);
    }

    #[test]
    fn test_prompt_argument_serialization() {
        let arg = PromptArgument {
            name: "language".to_string(),
            description: Some("Programming language".to_string()),
            required: Some(false),
        };

        let json = serde_json::to_value(&arg).unwrap();

        assert_eq!(json["name"], "language");
        assert_eq!(json["description"], "Programming language");
        assert_eq!(json["required"], false);
    }

    #[test]
    fn test_list_prompts_request_serialization() {
        let request = ListPromptsRequest {
            cursor: Some("page2".to_string()),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["cursor"], "page2");

        let default = ListPromptsRequest::default();
        let json = serde_json::to_value(&default).unwrap();
        assert!(json.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_list_prompts_result_serialization() {
        let result = ListPromptsResult {
            prompts: vec![
                Prompt {
                    name: "p1".to_string(),
                    description: None,
                    arguments: None,
                },
                Prompt {
                    name: "p2".to_string(),
                    description: None,
                    arguments: None,
                },
            ],
            next_cursor: Some("page3".to_string()),
        };

        let json = serde_json::to_value(&result).unwrap();

        assert_eq!(json["prompts"].as_array().unwrap().len(), 2);
        assert_eq!(json["nextCursor"], "page3");
    }

    #[test]
    fn test_get_prompt_request_serialization() {
        let request = GetPromptRequest {
            name: "analyze".to_string(),
            arguments: Some(HashMap::from([("input".to_string(), "data".to_string())])),
        };

        let json = serde_json::to_value(&request).unwrap();

        assert_eq!(json["name"], "analyze");
        assert_eq!(json["arguments"]["input"], "data");
    }

    #[test]
    fn test_get_prompt_result_serialization() {
        let result = GetPromptResult {
            description: Some("Analysis prompt".to_string()),
            messages: vec![
                PromptMessage {
                    role: Role::User,
                    content: Content::Text(TextContent {
                        text: "Analyze this".to_string(),
                    }),
                },
                PromptMessage {
                    role: Role::Assistant,
                    content: Content::Text(TextContent {
                        text: "I'll analyze it".to_string(),
                    }),
                },
            ],
        };

        let json = serde_json::to_value(&result).unwrap();

        assert_eq!(json["description"], "Analysis prompt");
        assert_eq!(json["messages"].as_array().unwrap().len(), 2);
        assert_eq!(json["messages"][0]["role"], "user");
        assert_eq!(json["messages"][1]["role"], "assistant");
    }

    // ============================================================================
    // Content Types Tests
    // ============================================================================

    #[test]
    fn test_text_content_serialization() {
        let content = TextContent {
            text: "Hello, World!".to_string(),
        };

        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["text"], "Hello, World!");

        // Test new constructor
        let content2 = TextContent::new("Test");
        assert_eq!(content2.text, "Test");
    }

    #[test]
    fn test_image_content_serialization() {
        let content = ImageContent::new("base64data", "image/png");

        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["data"], "base64data");
        assert_eq!(json["mimeType"], "image/png");

        // Roundtrip as Content enum
        let content_enum: Content = content.into();
        let json = serde_json::to_value(&content_enum).unwrap();
        assert_eq!(json["type"], "image");
        assert_eq!(json["data"], "base64data");
    }

    #[test]
    fn test_embedded_resource_serialization() {
        let resource =
            ResourceContent::text("file:///doc.txt", Some("text/plain".to_string()), "content");
        let embedded = EmbeddedResource::new(resource);

        let json = serde_json::to_value(&embedded).unwrap();
        assert_eq!(json["resource"]["uri"], "file:///doc.txt");

        // Roundtrip as Content enum
        let content_enum: Content = embedded.into();
        let json = serde_json::to_value(&content_enum).unwrap();
        assert_eq!(json["type"], "resource");
    }

    #[test]
    fn test_content_enum_variants() {
        // Text variant
        let text = Content::Text(TextContent {
            text: "Hello".to_string(),
        });
        let json = serde_json::to_value(&text).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello");

        // Image variant
        let image = Content::Image(ImageContent::new("data", "image/jpeg"));
        let json = serde_json::to_value(&image).unwrap();
        assert_eq!(json["type"], "image");
        assert_eq!(json["mimeType"], "image/jpeg");

        // Resource variant
        let resource = ResourceContent::text("uri", None, "text");
        let embedded = Content::Resource(EmbeddedResource::new(resource));
        let json = serde_json::to_value(&embedded).unwrap();
        assert_eq!(json["type"], "resource");

        // Deserialize back
        let text_json = json!({"type": "text", "text": "Hello"});
        let deserialized: Content = serde_json::from_value(text_json).unwrap();
        match deserialized {
            Content::Text(t) => assert_eq!(t.text, "Hello"),
            _ => panic!("Expected Text variant"),
        }

        let image_json = json!({"type": "image", "data": "abc", "mimeType": "image/png"});
        let deserialized: Content = serde_json::from_value(image_json).unwrap();
        match deserialized {
            Content::Image(i) => {
                assert_eq!(i.data, "abc");
                assert_eq!(i.mime_type, "image/png");
            }
            _ => panic!("Expected Image variant"),
        }
    }

    // ============================================================================
    // Notification Types Tests
    // ============================================================================

    #[test]
    fn test_progress_notification_serialization() {
        let notification = ProgressNotification {
            progress_token: "token123".to_string(),
            progress: 50.0,
            total: Some(100.0),
            message: Some("Halfway done".to_string()),
        };

        let json = serde_json::to_value(&notification).unwrap();

        assert_eq!(json["progressToken"], "token123");
        assert_eq!(json["progress"], 50.0);
        assert_eq!(json["total"], 100.0);
        assert_eq!(json["message"], "Halfway done");
    }

    #[test]
    fn test_logging_message_notification_serialization() {
        let notification = LoggingMessageNotification {
            level: LoggingLevel::Warning,
            message: "Something might be wrong".to_string(),
            logger: Some("test_logger".to_string()),
            data: Some(json!({ "code": 42 })),
        };

        let json = serde_json::to_value(&notification).unwrap();

        assert_eq!(json["level"], "warning");
        assert_eq!(json["message"], "Something might be wrong");
        assert_eq!(json["logger"], "test_logger");
        assert_eq!(json["data"]["code"], 42);
    }

    #[test]
    fn test_resource_updated_notification_serialization() {
        let notification = ResourceUpdatedNotification {
            uri: "file:///data.txt".to_string(),
        };

        let json = serde_json::to_value(&notification).unwrap();
        assert_eq!(json["uri"], "file:///data.txt");
    }

    #[test]
    fn test_list_changed_notifications_serialization() {
        // Resource list changed (empty notification)
        let resource_notification = ResourceListChangedNotification {};
        let json = serde_json::to_value(&resource_notification).unwrap();
        assert!(json.as_object().unwrap().is_empty());

        // Tool list changed (empty notification)
        let tool_notification = ToolListChangedNotification {};
        let json = serde_json::to_value(&tool_notification).unwrap();
        assert!(json.as_object().unwrap().is_empty());

        // Prompt list changed (empty notification)
        let prompt_notification = PromptListChangedNotification {};
        let json = serde_json::to_value(&prompt_notification).unwrap();
        assert!(json.as_object().unwrap().is_empty());
    }

    // ============================================================================
    // Edge Cases Tests
    // ============================================================================

    #[test]
    fn test_optional_fields_handling() {
        // Tool with minimal fields
        let tool = Tool {
            name: "minimal".to_string(),
            description: None,
            input_schema: json!({}),
            output_schema: None,
            annotations: None,
        };

        let json = serde_json::to_value(&tool).unwrap();
        assert!(json.get("description").is_none());
        assert!(json.get("outputSchema").is_none());
        assert!(json.get("annotations").is_none());

        // Deserialize with missing optional fields
        let minimal_json = json!({
            "name": "minimal",
            "inputSchema": {}
        });
        let deserialized: Tool = serde_json::from_value(minimal_json).unwrap();
        assert_eq!(deserialized.name, "minimal");
        assert_eq!(deserialized.description, None);
    }

    #[test]
    fn test_empty_collections_handling() {
        // Empty tools list
        let result = ListToolsResult {
            tools: vec![],
            next_cursor: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["tools"], json!([]));

        // Empty resources list
        let result = ListResourcesResult {
            resources: vec![],
            next_cursor: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["resources"], json!([]));

        // Empty prompts list
        let result = ListPromptsResult {
            prompts: vec![],
            next_cursor: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["prompts"], json!([]));

        // Empty experimental capabilities
        let caps = ClientCapabilities {
            experimental: Some(HashMap::new()),
            roots: None,
            sampling: None,
        };
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["experimental"], json!({}));
    }

    #[test]
    fn test_null_value_handling() {
        // JSON with explicit null values
        let json = json!({
            "name": "test",
            "description": null,
            "inputSchema": {},
            "outputSchema": null
        });

        let tool: Tool = serde_json::from_value(json).unwrap();
        assert_eq!(tool.name, "test");
        assert_eq!(tool.description, None);
        assert_eq!(tool.output_schema, None);
    }

    #[test]
    fn test_roundtrip_serialization() {
        // Complex type roundtrip: InitializeRequest
        let original = InitializeRequest {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities {
                experimental: Some(HashMap::from([("feature".to_string(), json!(true))])),
                roots: Some(RootsCapability {
                    list_changed: Some(true),
                }),
                sampling: Some(json!({})),
            },
            client_info: Implementation::new("TestClient", "1.0.0"),
        };

        let json = serde_json::to_value(&original).unwrap();
        let deserialized: InitializeRequest = serde_json::from_value(json).unwrap();

        assert_eq!(deserialized.protocol_version, original.protocol_version);
        assert_eq!(deserialized.client_info.name, original.client_info.name);
        assert_eq!(
            deserialized.client_info.version,
            original.client_info.version
        );

        // Complex type roundtrip: Tool with annotations
        let original = Tool {
            name: "complex_tool".to_string(),
            description: Some("A complex tool".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "arg1": { "type": "string" },
                    "arg2": { "type": "number" }
                }
            }),
            output_schema: Some(json!({ "type": "object" })),
            annotations: Some(ToolAnnotations {
                read_only_hint: Some(true),
                open_world_hint: Some(false),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
            }),
        };

        let json = serde_json::to_value(&original).unwrap();
        let deserialized: Tool = serde_json::from_value(json).unwrap();

        assert_eq!(deserialized.name, original.name);
        assert_eq!(deserialized.description, original.description);
        assert_eq!(
            deserialized.annotations.as_ref().unwrap().read_only_hint,
            Some(true)
        );
        assert_eq!(
            deserialized.annotations.as_ref().unwrap().destructive_hint,
            Some(false)
        );
    }

    #[test]
    fn test_error_response_roundtrip() {
        // Test error response creation and serialization
        let error = JsonRpcError::method_not_found("unknown_method");
        let response = JsonRpcResponse::error(RequestId::Number(42), error);

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 42);
        assert_eq!(json["error"]["code"], error_codes::METHOD_NOT_FOUND);
        assert!(json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown_method"));

        // Deserialize and verify
        let deserialized: JsonRpcResponse = serde_json::from_value(json).unwrap();
        assert!(deserialized.result.is_none());
        assert!(deserialized.error.is_some());
    }

    #[test]
    fn test_logging_level_variants() {
        // Test all logging levels
        let levels = vec![
            (LoggingLevel::Debug, "debug"),
            (LoggingLevel::Info, "info"),
            (LoggingLevel::Warning, "warning"),
            (LoggingLevel::Error, "error"),
        ];

        for (level, expected) in levels {
            let json = serde_json::to_value(&level).unwrap();
            assert_eq!(json, expected);

            let deserialized: LoggingLevel = serde_json::from_value(json).unwrap();
            assert_eq!(deserialized, level);
        }

        // Test default
        let default = LoggingLevel::default();
        assert_eq!(default, LoggingLevel::Info);
    }

    #[test]
    fn test_role_variants() {
        // Test role serialization
        let user = Role::User;
        let assistant = Role::Assistant;

        let user_json = serde_json::to_value(&user).unwrap();
        let assistant_json = serde_json::to_value(&assistant).unwrap();

        assert_eq!(user_json, "user");
        assert_eq!(assistant_json, "assistant");

        // Test deserialization
        let deserialized_user: Role = serde_json::from_value(json!("user")).unwrap();
        let deserialized_assistant: Role = serde_json::from_value(json!("assistant")).unwrap();

        assert_eq!(deserialized_user, Role::User);
        assert_eq!(deserialized_assistant, Role::Assistant);

        // Test default
        let default = Role::default();
        assert_eq!(default, Role::User);
    }

    #[test]
    fn test_request_notification_helpers() {
        // Test JsonRpcRequest::new
        let request = JsonRpcRequest::new(
            RequestId::String("test-id".to_string()),
            "test/method",
            Some(json!({ "key": "value" })),
        );
        assert_eq!(request.jsonrpc, JSON_RPC_VERSION);
        assert_eq!(request.id, Some(RequestId::String("test-id".to_string())));
        assert_eq!(request.method, "test/method");

        // Test JsonRpcRequest::notification
        let notification = JsonRpcNotification::new("test/notify", Some(json!({})));
        assert_eq!(notification.jsonrpc, JSON_RPC_VERSION);
        assert_eq!(notification.method, "test/notify");
    }

    #[test]
    fn test_error_factory_methods() {
        let parse_err = JsonRpcError::parse_error("Invalid JSON");
        assert_eq!(parse_err.code, error_codes::PARSE_ERROR);
        assert_eq!(parse_err.message, "Invalid JSON");

        let invalid_req = JsonRpcError::invalid_request("Missing method");
        assert_eq!(invalid_req.code, error_codes::INVALID_REQUEST);

        let method_not_found = JsonRpcError::method_not_found("foo/bar");
        assert_eq!(method_not_found.code, error_codes::METHOD_NOT_FOUND);
        assert!(method_not_found.message.contains("foo/bar"));

        let invalid_params = JsonRpcError::invalid_params("Missing required param");
        assert_eq!(invalid_params.code, error_codes::INVALID_PARAMS);

        let internal_err = JsonRpcError::internal_error("Something went wrong");
        assert_eq!(internal_err.code, error_codes::INTERNAL_ERROR);
    }

    #[test]
    fn test_empty_result() {
        let empty = EmptyResult::default();
        let json = serde_json::to_value(&empty).unwrap();
        assert!(json.as_object().unwrap().is_empty());

        // Deserialize
        let deserialized: EmptyResult = serde_json::from_value(json!({})).unwrap();
        let _ = deserialized; // Just verify it deserializes
    }

    #[test]
    fn test_subscribe_unsubscribe_requests() {
        let subscribe = SubscribeRequest {
            uri: "file:///test.txt".to_string(),
        };
        let json = serde_json::to_value(&subscribe).unwrap();
        assert_eq!(json["uri"], "file:///test.txt");

        let unsubscribe = UnsubscribeRequest {
            uri: "file:///test.txt".to_string(),
        };
        let json = serde_json::to_value(&unsubscribe).unwrap();
        assert_eq!(json["uri"], "file:///test.txt");
    }

    #[test]
    fn test_set_level_request() {
        let request = SetLevelRequest {
            level: LoggingLevel::Debug,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["level"], "debug");
    }
}
