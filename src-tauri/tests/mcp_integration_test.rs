// src-tauri/tests/mcp_integration_test.rs
//! MCP (Model Context Protocol) Integration Tests

mod common;

use common::{MockMcpServer, MockTransport};
use orchestrix_lib::mcp::{
    ClientError, ClientState, McpClient, Prompt, PromptArgument, Resource, Role, Tool,
};

fn create_sample_tool(name: &str, description: &str) -> Tool {
    Tool {
        name: name.to_string(),
        description: Some(description.to_string()),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        }),
        output_schema: None,
        annotations: None,
    }
}

fn create_sample_resource(uri: &str, name: &str, mime_type: Option<&str>) -> Resource {
    Resource {
        uri: uri.to_string(),
        name: name.to_string(),
        description: Some(format!("A sample resource: {}", name)),
        mime_type: mime_type.map(|s| s.to_string()),
        size: Some(1024),
    }
}

fn create_sample_prompt(name: &str, description: &str, has_args: bool) -> Prompt {
    let arguments = if has_args {
        Some(vec![PromptArgument {
            name: "topic".to_string(),
            description: Some("The topic to discuss".to_string()),
            required: Some(true),
        }])
    } else {
        None
    };

    Prompt {
        name: name.to_string(),
        description: Some(description.to_string()),
        arguments,
    }
}

async fn create_test_client() -> McpClient {
    let server = MockMcpServer::new();
    create_test_client_with_server(server).await
}

async fn create_test_client_with_server(server: MockMcpServer) -> McpClient {
    let transport = MockTransport::new(server);
    McpClient::new(transport.boxed())
}

#[tokio::test]
async fn test_full_initialization_flow() {
    let mut client = create_test_client().await;

    assert!(!client.is_initialized());
    assert!(matches!(client.state(), ClientState::Uninitialized));

    let _capabilities = client.initialize().await.unwrap();

    assert!(client.is_initialized());
    assert!(matches!(client.state(), ClientState::Initialized));
    assert!(client.server_capabilities().is_some());
}

#[tokio::test]
async fn test_protocol_version_negotiation() {
    let mut client = create_test_client().await;

    let _capabilities = client.initialize().await.unwrap();

    assert!(client.protocol_version().is_some());
}

#[tokio::test]
async fn test_capabilities_exchange() {
    let server = MockMcpServer::new();
    {
        let mut tools = server.tools.lock().unwrap();
        tools.push(create_sample_tool("test", "Test tool"));
    }
    {
        let mut resources = server.resources.lock().unwrap();
        resources.push(create_sample_resource("test://test", "Test", None));
    }
    {
        let mut prompts = server.prompts.lock().unwrap();
        prompts.push(create_sample_prompt("prompt", "Prompt", false));
    }

    let mut client = create_test_client_with_server(server).await;
    let capabilities = client.initialize().await.unwrap();

    assert!(capabilities.tools.is_some());
    assert!(capabilities.resources.is_some());
    assert!(capabilities.prompts.is_some());
}

#[tokio::test]
async fn test_initialization_failure() {
    let server = MockMcpServer::new();
    let transport = MockTransport::new(server);
    transport
        .set_should_fail_connection(true, "Connection refused")
        .await;

    let mut client = McpClient::new(transport.boxed());

    let result = client.initialize().await;
    assert!(result.is_err());
    assert!(!client.is_initialized());
}

#[tokio::test]
async fn test_double_initialization() {
    let mut client = create_test_client().await;

    let _caps1 = client.initialize().await.unwrap();
    let _caps2 = client.initialize().await.unwrap();

    assert!(client.is_initialized());
}

#[tokio::test]
async fn test_operations_before_initialization() {
    let client = create_test_client().await;

    let result = client.list_tools(None).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ClientError::NotInitialized));
}

#[tokio::test]
async fn test_discover_tools() {
    let server = MockMcpServer::new();
    {
        let mut tools = server.tools.lock().unwrap();
        tools.push(create_sample_tool("read_file", "Read a file"));
        tools.push(create_sample_tool("write_file", "Write a file"));
    }

    let mut client = create_test_client_with_server(server.clone()).await;
    client.initialize().await.unwrap();

    let result = client.list_tools(None).await.unwrap();

    assert_eq!(result.tools.len(), 2);
    assert!(result.tools.iter().any(|t| t.name == "read_file"));
    assert!(result.tools.iter().any(|t| t.name == "write_file"));
}

#[tokio::test]
async fn test_call_tool_success() {
    let server = MockMcpServer::new();
    {
        let mut tools = server.tools.lock().unwrap();
        tools.push(create_sample_tool("read_file", "Read a file"));
    }

    let mut client = create_test_client_with_server(server).await;
    client.initialize().await.unwrap();

    let result = client
        .call_tool(
            "read_file",
            Some(serde_json::json!({
                "path": "/test/file.txt"
            })),
        )
        .await
        .unwrap();

    assert!(!result.is_error.unwrap_or(false));
    assert!(!result.content.is_empty());
}

#[tokio::test]
async fn test_list_resources() {
    let server = MockMcpServer::new();
    {
        let mut resources = server.resources.lock().unwrap();
        resources.push(create_sample_resource(
            "file:///test.txt",
            "Test",
            Some("text/plain"),
        ));
    }

    let mut client = create_test_client_with_server(server).await;
    client.initialize().await.unwrap();

    let result = client.list_resources(None).await.unwrap();

    assert_eq!(result.resources.len(), 1);
    assert_eq!(result.resources[0].name, "Test");
}

#[tokio::test]
async fn test_read_resource_text() {
    let server = MockMcpServer::new();
    {
        let mut resources = server.resources.lock().unwrap();
        resources.push(create_sample_resource(
            "file:///test.txt",
            "Test",
            Some("text/plain"),
        ));
    }

    let mut client = create_test_client_with_server(server).await;
    client.initialize().await.unwrap();

    let result = client.read_resource("file:///test.txt").await.unwrap();

    assert_eq!(result.contents.len(), 1);
    assert!(result.contents[0].text.is_some());
}

#[tokio::test]
async fn test_list_prompts() {
    let server = MockMcpServer::new();
    {
        let mut prompts = server.prompts.lock().unwrap();
        prompts.push(create_sample_prompt("greeting", "A greeting", false));
    }

    let mut client = create_test_client_with_server(server).await;
    client.initialize().await.unwrap();

    let result = client.list_prompts(None).await.unwrap();

    assert_eq!(result.prompts.len(), 1);
    assert_eq!(result.prompts[0].name, "greeting");
}

#[tokio::test]
async fn test_get_prompt_with_args() {
    let server = MockMcpServer::new();
    {
        let mut prompts = server.prompts.lock().unwrap();
        prompts.push(create_sample_prompt("explain", "Explain", true));
    }

    let mut client = create_test_client_with_server(server).await;
    client.initialize().await.unwrap();

    let args: std::collections::HashMap<String, String> =
        [("topic".to_string(), "testing".to_string())]
            .into_iter()
            .collect();
    let result = client.get_prompt("explain", Some(args)).await.unwrap();

    assert!(!result.messages.is_empty());
}

#[tokio::test]
async fn test_client_close() {
    let mut client = create_test_client().await;
    client.initialize().await.unwrap();

    assert!(client.is_initialized());

    client.close().await.unwrap();

    assert!(!client.is_healthy().await);
}

#[tokio::test]
async fn test_concurrent_tool_calls() {
    let server = MockMcpServer::new();
    {
        let mut tools = server.tools.lock().unwrap();
        tools.push(create_sample_tool("tool1", "Tool 1"));
        tools.push(create_sample_tool("tool2", "Tool 2"));
    }

    let mut client = create_test_client_with_server(server).await;
    client.initialize().await.unwrap();

    let fut1 = client.call_tool("tool1", None);
    let fut2 = client.call_tool("tool2", None);

    let (res1, res2) = tokio::join!(fut1, fut2);

    assert!(res1.is_ok());
    assert!(res2.is_ok());
}
