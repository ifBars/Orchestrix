//! GLM (Z.AI) Provider Integration Tests
//!
//! These tests iteratively work to fix the 1210 "Invalid API parameter" error
//! by testing different request configurations against the live Z.AI API.

#[cfg(test)]
pub mod glm_tests {
    use crate::core::tool::ToolDescriptor;
    use crate::model::providers::glm::GlmClient;
    use crate::model::{AgentModelClient, WorkerActionRequest};
    use std::fs;

    /// Load GLM API key from the specified file path
    fn load_glm_api_key() -> String {
        fs::read_to_string(r"C:\Users\ghost\Desktop\Coding\glm-key.txt")
            .expect("Failed to read GLM API key file")
            .trim()
            .to_string()
    }

    // ====================================================================================
    // PHASE 1: MINIMAL REQUEST TESTS (Isolate the 1210 error)
    // ====================================================================================

    /// Test 1: Absolute minimal request - just model and messages
    /// This should work if the endpoint and key are valid
    #[tokio::test]
    async fn test_glm_minimal_request() {
        let api_key = load_glm_api_key();
        let client = GlmClient::new(api_key, Some("glm-4.7".to_string()), None);

        // Use the internal complete method which sends minimal parameters
        let result = client
            .complete("You are a helpful assistant.", "Say 'pong'", 100)
            .await;

        match result {
            Ok(response) => {
                println!("✅ Minimal request SUCCESS: {}", response);
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("429 Too Many Requests") || msg.contains("\"code\":\"1302\"") {
                    println!("⚠️ Minimal request rate-limited, skipping failure: {}", msg);
                } else {
                    println!("❌ Minimal request FAILED: {}", msg);
                    panic!("Minimal request should work. Error: {}", msg);
                }
            }
        }
    }

    /// Test 1b: Large max_tokens should be normalized to avoid 1210
    #[tokio::test]
    async fn test_glm_large_max_tokens_do_not_trigger_1210() {
        let api_key = load_glm_api_key();
        let client = GlmClient::new(api_key, Some("glm-4.7".to_string()), None);

        let result = client
            .complete("You are a helpful assistant.", "Say 'pong'", 180_000)
            .await;

        if let Err(e) = result {
            let msg = e.to_string();
            assert!(
                !msg.contains("\"code\":\"1210\"") && !msg.contains("\"code\":1210"),
                "Large max_tokens should be normalized, but got 1210: {}",
                msg
            );
            if msg.contains("429 Too Many Requests") || msg.contains("\"code\":\"1302\"") {
                println!(
                    "⚠️ Large max_tokens test rate-limited (acceptable): {}",
                    msg
                );
            }
        }
    }

    /// Test 2: Minimal request with different model name formats
    #[tokio::test]
    async fn test_glm_model_name_formats() {
        let api_key = load_glm_api_key();
        let model_variants = vec!["glm-4.7", "GLM-4.7", "glm-4.6", "GLM-4.6", "glm-5", "GLM-5"];

        for model in &model_variants {
            let client = GlmClient::new(api_key.clone(), Some(model.to_string()), None);
            let result = client.complete("Be brief.", "Hi", 50).await;

            match result {
                Ok(_) => println!("✅ Model '{}' works", model),
                Err(e) => println!("❌ Model '{}' failed: {}", model, e),
            }
        }
    }

    // ====================================================================================
    // PHASE 2: PARAMETER ISOLATION TESTS
    // ====================================================================================

    /// Test 3: Test with only required parameters
    /// Gradually add parameters to find which causes 1210
    #[tokio::test]
    async fn test_glm_parameter_isolation() {
        let api_key = load_glm_api_key();
        let client = GlmClient::new(api_key, Some("glm-4.7".to_string()), None);

        // Test A: No system message
        println!("\n=== Test A: No system message ===");
        let result = client.complete("", "Hello", 50).await;
        match result {
            Ok(r) => println!("✅ No system message works: {}", r),
            Err(e) => println!("❌ No system message failed: {}", e),
        }

        // Test B: With system message
        println!("\n=== Test B: With system message ===");
        let result = client.complete("You are helpful.", "Hello", 50).await;
        match result {
            Ok(r) => println!("✅ With system message works: {}", r),
            Err(e) => println!("❌ With system message failed: {}", e),
        }
    }

    // ====================================================================================
    // PHASE 3: TOOL-RELATED TESTS
    // ====================================================================================

    /// Test 4: Test with simple tool descriptor
    /// Tools often cause 1210 due to schema validation
    #[tokio::test]
    async fn test_glm_with_tools() {
        let api_key = load_glm_api_key();
        let client = GlmClient::new(api_key, Some("glm-4.7".to_string()), None);

        // Create a simple tool descriptor
        let tool = ToolDescriptor {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Input parameter"
                    }
                },
                "required": ["input"]
            }),
            output_schema: None,
        };

        let req = WorkerActionRequest {
            task_prompt: "Use the test_tool with input='hello'".to_string(),
            goal_summary: "Test tool calling".to_string(),
            context: "Testing GLM with tools".to_string(),
            available_tools: vec!["test_tool".to_string()],
            tool_descriptions: "test_tool: A test tool".to_string(),
            tool_descriptors: vec![tool],
            prior_observations: vec![],
            max_tokens: Some(500),
        };

        let result = client.decide_action(req).await;
        match result {
            Ok(decision) => println!("✅ Tool request SUCCESS: {:?}", decision),
            Err(e) => println!("❌ Tool request FAILED: {}", e),
        }
    }

    /// Test 5: Test tool schema variations
    #[tokio::test]
    async fn test_glm_tool_schema_variations() {
        let api_key = load_glm_api_key();

        let schema_variants = vec![
            // Variant 1: Minimal schema
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            // Variant 2: Schema with only type
            serde_json::json!({
                "type": "object"
            }),
            // Variant 3: Full schema with description
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path"
                    }
                },
                "required": ["path"]
            }),
        ];

        for (i, schema) in schema_variants.iter().enumerate() {
            println!("\n=== Schema Variant {} ===", i + 1);

            let client = GlmClient::new(api_key.clone(), Some("glm-4.7".to_string()), None);
            let tool = ToolDescriptor {
                name: format!("test_tool_{}", i),
                description: "Test tool".to_string(),
                input_schema: schema.clone(),
                output_schema: None,
            };

            let req = WorkerActionRequest {
                task_prompt: format!("Use test_tool_{}", i),
                goal_summary: "Test schema".to_string(),
                context: "Testing".to_string(),
                available_tools: vec![format!("test_tool_{}", i)],
                tool_descriptions: "Test".to_string(),
                tool_descriptors: vec![tool],
                prior_observations: vec![],
                max_tokens: Some(100),
            };

            let result = client.decide_action(req).await;
            match result {
                Ok(_) => println!("✅ Variant {} works", i + 1),
                Err(e) => println!("❌ Variant {} failed: {}", i + 1, e),
            }
        }
    }

    // ====================================================================================
    // PHASE 4: PLAN MODE TESTS
    // ====================================================================================

    /// Test 6: Test plan mode generation (what's currently failing)
    #[tokio::test]
    async fn test_glm_plan_mode() {
        let api_key = load_glm_api_key();
        let client = GlmClient::new(api_key, Some("glm-4.7".to_string()), None);

        let result = client
            .generate_plan_markdown("Create a simple hello world program", "", vec![])
            .await;

        match result {
            Ok(plan) => {
                println!("✅ Plan mode SUCCESS");
                println!("Plan preview: {}", &plan[..plan.len().min(200)]);
                assert!(!plan.is_empty());
            }
            Err(e) => {
                println!("❌ Plan mode FAILED: {}", e);
                // Don't panic - we expect this to fail while debugging
            }
        }
    }

    /// Test 7: Test plan mode with tools
    #[tokio::test]
    async fn test_glm_plan_mode_with_tools() {
        let api_key = load_glm_api_key();
        let client = GlmClient::new(api_key, Some("glm-4.7".to_string()), None);

        // Tools available in plan mode
        let tools = vec![
            ToolDescriptor {
                name: "fs.read".to_string(),
                description: "Read file contents".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }),
                output_schema: None,
            },
            ToolDescriptor {
                name: "fs.list".to_string(),
                description: "List directory contents".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }),
                output_schema: None,
            },
        ];

        let result = client
            .generate_plan_markdown("Explore the codebase and create a plan", "", tools)
            .await;

        match result {
            Ok(plan) => {
                println!("✅ Plan mode with tools SUCCESS");
                println!("Plan: {}", &plan[..plan.len().min(200)]);
            }
            Err(e) => {
                println!("❌ Plan mode with tools FAILED: {}", e);
            }
        }
    }

    // ====================================================================================
    // PHASE 5: ENDPOINT & CONFIGURATION TESTS
    // ====================================================================================

    /// Test 8: Test different endpoint configurations
    #[tokio::test]
    async fn test_glm_endpoint_configurations() {
        let api_key = load_glm_api_key();

        let endpoint_variants = vec![
            None, // Use default
            Some("https://api.z.ai/api/coding/paas/v4".to_string()),
            Some("https://api.z.ai/api/coding/paas/v4/".to_string()),
        ];

        for (i, endpoint) in endpoint_variants.iter().enumerate() {
            println!("\n=== Endpoint Variant {} ===", i + 1);

            let client = GlmClient::new(
                api_key.clone(),
                Some("glm-4.7".to_string()),
                endpoint.clone(),
            );

            let result = client.complete("Be brief.", "Hi", 50).await;
            match result {
                Ok(r) => println!("✅ Endpoint {} works: {}", i + 1, r),
                Err(e) => println!("❌ Endpoint {} failed: {}", i + 1, e),
            }
        }
    }

    // ====================================================================================
    // PHASE 6: STREAMING TESTS
    // ====================================================================================

    /// Test 9: Test streaming requests
    #[tokio::test]
    async fn test_glm_streaming() {
        let api_key = load_glm_api_key();
        let client = GlmClient::new(api_key, Some("glm-4.7".to_string()), None);

        let req = WorkerActionRequest {
            task_prompt: "Say hello".to_string(),
            goal_summary: "Simple greeting".to_string(),
            context: "Testing streaming".to_string(),
            available_tools: vec![],
            tool_descriptions: "".to_string(),
            tool_descriptors: vec![],
            prior_observations: vec![],
            max_tokens: Some(100),
        };

        let mut deltas = Vec::new();
        let result = client
            .decide_action_streaming(req, |delta| {
                deltas.push(format!("{:?}", delta));
                Ok(())
            })
            .await;

        match result {
            Ok(decision) => {
                println!("✅ Streaming SUCCESS");
                println!("Received {} deltas", deltas.len());
                println!("Decision: {:?}", decision);
            }
            Err(e) => {
                println!("❌ Streaming FAILED: {}", e);
            }
        }
    }

    // ====================================================================================
    // PHASE 7: DEBUGGING HELPERS
    // ====================================================================================

    /// Test 10: Print the exact request that would be sent
    /// This helps debug what's actually being serialized
    #[tokio::test]
    async fn test_glm_request_inspection() {
        use serde::Serialize;

        #[derive(Debug, Serialize)]
        struct TestRequest {
            model: String,
            messages: Vec<TestMessage>,
            temperature: f32,
            max_tokens: u32,
            stream: bool,
        }

        #[derive(Debug, Serialize)]
        struct TestMessage {
            role: String,
            content: String,
        }

        let req = TestRequest {
            model: "glm-4.7".to_string(),
            messages: vec![
                TestMessage {
                    role: "system".to_string(),
                    content: "You are helpful.".to_string(),
                },
                TestMessage {
                    role: "user".to_string(),
                    content: "Hello".to_string(),
                },
            ],
            temperature: 0.1,
            max_tokens: 100,
            stream: false,
        };

        let json_str = serde_json::to_string_pretty(&req).unwrap();
        println!("Request JSON that would be sent:\n{}", json_str);

        // Verify the structure
        assert!(json_str.contains("\"model\": \"glm-4.7\""));
        assert!(json_str.contains("\"temperature\": 0.1"));
    }

    /// Test 11: Full diagnostic run
    /// Run all variations and report what works
    #[tokio::test]
    async fn test_glm_full_diagnostic() {
        println!("\n");
        println!("╔════════════════════════════════════════════════════════════════╗");
        println!("║         GLM 1210 ERROR DIAGNOSTIC SUITE                        ║");
        println!("╚════════════════════════════════════════════════════════════════╝");

        let api_key = load_glm_api_key();

        // Test 1: Basic connectivity
        println!("\n📋 TEST 1: Basic connectivity");
        let client = GlmClient::new(api_key.clone(), Some("glm-4.7".to_string()), None);
        match client.complete("Be brief.", "Say 'ok'", 10).await {
            Ok(r) => println!("   ✅ PASS: {}", r.trim()),
            Err(e) => println!("   ❌ FAIL: {}", e),
        }

        // Test 2: Model variants
        println!("\n📋 TEST 2: Model name variants");
        for model in &["glm-4.7", "GLM-4.7"] {
            let client = GlmClient::new(api_key.clone(), Some(model.to_string()), None);
            match client.complete("Be brief.", "Hi", 10).await {
                Ok(_) => println!("   ✅ '{}' works", model),
                Err(e) => println!("   ❌ '{}' fails: {}", model, e),
            }
        }

        // Test 3: With tools (plan mode style)
        println!("\n📋 TEST 3: With tool descriptors");
        let client = GlmClient::new(api_key.clone(), Some("glm-4.7".to_string()), None);
        let tool = ToolDescriptor {
            name: "fs.read".to_string(),
            description: "Read a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
            output_schema: None,
        };
        let req = WorkerActionRequest {
            task_prompt: "Read file test.txt".to_string(),
            goal_summary: "Test".to_string(),
            context: "Testing".to_string(),
            available_tools: vec!["fs.read".to_string()],
            tool_descriptions: "fs.read: Read a file".to_string(),
            tool_descriptors: vec![tool],
            prior_observations: vec![],
            max_tokens: Some(100),
        };
        match client.decide_action(req).await {
            Ok(_) => println!("   ✅ Tool request works"),
            Err(e) => println!("   ❌ Tool request fails: {}", e),
        }

        // Test 4: Plan mode
        println!("\n📋 TEST 4: Plan mode generation");
        let client = GlmClient::new(api_key.clone(), Some("glm-4.7".to_string()), None);
        match client
            .generate_plan_markdown("Create a test plan", "", vec![])
            .await
        {
            Ok(plan) => println!("   ✅ Plan mode works ({} chars)", plan.len()),
            Err(e) => println!("   ❌ Plan mode fails: {}", e),
        }

        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!("║         END OF DIAGNOSTIC SUITE                                ║");
        println!("╚════════════════════════════════════════════════════════════════╝\n");
    }
}
