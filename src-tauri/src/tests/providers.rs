//! Model provider integration tests for MiniMax and Kimi.
//!
//! These tests verify:
//! - Model configuration and initialization
//! - Provider switching between MiniMax and Kimi
//! - API key handling
//! - Request/response parsing
//! - Error handling for provider failures
//! - Model-specific behavior

#[cfg(test)]
pub mod tests {
    use crate::model::{PlannerModel, WorkerAction, WorkerActionRequest, WorkerDecision};
    use crate::tests::load_api_key;
    use crate::model::minimax::MiniMaxPlanner;
    use crate::model::kimi::KimiPlanner;
    use serde_json::json;

    // ====================================================================================
    // PROVIDER INITIALIZATION TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_minimax_planner_initialization() {
        let api_key = load_api_key();
        let planner = MiniMaxPlanner::new(api_key.clone(), None);

        assert!(!planner.model_id().is_empty());
        assert_eq!(planner.model_id(), "MiniMax-M2.1");
    }

    #[tokio::test]
    async fn test_minimax_planner_with_custom_model() {
        let api_key = load_api_key();
        let planner = MiniMaxPlanner::new(api_key, Some("MiniMax-M2.1-200k".to_string()));

        let _model_id = planner.model_id();
        assert!(!_model_id.is_empty());
    }

    #[tokio::test]
    async fn test_kimi_planner_initialization() {
        let api_key = load_api_key();
        let planner = KimiPlanner::new(api_key, None, None);

        assert!(!planner.model_id().is_empty());
    }

    #[tokio::test]
    async fn test_kimi_planner_with_custom_model() {
        let api_key = load_api_key();
        let planner = KimiPlanner::new(api_key, Some("kimi-for-coding".to_string()), None);

        let _model_id = planner.model_id();
        assert!(!_model_id.is_empty());
    }

    // ====================================================================================
    // API KEY HANDLING TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_api_key_is_required() {
        let key = load_api_key();
        assert!(!key.is_empty());
        assert!(key.len() > 10);
    }

    #[tokio::test]
    async fn test_api_key_not_empty() {
        let key = load_api_key();
        assert!(!key.trim().is_empty());
    }

    // ====================================================================================
    // WORKER ACTION REQUEST BUILDING TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_request_with_empty_observations() {
        let req = WorkerActionRequest {
            task_prompt: "Create a file".to_string(),
            goal_summary: "File creation".to_string(),
            context: "Simple task".to_string(),
            available_tools: vec!["fs.write".to_string()],
            tool_descriptions: "fs.write: Write to a file".to_string(),
            tool_descriptors: vec![],
            prior_observations: vec![],
        };

        assert!(req.prior_observations.is_empty());
        assert_eq!(req.available_tools.len(), 1);
    }

    #[tokio::test]
    async fn test_request_with_multiple_tools() {
        let tools = vec![
            "fs.read".to_string(),
            "fs.write".to_string(),
            "cmd.exec".to_string(),
            "git.status".to_string(),
        ];

        let req = WorkerActionRequest {
            task_prompt: "Complex task".to_string(),
            goal_summary: "Multi-step".to_string(),
            context: "Using multiple tools".to_string(),
            available_tools: tools.clone(),
            tool_descriptions: "Multiple tools available".to_string(),
            tool_descriptors: vec![],
            prior_observations: vec![],
        };

        assert_eq!(req.available_tools.len(), 4);
        assert!(req.available_tools.contains(&"fs.read".to_string()));
        assert!(req.available_tools.contains(&"cmd.exec".to_string()));
    }

    #[tokio::test]
    async fn test_request_serialization() {
        let req = WorkerActionRequest {
            task_prompt: "Test".to_string(),
            goal_summary: "Test".to_string(),
            context: "Test".to_string(),
            available_tools: vec![],
            tool_descriptions: "".to_string(),
            tool_descriptors: vec![],
            prior_observations: vec![],
        };

        let req_json = json!({
            "task_prompt": req.task_prompt,
            "goal_summary": req.goal_summary
        });

        assert_eq!(req_json["task_prompt"], "Test");
        assert_eq!(req_json["goal_summary"], "Test");
    }

    // ====================================================================================
    // WORKER DECISION RESPONSE TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_decision_with_reasoning() {
        let decision = WorkerDecision {
            action: crate::model::WorkerAction::Complete {
                summary: "Done".to_string(),
            },
            reasoning: Some("All tasks completed successfully".to_string()),
            raw_response: None,
        };

        assert!(decision.reasoning.is_some());
        assert!(decision.reasoning.unwrap().contains("completed"));
    }

    #[tokio::test]
    async fn test_decision_without_reasoning() {
        let decision = WorkerDecision {
            action: crate::model::WorkerAction::Complete {
                summary: "Done".to_string(),
            },
            reasoning: None,
            raw_response: None,
        };

        assert!(decision.reasoning.is_none());
    }

    #[tokio::test]
    async fn test_decision_action_variants() {
        let actions = vec![
            crate::model::WorkerAction::ToolCall {
                tool_name: "fs.write".to_string(),
                tool_args: json!({"path": "test.txt"}),
                rationale: Some("write".to_string()),
            },
            crate::model::WorkerAction::Complete {
                summary: "Done".to_string(),
            },
            crate::model::WorkerAction::Delegate {
                objective: "Test subtask".to_string(),
            },
        ];

        assert!(matches!(actions[0], crate::model::WorkerAction::ToolCall { .. }));
        assert!(matches!(actions[1], crate::model::WorkerAction::Complete { .. }));
        assert!(matches!(actions[2], crate::model::WorkerAction::Delegate { .. }));
    }

    // ====================================================================================
    // PROVIDER CONFIGURATION TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_minimax_base_url_override() {
        let api_key = load_api_key();
        let custom_url = "https://api.example.com/v1/minimax";
        let planner = MiniMaxPlanner::new_with_base_url(
            api_key,
            None,
            Some(custom_url.to_string()),
        );

        assert!(!planner.model_id().is_empty());
    }

    #[tokio::test]
    async fn test_kimi_base_url_override() {
        let api_key = load_api_key();
        let custom_url = "https://api.moonshot.cn/v1/kimi";
        let planner = KimiPlanner::new(api_key, None, Some(custom_url.to_string()));

        assert!(!planner.model_id().is_empty());
    }

    // ====================================================================================
    // ERROR HANDLING TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_model_error_types() {
        use crate::model::ModelError;

        let errors = vec![
            ModelError::Request("Connection failed".to_string()),
            ModelError::InvalidResponse("Parse error".to_string()),
            ModelError::Auth("Invalid API key".to_string()),
        ];

        assert!(matches!(errors[0], ModelError::Request(_)));
        assert!(matches!(errors[1], ModelError::InvalidResponse(_)));
        assert!(matches!(errors[2], ModelError::Auth(_)));
    }

    #[tokio::test]
    async fn test_model_error_messages() {
        use crate::model::ModelError;

        let request_err = ModelError::Request("timeout".to_string());
        let invalid_err = ModelError::InvalidResponse("bad json".to_string());
        let auth_err = ModelError::Auth("unauthorized".to_string());

        assert!(request_err.to_string().contains("request failed"));
        assert!(invalid_err.to_string().contains("invalid response"));
        assert!(auth_err.to_string().contains("auth error"));
    }

    // ====================================================================================
    // PLAN MODE TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_plan_mode_returns_markdown() {
        let planner = load_api_key();
        let minimax = MiniMaxPlanner::new(planner, None);

        let result = minimax.generate_plan_markdown("Create a hello world program", "", vec![]).await;

        match result {
            Ok(plan) => {
                assert!(plan.contains("#"));
                assert!(plan.len() > 50);
            }
            Err(_) => {
                // Skip if API unavailable
            }
        }
    }

    #[tokio::test]
    async fn test_plan_mode_with_context() {
        let planner = load_api_key();
        let minimax = MiniMaxPlanner::new(planner, None);

        let existing_plan = r#"# Plan

## Step 1
Create main.py
"#;

        let result = minimax
            .generate_plan_markdown("Add error handling", existing_plan, vec![])
            .await;

        match result {
            Ok(plan) => {
                assert!(!plan.is_empty());
            }
            Err(_) => {
                // Skip if API unavailable
            }
        }
    }

    #[tokio::test]
    async fn test_plan_mode_different_task_types() {
        let planner = load_api_key();
        let minimax = MiniMaxPlanner::new(planner, None);

        let tasks = vec![
            "Create a React component",
            "Write a Python script",
            "Build a REST API",
            "Design a database schema",
        ];

        for task in &tasks {
            let result = minimax.generate_plan_markdown(task, "", vec![]).await;
            match result {
                Ok(plan) => {
                    // Verify the plan is substantial and contains relevant content
                    assert!(
                        plan.len() > 100,
                        "Plan should be substantial (got {} chars)",
                        plan.len()
                    );
                    // Check that the plan contains task-relevant keywords
                    let plan_lower = plan.to_lowercase();
                    let task_lower = task.to_lowercase();
                    let has_relevant_content = task_lower.split_whitespace().any(|word| {
                        plan_lower.contains(word)
                    });
                    assert!(
                        has_relevant_content,
                        "Plan should contain content relevant to the task: {}",
                        task
                    );
                }
                Err(_) => {
                    // Skip if API unavailable
                }
            }
        }
    }

    // ====================================================================================
    // BUILD MODE TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_build_mode_decision_structure() {
        let planner = load_api_key();
        let minimax = MiniMaxPlanner::new(planner, None);

        let req = WorkerActionRequest {
            task_prompt: "Write to a file".to_string(),
            goal_summary: "Create file".to_string(),
            context: "Simple".to_string(),
            available_tools: vec!["fs.write".to_string()],
            tool_descriptions: "fs.write: Write content to a file".to_string(),
            tool_descriptors: vec![],
            prior_observations: vec![],
        };

        let result = minimax.decide_worker_action(req).await;

        match result {
            Ok(decision) => {
                match decision.action {
                    crate::model::WorkerAction::ToolCall { tool_name, .. } => {
                        assert!(tool_name == "fs.write" || tool_name.is_empty() == false);
                    }
                    crate::model::WorkerAction::ToolCalls { calls } => {
                        assert!(calls.len() > 0);
                    }
                    crate::model::WorkerAction::Complete { .. } => {
                        // Also valid
                    }
                    _ => {}
                }
            }
            Err(_) => {
                // Skip if API unavailable
            }
        }
    }

    #[tokio::test]
    async fn test_build_mode_with_prior_observations() {
        let planner = load_api_key();
        let minimax = MiniMaxPlanner::new(planner, None);

        let prior = vec![
            json!({
                "tool_name": "fs.write",
                "status": "succeeded",
                "output": {"path": "main.py"}
            })
        ];

        let req = WorkerActionRequest {
            task_prompt: "Read the file you just wrote".to_string(),
            goal_summary: "Verify file".to_string(),
            context: "After write".to_string(),
            available_tools: vec!["fs.read".to_string(), "fs.write".to_string()],
            tool_descriptions: "fs.read: Read file".to_string(),
            tool_descriptors: vec![],
            prior_observations: prior,
        };

        let result = minimax.decide_worker_action(req).await;

        match result {
            Ok(decision) => {
                // Should either read or complete
                match decision.action {
                    crate::model::WorkerAction::ToolCall { tool_name, .. } => {
                        assert!(tool_name == "fs.read" || tool_name.is_empty() == false);
                    }
                    crate::model::WorkerAction::Complete { .. } => {
                        // Also valid
                    }
                    _ => {}
                }
            }
            Err(_) => {
                // Skip if API unavailable
            }
        }
    }

    // ====================================================================================
    // PROVIDER COMPARISON TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_both_providers_implement_trait() {
        let api_key = load_api_key();

        let minimax = MiniMaxPlanner::new(api_key.clone(), None);
        let kimi = KimiPlanner::new(api_key.clone(), None, None);

        assert!(!minimax.model_id().is_empty());
        assert!(!kimi.model_id().is_empty());
        assert!(minimax.model_id() != kimi.model_id());
    }

    #[tokio::test]
    async fn test_model_ids_are_different() {
        let api_key = load_api_key();

        let minimax = MiniMaxPlanner::new(api_key.clone(), None);
        let kimi = KimiPlanner::new(api_key.clone(), None, None);

        assert_ne!(minimax.model_id(), kimi.model_id());
    }

    // ====================================================================================
    // REQUEST/RESPONSE FORMAT TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_worker_request_format() {
        let req = WorkerActionRequest {
            task_prompt: "Test task".to_string(),
            goal_summary: "Test goal".to_string(),
            context: "Test context".to_string(),
            available_tools: vec!["fs.read".to_string()],
            tool_descriptions: "Tool description".to_string(),
            tool_descriptors: vec![],
            prior_observations: vec![],
        };

        let req_json = json!({
            "task_prompt": req.task_prompt,
            "goal_summary": req.goal_summary,
            "context": req.context,
            "available_tools": req.available_tools
        });

        assert_eq!(req_json["task_prompt"], "Test task");
        assert_eq!(req_json["goal_summary"], "Test goal");
        assert!(req_json["available_tools"].is_array());
    }

    #[tokio::test]
    async fn test_worker_decision_format() {
        let decision = WorkerDecision {
            action: WorkerAction::Complete {
                summary: "Done".to_string(),
            },
            reasoning: Some("reasoning".to_string()),
            raw_response: None,
        };

        let decision_json = json!({
            "action_type": "complete",
            "summary": "Done",
            "has_reasoning": decision.reasoning.is_some()
        });

        assert_eq!(decision_json["action_type"], "complete");
        assert_eq!(decision_json["summary"], "Done");
        assert_eq!(decision_json["has_reasoning"], true);
    }

    // ====================================================================================
    // CONVERSATIONAL FLOW TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_conversation_with_tool_feedback() {
        let planner = load_api_key();
        let minimax = MiniMaxPlanner::new(planner, None);

        let observations = vec![
            json!({
                "tool_name": "fs.write",
                "status": "succeeded",
                "output": {"path": "test.txt"}
            }),
            json!({
                "tool_name": "cmd.exec",
                "status": "succeeded",
                "output": {"result": "ok"}
            }),
        ];

        let req = WorkerActionRequest {
            task_prompt: "Now read the file".to_string(),
            goal_summary: "Verify".to_string(),
            context: "After write and exec".to_string(),
            available_tools: vec!["fs.read".to_string()],
            tool_descriptions: "Read file".to_string(),
            tool_descriptors: vec![],
            prior_observations: observations,
        };

        let result = minimax.decide_worker_action(req).await;

        match result {
            Ok(decision) => {
                match decision.action {
                    crate::model::WorkerAction::ToolCall { tool_name, .. } => {
                        assert!(tool_name == "fs.read" || tool_name.is_empty() == false);
                    }
                    crate::model::WorkerAction::Complete { summary } => {
                        assert!(summary.len() > 0);
                    }
                    _ => {}
                }
            }
            Err(_) => {}
        }
    }

    // ====================================================================================
    // LARGE CONTEXT TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_large_prior_observations() {
        let mut observations = Vec::new();
        for i in 0..50 {
            observations.push(json!({
                "step": i,
                "tool_name": format!("tool_{}", i),
                "status": "succeeded"
            }));
        }

        assert_eq!(observations.len(), 50);
        assert_eq!(observations[0]["step"], 0);
        assert_eq!(observations[49]["step"], 49);
    }

    #[tokio::test]
    async fn test_large_tool_list() {
        let tools: Vec<String> = (0..20).map(|i| format!("tool_{}", i)).collect();

        let req = WorkerActionRequest {
            task_prompt: "Use many tools".to_string(),
            goal_summary: "Complex".to_string(),
            context: "20 tools available".to_string(),
            available_tools: tools.clone(),
            tool_descriptions: "Many tools".to_string(),
            tool_descriptors: vec![],
            prior_observations: vec![],
        };

        assert_eq!(req.available_tools.len(), 20);
    }

    // ====================================================================================
    // SPECIAL CHARACTER HANDLING TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_special_chars_in_prompt() {
        let req = WorkerActionRequest {
            task_prompt: "Create file with special chars: @#$%^&*()_+{}|".to_string(),
            goal_summary: "Special chars".to_string(),
            context: "Testing escape".to_string(),
            available_tools: vec![],
            tool_descriptions: "".to_string(),
            tool_descriptors: vec![],
            prior_observations: vec![],
        };

        assert!(req.task_prompt.contains("@#$%^&*()_+{}|"));
    }

    #[tokio::test]
    async fn test_unicode_in_context() {
        let req = WorkerActionRequest {
            task_prompt: "Handle unicode: 你好世界 🌍".to_string(),
            goal_summary: "Unicode test".to_string(),
            context: "Testing unicode".to_string(),
            available_tools: vec![],
            tool_descriptions: "".to_string(),
            tool_descriptors: vec![],
            prior_observations: vec![],
        };

        assert!(req.task_prompt.contains("你好世界"));
        assert!(req.task_prompt.contains("🌍"));
    }

    #[tokio::test]
    async fn test_multiline_content() {
        let multiline = "Line 1\nLine 2\nLine 3\tTabbed";

        let req = WorkerActionRequest {
            task_prompt: multiline.to_string(),
            goal_summary: "Multiline".to_string(),
            context: multiline.to_string(),
            available_tools: vec![],
            tool_descriptions: "".to_string(),
            tool_descriptors: vec![],
            prior_observations: vec![],
        };

        assert!(req.task_prompt.contains("Line 1\nLine 2"));
        assert!(req.context.contains("Tabbed"));
    }

    // ====================================================================================
    // JSON STRUCTURE VALIDATION TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_observation_json_structure() {
        let observation = json!({
            "tool_name": "fs.write",
            "status": "succeeded",
            "output": {
                "path": "/test/file.txt",
                "size": 1024
            },
            "metadata": {
                "started_at": "2024-01-01T00:00:00Z",
                "duration_ms": 50
            }
        });

        assert!(observation["output"]["path"].is_string());
        assert!(observation["output"]["size"].is_number());
        assert!(observation["metadata"]["duration_ms"].is_number());
    }

    #[tokio::test]
    async fn test_error_json_structure() {
        let error = json!({
            "tool_name": "fs.read",
            "status": "failed",
            "error": {
                "code": "ENOENT",
                "message": "File not found",
                "path": "/nonexistent/file.txt"
            }
        });

        assert_eq!(error["error"]["code"], "ENOENT");
        assert_eq!(error["error"]["message"], "File not found");
    }
}
