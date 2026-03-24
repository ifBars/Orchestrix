//! Agentic coding benchmark for LLM evaluation with real tool calls.
//!
//! Tests models on practical coding tasks using actual filesystem and command
//! execution tools in a temporary workspace. Defaults to MiniMax M2.1.

use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::bench::core::{BenchmarkRunMetadata, WorkloadKind};
use crate::bench::llm::{api_key_env_keys, first_non_empty_env, LlmProviderConfig, LlmProviderId};
use crate::core::tool::ToolDescriptor;
use crate::model::{
    AgentModelClient, GlmClient, KimiClient, MiniMaxClient, ModalClient, WorkerAction,
    WorkerActionRequest, WorkerDecision,
};
use crate::policy::PolicyEngine;
use crate::tools::args::{schema_for_type, CmdExecArgs};
use crate::tools::{ToolCallInput, ToolRegistry};

// ---------------------------------------------------------------------------
// Configuration and types
// ---------------------------------------------------------------------------

const DEFAULT_MAX_TOKENS: u32 = 4096;
const DEFAULT_TIMEOUT_SECONDS: u64 = 120;

#[derive(Debug, Clone)]
pub struct AgenticCodingBenchOptions {
    pub providers: Vec<LlmProviderId>,
    pub provider_configs: Vec<LlmProviderConfig>,
    pub max_tokens: u32,
    pub timeout_seconds: u64,
    pub task_filter: Vec<String>,
    pub scratch_root: Option<PathBuf>,
    pub retain_failed_workspaces: bool,
}

impl Default for AgenticCodingBenchOptions {
    fn default() -> Self {
        Self {
            // Default to MiniMax M2.1 as requested
            providers: vec![LlmProviderId::MiniMax],
            provider_configs: vec![LlmProviderConfig {
                provider: LlmProviderId::MiniMax,
                api_key: None,
                model: Some("MiniMax-M2.1".to_string()),
                base_url: None,
                max_tokens: Some(DEFAULT_MAX_TOKENS),
            }],
            max_tokens: DEFAULT_MAX_TOKENS,
            timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
            task_filter: Vec::new(),
            scratch_root: None,
            retain_failed_workspaces: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticCodingBenchReport {
    pub metadata: BenchmarkRunMetadata,
    pub providers: Vec<AgenticCodingProviderResult>,
    pub tasks: Vec<AgenticCodingTaskDescriptor>,
    pub overall_winner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticCodingProviderResult {
    pub provider: String,
    pub model: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub total_duration_ms: f64,
    pub tasks: Vec<AgenticCodingTaskResult>,
    pub aggregate: AgenticCodingAggregateResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticCodingTaskDescriptor {
    pub task_id: String,
    pub task_label: String,
    pub description: String,
    pub category: AgenticCodingCategory,
    pub max_turns: usize,
    pub expected_files: Vec<String>,
    pub validation_commands: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgenticCodingCategory {
    FileOperations,
    CommandExecution,
    CodeGeneration,
    MultiStepTask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticCodingTaskResult {
    pub task_id: String,
    pub status: String,
    pub error: Option<String>,
    pub duration_ms: f64,
    pub turns_taken: usize,
    pub tool_calls_made: usize,
    pub success: bool,
    pub validation_passed: bool,
    pub completion_signaled: bool,
    pub validation_notes: Vec<String>,
    pub retained_workspace: Option<String>,
    pub tool_trace: Vec<AgenticCodingToolTraceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticCodingToolTraceEntry {
    pub tool_name: String,
    pub tool_args: serde_json::Value,
    pub observation: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticCodingAggregateResult {
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    pub avg_duration_ms: f64,
    pub total_tool_calls: usize,
    pub success_rate: f64,
}

#[derive(Debug, Clone)]
struct CodingTaskDefinition {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    category: AgenticCodingCategory,
    max_turns: usize,
    #[allow(dead_code)]
    system_prompt: &'static str,
    initial_prompt: &'static str,
    setup_files: Vec<(&'static str, &'static str)>,
    expected_files: Vec<&'static str>,
    validation_commands: Vec<&'static str>,
}

#[derive(Debug, Clone)]
struct ValidationOutcome {
    passed: bool,
    notes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Task definitions - focused on real agentic coding scenarios
// ---------------------------------------------------------------------------

fn coding_tasks() -> Vec<CodingTaskDefinition> {
    vec![
        // Task 1: Basic file creation and reading
        CodingTaskDefinition {
            id: "file_create_read",
            label: "File Create & Read",
            description: "Create a JSON config file and verify it can be read back",
            category: AgenticCodingCategory::FileOperations,
            max_turns: 5,
            system_prompt: "You are an autonomous coding agent. Use available tools to complete tasks efficiently. Always verify your work.",
            initial_prompt: "Create a file named 'config.json' in the workspace with the following content: {\"name\": \"my-app\", \"version\": \"1.0.0\", \"port\": 3000}. Then read it back to confirm it was written correctly.",
            setup_files: vec![],
            expected_files: vec!["config.json"],
            validation_commands: vec![],
        },
        // Task 2: List directory and analyze
        CodingTaskDefinition {
            id: "list_and_analyze",
            label: "Directory Analysis",
            description: "List files and generate a summary report",
            category: AgenticCodingCategory::FileOperations,
            max_turns: 6,
            system_prompt: "You are an autonomous coding agent. Analyze directory contents and create summary reports.",
            initial_prompt: "The workspace contains several source files. List all files, then create a file called 'file_inventory.txt' that contains a list of all files found with their approximate sizes (you can estimate based on typical file sizes). Format it as a simple text report.",
            setup_files: vec![
                ("src/main.py", "# Main entry point\ndef main():\n    print('Hello World')\n\nif __name__ == '__main__':\n    main()\n"),
                ("src/utils.py", "# Utility functions\ndef helper():\n    pass\n"),
                ("README.md", "# My Project\n\nThis is a sample project.\n"),
                ("requirements.txt", "requests\npytest\n"),
            ],
            expected_files: vec!["file_inventory.txt"],
            validation_commands: vec![],
        },
        // Task 3: Command execution - count files
        CodingTaskDefinition {
            id: "command_count_files",
            label: "Command: Count Files",
            description: "Use shell commands to count and report file statistics",
            category: AgenticCodingCategory::CommandExecution,
            max_turns: 5,
            system_prompt: "You are an autonomous coding agent. Use shell commands when appropriate for system operations.",
            initial_prompt: "Use a shell command to count how many files (not directories) exist in the current workspace (recursively), then create a file called 'count_report.txt' with the count.",
            setup_files: vec![
                ("data/a.txt", "content a"),
                ("data/b.txt", "content b"),
                ("data/c.txt", "content c"),
                ("docs/readme.md", "# Docs"),
            ],
            expected_files: vec!["count_report.txt"],
            validation_commands: vec![],
        },
        // Task 4: Code generation - Python function
        CodingTaskDefinition {
            id: "code_generate_function",
            label: "Generate Python Function",
            description: "Generate a utility function with proper error handling",
            category: AgenticCodingCategory::CodeGeneration,
            max_turns: 6,
            system_prompt: "You are an autonomous coding agent. Write clean, well-documented code with proper error handling.",
            initial_prompt: "Create a file 'math_utils.py' containing a function called 'safe_divide' that takes two arguments (a, b) and returns a/b. It should handle division by zero by returning None and include a docstring. Also include a few test cases at the bottom that run when the file is executed directly.",
            setup_files: vec![],
            expected_files: vec!["math_utils.py"],
            validation_commands: vec!["python math_utils.py"],
        },
        // Task 5: Multi-step refactoring task
        CodingTaskDefinition {
            id: "multistep_refactor",
            label: "Multi-step Refactor",
            description: "Read, modify, and validate code changes",
            category: AgenticCodingCategory::MultiStepTask,
            max_turns: 10,
            system_prompt: "You are an autonomous coding agent. Follow multi-step workflows: read first, understand the code, then make targeted changes, and verify results.",
            initial_prompt: "The file 'calculator.py' has a bug. Read it first, then fix the bug in the 'multiply' function (it accidentally adds instead of multiplies), and add a test case that verifies the fix. Finally, run the file to confirm it works.",
            setup_files: vec![
                ("calculator.py", "# Simple calculator\n\ndef add(a, b):\n    return a + b\n\ndef multiply(a, b):\n    # Bug: should multiply, not add\n    return a + b\n\ndef subtract(a, b):\n    return a - b\n\nif __name__ == '__main__':\n    print('2 + 3 =', add(2, 3))\n    print('5 - 2 =', subtract(5, 2))\n    print('4 * 3 =', multiply(4, 3))  # Should be 12, not 7\n"),
            ],
            expected_files: vec!["calculator.py"],
            validation_commands: vec!["python calculator.py"],
        },
        // Task 6: Search and replace pattern
        CodingTaskDefinition {
            id: "search_and_update",
            label: "Search & Update",
            description: "Find patterns in files and update them",
            category: AgenticCodingCategory::CodeGeneration,
            max_turns: 8,
            system_prompt: "You are an autonomous coding agent. Search for patterns and make precise updates.",
            initial_prompt: "Search for all files containing the word 'TODO' in the workspace. Create a file 'todo_report.md' listing each file and the TODO items found. Then update each TODO comment to include today's date in format [YYYY-MM-DD] at the end.",
            setup_files: vec![
                ("main.py", "# TODO: Implement main function\n\ndef main():\n    pass  # TODO: Add logic here\n"),
                ("utils.py", "# Helper functions\n\ndef helper():\n    # TODO: Optimize this later\n    pass\n"),
            ],
            expected_files: vec!["todo_report.md"],
            validation_commands: vec![],
        },
    ]
}

// ---------------------------------------------------------------------------
// Tool descriptors for agentic coding
// ---------------------------------------------------------------------------

fn agentic_coding_tools() -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            name: "fs.read".to_string(),
            description:
                "Read the contents of a file. Provide the relative path from workspace root."
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Relative path to the file"}
                },
                "required": ["path"]
            }),
            output_schema: None,
        },
        ToolDescriptor {
            name: "fs.write".to_string(),
            description: "Write content to a file. Creates directories if needed.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Relative path to the file"},
                    "content": {"type": "string", "description": "Full file content"}
                },
                "required": ["path", "content"]
            }),
            output_schema: None,
        },
        ToolDescriptor {
            name: "fs.list".to_string(),
            description: "List files and directories at a path.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Relative path to list (default: current directory)"}
                },
                "required": []
            }),
            output_schema: None,
        },
        ToolDescriptor {
            name: "cmd.exec".to_string(),
            description: concat!(
                "Execute a command. Prefer the structured form with 'cmd' and 'args' because it is more portable. ",
                "Use 'command' only when you truly need a shell expression. ",
                "Optionally pass 'workdir' relative to the workspace root."
            )
            .to_string(),
            input_schema: schema_for_type::<CmdExecArgs>(),
            output_schema: None,
        },
        ToolDescriptor {
            name: "search.rg".to_string(),
            description: "Search for patterns in files using ripgrep.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Search pattern"},
                    "path": {"type": "string", "description": "Directory to search (default: workspace root)"}
                },
                "required": ["pattern"]
            }),
            output_schema: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// Main benchmark runner
// ---------------------------------------------------------------------------

pub async fn run_agentic_coding_benchmark(
    options: AgenticCodingBenchOptions,
) -> AgenticCodingBenchReport {
    let _start_time = Instant::now();
    let task_definitions = resolve_task_definitions(&options.task_filter);
    let tasks = build_task_descriptors_from_definitions(&task_definitions);

    let mut provider_results = Vec::new();

    for provider_id in &options.providers {
        let provider_result =
            run_provider_coding_benchmark(*provider_id, &options, &task_definitions).await;
        provider_results.push(provider_result);
    }

    // Determine overall winner based on success rate
    let overall_winner = provider_results
        .iter()
        .filter(|p| p.aggregate.success_rate > 0.5)
        .max_by(|a, b| {
            a.aggregate
                .success_rate
                .partial_cmp(&b.aggregate.success_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|p| p.provider.clone());

    AgenticCodingBenchReport {
        metadata: BenchmarkRunMetadata::new(
            WorkloadKind::LlmAgenticCoding,
            0, // warmup_iterations
            1, // measured_iterations (one run per task)
            vec![],
        ),
        providers: provider_results,
        tasks,
        overall_winner,
    }
}

async fn run_provider_coding_benchmark(
    provider_id: LlmProviderId,
    options: &AgenticCodingBenchOptions,
    task_definitions: &[CodingTaskDefinition],
) -> AgenticCodingProviderResult {
    let provider_start = Instant::now();

    // Get provider config
    let config = options
        .provider_configs
        .iter()
        .find(|c| c.provider == provider_id)
        .cloned()
        .unwrap_or_else(|| LlmProviderConfig {
            provider: provider_id,
            api_key: None,
            model: None,
            base_url: None,
            max_tokens: Some(options.max_tokens),
        });

    // Create the model client
    let client = match create_agentic_coding_client(provider_id, &config).await {
        Ok(client) => client,
        Err(error) => {
            return AgenticCodingProviderResult {
                provider: provider_id.to_string(),
                model: config.model.clone(),
                status: "error".to_string(),
                error: Some(error),
                total_duration_ms: provider_start.elapsed().as_secs_f64() * 1000.0,
                tasks: vec![],
                aggregate: AgenticCodingAggregateResult {
                    tasks_completed: 0,
                    tasks_failed: 0,
                    avg_duration_ms: 0.0,
                    total_tool_calls: 0,
                    success_rate: 0.0,
                },
            };
        }
    };

    let tools = agentic_coding_tools();
    let mut task_results = Vec::new();
    let mut total_tool_calls = 0usize;
    let mut completed_count = 0usize;
    let mut failed_count = 0usize;

    // Run each task
    for task_def in task_definitions {
        let task_result = run_coding_task(&client, task_def, &tools, options).await;

        total_tool_calls += task_result.tool_calls_made;
        if task_result.success {
            completed_count += 1;
        } else {
            failed_count += 1;
        }
        task_results.push(task_result);
    }

    let total_duration = provider_start.elapsed();
    let avg_duration = if !task_results.is_empty() {
        total_duration.as_secs_f64() * 1000.0 / task_results.len() as f64
    } else {
        0.0
    };

    let success_rate = if !task_results.is_empty() {
        completed_count as f64 / task_results.len() as f64
    } else {
        0.0
    };

    AgenticCodingProviderResult {
        provider: provider_id.to_string(),
        model: config.model.clone(),
        status: if failed_count == 0 {
            "completed".to_string()
        } else {
            "partial".to_string()
        },
        error: None,
        total_duration_ms: total_duration.as_secs_f64() * 1000.0,
        tasks: task_results,
        aggregate: AgenticCodingAggregateResult {
            tasks_completed: completed_count,
            tasks_failed: failed_count,
            avg_duration_ms: avg_duration,
            total_tool_calls,
            success_rate,
        },
    }
}

async fn run_coding_task(
    client: &AgenticCodingClient,
    task: &CodingTaskDefinition,
    tools: &[ToolDescriptor],
    options: &AgenticCodingBenchOptions,
) -> AgenticCodingTaskResult {
    let task_start = Instant::now();
    let workspace = match create_temp_workspace(&task.setup_files, options).await {
        Ok(w) => w,
        Err(error) => {
            return AgenticCodingTaskResult {
                task_id: task.id.to_string(),
                status: "error".to_string(),
                error: Some(format!("Failed to create workspace: {error}")),
                duration_ms: 0.0,
                turns_taken: 0,
                tool_calls_made: 0,
                success: false,
                validation_passed: false,
                completion_signaled: false,
                validation_notes: vec![],
                retained_workspace: None,
                tool_trace: vec![],
            };
        }
    };

    let task_prompt = task_prompt_for_benchmark(task);
    let mut context = benchmark_context(task);
    let mut prior_observations: Vec<serde_json::Value> = vec![];

    let mut tool_calls_made = 0usize;
    let mut turn = 0usize;
    let max_turns = task.max_turns;
    let mut tool_trace: Vec<AgenticCodingToolTraceEntry> = vec![];

    // Tool registry for executing real tools
    let tool_registry = ToolRegistry::default();

    loop {
        if turn >= max_turns {
            break;
        }

        // Check timeout
        if task_start.elapsed().as_secs() > options.timeout_seconds {
            break;
        }

        // Get model decision
        let request = WorkerActionRequest {
            task_prompt: task_prompt.clone(),
            goal_summary: task.description.to_string(),
            context: context.clone(),
            available_tools: tools.iter().map(|t| t.name.clone()).collect(),
            tool_descriptors: tools.to_vec(),
            prior_observations: prior_observations.clone(),
            max_tokens: Some(options.max_tokens),
        };

        let decision = match client.decide_action(request).await {
            Ok(d) => d,
            Err(error) => {
                return AgenticCodingTaskResult {
                    task_id: task.id.to_string(),
                    status: "error".to_string(),
                    error: Some(format!("Model error: {error}")),
                    duration_ms: task_start.elapsed().as_secs_f64() * 1000.0,
                    turns_taken: turn,
                    tool_calls_made,
                    success: false,
                    validation_passed: false,
                    completion_signaled: false,
                    validation_notes: vec![],
                    retained_workspace: None,
                    tool_trace,
                };
            }
        };

        turn += 1;

        match decision.action {
            WorkerAction::Complete { summary: _ } => {
                // Task claims completion, validate it
                let validation = validate_task_completion(&workspace, task).await;
                let duration = task_start.elapsed();

                let retained_workspace = if validation.passed || !options.retain_failed_workspaces {
                    let _ = tokio::fs::remove_dir_all(&workspace).await;
                    None
                } else {
                    Some(workspace.to_string_lossy().to_string())
                };

                return AgenticCodingTaskResult {
                    task_id: task.id.to_string(),
                    status: if validation.passed {
                        "completed".to_string()
                    } else {
                        "completed_invalid".to_string()
                    },
                    error: if validation.passed {
                        None
                    } else {
                        Some("Model signaled completion, but validation failed".to_string())
                    },
                    duration_ms: duration.as_secs_f64() * 1000.0,
                    turns_taken: turn,
                    tool_calls_made,
                    success: validation.passed,
                    validation_passed: validation.passed,
                    completion_signaled: true,
                    validation_notes: validation.notes,
                    retained_workspace,
                    tool_trace,
                };
            }
            WorkerAction::ToolCalls { calls } => {
                tool_calls_made += calls.len();

                // Execute tool calls
                let mut observations = Vec::new();
                for call in calls {
                    let observation = execute_tool_call(
                        &tool_registry,
                        &call.tool_name,
                        &call.tool_args,
                        &workspace,
                    )
                    .await;
                    tool_trace.push(AgenticCodingToolTraceEntry {
                        tool_name: call.tool_name.clone(),
                        tool_args: call.tool_args.clone(),
                        observation: observation.clone(),
                    });
                    observations.push(observation);
                }

                // Add observations to prior_observations
                for observation in observations {
                    prior_observations.push(observation);
                }

                context = benchmark_context(task);
            }
            WorkerAction::ToolCall {
                tool_name,
                tool_args,
                ..
            } => {
                // Single tool call (legacy format, convert to batch)
                tool_calls_made += 1;

                let observation =
                    execute_tool_call(&tool_registry, &tool_name, &tool_args, &workspace).await;
                tool_trace.push(AgenticCodingToolTraceEntry {
                    tool_name: tool_name.clone(),
                    tool_args: tool_args.clone(),
                    observation: observation.clone(),
                });

                prior_observations.push(observation);

                context = format!(
                    "{}\n\nLatest tool executed: {}. Use prior observations to decide whether to return completion now.",
                    benchmark_context(task),
                    tool_name
                );
            }
            WorkerAction::Delegate { .. } => {
                // Delegation not supported in benchmark
                context = format!(
                    "{}\n\n[Note: Sub-agent delegation is not available in this benchmark. Please complete the task directly or make tool calls.]",
                    benchmark_context(task)
                );
            }
        }
    }

    // Max turns reached
    let validation = validate_task_completion(&workspace, task).await;
    let duration = task_start.elapsed();

    let retained_workspace = if options.retain_failed_workspaces {
        Some(workspace.to_string_lossy().to_string())
    } else {
        let _ = tokio::fs::remove_dir_all(&workspace).await;
        None
    };

    AgenticCodingTaskResult {
        task_id: task.id.to_string(),
        status: "max_turns".to_string(),
        error: Some("Reached maximum turns without completion".to_string()),
        duration_ms: duration.as_secs_f64() * 1000.0,
        turns_taken: turn,
        tool_calls_made,
        success: false,
        validation_passed: validation.passed,
        completion_signaled: false,
        validation_notes: validation.notes,
        retained_workspace,
        tool_trace,
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

async fn create_temp_workspace(
    setup_files: &[(&str, &str)],
    options: &AgenticCodingBenchOptions,
) -> Result<PathBuf, String> {
    let temp_dir = options
        .scratch_root
        .clone()
        .unwrap_or_else(std::env::temp_dir);
    let workspace_name = format!("orchestrix_agentic_bench_{}", uuid::Uuid::new_v4());
    let workspace_path = temp_dir.join(&workspace_name);

    fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| format!("Failed to create scratch root: {e}"))?;

    fs::create_dir_all(&workspace_path)
        .await
        .map_err(|e| format!("Failed to create workspace: {e}"))?;

    // Create setup files
    for (relative_path, content) in setup_files {
        let file_path = workspace_path.join(relative_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create directory: {e}"))?;
        }
        fs::write(&file_path, content)
            .await
            .map_err(|e| format!("Failed to write file: {e}"))?;
    }

    Ok(workspace_path)
}

fn benchmark_context(task: &CodingTaskDefinition) -> String {
    let today = Local::now().format("%Y-%m-%d").to_string();
    format!(
        "Benchmark harness context:\n- Treat this as a tool-using coding task.\n- Use prior observations for tool results instead of repeating the same reads or commands.\n- If the task is already satisfied, return completion immediately.\n- A task is not complete until every requested output file is actually written.\n- Prefer `cmd.exec` with structured `cmd` + `args` over a shell `command` string when quoting or platform differences matter.\n- Today's local benchmark date is {today}. If the task asks for today's date, use exactly that value.\n- Do not keep exploring once validation-relevant work is done.\n\nTask category: {:?}\nMax turns: {}",
        task.category, task.max_turns
    )
}

fn task_prompt_for_benchmark(task: &CodingTaskDefinition) -> String {
    let today = Local::now().format("%Y-%m-%d").to_string();

    match task.id {
        "command_count_files" => format!(
            "{}\n\nPortable command hint: prefer cmd.exec with structured fields like {{\"cmd\": \"powershell\", \"args\": [\"-NoProfile\", \"-Command\", \"...\"]}} instead of a single shell command string. After you obtain the count, you still need to write count_report.txt.",
            task.initial_prompt
        ),
        "search_and_update" => format!(
            "{}\n\nFor this benchmark run, today's date is [{}]. Use that exact date in the TODO comments.",
            task.initial_prompt, today
        ),
        _ => task.initial_prompt.to_string(),
    }
}

async fn execute_tool_call(
    registry: &ToolRegistry,
    tool_name: &str,
    arguments: &serde_json::Value,
    workspace: &PathBuf,
) -> serde_json::Value {
    // Adapt arguments to include workspace context if needed
    let mut adapted_args = arguments.clone();

    // For filesystem tools, ensure paths are relative to workspace
    if tool_name.starts_with("fs.") || tool_name == "search.rg" {
        if let Some(path) = arguments.get("path").and_then(|p| p.as_str()) {
            let full_path = workspace.join(path);
            adapted_args["path"] = serde_json::json!(full_path.to_string_lossy().to_string());
        }
    }

    // For command execution, run in workspace
    if tool_name == "cmd.exec" {
        adapted_args["workdir"] = serde_json::json!(".");
    }

    // For search, set path to workspace
    if tool_name == "search.rg" {
        adapted_args["path"] = serde_json::json!(workspace.to_string_lossy().to_string());
    }

    let input = ToolCallInput {
        name: tool_name.to_string(),
        args: adapted_args,
    };

    // Create a policy engine for the workspace
    let policy = PolicyEngine::new(workspace.clone());

    match registry.invoke(&policy, workspace, input) {
        Ok(output) => serde_json::json!({
            "tool_name": tool_name,
            "status": "success",
            "result": output.data
        }),
        Err(error) => serde_json::json!({
            "tool_name": tool_name,
            "status": "error",
            "error": error.to_string()
        }),
    }
}

async fn validate_task_completion(
    workspace: &PathBuf,
    task: &CodingTaskDefinition,
) -> ValidationOutcome {
    let mut notes = Vec::new();

    // Check expected files exist
    for expected_file in &task.expected_files {
        let file_path = workspace.join(expected_file);
        if !file_path.exists() {
            return ValidationOutcome {
                passed: false,
                notes: vec![format!("Missing expected file: {expected_file}")],
            };
        }
        notes.push(format!("Found expected file: {expected_file}"));
    }

    // Run validation commands
    for command in &task.validation_commands {
        match run_validation_command(workspace, command).await {
            Ok(result) if result.status.success() => {
                notes.push(format!("Validation command passed: {command}"));
            }
            Ok(_) | Err(_) => {
                return ValidationOutcome {
                    passed: false,
                    notes: vec![format!("Validation command failed: {command}")],
                };
            }
        }
    }

    let semantic = validate_task_semantics(workspace, task).await;
    notes.extend(semantic.notes.clone());

    ValidationOutcome {
        passed: semantic.passed,
        notes,
    }
}

fn get_api_key_from_env(provider_id: LlmProviderId) -> Option<String> {
    let keys = api_key_env_keys(provider_id);
    first_non_empty_env(&keys)
}

fn resolve_task_definitions(task_filter: &[String]) -> Vec<CodingTaskDefinition> {
    let requested: Vec<String> = task_filter
        .iter()
        .map(|task_id| task_id.trim().to_string())
        .filter(|task_id| !task_id.is_empty())
        .collect();

    if requested.is_empty() {
        return coding_tasks();
    }

    let all_tasks = coding_tasks();
    let mut selected = Vec::new();

    for requested_id in requested {
        if let Some(task) = all_tasks
            .iter()
            .find(|task| task.id.eq_ignore_ascii_case(requested_id.as_str()))
        {
            if !selected
                .iter()
                .any(|existing: &CodingTaskDefinition| existing.id.eq_ignore_ascii_case(task.id))
            {
                selected.push(task.clone());
            }
        }
    }

    selected
}

fn build_task_descriptors_from_definitions(
    task_definitions: &[CodingTaskDefinition],
) -> Vec<AgenticCodingTaskDescriptor> {
    task_definitions
        .iter()
        .map(|task| AgenticCodingTaskDescriptor {
            task_id: task.id.to_string(),
            task_label: task.label.to_string(),
            description: task.description.to_string(),
            category: task.category,
            max_turns: task.max_turns,
            expected_files: task.expected_files.iter().map(|s| s.to_string()).collect(),
            validation_commands: task
                .validation_commands
                .iter()
                .map(|s| s.to_string())
                .collect(),
        })
        .collect()
}

async fn run_validation_command(
    workspace: &PathBuf,
    command: &str,
) -> Result<std::process::Output, std::io::Error> {
    #[cfg(target_os = "windows")]
    {
        tokio::process::Command::new("cmd")
            .args(["/C", command])
            .current_dir(workspace)
            .output()
            .await
    }

    #[cfg(not(target_os = "windows"))]
    {
        tokio::process::Command::new("sh")
            .args(["-lc", command])
            .current_dir(workspace)
            .output()
            .await
    }
}

async fn validate_task_semantics(
    workspace: &Path,
    task: &CodingTaskDefinition,
) -> ValidationOutcome {
    match task.id {
        "file_create_read" => validate_file_create_read(workspace).await,
        "list_and_analyze" => validate_list_and_analyze(workspace).await,
        "command_count_files" => validate_command_count_files(workspace).await,
        "code_generate_function" => validate_code_generate_function(workspace).await,
        "multistep_refactor" => validate_multistep_refactor(workspace).await,
        "search_and_update" => validate_search_and_update(workspace).await,
        _ => ValidationOutcome {
            passed: true,
            notes: vec!["No semantic validator defined for task".to_string()],
        },
    }
}

async fn validate_file_create_read(workspace: &Path) -> ValidationOutcome {
    let path = workspace.join("config.json");
    let content = match fs::read_to_string(&path).await {
        Ok(value) => value,
        Err(error) => {
            return ValidationOutcome {
                passed: false,
                notes: vec![format!("Failed to read config.json: {error}")],
            };
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(value) => value,
        Err(error) => {
            return ValidationOutcome {
                passed: false,
                notes: vec![format!("config.json is not valid JSON: {error}")],
            };
        }
    };

    let name_ok = parsed.get("name").and_then(|v| v.as_str()) == Some("my-app");
    let version_ok = parsed.get("version").and_then(|v| v.as_str()) == Some("1.0.0");
    let port_ok = parsed.get("port").and_then(|v| v.as_i64()) == Some(3000);

    ValidationOutcome {
        passed: name_ok && version_ok && port_ok,
        notes: vec![
            format!("config.json name matches: {name_ok}"),
            format!("config.json version matches: {version_ok}"),
            format!("config.json port matches: {port_ok}"),
        ],
    }
}

async fn validate_list_and_analyze(workspace: &Path) -> ValidationOutcome {
    let report = match fs::read_to_string(workspace.join("file_inventory.txt")).await {
        Ok(value) => value,
        Err(error) => {
            return ValidationOutcome {
                passed: false,
                notes: vec![format!("Failed to read file_inventory.txt: {error}")],
            };
        }
    };

    let expected_entries = [
        "src/main.py",
        "src/utils.py",
        "README.md",
        "requirements.txt",
    ];
    let mut notes = Vec::new();
    let mut passed = true;

    for entry in expected_entries {
        let present = report.contains(entry);
        notes.push(format!("Inventory mentions {entry}: {present}"));
        passed &= present;
    }

    ValidationOutcome { passed, notes }
}

async fn validate_command_count_files(workspace: &Path) -> ValidationOutcome {
    let report = match fs::read_to_string(workspace.join("count_report.txt")).await {
        Ok(value) => value,
        Err(error) => {
            return ValidationOutcome {
                passed: false,
                notes: vec![format!("Failed to read count_report.txt: {error}")],
            };
        }
    };

    let expected_count = 4_i64;
    let numbers = report
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|token| !token.is_empty())
        .filter_map(|token| token.parse::<i64>().ok())
        .collect::<Vec<_>>();
    let mentions_expected_count = numbers.contains(&expected_count);

    ValidationOutcome {
        passed: mentions_expected_count,
        notes: vec![
            format!("Parsed numeric values from report: {:?}", numbers),
            format!(
                "Report includes expected file count {expected_count}: {mentions_expected_count}"
            ),
        ],
    }
}

async fn validate_code_generate_function(workspace: &Path) -> ValidationOutcome {
    let content = match fs::read_to_string(workspace.join("math_utils.py")).await {
        Ok(value) => value,
        Err(error) => {
            return ValidationOutcome {
                passed: false,
                notes: vec![format!("Failed to read math_utils.py: {error}")],
            };
        }
    };

    let has_function = content.contains("def safe_divide");
    let handles_zero = content.contains("return None");
    let has_docstring = content.contains("\"\"\"");
    let has_main_guard = content.contains("if __name__ == '__main__':")
        || content.contains("if __name__ == \"__main__\":");

    ValidationOutcome {
        passed: has_function && handles_zero && has_docstring && has_main_guard,
        notes: vec![
            format!("Defines safe_divide: {has_function}"),
            format!("Handles division by zero with return None: {handles_zero}"),
            format!("Includes docstring: {has_docstring}"),
            format!("Includes executable test block: {has_main_guard}"),
        ],
    }
}

async fn validate_multistep_refactor(workspace: &Path) -> ValidationOutcome {
    let content = match fs::read_to_string(workspace.join("calculator.py")).await {
        Ok(value) => value,
        Err(error) => {
            return ValidationOutcome {
                passed: false,
                notes: vec![format!("Failed to read calculator.py: {error}")],
            };
        }
    };

    let multiply_fixed = content.contains("return a * b");
    let file_mentions_test = content.contains("12") || content.contains("assert");

    ValidationOutcome {
        passed: multiply_fixed && file_mentions_test,
        notes: vec![
            format!("multiply function uses multiplication: {multiply_fixed}"),
            format!(
                "file contains a fix-verification test or expected output: {file_mentions_test}"
            ),
        ],
    }
}

async fn validate_search_and_update(workspace: &Path) -> ValidationOutcome {
    let report = match fs::read_to_string(workspace.join("todo_report.md")).await {
        Ok(value) => value,
        Err(error) => {
            return ValidationOutcome {
                passed: false,
                notes: vec![format!("Failed to read todo_report.md: {error}")],
            };
        }
    };
    let main_py = match fs::read_to_string(workspace.join("main.py")).await {
        Ok(value) => value,
        Err(error) => {
            return ValidationOutcome {
                passed: false,
                notes: vec![format!("Failed to read main.py: {error}")],
            };
        }
    };
    let utils_py = match fs::read_to_string(workspace.join("utils.py")).await {
        Ok(value) => value,
        Err(error) => {
            return ValidationOutcome {
                passed: false,
                notes: vec![format!("Failed to read utils.py: {error}")],
            };
        }
    };

    let today = Local::now().format("%Y-%m-%d").to_string();
    let date_tag = format!("[{today}]");
    let report_mentions_files = report.contains("main.py") && report.contains("utils.py");
    let main_todos_dated = main_py.matches(&date_tag).count() >= 2;
    let utils_todos_dated = utils_py.matches(&date_tag).count() >= 1;

    ValidationOutcome {
        passed: report_mentions_files && main_todos_dated && utils_todos_dated,
        notes: vec![
            format!("todo_report mentions both files: {report_mentions_files}"),
            format!("main.py TODOs dated with {date_tag}: {main_todos_dated}"),
            format!("utils.py TODOs dated with {date_tag}: {utils_todos_dated}"),
        ],
    }
}

// ---------------------------------------------------------------------------
// Public API for CLI
// ---------------------------------------------------------------------------

pub async fn run_quick_agentic_benchmark() -> AgenticCodingBenchReport {
    let options = AgenticCodingBenchOptions::default();
    run_agentic_coding_benchmark(options).await
}

pub fn available_agentic_coding_tasks() -> Vec<AgenticCodingTaskDescriptor> {
    build_task_descriptors_from_definitions(&coding_tasks())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticCodingScenarioDescriptor {
    pub scenario_id: String,
    pub name: String,
    pub description: String,
    pub task_count: usize,
    pub estimated_duration_seconds: u64,
}

pub fn available_agentic_coding_scenarios() -> Vec<AgenticCodingScenarioDescriptor> {
    vec![AgenticCodingScenarioDescriptor {
        scenario_id: "quick_agentic_coding".to_string(),
        name: "Quick Agentic Coding".to_string(),
        description: "6 tasks testing file operations, command execution, code generation, and multi-step workflows. Defaults to MiniMax M2.1. Estimated 1-2 minutes.".to_string(),
        task_count: 6,
        estimated_duration_seconds: 120,
    }]
}

// ---------------------------------------------------------------------------
// Agentic coding client using AgentModelClient trait
// ---------------------------------------------------------------------------

enum AgenticCodingClient {
    MiniMax(MiniMaxClient),
    Kimi(KimiClient),
    Zhipu(GlmClient),
    Modal(ModalClient),
}

impl AgentModelClient for AgenticCodingClient {
    fn model_id(&self) -> String {
        match self {
            Self::MiniMax(c) => c.model_id(),
            Self::Kimi(c) => c.model_id(),
            Self::Zhipu(c) => c.model_id(),
            Self::Modal(c) => c.model_id(),
        }
    }

    async fn decide_action(
        &self,
        req: WorkerActionRequest,
    ) -> Result<WorkerDecision, crate::model::ModelError> {
        match self {
            Self::MiniMax(c) => c.decide_action(req).await,
            Self::Kimi(c) => c.decide_action(req).await,
            Self::Zhipu(c) => c.decide_action(req).await,
            Self::Modal(c) => c.decide_action(req).await,
        }
    }
}

async fn create_agentic_coding_client(
    provider_id: LlmProviderId,
    config: &LlmProviderConfig,
) -> Result<AgenticCodingClient, String> {
    let api_key = config
        .api_key
        .clone()
        .or_else(|| get_api_key_from_env(provider_id))
        .ok_or_else(|| format!("No API key found for {}", provider_id.as_str()))?;

    let model = config.model.clone();
    let base_url = config.base_url.clone();

    match provider_id {
        LlmProviderId::MiniMax => {
            let client = MiniMaxClient::new(api_key, model);
            Ok(AgenticCodingClient::MiniMax(client))
        }
        LlmProviderId::Kimi => {
            let client = KimiClient::new(api_key, model, base_url);
            Ok(AgenticCodingClient::Kimi(client))
        }
        LlmProviderId::Zhipu => {
            let client = GlmClient::new(api_key, model, base_url);
            Ok(AgenticCodingClient::Zhipu(client))
        }
        LlmProviderId::Modal => {
            let client = ModalClient::new(api_key, model, base_url);
            Ok(AgenticCodingClient::Modal(client))
        }
    }
}
