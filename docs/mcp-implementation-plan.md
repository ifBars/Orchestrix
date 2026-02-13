# MCP Full Implementation Plan

## Overview
Implement complete MCP (Model Context Protocol) support with comprehensive unit and integration tests for the Orchestrix codebase.

## Current State Analysis

### Existing Files
1. `src-tauri/src/mcp/mod.rs` - Main MCP client manager (795 lines)
2. `src-tauri/src/mcp/transport.rs` - Transport implementations (549 lines) 
3. `src-tauri/src/mcp/connection.rs` - Connection pooling (302 lines)
4. `src-tauri/src/mcp/filtering.rs` - Tool filtering with tests (328 lines)
5. `src-tauri/src/mcp/events.rs` - Event types (262 lines)
6. `src-tauri/src/commands/mcp.rs` - Tauri commands (641 lines)

### What's Missing
1. Full MCP protocol types (Initialize, Resources, Prompts)
2. Proper JSON-RPC request/response handling
3. Resource and prompt support
4. Comprehensive unit tests
5. Integration tests with mock servers
6. Error handling improvements
7. Protocol version negotiation

## Implementation Tasks

### Task 1: MCP Protocol Types Module
**File**: `src-tauri/src/mcp/types.rs`

Implement complete MCP protocol types per specification 2025-06-18:
- JSON-RPC base types (Request, Response, Notification, Error)
- Initialize types (InitializeRequest, InitializeResponse, ServerCapabilities)
- Tool types (Tool, ToolAnnotations, CallToolRequest, CallToolResult)
- Resource types (Resource, ResourceTemplate, ReadResourceRequest)
- Prompt types (Prompt, PromptArgument, GetPromptRequest)
- Content types (TextContent, ImageContent, EmbeddedResource)

Requirements:
- All types must derive Serialize, Deserialize, Debug, Clone
- Use proper JSON field naming (camelCase)
- Include comprehensive documentation
- Add validation methods where appropriate

### Task 2: JSON-RPC Client Module
**File**: `src-tauri/src/mcp/jsonrpc.rs`

Implement robust JSON-RPC client:
- Request ID generation (atomic counter)
- Request/response matching
- Timeout handling
- Error parsing and propagation
- Notification handling
- Batch request support (optional)

Requirements:
- Type-safe request/response handling
- Proper error types with error codes
- Async/await throughout
- Comprehensive logging

### Task 3: Enhanced Transport Layer
**File**: `src-tauri/src/mcp/transport.rs` (enhance existing)

Enhance existing transport implementations:

**StdioTransport**:
- Add proper initialization handshake with protocol version negotiation
- Handle server capabilities
- Support for notifications
- Better error handling

**HttpTransport**:
- Support for SSE streaming
- Proper connection pooling
- Retry logic with exponential backoff
- Health check endpoint support

**SseTransport**:
- Proper SSE event parsing
- Reconnection logic
- Event stream multiplexing

### Task 4: MCP Client Implementation
**File**: `src-tauri/src/mcp/client.rs` (new)

Create high-level MCP client:
- Initialize connection with server
- Discover tools, resources, prompts
- Call tools with proper error handling
- Read resources
- Get prompts
- Subscribe to resources (if supported)

Requirements:
- Automatic reconnection
- Health monitoring
- Event emission
- Resource caching

### Task 5: Resource and Prompt Support
**Files**: 
- `src-tauri/src/mcp/mod.rs` (enhance)
- `src-tauri/src/commands/mcp.rs` (enhance)

Add to McpClientManager:
- `list_resources()` - List available resources
- `read_resource(uri)` - Read resource content
- `list_prompts()` - List available prompts
- `get_prompt(name, args)` - Get prompt with arguments
- Cache management for resources

Add Tauri commands:
- `list_mcp_resources`
- `read_mcp_resource`
- `list_mcp_prompts`
- `get_mcp_prompt`

### Task 6: Unit Tests - Transport Layer
**File**: `src-tauri/src/mcp/transport_test.rs`

Create comprehensive tests:
- Mock transport for testing
- StdioTransport tests (with mock process)
- HttpTransport tests (with mock server)
- Request/response serialization tests
- Error handling tests
- Timeout tests

Use `mockall` for mocking and `tokio::test` for async tests.

### Task 7: Unit Tests - Protocol Types
**File**: `src-tauri/src/mcp/types_test.rs`

Test all protocol types:
- Serialization/deserialization tests
- Validation tests
- JSON-RPC message tests
- Edge case handling

### Task 8: Unit Tests - Filtering
**File**: `src-tauri/src/mcp/filtering_test.rs`

Expand existing tests:
- Tool filter with glob patterns
- Approval policy edge cases
- Combined filter scenarios
- Performance tests (optional)

### Task 9: Unit Tests - Connection Manager
**File**: `src-tauri/src/mcp/connection_test.rs`

Test connection management:
- Pool creation and sizing
- Connection acquisition
- Health checking
- Connection reuse
- Error scenarios

### Task 10: Integration Tests
**Files**:
- `src-tauri/tests/mcp_integration_test.rs`
- `src-tauri/tests/mcp_commands_test.rs`

Create integration tests:
- Mock MCP server implementation
- Full client lifecycle tests
- Tool discovery and calling
- Resource operations
- Prompt operations
- Error scenarios
- Concurrent operations

Add mock server that implements MCP protocol for testing.

### Task 11: Update Commands
**File**: `src-tauri/src/commands/mcp.rs` (enhance)

Update existing commands to use new types:
- Add resource/prompt commands
- Improve error messages
- Add validation
- Update views

## Testing Strategy

### Unit Tests
- Each module has corresponding `*_test.rs` file
- Use `#[cfg(test)]` modules
- Mock external dependencies
- Test both success and error paths
- Aim for >80% coverage

### Integration Tests
- Separate `tests/` directory
- Mock MCP server using tokio
- Test full workflows
- Test concurrent access
- Test error recovery

### Test Utilities
**File**: `src-tauri/src/mcp/test_utils.rs`

Shared test utilities:
- Mock transport implementations
- Test fixtures
- Assertion helpers
- Mock server builder

## Dependencies to Add

```toml
[dependencies]
# Already present: serde, serde_json, tokio, async-trait, reqwest, chrono

[dev-dependencies]
mockall = "0.12"
wiremock = "0.6"
tokio-test = "0.4"
pretty_assertions = "1.4"
```

## Acceptance Criteria

1. All MCP protocol types implemented per spec 2025-06-18
2. JSON-RPC handling is robust with proper error types
3. Transport layer supports stdio, HTTP, and SSE
4. Unit tests cover all public APIs
5. Integration tests verify end-to-end workflows
6. All tests pass (`cargo test`)
7. No compiler warnings
8. Documentation complete

## File Structure

```
src-tauri/src/mcp/
├── mod.rs              # Main module, re-exports
├── types.rs            # MCP protocol types
├── types_test.rs       # Type tests
├── jsonrpc.rs          # JSON-RPC client
├── jsonrpc_test.rs     # JSON-RPC tests
├── transport.rs        # Transport implementations
├── transport_test.rs   # Transport tests
├── client.rs           # High-level client
├── client_test.rs      # Client tests
├── connection.rs       # Connection management
├── connection_test.rs  # Connection tests
├── filtering.rs        # Tool filtering (existing)
├── filtering_test.rs   # Filtering tests (expand)
├── events.rs           # Event types (existing)
├── test_utils.rs       # Test utilities
└── README.md           # Module documentation

src-tauri/tests/
├── mcp_integration_test.rs
├── mcp_commands_test.rs
└── mock_mcp_server.rs

src-tauri/src/commands/
└── mcp.rs              # Enhanced commands
```

## Implementation Order

1. Task 1: Protocol types (foundation)
2. Task 2: JSON-RPC client (foundation)
3. Task 3: Enhanced transport (depends on 2)
4. Task 6, 7: Unit tests for types and transport
5. Task 4: MCP client (depends on 1, 2, 3)
6. Task 5: Resources/prompts (depends on 4)
7. Task 9: Connection tests
8. Task 8: Filtering tests (expand existing)
9. Task 10: Integration tests
10. Task 11: Update commands

## Notes

- Follow existing code style in the codebase
- Use `AppError` for error handling
- Maintain backward compatibility with existing commands
- Ensure async functions are Send + Sync
- Use tracing for logging (if available) or eprintln
- Keep test files focused and fast
- Use fixtures for complex test data
