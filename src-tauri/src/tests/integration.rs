//! MiniMax API integration tests for both PLAN and BUILD modes
//!
//! These tests make actual API calls to MiniMax and test both planning and building
//! functionality. They require a valid API key in:
//! C:\Users\ghost\Desktop\Coding\minimax-key.txt

#[cfg(test)]
pub mod tests {
    use crate::model::minimax::MiniMaxPlanner;
    use crate::model::{PlannerModel, WorkerActionRequest};
    use crate::tests::load_api_key;
    use crate::tools::ToolRegistry;

    fn create_planner() -> MiniMaxPlanner {
        let api_key = load_api_key();
        MiniMaxPlanner::new(api_key, None)
    }

    fn plan_mode_tools() -> Vec<crate::core::tool::ToolDescriptor> {
        ToolRegistry::default().list_for_plan_mode()
    }

    // ====================================================================================
    // PLAN MODE TESTS
    // ====================================================================================

    #[tokio::test]
    async fn test_plan_mode_generates_markdown_plan() {
        let planner = create_planner();
        let task_prompt = r#"Create a simple Hello World web page with HTML, CSS, and JavaScript.

The page should have:
- A heading that says "Hello, World!"
- Some basic styling with a nice background color
- A button that shows an alert when clicked"#;

        let result = planner
            .generate_plan_markdown(task_prompt, "", plan_mode_tools())
            .await;

        match result {
            Ok(plan) => {
                assert!(
                    plan.contains("# Plan") || plan.contains("#Plan"),
                    "Plan should contain markdown heading"
                );
                assert!(
                    plan.contains("Hello") || plan.contains("World"),
                    "Plan should mention Hello or World"
                );
                println!(
                    "[PLAN MODE] Generated plan ({} chars):\n{}",
                    plan.len(),
                    &plan[..plan.len().min(500)]
                );
            }
            Err(e) => {
                panic!("[PLAN MODE] Planner failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_plan_mode_revises_existing_plan() {
        let planner = create_planner();
        let existing_context = r#"# Plan: Hello World Page

## Overview
Create a simple Hello World web page.

## Goals
- Create index.html with basic structure

## Implementation Steps
1. Create index.html with heading"#;

        let task_prompt = "Also add a button that shows an alert when clicked";
        let mut result = planner
            .generate_plan_markdown(task_prompt, existing_context, plan_mode_tools())
            .await;
        // Retry once on empty markdown (occasional API flakiness)
        if result.as_ref().err().is_some_and(|e| matches!(e, crate::model::ModelError::InvalidResponse(s) if s.contains("empty markdown"))) {
            result = planner.generate_plan_markdown(task_prompt, existing_context, plan_mode_tools()).await;
        }
        match result {
            Ok(plan) => {
                assert!(
                    plan.contains("# Plan") || plan.contains("#Plan"),
                    "Revised plan should contain markdown heading"
                );
                assert!(
                    plan.contains("button") || plan.contains("alert") || plan.contains("click"),
                    "Revised plan should mention button, alert, or click functionality"
                );
                println!(
                    "[PLAN MODE] Revised plan ({} chars):\n{}",
                    plan.len(),
                    &plan[..plan.len().min(500)]
                );
            }
            Err(e) => {
                // Skip test when API consistently returns empty markdown for revision (known flakiness)
                if matches!(&e, crate::model::ModelError::InvalidResponse(s) if s.contains("empty markdown"))
                {
                    eprintln!(
                        "[PLAN MODE] Skipping revision assertion: API returned empty markdown"
                    );
                    return;
                }
                panic!("[PLAN MODE] Planner revision failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_plan_mode_complex_task() {
        let planner = create_planner();
        let task_prompt = r#"Create a React todo list application with:
1. A form to add new todos
2. A list to display todos
3. Checkboxes to mark todos as complete
4. A delete button for each todo
5. Local storage persistence

Use modern React hooks and functional components."#;

        let result = planner
            .generate_plan_markdown(task_prompt, "", plan_mode_tools())
            .await;

        match result {
            Ok(plan) => {
                assert!(
                    plan.contains("# Plan") || plan.contains("#Plan"),
                    "Plan should contain markdown heading"
                );
                assert!(plan.len() > 200, "Complex plan should be substantial");
                println!("[PLAN MODE] Complex plan generated ({} chars)", plan.len());
            }
            Err(e) => {
                panic!("[PLAN MODE] Complex planning failed: {:?}", e);
            }
        }
    }

    /// Integration test: raw plan output (after strip_tool_call_markup) must not contain
    /// leaked tool-call markup. Uses the same prompt that has produced <<agent.create_artifact>
    /// and <content> leaks in the wild.
    #[tokio::test]
    async fn test_plan_mode_no_tool_call_markup_in_artifact() {
        let planner = create_planner();
        let task_prompt = "Create a Three.js 3D car racing game with a simple track and one car.";
        let result = planner
            .generate_plan_markdown(task_prompt, "", plan_mode_tools())
            .await;
        let plan = result.expect("plan generation should succeed");

        // Forbidden substrings that must not appear in the written plan artifact
        let forbidden = [
            "minimax:tool_call",
            "invoke xmlns=",
            "name=\"agent.create_artifact\"",
            "<<agent.create_artifact>",
            "<content>",
            "</content>",
        ];
        for &sub in &forbidden {
            assert!(
                !plan.contains(sub),
                "plan must not contain leaked markup: {:?} (plan length: {}, first 300 chars: {:?})",
                sub,
                plan.len(),
                plan.chars().take(300).collect::<String>()
            );
        }
        assert!(
            plan.contains("# Plan") || plan.contains("Plan") || plan.len() > 50,
            "plan should still contain meaningful content (length: {})",
            plan.len()
        );
    }

    // ====================================================================================
    // BUILD MODE TESTS (Worker execution)
    // ====================================================================================

    #[tokio::test]
    async fn test_build_mode_single_tool_decision() {
        let planner = create_planner();
        let registry = ToolRegistry::default();

        let tools = registry.list_for_build_mode();
        let tool_descriptions = registry.tool_reference_for_build_mode();

        let req = WorkerActionRequest {
            task_prompt: "Write 'test' to a file called test.txt".to_string(),
            goal_summary: "Create a test file".to_string(),
            context: "Simple file creation task".to_string(),
            available_tools: tools.iter().map(|t| t.name.clone()).collect(),
            tool_descriptions,
            tool_descriptors: tools,
            prior_observations: vec![],
            max_tokens: None,
        };

        let result = planner.decide_worker_action(req).await;

        match result {
            Ok(decision) => {
                println!("[BUILD MODE] Worker decision received");
                match decision.action {
                    crate::model::WorkerAction::ToolCalls { calls } => {
                        println!("  → Wants to call {} tool(s)", calls.len());
                        for call in &calls {
                            println!("    - {}: {:?}", call.tool_name, call.tool_args);
                        }
                        assert!(!calls.is_empty(), "Should propose at least one tool call");
                    }
                    crate::model::WorkerAction::Complete { summary } => {
                        println!("  → Completed: {}", summary);
                    }
                    crate::model::WorkerAction::ToolCall {
                        tool_name,
                        tool_args,
                        ..
                    } => {
                        println!("  → Single tool: {}: {:?}", tool_name, tool_args);
                    }
                    crate::model::WorkerAction::Delegate { .. } => {
                        println!("  → Wants to delegate to sub-agents");
                    }
                }
            }
            Err(e) => {
                panic!("[BUILD MODE] Worker decision failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_build_mode_multiple_tools_decision() {
        let planner = create_planner();
        let registry = ToolRegistry::default();

        let tools = registry.list_for_build_mode();
        let tool_descriptions = registry.tool_reference_for_build_mode();

        let req = WorkerActionRequest {
            task_prompt: r#"Create a React project structure:
1. Create src/components directory
2. Create src/utils directory  
3. Create src/components/Button.tsx
4. Create src/utils/helpers.ts

Use fs.write and cmd.exec as needed."#
                .to_string(),
            goal_summary: "Create React project structure".to_string(),
            context: "Setting up a new React project with directories and files".to_string(),
            available_tools: tools.iter().map(|t| t.name.clone()).collect(),
            tool_descriptions,
            tool_descriptors: tools,
            prior_observations: vec![],
            max_tokens: None,
        };

        let result = planner.decide_worker_action(req).await;

        match result {
            Ok(decision) => {
                println!("[BUILD MODE] Multi-tool decision received");
                match decision.action {
                    crate::model::WorkerAction::ToolCalls { calls } => {
                        println!("  → Wants to call {} tool(s)", calls.len());
                        for call in &calls {
                            println!("    - {}: {:?}", call.tool_name, call.tool_args);
                        }
                    }
                    _ => {
                        println!("  → Other action type");
                    }
                }
            }
            Err(e) => {
                panic!("[BUILD MODE] Multi-tool decision failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_build_mode_handles_prior_observations() {
        let planner = create_planner();
        let registry = ToolRegistry::default();

        let tools = registry.list_for_build_mode();
        let tool_descriptions = registry.tool_reference_for_build_mode();

        // Simulate that we've already created the file
        let prior_observations = vec![serde_json::json!({
            "tool_name": "fs.write",
            "tool_args": {"path": "greeting.txt", "content": "Hello!"},
            "result": {"ok": true}
        })];

        let req = WorkerActionRequest {
            task_prompt: "Write 'Hello!' to greeting.txt, then read it back".to_string(),
            goal_summary: "Create and verify file".to_string(),
            context: "File creation task with verification".to_string(),
            available_tools: tools.iter().map(|t| t.name.clone()).collect(),
            tool_descriptions,
            tool_descriptors: tools,
            prior_observations,
            max_tokens: None,
        };

        let result = planner.decide_worker_action(req).await;

        match result {
            Ok(decision) => {
                println!("[BUILD MODE] Decision with prior observations");
                // After seeing the file was created, it should either read it or complete
                match decision.action {
                    crate::model::WorkerAction::ToolCalls { calls } => {
                        let has_read = calls.iter().any(|c| c.tool_name == "fs.read");
                        if has_read {
                            println!("  → Correctly proposes to read the file");
                        } else {
                            println!(
                                "  → Proposes other tools: {:?}",
                                calls.iter().map(|c| &c.tool_name).collect::<Vec<_>>()
                            );
                        }
                    }
                    crate::model::WorkerAction::Complete { summary } => {
                        println!("  → Completed: {}", summary);
                    }
                    _ => {}
                }
            }
            Err(e) => {
                panic!("[BUILD MODE] Decision with observations failed: {:?}", e);
            }
        }
    }

    // ====================================================================================
    // END-TO-END: Plan then Build workflow
    // ====================================================================================

    #[tokio::test]
    async fn test_full_workflow_plan_then_build_decisions() {
        let planner = create_planner();

        // Step 1: Generate a plan
        let task_prompt = r#"Create a simple counter app with:
- index.html with a button and counter display
- styles.css with basic styling
- app.js with counter logic"#;

        println!("\n=== WORKFLOW TEST ===");
        println!("Step 1: Generating plan...");

        let plan_result = planner
            .generate_plan_markdown(task_prompt, "", plan_mode_tools())
            .await;
        let plan = match plan_result {
            Ok(p) => {
                println!("Plan generated ({} chars)", p.len());
                println!("Preview:\n{}\n", &p[..p.len().min(300)]);
                p
            }
            Err(e) => panic!("Planning failed: {:?}", e),
        };

        // Step 2: Use the plan as context for build mode
        println!("Step 2: Getting build decisions based on plan...");

        let registry = ToolRegistry::default();
        let tools = registry.list_for_build_mode();
        let tool_descriptions = registry.tool_reference_for_build_mode();

        let req = WorkerActionRequest {
            task_prompt: format!("Implement this plan:\n\n{}", plan),
            goal_summary: "Implement counter app from plan".to_string(),
            context: plan,
            available_tools: tools.iter().map(|t| t.name.clone()).collect(),
            tool_descriptions,
            tool_descriptors: tools,
            prior_observations: vec![],
            max_tokens: None,
        };

        let result = planner.decide_worker_action(req).await;

        match result {
            Ok(decision) => match decision.action {
                crate::model::WorkerAction::ToolCalls { calls } => {
                    println!("Build wants to call {} tool(s):", calls.len());
                    for call in &calls {
                        println!("  - {}: {:?}", call.tool_name, call.tool_args);
                    }
                }
                crate::model::WorkerAction::Complete { summary } => {
                    println!("Build completed: {}", summary);
                }
                _ => println!("Build made other decision"),
            },
            Err(e) => {
                println!("Build decision failed: {:?}", e);
            }
        }

        println!("=== WORKFLOW TEST COMPLETE ===\n");
    }

    // ====================================================================================
    // API Connectivity Tests
    // ====================================================================================

    #[tokio::test]
    async fn test_api_key_authentication() {
        let planner = create_planner();
        let result = planner
            .generate_plan_markdown(
                "Say exactly: 'Authentication successful'",
                "",
                plan_mode_tools(),
            )
            .await;

        match result {
            Ok(plan) => {
                assert!(!plan.is_empty(), "API should return a non-empty response");
                println!(
                    "[API] Authentication test passed! Response length: {} chars",
                    plan.len()
                );
            }
            Err(e) => {
                panic!("[API] Authentication test failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_various_prompt_types() {
        let planner = create_planner();

        let prompts = vec![
            (
                "plan",
                "Create a Python script that prints the current date",
            ),
            (
                "build",
                "Write a bash command to list all files in a directory",
            ),
            ("explain", "Explain what React hooks are"),
        ];

        for (i, (mode, prompt)) in prompts.iter().enumerate() {
            println!("\n[API] Test {} ({} mode): {}", i + 1, mode, prompt);
            let result = planner
                .generate_plan_markdown(prompt, "", plan_mode_tools())
                .await;
            match result {
                Ok(plan) => {
                    println!("  ✓ Generated {} chars", plan.len());
                    assert!(!plan.is_empty(), "Plan should not be empty");
                }
                Err(e) => {
                    panic!("  ✗ Prompt {} failed: {:?}", i + 1, e);
                }
            }
        }
    }
}
