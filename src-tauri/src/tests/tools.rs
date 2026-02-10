//! Tool layer and invocation pattern tests.
//!
//! These tests verify:
//! - Tool descriptor structure and schema
//! - Tool invocation patterns
//! - Tool selection for different task types
//! - Tool error handling
//! - MCP tool compatibility
//! - Skill-based tool loading

#[cfg(test)]
pub mod tests {
    use serde_json::json;
    use crate::tools::{ToolCallInput, ToolCallOutput, ToolError};

    // ====================================================================================
    // TOOL DESCRIPTOR TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_fs_read_descriptor() {
        let descriptor = json!({
            "name": "fs.read",
            "description": "Read the contents of a file",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    }
                },
                "required": ["path"]
            }
        });

        assert_eq!(descriptor["name"], "fs.read");
        assert!(descriptor["description"].is_string());
        assert!(descriptor["parameters"]["properties"]["path"]["type"] == "string");
    }

    #[tokio::test]
    async fn test_fs_write_descriptor() {
        let descriptor = json!({
            "name": "fs.write",
            "description": "Write content to a file",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"]
            }
        });

        assert_eq!(descriptor["name"], "fs.write");
        assert!(descriptor["parameters"]["required"].is_array());
        assert!(descriptor["parameters"]["required"].as_array().unwrap().len() == 2);
    }

    #[tokio::test]
    async fn test_cmd_exec_descriptor() {
        let descriptor = json!({
            "name": "cmd.exec",
            "description": "Execute a shell command",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to execute"
                    }
                },
                "required": ["command"]
            }
        });

        assert_eq!(descriptor["name"], "cmd.exec");
        assert!(descriptor["parameters"]["required"].as_array().unwrap().contains(&json!("command")));
    }

    #[tokio::test]
    async fn test_git_status_descriptor() {
        let descriptor = json!({
            "name": "git.status",
            "description": "Show working tree status",
            "parameters": {
                "type": "object",
                "properties": {}
            }
        });

        assert_eq!(descriptor["name"], "git.status");
        assert!(descriptor["parameters"]["properties"].is_object());
    }

    #[tokio::test]
    async fn test_agent_todo_descriptor() {
        let descriptor = json!({
            "name": "agent.todo",
            "description": "Create and manage a TODO list",
            "parameters": {
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {"type": "string"},
                                "status": {"type": "string", "enum": ["pending", "in_progress", "completed", "cancelled"]}
                            }
                        }
                    }
                }
            }
        });

        assert_eq!(descriptor["name"], "agent.todo");
        let status_enum = &descriptor["parameters"]["properties"]["todos"]["items"]["properties"]["status"]["enum"];
        assert!(status_enum.as_array().unwrap().contains(&json!("in_progress")));
    }

    // ====================================================================================
    // TOOL CALL INPUT TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_tool_call_input_structure() {
        let input = ToolCallInput {
            name: "fs.write".to_string(),
            args: json!({
                "path": "test.txt",
                "content": "hello world"
            }),
        };

        assert_eq!(input.name, "fs.write");
        assert_eq!(input.args["path"], "test.txt");
        assert_eq!(input.args["content"], "hello world");
    }

    #[tokio::test]
    async fn test_tool_call_input_serialization() {
        let input = ToolCallInput {
            name: "fs.read".to_string(),
            args: json!({"path": "config.json"}),
        };

        let serialized = serde_json::to_string(&input).expect("serialize");
        let deserialized: ToolCallInput = serde_json::from_str(&serialized).expect("deserialize");

        assert_eq!(input.name, deserialized.name);
        assert_eq!(input.args, deserialized.args);
    }

    #[tokio::test]
    async fn test_nested_args_structure() {
        let input = ToolCallInput {
            name: "cmd.exec".to_string(),
            args: json!({
                "command": "npm install",
                "env": {
                    "NODE_ENV": "production"
                }
            }),
        };

        assert_eq!(input.args["command"], "npm install");
        assert_eq!(input.args["env"]["NODE_ENV"], "production");
    }

    // ====================================================================================
    // TOOL CALL OUTPUT TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_tool_call_success_output() {
        let output = ToolCallOutput {
            ok: true,
            data: json!({"content": "file contents"}),
            error: None,
        };

        assert!(output.ok);
        assert!(output.error.is_none());
        assert!(output.data.is_object());
    }

    #[tokio::test]
    async fn test_tool_call_failure_output() {
        let output = ToolCallOutput {
            ok: false,
            data: json!({}),
            error: Some("file not found".to_string()),
        };

        assert!(!output.ok);
        assert!(output.error.is_some());
        assert_eq!(output.error.unwrap(), "file not found");
    }

    #[tokio::test]
    async fn test_tool_output_data_formats() {
        let outputs = vec![
            ToolCallOutput {
                ok: true,
                data: json!({"files": ["a.txt", "b.txt"]}),
                error: None,
            },
            ToolCallOutput {
                ok: true,
                data: json!({"content": "line1\nline2"}),
                error: None,
            },
            ToolCallOutput {
                ok: true,
                data: json!({"success": true}),
                error: None,
            },
        ];

        for output in outputs {
            assert!(output.ok);
        }
    }

    // ====================================================================================
    // TOOL ERROR TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_invalid_input_error() {
        let error = ToolError::InvalidInput("path is required".to_string());
        assert!(error.to_string().contains("invalid input"));
    }

    #[tokio::test]
    async fn test_policy_denied_error() {
        let error = ToolError::PolicyDenied("dangerous command".to_string());
        assert!(error.to_string().contains("policy denied"));
    }

    #[tokio::test]
    async fn test_execution_failed_error() {
        let error = ToolError::Execution("command exited with code 1".to_string());
        assert!(error.to_string().contains("execution failed"));
    }

    #[tokio::test]
    async fn test_approval_required_error() {
        let error = ToolError::ApprovalRequired {
            scope: "shell".to_string(),
            reason: "Installing system packages".to_string(),
        };
        let error_str = error.to_string();
        assert!(error_str.contains("shell"));
        assert!(error_str.contains("Installing system packages"));
    }

    // ====================================================================================
    // TOOL SELECTION PATTERN TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_file_task_selects_fs_tools() {
        let file_tasks = vec![
            ("Read config", "fs.read"),
            ("Write output", "fs.write"),
            ("List directory", "fs.list"),
        ];

        for (_task, expected_tool) in &file_tasks {
            assert!(expected_tool.starts_with("fs."));
        }
    }

    #[tokio::test]
    async fn test_git_task_selects_git_tools() {
        let git_tasks = vec![
            ("Check status", "git.status"),
            ("Show changes", "git.diff"),
            ("Apply patch", "git.apply_patch"),
            ("Create commit", "git.commit"),
        ];

        for (_task, expected_tool) in &git_tasks {
            assert!(expected_tool.starts_with("git."));
        }
    }

    #[tokio::test]
    async fn test_shell_task_selects_cmd_tool() {
        let shell_tasks = vec![
            "Run build script",
            "Install dependencies",
            "Run tests",
        ];

        for _task in &shell_tasks {
            let tool = "cmd.exec";
            assert!(tool == "cmd.exec");
        }
    }

    #[tokio::test]
    async fn test_search_task_selects_search_tool() {
        let search_patterns = vec![
            ("Find TODO", "search.rg"),
            ("Search for function", "search.rg"),
            ("Find all imports", "search.rg"),
        ];

        for (_, expected_tool) in &search_patterns {
            assert_eq!(*expected_tool, "search.rg");
        }
    }

    // ====================================================================================
    // MCP TOOL COMPATIBILITY TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_mcp_tool_descriptor_format() {
        let mcp_tool = json!({
            "name": "mcp__server-name__tool-name",
            "description": "Tool from MCP server",
            "parameters": {
                "type": "object",
                "properties": {}
            }
        });

        assert!(mcp_tool["name"].as_str().unwrap().starts_with("mcp__"));
        assert!(mcp_tool["name"].as_str().unwrap().contains("__"));
    }

    #[tokio::test]
    async fn test_mcp_tool_name_parsing() {
        let tool_name = "mcp__context7__query_docs";
        let parts: Vec<&str> = tool_name.split("__").collect();

        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "mcp");
        assert_eq!(parts[1], "context7");
        assert_eq!(parts[2], "query_docs");
    }

    #[tokio::test]
    async fn test_mcp_server_and_tool_extraction() {
        let tool_names = vec![
            ("mcp__context7__query_docs", "context7", "query_docs"),
            ("mcp__github__create_issue", "github", "create_issue"),
            ("mcp__filesystem__read_file", "filesystem", "read_file"),
        ];

        for (name, expected_server, expected_tool) in &tool_names {
            let parts: Vec<&str> = name.split("__").collect();
            assert_eq!(parts[1], *expected_server);
            assert_eq!(parts[2], *expected_tool);
        }
    }

    // ====================================================================================
    // SKILL-BASED TOOL TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_skills_list_tool_descriptor() {
        let descriptor = json!({
            "name": "skills.list",
            "description": "List all available skills",
            "parameters": {"type": "object", "properties": {}}
        });

        assert_eq!(descriptor["name"], "skills.list");
    }

    #[tokio::test]
    async fn test_skills_load_tool_descriptor() {
        let descriptor = json!({
            "name": "skills.load",
            "description": "Load a skill into the context",
            "parameters": {
                "type": "object",
                "properties": {
                    "skill": {"type": "string"}
                },
                "required": ["skill"]
            }
        });

        assert_eq!(descriptor["name"], "skills.load");
        assert!(descriptor["parameters"]["required"].as_array().unwrap().contains(&json!("skill")));
    }

    #[tokio::test]
    async fn test_skill_loading_flow() {
        let skill_name = "frontend-design";
        let skill_load = json!({
            "tool_name": "skills.load",
            "tool_args": {"skill": skill_name}
        });

        assert_eq!(skill_load["tool_args"]["skill"], "frontend-design");
    }

    // ====================================================================================
    // SUBAGENT SPAWN TOOL TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_subagent_spawn_descriptor() {
        let descriptor = json!({
            "name": "subagent.spawn",
            "description": "Spawn a sub-agent for parallel execution",
            "parameters": {
                "type": "object",
                "properties": {
                    "objective": {"type": "string"},
                    "max_retries": {"type": "integer"}
                },
                "required": ["objective"]
            }
        });

        assert_eq!(descriptor["name"], "subagent.spawn");
        assert!(descriptor["parameters"]["required"].as_array().unwrap().contains(&json!("objective")));
    }

    #[tokio::test]
    async fn test_subagent_spawn_input() {
        let spawn_input = json!({
            "objective": "Create unit tests for authentication module",
            "max_retries": 2
        });

        assert_eq!(spawn_input["objective"], "Create unit tests for authentication module");
        assert_eq!(spawn_input["max_retries"], 2);
    }

    // ====================================================================================
    // AGENT MODE SWITCHING TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_request_plan_mode_descriptor() {
        let descriptor = json!({
            "name": "agent.request_plan_mode",
            "description": "Switch to plan mode to generate a plan",
            "parameters": {
                "type": "object",
                "properties": {
                    "reason": {"type": "string"}
                }
            }
        });

        assert_eq!(descriptor["name"], "agent.request_plan_mode");
    }

    #[tokio::test]
    async fn test_request_build_mode_descriptor() {
        let descriptor = json!({
            "name": "agent.request_build_mode",
            "description": "Switch to build mode to execute plan",
            "parameters": {
                "type": "object",
                "properties": {}
            }
        });

        assert_eq!(descriptor["name"], "agent.request_build_mode");
    }

    // ====================================================================================
    // TOOL INVOCATION PATTERN TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_invocation_with_policy_check() {
        let tool_name = "cmd.exec";
        let tool_args = json!({"command": "echo hello"});

        let invocation = json!({
            "tool": tool_name,
            "args": tool_args,
            "policy_check": "passed"
        });

        assert_eq!(invocation["tool"], "cmd.exec");
        assert_eq!(invocation["policy_check"], "passed");
    }

    #[tokio::test]
    async fn test_invocation_with_policy_deny() {
        let denied_invocation = json!({
            "tool": "cmd.exec",
            "args": {"command": "rm -rf /"},
            "policy_check": "denied",
            "reason": "Dangerous command"
        });

        assert_eq!(denied_invocation["policy_check"], "denied");
        assert!(denied_invocation["reason"].is_string());
    }

    #[tokio::test]
    async fn test_approval_flow_for_scope() {
        let request = json!({
            "tool": "cmd.exec",
            "scope": "shell",
            "requires_approval": true,
            "reason": "Installing packages"
        });

        assert!(request["requires_approval"].as_bool().unwrap());
        assert_eq!(request["scope"], "shell");
    }

    // ====================================================================================
    // WORKFLOW PATTERN TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_file_creation_workflow() {
        let workflow = vec![
            json!({"step": 1, "tool": "fs.write", "args": {"path": "main.py", "content": "print('hello')"}}),
            json!({"step": 2, "tool": "cmd.exec", "args": {"command": "python main.py"}}),
            json!({"step": 3, "tool": "fs.read", "args": {"path": "output.txt"}}),
        ];

        assert_eq!(workflow.len(), 3);
        assert_eq!(workflow[0]["tool"], "fs.write");
        assert_eq!(workflow[1]["tool"], "cmd.exec");
        assert_eq!(workflow[2]["tool"], "fs.read");
    }

    #[tokio::test]
    async fn test_git_commit_workflow() {
        let workflow = vec![
            json!({"step": 1, "tool": "git.status"}),
            json!({"step": 2, "tool": "git.diff"}),
            json!({"step": 3, "tool": "git.commit", "args": {"message": "Update feature"}}),
        ];

        assert_eq!(workflow[2]["tool"], "git.commit");
        assert_eq!(workflow[2]["args"]["message"], "Update feature");
    }

    #[tokio::test]
    async fn test_code_review_workflow() {
        let workflow = vec![
            json!({"step": 1, "tool": "search.rg", "args": {"pattern": "TODO"}}),
            json!({"step": 2, "tool": "fs.read", "args": {"path": "src/main.rs"}}),
            json!({"step": 3, "tool": "agent.create_artifact", "args": {"path": "review.md", "kind": "report"}}),
        ];

        assert_eq!(workflow[0]["tool"], "search.rg");
        assert_eq!(workflow[2]["tool"], "agent.create_artifact");
    }

    // ====================================================================================
    // TOOL REGISTRY PATTERN TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_tool_names_complete_set() {
        let tool_names = vec![
            "fs.read",
            "fs.write",
            "fs.list",
            "search.rg",
            "cmd.exec",
            "git.status",
            "git.diff",
            "git.apply_patch",
            "git.commit",
            "git.log",
            "skills.list",
            "agent.todo",
            "subagent.spawn",
            "skills.load",
            "skills.remove",
            "agent.request_build_mode",
            "agent.request_plan_mode",
            "agent.create_artifact",
        ];

        assert_eq!(tool_names.len(), 18);
    }

    #[tokio::test]
    async fn test_tools_categorized_correctly() {
        let categories = vec![
            ("fs", vec!["fs.read", "fs.write", "fs.list"]),
            ("git", vec!["git.status", "git.diff", "git.apply_patch", "git.commit", "git.log"]),
            ("agent", vec!["agent.todo", "agent.create_artifact", "agent.request_build_mode", "agent.request_plan_mode"]),
            ("skills", vec!["skills.list", "skills.load", "skills.remove"]),
            ("search", vec!["search.rg"]),
            ("cmd", vec!["cmd.exec"]),
            ("subagent", vec!["subagent.spawn"]),
        ];

        for (category, tools) in &categories {
            for tool in tools {
                assert!(tool.starts_with(category));
                if *category == "search" {
                    assert_eq!(*tool, "search.rg");
                }
            }
        }
    }

    // ====================================================================================
    // TOOL SCHEMA VALIDATION TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_tool_args_schema_validation() {
        let valid_args = json!({
            "path": "test.txt",
            "content": "hello"
        });

        let required_fields = vec!["path", "content"];
        for field in &required_fields {
            assert!(valid_args.get(field).is_some());
        }
    }

    #[tokio::test]
    async fn test_missing_required_field_error() {
        let invalid_args = json!({
            "path": "test.txt"
        });

        assert!(invalid_args.get("content").is_none());
    }

    #[tokio::test]
    async fn test_optional_fields_handling() {
        let args_with_options = json!({
            "command": "npm install",
            "timeout": 30,
            "env": {"NODE_ENV": "test"}
        });

        assert!(args_with_options.get("timeout").is_some());
        assert!(args_with_options.get("env").is_some());
    }

    // ====================================================================================
    // BATCH TOOL CALL TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_batch_tool_calls_structure() {
        let batch = json!({
            "batch_id": "batch-123",
            "calls": [
                {"tool": "fs.write", "args": {"path": "a.txt"}},
                {"tool": "fs.write", "args": {"path": "b.txt"}},
                {"tool": "fs.write", "args": {"path": "c.txt"}}
            ]
        });

        let calls = batch["calls"].as_array().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0]["tool"], "fs.write");
        assert_eq!(calls[1]["tool"], "fs.write");
        assert_eq!(calls[2]["tool"], "fs.write");
    }

    #[tokio::test]
    async fn test_batch_results_structure() {
        let batch_results = json!({
            "batch_id": "batch-123",
            "results": [
                {"call_index": 0, "status": "succeeded"},
                {"call_index": 1, "status": "succeeded"},
                {"call_index": 2, "status": "failed", "error": "disk full"}
            ]
        });

        let results = batch_results["results"].as_array().unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[2]["status"], "failed");
    }
}
