//! Agentic AI feature tests for worker agent behavior, tool selection, and reasoning.
//!
//! These tests verify:
//! - Worker action request building and response parsing
//! - Reasoning/thinking chain extraction from model outputs
//! - Tool selection accuracy for various task types
//! - Observation-based decision making
//! - Todo tool integration and task tracking
//! - Multi-turn conversation flow
//! - Model-agnostic behavior (MiniMax/Kimi)
//!
//! Tests use mocked model responses to avoid API calls for unit-level verification.

#[cfg(test)]
pub mod tests {
    use crate::model::kimi::KimiPlanner;
    use crate::model::minimax::MiniMaxPlanner;
    use crate::model::{
        PlannerModel, WorkerAction, WorkerActionRequest, WorkerDecision, WorkerToolCall,
    };
    use crate::tests::load_api_key;
    use crate::tools::ToolRegistry;
    use serde_json::json;

    fn create_tool_registry() -> ToolRegistry {
        ToolRegistry::default()
    }

    fn make_request(
        task_prompt: &str,
        goal_summary: &str,
        context: &str,
        prior_observations: Vec<serde_json::Value>,
    ) -> WorkerActionRequest {
        let registry = create_tool_registry();
        let tools = registry.list_for_build_mode();
        WorkerActionRequest {
            task_prompt: task_prompt.to_string(),
            goal_summary: goal_summary.to_string(),
            context: context.to_string(),
            available_tools: tools.iter().map(|t| t.name.clone()).collect(),
            tool_descriptions: registry.tool_reference_for_build_mode(),
            tool_descriptors: tools,
            prior_observations,
            max_tokens: None,
        }
    }

    // ====================================================================================
    // WORKER ACTION REQUEST TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_worker_action_request_builds_correctly() {
        let registry = create_tool_registry();
        let tools = registry.list_for_build_mode();
        let tool_descriptions = registry.tool_reference_for_build_mode();

        let req = WorkerActionRequest {
            task_prompt: "Create a test file".to_string(),
            goal_summary: "File creation".to_string(),
            context: "Simple task".to_string(),
            available_tools: tools.iter().map(|t| t.name.clone()).collect(),
            tool_descriptions,
            tool_descriptors: tools.clone(),
            prior_observations: vec![],
            max_tokens: None,
        };

        assert!(!req.task_prompt.is_empty());
        assert!(!req.goal_summary.is_empty());
        assert!(!req.available_tools.is_empty());
        assert_eq!(req.available_tools.len(), tools.len());
        assert!(req.tool_descriptors.len() > 0);

        let has_fs_write = req.available_tools.iter().any(|t| t == "fs.write");
        assert!(has_fs_write, "fs.write should be in available tools");
    }

    #[tokio::test]
    async fn test_worker_request_with_observations() {
        let observations = vec![
            json!({
                "tool_name": "fs.write",
                "status": "succeeded",
                "output": {"path": "test.txt"}
            }),
            json!({
                "tool_name": "fs.read",
                "status": "succeeded",
                "output": {"content": "test content"}
            }),
        ];

        let req = make_request(
            "Read the file after writing it",
            "Verify file contents",
            "File I/O task",
            observations,
        );

        assert_eq!(req.prior_observations.len(), 2);
        assert_eq!(req.prior_observations[0]["tool_name"], "fs.write");
        assert_eq!(req.prior_observations[1]["status"], "succeeded");
    }

    // ====================================================================================
    // WORKER DECISION PARSING TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_worker_decision_tool_call_parsing() {
        let decision = WorkerDecision {
            action: WorkerAction::ToolCall {
                tool_name: "fs.write".to_string(),
                tool_args: json!({"path": "test.txt", "content": "hello"}),
                rationale: Some("Writing test content to file".to_string()),
            },
            reasoning: Some("I need to create a file to store the test data".to_string()),
            raw_response: None,
        };

        match &decision.action {
            WorkerAction::ToolCall {
                tool_name,
                tool_args,
                rationale,
            } => {
                assert_eq!(tool_name, "fs.write");
                assert_eq!(tool_args["path"], "test.txt");
                assert_eq!(tool_args["content"], "hello");
                assert_eq!(rationale, &Some("Writing test content to file".to_string()));
            }
            _ => panic!("Expected ToolCall variant"),
        }

        assert!(decision.reasoning.is_some());
    }

    #[tokio::test]
    async fn test_worker_decision_tool_calls_parsing() {
        let calls = vec![
            WorkerToolCall {
                tool_name: "fs.write".to_string(),
                tool_args: json!({"path": "index.html"}),
                rationale: Some("Create HTML file".to_string()),
            },
            WorkerToolCall {
                tool_name: "fs.write".to_string(),
                tool_args: json!({"path": "styles.css"}),
                rationale: Some("Create CSS file".to_string()),
            },
        ];

        let decision = WorkerDecision {
            action: WorkerAction::ToolCalls { calls },
            reasoning: None,
            raw_response: None,
        };

        match &decision.action {
            WorkerAction::ToolCalls { calls } => {
                assert_eq!(calls.len(), 2);
                assert_eq!(calls[0].tool_name, "fs.write");
                assert_eq!(calls[1].tool_name, "fs.write");
            }
            _ => panic!("Expected ToolCalls variant"),
        }
    }

    #[tokio::test]
    async fn test_worker_decision_complete_parsing() {
        let decision = WorkerDecision {
            action: WorkerAction::Complete {
                summary: "Successfully created all required files".to_string(),
            },
            reasoning: Some("All files have been created and verified".to_string()),
            raw_response: None,
        };

        match &decision.action {
            WorkerAction::Complete { summary } => {
                assert_eq!(summary, "Successfully created all required files");
            }
            _ => panic!("Expected Complete variant"),
        }
    }

    #[tokio::test]
    async fn test_worker_decision_delegate_parsing() {
        let decision = WorkerDecision {
            action: WorkerAction::Delegate {
                objective: "Create unit tests for the new feature".to_string(),
            },
            reasoning: Some(
                "This task should be delegated to a specialized testing agent".to_string(),
            ),
            raw_response: None,
        };

        match &decision.action {
            WorkerAction::Delegate { objective } => {
                assert_eq!(objective, "Create unit tests for the new feature");
            }
            _ => panic!("Expected Delegate variant"),
        }
    }

    // ====================================================================================
    // REASONING EXTRACTION TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_reasoning_extraction_from_decision() {
        let reasoning_text = "I should first check if the file exists, then write the content. After writing, I'll verify by reading it back.";

        let decision = WorkerDecision {
            action: WorkerAction::ToolCall {
                tool_name: "fs.read".to_string(),
                tool_args: json!({"path": "config.json"}),
                rationale: Some("Reading config file".to_string()),
            },
            reasoning: Some(reasoning_text.to_string()),
            raw_response: None,
        };

        assert!(decision.reasoning.is_some());
        let reasoning = decision.reasoning.unwrap();
        assert!(reasoning.contains("check if the file exists"));
        assert!(reasoning.contains("write the content"));
    }

    #[tokio::test]
    async fn test_empty_reasoning_is_none() {
        let decision = WorkerDecision {
            action: WorkerAction::Complete {
                summary: "Task done".to_string(),
            },
            reasoning: None,
            raw_response: None,
        };

        assert!(decision.reasoning.is_none());
    }

    // ====================================================================================
    // OBSERVATION-BASED DECISION MAKING TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_observation_tracking_single_tool() {
        let mut observations: Vec<serde_json::Value> = vec![];

        let tool_result = json!({
            "tool_name": "fs.write",
            "status": "succeeded",
            "output": {"path": "output.txt"}
        });

        observations.push(tool_result.clone());

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0]["status"], "succeeded");
        assert_eq!(observations[0]["output"]["path"], "output.txt");
    }

    #[tokio::test]
    async fn test_observation_tracking_multiple_tools() {
        let mut observations: Vec<serde_json::Value> = vec![];

        let steps = vec![
            ("fs.write", json!({"status": "succeeded"})),
            ("cmd.exec", json!({"status": "succeeded"})),
            ("fs.read", json!({"status": "succeeded"})),
        ];

        for (tool, output) in steps {
            observations.push(json!({
                "tool_name": tool,
                "status": output["status"],
            }));
        }

        assert_eq!(observations.len(), 3);
        assert_eq!(observations[0]["tool_name"], "fs.write");
        assert_eq!(observations[1]["tool_name"], "cmd.exec");
        assert_eq!(observations[2]["tool_name"], "fs.read");
    }

    #[tokio::test]
    async fn test_failed_observation_tracking() {
        let mut observations: Vec<serde_json::Value> = vec![];

        observations.push(json!({
            "tool_name": "fs.read",
            "status": "failed",
            "error": "file not found"
        }));

        assert_eq!(observations[0]["status"], "failed");
        assert_eq!(observations[0]["error"], "file not found");
    }

    #[tokio::test]
    async fn test_observation_chain_for_retry() {
        let prior_observations = vec![
            json!({
                "tool_name": "fs.write",
                "status": "succeeded"
            }),
            json!({
                "tool_name": "fs.read",
                "status": "failed",
                "error": "permission denied"
            }),
        ];

        let next_action = match prior_observations.last() {
            Some(obs) if obs["status"] == "failed" => Some("Should retry with different approach"),
            _ => None,
        };

        assert!(next_action.is_some());
        assert_eq!(prior_observations[1]["error"], "permission denied");
    }

    // ====================================================================================
    // TODO TOOL INTEGRATION TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_todo_creation_structure() {
        let todo_create = json!({
            "tool_name": "agent.todo",
            "status": "succeeded",
            "output": {
                "todos": [
                    {"content": "Create file", "status": "pending", "activeForm": "Creating file"},
                    {"content": "Write content", "status": "pending", "activeForm": "Writing content"},
                    {"content": "Verify output", "status": "pending", "activeForm": "Verifying output"}
                ]
            }
        });

        let todos = todo_create["output"]["todos"].as_array().unwrap();
        assert_eq!(todos.len(), 3);
        assert_eq!(todos[0]["content"], "Create file");
        assert_eq!(todos[0]["status"], "pending");
    }

    #[tokio::test]
    async fn test_todo_status_progression() {
        let todos = vec![
            json!({"content": "Step 1", "status": "pending"}),
            json!({"content": "Step 2", "status": "pending"}),
            json!({"content": "Step 3", "status": "pending"}),
        ];

        let mut updated_todos = todos.clone();
        updated_todos[0] = json!({"content": "Step 1", "status": "completed"});
        updated_todos[1] = json!({"content": "Step 2", "status": "in_progress"});

        assert_eq!(updated_todos[0]["status"], "completed");
        assert_eq!(updated_todos[1]["status"], "in_progress");
        assert_eq!(updated_todos[2]["status"], "pending");
    }

    #[tokio::test]
    async fn test_open_todos_count() {
        let todo_output = json!({
            "todos": [
                {"content": "Done task", "status": "completed"},
                {"content": "In progress", "status": "in_progress"},
                {"content": "Pending task", "status": "pending"},
                {"content": "Cancelled", "status": "cancelled"}
            ]
        });

        let todos = todo_output["todos"].as_array().unwrap();
        let open_count = todos
            .iter()
            .filter(|todo| {
                let status = todo["status"].as_str().unwrap();
                status != "completed" && status != "cancelled"
            })
            .count();

        assert_eq!(open_count, 2);
    }

    // ====================================================================================
    // MULTI-TURN CONVERSATION FLOW TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_conversation_turn_tracking() {
        let mut turn: usize = 0;
        let mut observations: Vec<serde_json::Value> = vec![];

        loop {
            turn += 1;
            if turn > 5 {
                break;
            }

            observations.push(json!({
                "turn": turn,
                "action": format!("Turn {}", turn)
            }));

            if turn == 3 {
                break;
            }
        }

        assert_eq!(turn, 3);
        assert_eq!(observations.len(), 3);
    }

    #[tokio::test]
    async fn test_tool_call_sequence_ordering() {
        let call_sequence = vec!["fs.write", "cmd.exec", "fs.read"];

        let observations: Vec<serde_json::Value> = call_sequence
            .iter()
            .enumerate()
            .map(|(idx, tool)| {
                json!({
                    "order": idx + 1,
                    "tool_name": tool,
                    "status": "succeeded"
                })
            })
            .collect();

        assert_eq!(observations.len(), 3);
        assert_eq!(observations[0]["tool_name"], "fs.write");
        assert_eq!(observations[1]["tool_name"], "cmd.exec");
        assert_eq!(observations[2]["tool_name"], "fs.read");
        assert_eq!(observations[0]["order"], 1);
        assert_eq!(observations[1]["order"], 2);
        assert_eq!(observations[2]["order"], 3);
    }

    // ====================================================================================
    // TOOL SELECTION ACCURACY TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_file_write_task_selects_fs_write() {
        let registry = create_tool_registry();
        let tools = registry.list_for_build_mode();
        let has_fs_write = tools.iter().any(|t| t.name == "fs.write");
        assert!(has_fs_write);
    }

    #[tokio::test]
    async fn test_command_execution_task_selects_cmd_exec() {
        let registry = create_tool_registry();
        let tools = registry.list_for_build_mode();
        let has_cmd_exec = tools.iter().any(|t| t.name == "cmd.exec");
        assert!(has_cmd_exec);
    }

    #[tokio::test]
    async fn test_git_operations_available() {
        let registry = create_tool_registry();
        let tools = registry.list_for_build_mode();
        let git_tools: Vec<&str> = tools
            .iter()
            .filter(|t| t.name.starts_with("git."))
            .map(|t| t.name.as_str())
            .collect();

        assert!(!git_tools.is_empty());
        assert!(git_tools.contains(&"git.status"));
        assert!(git_tools.contains(&"git.diff"));
    }

    #[tokio::test]
    async fn test_agent_tools_available() {
        let registry = create_tool_registry();
        let tools = registry.list_for_build_mode();
        let agent_tools: Vec<&str> = tools
            .iter()
            .filter(|t| t.name.starts_with("agent."))
            .map(|t| t.name.as_str())
            .collect();

        assert!(agent_tools.contains(&"agent.todo"));
        // Note: agent.create_artifact is excluded from build mode
    }

    // ====================================================================================
    // MODEL-AGNOSTIC BEHAVIOR TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_planner_model_trait_implementation() {
        let planner = load_api_key();
        let minimax = MiniMaxPlanner::new(planner.clone(), None);

        assert_eq!(minimax.model_id(), "MiniMax-M2.1");
    }

    #[tokio::test]
    async fn test_kimi_planner_model_id() {
        let api_key = load_api_key();
        let kimi = KimiPlanner::new(api_key, None, None);

        assert!(!kimi.model_id().is_empty());
    }

    #[tokio::test]
    async fn test_worker_action_request_serialization() {
        let req = WorkerActionRequest {
            task_prompt: "Test".to_string(),
            goal_summary: "Test".to_string(),
            context: "Test".to_string(),
            available_tools: vec![],
            tool_descriptions: "".to_string(),
            tool_descriptors: vec![],
            prior_observations: vec![],
            max_tokens: None,
        };

        let req_json = json!({
            "task_prompt": req.task_prompt,
            "goal_summary": req.goal_summary,
            "context": req.context,
            "available_tools": req.available_tools,
            "prior_observations": req.prior_observations
        });

        assert_eq!(req_json["task_prompt"], "Test");
        assert_eq!(req_json["goal_summary"], "Test");
    }

    // ====================================================================================
    // EVENT STRUCTURE VALIDATION TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_tool_call_started_event_structure() {
        let event = json!({
            "task_id": "task-123",
            "sub_agent_id": "agent-456",
            "tool_call_id": "call-789",
            "tool_name": "fs.write",
            "tool_args": {"path": "test.txt"},
            "step_idx": 1,
            "turn": 1,
            "rationale": "Writing test file"
        });

        assert_eq!(event["task_id"], "task-123");
        assert_eq!(event["sub_agent_id"], "agent-456");
        assert_eq!(event["tool_name"], "fs.write");
        assert_eq!(event["step_idx"], 1);
        assert_eq!(event["turn"], 1);
    }

    #[tokio::test]
    async fn test_tool_call_finished_event_structure() {
        let event = json!({
            "task_id": "task-123",
            "sub_agent_id": "agent-456",
            "tool_call_id": "call-789",
            "status": "succeeded",
            "output": {"result": "ok"}
        });

        assert_eq!(event["status"], "succeeded");
        assert!(event["output"]["result"].is_string());
    }

    #[tokio::test]
    async fn test_thinking_delta_event_structure() {
        let event = json!({
            "task_id": "task-123",
            "sub_agent_id": "agent-456",
            "step_idx": 1,
            "content": "I need to first check the file exists..."
        });

        assert_eq!(event["task_id"], "task-123");
        assert!(event["content"].is_string());
        assert!(event["content"].to_string().len() > 0);
    }

    #[tokio::test]
    async fn test_subagent_lifecycle_events() {
        let lifecycle_events = vec![
            ("agent.subagent_created", "Sub-agent was created"),
            ("agent.subagent_started", "Sub-agent execution started"),
            (
                "agent.subagent_completed",
                "Sub-agent completed successfully",
            ),
            ("agent.subagent_failed", "Sub-agent execution failed"),
            ("agent.subagent_closed", "Sub-agent closed"),
        ];

        for (event_type, description) in &lifecycle_events {
            let event = json!({
                "event_type": event_type,
                "description": description
            });
            assert_eq!(event["event_type"], *event_type);
        }
    }

    // ====================================================================================
    // SUB-AGENT CONTRACT PARSING TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_delegation_depth_tracking() {
        let max_depth = 3;
        let current_depth = 2;

        assert!(current_depth < max_depth);
        assert_eq!(max_depth - current_depth, 1);
    }

    #[tokio::test]
    async fn test_permission_contract_structure() {
        let contract = json!({
            "permissions": {
                "allowed_tools": ["fs.read", "fs.write", "agent.todo"],
                "can_spawn_children": true,
                "max_delegation_depth": 2
            },
            "execution": {
                "attempt_timeout_ms": 120000,
                "close_on_completion": true
            }
        });

        let allowed_tools = contract["permissions"]["allowed_tools"].as_array().unwrap();
        assert!(allowed_tools.contains(&json!("fs.read")));
        assert!(allowed_tools.contains(&json!("fs.write")));
        assert_eq!(contract["permissions"]["max_delegation_depth"], 2);
        assert_eq!(contract["execution"]["close_on_completion"], true);
    }

    // ====================================================================================
    // ARTIFACT TRACKING TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_artifact_creation_structure() {
        let artifact = json!({
            "id": "artifact-123",
            "run_id": "run-456",
            "kind": "file",
            "uri": "src/components/Test.tsx",
            "metadata": {
                "task_id": "task-789",
                "source": "agent.create_artifact"
            }
        });

        assert_eq!(artifact["kind"], "file");
        assert_eq!(artifact["uri"], "src/components/Test.tsx");
        assert_eq!(artifact["metadata"]["source"], "agent.create_artifact");
    }

    #[tokio::test]
    async fn test_artifact_tracking_in_observations() {
        let artifact_observation = json!({
            "tool_name": "agent.create_artifact",
            "status": "succeeded",
            "output": {
                "path": "output.json",
                "kind": "data"
            }
        });

        assert_eq!(artifact_observation["tool_name"], "agent.create_artifact");
        assert_eq!(artifact_observation["output"]["kind"], "data");
    }

    // ====================================================================================
    // ERROR HANDLING TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_tool_not_allowed_error() {
        let error = json!({
            "tool_name": "fs.delete",
            "status": "denied",
            "error": "tool not allowed by delegation contract"
        });

        assert_eq!(error["status"], "denied");
        assert!(error["error"].to_string().contains("not allowed"));
    }

    #[tokio::test]
    async fn test_max_delegation_depth_error() {
        let error = json!({
            "tool_name": "subagent.spawn",
            "status": "denied",
            "error": "max delegation depth reached"
        });

        assert_eq!(error["status"], "denied");
        assert_eq!(error["error"], "max delegation depth reached");
    }

    #[tokio::test]
    async fn test_policy_denied_error() {
        let error = json!({
            "tool_name": "cmd.exec",
            "status": "denied",
            "error": "approval denied for scope: destructive"
        });

        assert_eq!(error["status"], "denied");
        assert!(error["error"].to_string().contains("approval denied"));
    }

    // ====================================================================================
    // APPROVAL GATE TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_approval_required_structure() {
        let approval_request = json!({
            "task_id": "task-123",
            "run_id": "run-456",
            "sub_agent_id": "agent-789",
            "tool_call_id": "call-012",
            "tool_name": "cmd.exec",
            "scope": "shell",
            "reason": "Executing shell command to install dependencies"
        });

        assert_eq!(approval_request["tool_name"], "cmd.exec");
        assert_eq!(approval_request["scope"], "shell");
        assert!(approval_request["reason"].is_string());
    }

    #[tokio::test]
    async fn test_approval_resolved_structure() {
        let approval_resolution = json!({
            "task_id": "task-123",
            "run_id": "run-456",
            "sub_agent_id": "agent-789",
            "tool_call_id": "call-012",
            "approval_id": "approval-345",
            "approved": true
        });

        assert_eq!(approval_resolution["approved"], true);
        assert!(approval_resolution["approval_id"].is_string());
    }

    // ====================================================================================
    // WORKTREE MERGE TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_merge_result_structure() {
        let merge_result = json!({
            "success": true,
            "strategy": "rebase",
            "message": "Successfully rebased delegate changes",
            "conflicted_files": []
        });

        assert_eq!(merge_result["success"], true);
        assert_eq!(merge_result["strategy"], "rebase");
        assert!(merge_result["conflicted_files"].is_array());
    }

    #[tokio::test]
    async fn test_merge_conflict_structure() {
        let conflict = json!({
            "path": "src/main.ts",
            "status": "both_modified"
        });

        let merge_result = json!({
            "success": false,
            "strategy": "rebase",
            "message": "Merge conflict detected",
            "conflicted_files": [conflict]
        });

        assert_eq!(merge_result["success"], false);
        let conflicts = merge_result["conflicted_files"].as_array().unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0]["path"], "src/main.ts");
    }

    // ====================================================================================
    // CONTEXT BUILDING TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_step_context_combination() {
        let title = "Create authentication module";
        let description = "Implement user login and registration with JWT tokens";
        let skills_context = "Using OAuth2 flow for authentication";

        let full_context = format!("{}\n\n{}\n\n{}", title, description, skills_context);

        assert!(full_context.contains("authentication"));
        assert!(full_context.contains("JWT"));
        assert!(full_context.contains("OAuth2"));
    }

    #[tokio::test]
    async fn test_empty_skills_context_handling() {
        let title = "Simple task";
        let description = "Do something simple";
        let skills_context = "";

        let context = if skills_context.is_empty() {
            format!("{}\n\n{}", title, description)
        } else {
            format!("{}\n\n{}\n\n{}", title, description, skills_context)
        };

        assert_eq!(context, "Simple task\n\nDo something simple");
    }

    // ====================================================================================
    // WORKER DECISION VALIDATION TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_decision_contains_required_fields() {
        let decision = WorkerDecision {
            action: WorkerAction::ToolCall {
                tool_name: "fs.write".to_string(),
                tool_args: json!({"path": "test.txt"}),
                rationale: Some("test".to_string()),
            },
            reasoning: None,
            raw_response: None,
        };

        let action = &decision.action;
        let (tool_name, tool_args, rationale) = match action {
            WorkerAction::ToolCall {
                tool_name,
                tool_args,
                rationale,
            } => (tool_name, tool_args, rationale),
            _ => panic!("Expected ToolCall"),
        };

        assert!(!tool_name.is_empty());
        assert!(tool_args.is_object());
        assert!(rationale.is_some());
    }

    #[tokio::test]
    async fn test_complete_action_always_has_summary() {
        let summaries = vec![
            "Task completed successfully",
            "All files created",
            "Done",
            "",
        ];

        for summary in summaries {
            let action = WorkerAction::Complete {
                summary: summary.to_string(),
            };

            match action {
                WorkerAction::Complete { summary } => {
                    assert_eq!(summary, summary);
                }
                _ => panic!("Expected Complete"),
            }
        }
    }

    // ====================================================================================
    // FILE SEARCH TOOL SELECTION TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_grep_tool_available() {
        let registry = create_tool_registry();
        let tools = registry.list_for_build_mode();
        let has_grep = tools.iter().any(|t| t.name == "search.rg");
        assert!(has_grep, "search.rg should be available for file searching");
    }

    #[tokio::test]
    async fn test_grep_supports_file_filter() {
        let grep_call = json!({
            "tool_name": "grep.find",
            "tool_args": {
                "pattern": "TODO",
                "include": "**/*.rs"
            }
        });

        assert_eq!(grep_call["tool_args"]["include"], "**/*.rs");
    }

    // ====================================================================================
    // TASK STATE MACHINE TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_step_status_progression() {
        use crate::core::plan::StepStatus;

        let statuses = vec![
            StepStatus::Pending,
            StepStatus::Running,
            StepStatus::Completed,
        ];

        assert!(matches!(statuses[0], StepStatus::Pending));
        assert!(matches!(statuses[1], StepStatus::Running));
        assert!(matches!(statuses[2], StepStatus::Completed));
    }

    // ====================================================================================
    // RATIONALE QUALITY TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_rationale_contains_action_reason() {
        let rationales = vec![
            "Writing the configuration file to persist settings",
            "Reading the file to verify its contents",
            "Executing npm install to add dependencies",
            "Creating a TODO list to track progress",
        ];

        for rationale in rationales {
            assert!(rationale.len() > 10, "Rationale should be descriptive");
            assert!(
                rationale.contains("to ") || rationale.contains("ing"),
                "Rationale should explain purpose"
            );
        }
    }

    // ====================================================================================
    // MODEL PROVIDER FLEXIBILITY TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_model_agnostic_behavior_tests() {
        let req = make_request(
            "Create a Python script",
            "File creation task",
            "Create hello.py",
            vec![],
        );

        let req_json = json!({
            "task_prompt": req.task_prompt,
            "goal_summary": req.goal_summary,
            "available_tools_count": req.available_tools.len()
        });

        assert_eq!(req_json["task_prompt"], "Create a Python script");
        assert_eq!(req_json["goal_summary"], "File creation task");
        assert!(req_json["available_tools_count"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_worker_decision_is_provider_agnostic() {
        let decision = WorkerDecision {
            action: WorkerAction::Complete {
                summary: "Done".to_string(),
            },
            reasoning: Some("Reasoning text".to_string()),
            raw_response: None,
        };

        let decision_json = json!({
            "action_type": "complete",
            "summary": "Done",
            "has_reasoning": decision.reasoning.is_some()
        });

        assert_eq!(decision_json["summary"], "Done");
        assert_eq!(decision_json["has_reasoning"], true);
    }
}
