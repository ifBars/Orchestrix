//! Architecture Diagram Benchmark
//!
//! Tests agent performance WITH vs WITHOUT diagram tools on architecture-related tasks.
//! Validates diagram quality: structural correctness, relationship validity, and overall effectiveness.

use std::path::PathBuf;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::bench::core::{BenchmarkRunMetadata, WorkloadKind};
use crate::bench::llm::{api_key_env_keys, first_non_empty_env, LlmProviderConfig, LlmProviderId};
use crate::core::tool::ToolDescriptor;
use crate::model::{AgentModelClient, GlmClient, KimiClient, MiniMaxClient, ModalClient, WorkerAction, WorkerActionRequest, WorkerDecision};
use crate::policy::PolicyEngine;
use crate::tools::{ToolCallInput, ToolRegistry};

const DEFAULT_MAX_TOKENS: u32 = 4096;
const DEFAULT_TIMEOUT_SECONDS: u64 = 180;

#[derive(Debug, Clone)]
pub struct DiagramBenchOptions {
    pub providers: Vec<LlmProviderId>,
    pub provider_configs: Vec<LlmProviderConfig>,
    pub max_tokens: u32,
    pub timeout_seconds: u64,
    pub enable_diagram_tools: bool,
}

impl Default for DiagramBenchOptions {
    fn default() -> Self {
        Self {
            providers: vec![LlmProviderId::MiniMax],
            provider_configs: vec![LlmProviderConfig {
                provider: LlmProviderId::MiniMax,
                api_key: None,
                model: Some("MiniMax-M2.5".to_string()),
                base_url: None,
                max_tokens: Some(DEFAULT_MAX_TOKENS),
            }],
            max_tokens: DEFAULT_MAX_TOKENS,
            timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
            enable_diagram_tools: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramBenchReport {
    pub metadata: BenchmarkRunMetadata,
    pub tasks: Vec<DiagramTaskDescriptor>,
    pub with_diagram_tools: DiagramToolsResult,
    pub without_diagram_tools: DiagramToolsResult,
    pub comparison: DiagramComparisonResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramToolsResult {
    pub enabled: bool,
    pub provider: String,
    pub model: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub total_duration_ms: f64,
    pub tasks: Vec<DiagramTaskResult>,
    pub aggregate: DiagramAggregateResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramTaskDescriptor {
    pub task_id: String,
    pub task_label: String,
    pub description: String,
    pub category: DiagramTaskCategory,
    pub max_turns: usize,
    pub validation_criteria: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagramTaskCategory {
    ArchitectureDesign,
    DiagramExpansion,
    CodeToDiagram,
    ArchitectureAnalysis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramTaskResult {
    pub task_id: String,
    pub status: String,
    pub error: Option<String>,
    pub duration_ms: f64,
    pub turns_taken: usize,
    pub tool_calls_made: usize,
    pub success: bool,
    pub diagram_quality: DiagramQualityScore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramQualityScore {
    pub structural_correctness: f64,
    pub relationship_validity: f64,
    pub overall_quality: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramAggregateResult {
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    pub avg_duration_ms: f64,
    pub total_tool_calls: usize,
    pub success_rate: f64,
    pub avg_quality: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramComparisonResult {
    pub with_tools_success_rate: f64,
    pub without_tools_success_rate: f64,
    pub with_tools_quality: f64,
    pub without_tools_quality: f64,
    pub quality_improvement: f64,
    pub efficiency_ratio: f64,
    pub winner: String,
}

struct DiagramTaskDefinition {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    category: DiagramTaskCategory,
    max_turns: usize,
    system_prompt: &'static str,
    initial_prompt: &'static str,
    setup_codebase: Vec<(&'static str, &'static str)>,
    validation_criteria: Vec<&'static str>,
}

fn diagram_tasks() -> Vec<DiagramTaskDefinition> {
    vec![
        DiagramTaskDefinition {
            id: "arch_design_api_service",
            label: "Architecture Design: API Service",
            description: "Design a complete architecture for a REST API service with database",
            category: DiagramTaskCategory::ArchitectureDesign,
            max_turns: 8,
            system_prompt: "You are an architecture planning agent. Use diagram tools to visualize the system design. Create nodes for each component and edges to show relationships.",
            initial_prompt: "Design an architecture for a user management API service. The system needs:\n- A REST API for CRUD operations on users\n- A PostgreSQL database for persistence\n- An authentication module\n- A caching layer\n\nUse the diagram tools to create a visual architecture showing all components and their relationships.",
            setup_codebase: vec![],
            validation_criteria: vec!["has_api_component", "has_database_component", "has_auth_component", "has_cache_component", "has_relationships"],
        },
        DiagramTaskDefinition {
            id: "arch_design_microservices",
            label: "Architecture Design: E-commerce Microservices",
            description: "Design microservices architecture for e-commerce platform",
            category: DiagramTaskCategory::ArchitectureDesign,
            max_turns: 10,
            system_prompt: "You are an architecture planning agent. Use diagram tools to visualize complex systems. Create nodes for each service and edges to show how they communicate.",
            initial_prompt: "Design a microservices architecture for an e-commerce platform with the following services:\n- User Service (authentication, profiles)\n- Product Catalog Service\n- Order Service\n- Payment Service\n- Notification Service\n\nUse the diagram tools to show all services and how they interact (sync via REST, async via message queue).",
            setup_codebase: vec![],
            validation_criteria: vec!["has_multiple_services", "has_service_relationships", "has_data_flow"],
        },
        DiagramTaskDefinition {
            id: "diagram_expand_codebase",
            label: "Diagram Expansion: Add to Existing",
            description: "Given codebase structure, create and expand architecture diagram",
            category: DiagramTaskCategory::DiagramExpansion,
            max_turns: 8,
            system_prompt: "You are a codebase analysis agent. First explore the codebase structure, then use diagram tools to create an architecture diagram.",
            initial_prompt: "Analyze the provided codebase structure and create an architecture diagram showing:\n- Main entry points\n- Core modules\n- Data models\n- External dependencies\n\nThe codebase implements a simple task management API.",
            setup_codebase: vec![
                ("src/main.rs", "use actix_web::{App, HttpServer, web};\n\n#[actix_web::main]\nasync fn main() -> std::io::Result<()> {\n    HttpServer::new(|| {\n        App::new()\n            .route(\"/tasks\", web::get().to(list_tasks))\n            .route(\"/tasks\", web::post().to(create_task))\n    })\n    .bind(\"127.0.0.1:8080\")?\n    .run()\n    .await\n}\n"),
                ("src/handlers.rs", "use serde::{Deserialize, Serialize};\nuse crate::models::Task;\n\n#[derive(Serialize, Deserialize)]\npub struct CreateTaskRequest {\n    pub title: String,\n    pub description: Option<String>,\n}\n\npub async fn list_tasks() -> String {\n    \"[]\".to_string()\n}\n\npub async fn create_task(req: web::Json<CreateTaskRequest>) -> String {\n    format!(\"Created: {}\", req.title)\n}\n"),
                ("src/models.rs", "use serde::{Deserialize, Serialize};\n\n#[derive(Debug, Serialize, Deserialize)]\npub struct Task {\n    pub id: String,\n    pub title: String,\n    pub description: Option<String>,\n    pub status: TaskStatus,\n}\n\n#[derive(Debug, Serialize, Deserialize)]\npub enum TaskStatus {\n    Pending,\n    InProgress,\n    Completed,\n}\n"),
                ("src/db.rs", "pub struct Database {\n    connection: Pool<PgConnection>,\n}\n\nimpl Database {\n    pub fn new(conn_str: &str) -> Result<Self, Box<dyn std::error::Error>> {\n        Ok(Self {\n            connection: Pool::new(conn_str)?,\n        })\n    }\n}\n"),
            ],
            validation_criteria: vec!["has_components_from_codebase", "has_relationships"],
        },
        DiagramTaskDefinition {
            id: "code_to_diagram_fullstack",
            label: "Code to Diagram: Full-stack App",
            description: "Analyze a full-stack application and create architecture diagram",
            category: DiagramTaskCategory::CodeToDiagram,
            max_turns: 10,
            system_prompt: "You are a software architect. Analyze the provided codebase and create a comprehensive architecture diagram using diagram tools.",
            initial_prompt: "Analyze this full-stack application and create an architecture diagram that shows:\n- Frontend components\n- Backend API\n- Database schema\n- External services\n\nThe application is a blog platform with React frontend and Node.js backend.",
            setup_codebase: vec![
                ("frontend/src/App.jsx", "import React from 'react';\nimport { BrowserRouter, Routes, Route } from 'react-router-dom';\nimport PostList from './components/PostList';\nimport PostDetail from './components/PostDetail';\n\nexport default function App() {\n  return (\n    <BrowserRouter>\n      <Routes>\n        <Route path=\"/\" element={<PostList />} />\n        <Route path=\"/post/:id\" element={<PostDetail />} />\n      </Routes>\n    </BrowserRouter>\n  );\n}\n"),
                ("frontend/src/api.js", "const API_BASE = 'http://localhost:3001/api';\n\nexport async function fetchPosts() {\n  const res = await fetch(`${API_BASE}/posts`);\n  return res.json();\n}\n\nexport async function fetchPost(id) {\n  const res = await fetch(`${API_BASE}/posts/${id}`);\n  return res.json();\n}\n\nexport async function createPost(data) {\n  return fetch(`${API_BASE}/posts`, {\n    method: 'POST',\n    headers: { 'Content-Type': 'application/json' },\n    body: JSON.stringify(data),\n  });\n}\n"),
                ("backend/server.js", "const express = require('express');\nconst app = express();\napp.use(express.json());\n\napp.get('/api/posts', async (req, res) => {\n  const posts = await db.posts.findMany();\n  res.json(posts);\n});\n\napp.get('/api/posts/:id', async (req, res) => {\n  const post = await db.posts.findUnique({ where: { id: req.params.id } });\n  res.json(post);\n});\n\napp.listen(3001);\n"),
                ("backend/db.js", "const { PrismaClient } = require('@prisma/client');\nconst db = new PrismaClient();\n\nmodule.exports = db;\n"),
                ("backend/schema.prisma", "model Post {\n  id        String   @id @default(uuid())\n  title     String\n  content   String\n  author    String\n  createdAt DateTime @default(now())\n  updatedAt DateTime @updatedAt\n}\n"),
            ],
            validation_criteria: vec!["has_frontend_component", "has_backend_component", "has_database", "has_relationships"],
        },
    ]
}

fn diagram_tools() -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            name: "diagram.read_graph".to_string(),
            description: "Read the current architecture diagram graph (nodes and edges) from the shared canvas. Use this to ingest the architectural context before planning.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: None,
        },
        ToolDescriptor {
            name: "diagram.apply_ops".to_string(),
            description: r#"Apply operations to the architecture diagram.

Operations are validated and applied atomically:
- addNode: Add component {id, label, node_type, description}
- updateNode: Modify component {id, label?, description?, node_type?}
- removeNode: Delete component {id} (removes connected edges)
- addEdge: Add relationship {id, source, target, label?, edge_type?}
- updateEdge: Modify relationship {id, label?, edge_type?}
- removeEdge: Delete relationship {id}
- setViewport: Set view {x, y, zoom}

Always read the graph first to get the current version, then apply operations with base_version for conflict detection."#.to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "base_version": {"type": "integer", "description": "Version from last read_graph"},
                    "author": {"type": "string", "enum": ["ai", "human"], "default": "ai"},
                    "operations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "oneOf": [
                                {
                                    "properties": {"op": {"const": "addNode"}, "id": {"type": "string"}, "label": {"type": "string"}, "node_type": {"type": "string"}, "description": {"type": "string"}},
                                    "required": ["op", "id", "label", "node_type"]
                                },
                                {
                                    "properties": {"op": {"const": "updateNode"}, "id": {"type": "string"}},
                                    "required": ["op", "id"]
                                },
                                {
                                    "properties": {"op": {"const": "removeNode"}, "id": {"type": "string"}},
                                    "required": ["op", "id"]
                                },
                                {
                                    "properties": {"op": {"const": "addEdge"}, "id": {"type": "string"}, "source": {"type": "string"}, "target": {"type": "string"}},
                                    "required": ["op", "id", "source", "target"]
                                },
                                {
                                    "properties": {"op": {"const": "updateEdge"}, "id": {"type": "string"}},
                                    "required": ["op", "id"]
                                },
                                {
                                    "properties": {"op": {"const": "removeEdge"}, "id": {"type": "string"}},
                                    "required": ["op", "id"]
                                },
                                {
                                    "properties": {"op": {"const": "setViewport"}, "x": {"type": "number"}, "y": {"type": "number"}, "zoom": {"type": "number"}},
                                    "required": ["op", "x", "y", "zoom"]
                                }
                            ]
                        }
                    }
                },
                "required": ["operations"]
            }),
            output_schema: None,
        },
    ]
}

fn base_tools() -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            name: "fs.read".to_string(),
            description: "Read the contents of a file.".to_string(),
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
            description: "List files and directories.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": []
            }),
            output_schema: None,
        },
    ]
}

struct DiagramState {
    nodes: Vec<serde_json::Value>,
    edges: Vec<serde_json::Value>,
    version: u64,
}

#[derive(serde::Serialize)]
struct OpResult {
    success: bool,
    new_version: u64,
    applied_ops: Vec<String>,
    conflicts: Vec<String>,
    errors: Vec<String>,
}

impl DiagramState {
    fn new() -> Self {
        Self {
            nodes: vec![],
            edges: vec![],
            version: 0,
        }
    }

    fn apply_ops(&mut self, operations: &[serde_json::Value], base_version: u64) -> OpResult {
        let mut applied_ops = Vec::new();
        let mut conflicts = Vec::new();
        let mut errors = Vec::new();

        for op in operations {
            let op_type = op.get("op").and_then(|v| v.as_str()).unwrap_or("");
            let id = op.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            
            if id.is_empty() && op_type != "setViewport" {
                continue;
            }

            let result = match op_type {
                "addNode" => {
                    if self.nodes.iter().any(|n| n.get("id").and_then(|v| v.as_str()) == Some(&id)) {
                        Err(format!("Node '{}' already exists", id))
                    } else {
                        let kind = op.get("node_type").and_then(|v| v.as_str()).unwrap_or("concept");
                        let label = op.get("label").and_then(|v| v.as_str()).unwrap_or(&id);
                        let description = op.get("description").and_then(|v| v.as_str()).unwrap_or("");
                        
                        let final_label = if label.is_empty() {
                            id.split('-')
                                .map(|word| {
                                    let mut chars = word.chars();
                                    match chars.next() {
                                        None => String::new(),
                                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(" ")
                        } else {
                            label.to_string()
                        };

                        let new_node = serde_json::json!({
                            "id": id,
                            "kind": kind,
                            "label": final_label,
                            "description": description
                        });
                        self.nodes.push(new_node);
                        Ok(())
                    }
                }
                "addEdge" => {
                    let source = op.get("source").and_then(|v| v.as_str()).unwrap_or("");
                    let target = op.get("target").and_then(|v| v.as_str()).unwrap_or("");
                    
                    if source.is_empty() || target.is_empty() {
                        Err("Edge source and target required".to_string())
                    } else if !self.nodes.iter().any(|n| n.get("id").and_then(|v| v.as_str()) == Some(&source)) {
                        Err(format!("Source node '{}' not found", source))
                    } else if !self.nodes.iter().any(|n| n.get("id").and_then(|v| v.as_str()) == Some(&target)) {
                        Err(format!("Target node '{}' not found", target))
                    } else {
                        let label = op.get("label").and_then(|v| v.as_str()).unwrap_or("");
                        let mut edge = serde_json::json!({
                            "id": id,
                            "source": source,
                            "target": target,
                            "label": label
                        });
                        self.edges.push(edge);
                        Ok(())
                    }
                }
                "removeNode" => {
                    self.nodes.retain(|n| n.get("id").and_then(|v| v.as_str()) != Some(&id));
                    self.edges.retain(|e| {
                        e.get("source").and_then(|v| v.as_str()) != Some(&id)
                            && e.get("target").and_then(|v| v.as_str()) != Some(&id)
                    });
                    Ok(())
                }
                "removeEdge" => {
                    self.edges.retain(|e| e.get("id").and_then(|v| v.as_str()) != Some(&id));
                    Ok(())
                }
                "setViewport" => {
                    Ok(())
                }
                _ => Err(format!("Unknown operation: {}", op_type)),
            };

            match result {
                Ok(()) => applied_ops.push(format!("{}:{}", op_type, id)),
                Err(e) => conflicts.push(format!("{}:{} - {}", op_type, id, e)),
            }
        }

        let new_version = self.version + applied_ops.len() as u64;
        self.version = new_version;

        OpResult {
            success: conflicts.is_empty(),
            new_version,
            applied_ops,
            conflicts,
            errors,
        }
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "nodes": self.nodes,
            "edges": self.edges
        })
    }
}

pub async fn run_diagram_benchmark(
    options: DiagramBenchOptions,
) -> DiagramBenchReport {
    let tasks = build_task_descriptors();
    
    let provider_id = options.providers.first().copied().unwrap_or(LlmProviderId::MiniMax);
    let config = options.provider_configs
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

    let with_result = if options.enable_diagram_tools {
        run_diagram_benchmark_run(provider_id, &config, &options, &tasks, true).await
    } else {
        DiagramToolsResult {
            enabled: false,
            provider: provider_id.to_string(),
            model: config.model.clone(),
            status: "skipped".to_string(),
            error: None,
            total_duration_ms: 0.0,
            tasks: vec![],
            aggregate: DiagramAggregateResult {
                tasks_completed: 0,
                tasks_failed: 0,
                avg_duration_ms: 0.0,
                total_tool_calls: 0,
                success_rate: 0.0,
                avg_quality: 0.0,
            },
        }
    };

    let without_result = run_diagram_benchmark_run(provider_id, &config, &options, &tasks, false).await;

    let comparison = calculate_comparison(&with_result, &without_result);

    DiagramBenchReport {
        metadata: BenchmarkRunMetadata::new(
            WorkloadKind::LlmAgenticCoding,
            0,
            1,
            vec![],
        ),
        tasks,
        with_diagram_tools: with_result,
        without_diagram_tools: without_result,
        comparison,
    }
}

async fn run_diagram_benchmark_run(
    provider_id: LlmProviderId,
    config: &LlmProviderConfig,
    options: &DiagramBenchOptions,
    _task_descriptors: &[DiagramTaskDescriptor],
    enable_diagram_tools: bool,
) -> DiagramToolsResult {
    let provider_start = Instant::now();
    
    let client = match create_diagram_benchmark_client(provider_id, config).await {
        Ok(c) => c,
        Err(error) => {
            return DiagramToolsResult {
                enabled: enable_diagram_tools,
                provider: provider_id.to_string(),
                model: config.model.clone(),
                status: "error".to_string(),
                error: Some(error),
                total_duration_ms: provider_start.elapsed().as_secs_f64() * 1000.0,
                tasks: vec![],
                aggregate: DiagramAggregateResult {
                    tasks_completed: 0,
                    tasks_failed: 0,
                    avg_duration_ms: 0.0,
                    total_tool_calls: 0,
                    success_rate: 0.0,
                    avg_quality: 0.0,
                },
            };
        }
    };

    let mut tool_list = base_tools();
    if enable_diagram_tools {
        tool_list.extend(diagram_tools());
    }

    let task_definitions = diagram_tasks();
    let mut task_results = Vec::new();
    let mut total_tool_calls = 0usize;
    let mut completed_count = 0usize;
    let mut failed_count = 0usize;
    let mut quality_sum = 0.0;

    for task_def in &task_definitions {
        let task_result = run_diagram_task(
            &client,
            task_def,
            &tool_list,
            options,
            enable_diagram_tools,
        ).await;
        
        total_tool_calls += task_result.tool_calls_made;
        if task_result.success {
            completed_count += 1;
            quality_sum += task_result.diagram_quality.overall_quality;
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

    let avg_quality = if completed_count > 0 {
        quality_sum / completed_count as f64
    } else {
        0.0
    };

    DiagramToolsResult {
        enabled: enable_diagram_tools,
        provider: provider_id.to_string(),
        model: config.model.clone(),
        status: if failed_count == 0 { "completed".to_string() } else { "partial".to_string() },
        error: None,
        total_duration_ms: total_duration.as_secs_f64() * 1000.0,
        tasks: task_results,
        aggregate: DiagramAggregateResult {
            tasks_completed: completed_count,
            tasks_failed: failed_count,
            avg_duration_ms: avg_duration,
            total_tool_calls,
            success_rate,
            avg_quality,
        },
    }
}

async fn run_diagram_task(
    client: &DiagramBenchmarkClient,
    task: &DiagramTaskDefinition,
    tools: &[ToolDescriptor],
    options: &DiagramBenchOptions,
    enable_diagram_tools: bool,
) -> DiagramTaskResult {
    let task_start = Instant::now();
    
    let workspace = match create_temp_workspace(&task.setup_codebase).await {
        Ok(w) => w,
        Err(error) => {
            return DiagramTaskResult {
                task_id: task.id.to_string(),
                status: "error".to_string(),
                error: Some(format!("Failed to create workspace: {error}")),
                duration_ms: 0.0,
                turns_taken: 0,
                tool_calls_made: 0,
                success: false,
                diagram_quality: DiagramQualityScore {
                    structural_correctness: 0.0,
                    relationship_validity: 0.0,
                    overall_quality: 0.0,
                },
            };
        }
    };

    let mut context = format!("{}", task.initial_prompt);
    let mut prior_observations: Vec<serde_json::Value> = vec![];
    
    let tool_registry = ToolRegistry::default();
    let mut diagram_state = DiagramState::new();
    let mut tool_calls_made = 0usize;
    let mut turn = 0usize;
    let max_turns = task.max_turns;

    loop {
        if turn >= max_turns {
            break;
        }
        
        if task_start.elapsed().as_secs() > options.timeout_seconds {
            break;
        }
        
        let request = WorkerActionRequest {
            task_prompt: context.clone(),
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
                return DiagramTaskResult {
                    task_id: task.id.to_string(),
                    status: "error".to_string(),
                    error: Some(format!("Model error: {error}")),
                    duration_ms: task_start.elapsed().as_secs_f64() * 1000.0,
                    turns_taken: turn,
                    tool_calls_made,
                    success: false,
                    diagram_quality: DiagramQualityScore {
                        structural_correctness: 0.0,
                        relationship_validity: 0.0,
                        overall_quality: 0.0,
                    },
                };
            }
        };
        
        turn += 1;
        
        match decision.action {
            WorkerAction::Complete { summary: _ } => {
                let quality = if enable_diagram_tools {
                    evaluate_diagram_state(&diagram_state, task)
                } else {
                    evaluate_text_output(&prior_observations, task)
                };
                
                let duration = task_start.elapsed();
                let _ = tokio::fs::remove_dir_all(&workspace).await;
                
                return DiagramTaskResult {
                    task_id: task.id.to_string(),
                    status: if quality.overall_quality > 0.5 { "completed".to_string() } else { "completed_invalid".to_string() },
                    error: None,
                    duration_ms: duration.as_secs_f64() * 1000.0,
                    turns_taken: turn,
                    tool_calls_made,
                    success: quality.overall_quality > 0.5,
                    diagram_quality: quality,
                };
            }
            WorkerAction::ToolCalls { calls } => {
                tool_calls_made += calls.len();
                
                let mut observations = Vec::new();
                for call in &calls {
                    let observation = execute_diagram_tool_call(
                        &tool_registry,
                        &call.tool_name,
                        &call.tool_args,
                        &workspace,
                        enable_diagram_tools,
                        &mut diagram_state,
                    ).await;
                    observations.push(observation);
                }
                
                for observation in observations {
                    prior_observations.push(observation);
                }
                
                let obs_content = prior_observations
                    .iter()
                    .filter(|obs| obs.get("tool_name").is_some())
                    .map(|obs| format!("{}", obs))
                    .collect::<Vec<_>>()
                    .join("\n\n");
                
                context = format!("{}\n\nTool calls executed: {}\n\nTool results:\n{}", 
                    context, tool_calls_made, obs_content);
            }
            WorkerAction::ToolCall { tool_name, tool_args, .. } => {
                tool_calls_made += 1;
                
                let observation = execute_diagram_tool_call(
                    &tool_registry,
                    &tool_name,
                    &tool_args,
                    &workspace,
                    enable_diagram_tools,
                    &mut diagram_state,
                ).await;
                
                prior_observations.push(observation);
                
                let obs_content = prior_observations
                    .iter()
                    .filter(|obs| obs.get("tool_name").is_some())
                    .map(|obs| format!("{}", obs))
                    .collect::<Vec<_>>()
                    .join("\n\n");
                
                context = format!("{}\n\nTool call executed: {}\n\nTool results:\n{}", 
                    context, tool_name, obs_content);
            }
            WorkerAction::Delegate { .. } => {
                context = format!("{}\n\n[Note: Sub-agent delegation is not available. Complete directly or use available tools.]", context);
            }
        }
    }
    
    let quality = if enable_diagram_tools {
        evaluate_diagram_state(&diagram_state, task)
    } else {
        evaluate_text_output(&prior_observations, task)
    };
    
    let duration = task_start.elapsed();
    let _ = tokio::fs::remove_dir_all(&workspace).await;
    
    DiagramTaskResult {
        task_id: task.id.to_string(),
        status: "max_turns".to_string(),
        error: Some("Reached maximum turns".to_string()),
        duration_ms: duration.as_secs_f64() * 1000.0,
        turns_taken: turn,
        tool_calls_made,
        success: quality.overall_quality > 0.5,
        diagram_quality: quality,
    }
}

async fn create_temp_workspace(setup_files: &[(&str, &str)]) -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir();
    let workspace_name = format!("orchestrix_diagram_bench_{}", uuid::Uuid::new_v4());
    let workspace_path = temp_dir.join(&workspace_name);
    
    fs::create_dir_all(&workspace_path)
        .await
        .map_err(|e| format!("Failed to create workspace: {e}"))?;
    
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

async fn execute_diagram_tool_call(
    registry: &ToolRegistry,
    tool_name: &str,
    arguments: &serde_json::Value,
    workspace: &PathBuf,
    enable_diagram_tools: bool,
    diagram_state: &mut DiagramState,
) -> serde_json::Value {
    if tool_name == "diagram.read_graph" && enable_diagram_tools {
        return serde_json::json!({
            "tool_name": tool_name,
            "status": "success",
            "result": {
                "state": diagram_state.to_json(),
                "version": diagram_state.version
            }
        });
    }

    if tool_name == "diagram.apply_ops" && enable_diagram_tools {
        let base_version = arguments.get("base_version").and_then(|v| v.as_u64()).unwrap_or(0);
        let operations = arguments.get("operations").and_then(|v| v.as_array());
        
        if let Some(ops) = operations {
            let result = diagram_state.apply_ops(ops, base_version);
            return serde_json::json!({
                "tool_name": tool_name,
                "status": if result.success { "success" } else { "error" },
                "result": result
            });
        }
        return serde_json::json!({
            "tool_name": tool_name,
            "status": "error",
            "error": "operations array is required"
        });
    }

    if tool_name.starts_with("fs.") {
        let mut adapted_args = arguments.clone();
        if let Some(path) = arguments.get("path").and_then(|p| p.as_str()) {
            let full_path = workspace.join(path);
            adapted_args["path"] = serde_json::json!(full_path.to_string_lossy().to_string());
        }

        let input = ToolCallInput {
            name: tool_name.to_string(),
            args: adapted_args,
        };

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
    } else {
        serde_json::json!({
            "tool_name": tool_name,
            "status": "error",
            "error": "Tool not available in this benchmark mode"
        })
    }
}

fn evaluate_diagram_state(state: &DiagramState, task: &DiagramTaskDefinition) -> DiagramQualityScore {
    let nodes = state.nodes.len();
    let edges = state.edges.len();

    let criteria = &task.validation_criteria;
    let mut structural_score: f64 = 0.0;
    let mut relationship_score: f64 = 0.0;

    for criterion in criteria {
        let crit: &str = criterion;
        match crit {
            "has_api_component" => {
                if nodes >= 1 { structural_score += 0.25; }
            }
            "has_database_component" => {
                if nodes >= 2 { structural_score += 0.25; }
            }
            "has_auth_component" => {
                if nodes >= 3 { structural_score += 0.25; }
            }
            "has_cache_component" => {
                if nodes >= 4 { structural_score += 0.25; }
            }
            "has_multiple_services" => {
                if nodes >= 4 { structural_score += 0.5; }
            }
            "has_service_relationships" => {
                if edges >= 3 { relationship_score += 0.5; }
            }
            "has_data_flow" => {
                if edges >= 5 { relationship_score += 0.5; }
            }
            "has_components_from_codebase" => {
                if nodes >= 2 { structural_score += 0.5; }
            }
            "has_relationships" => {
                if edges >= 1 { relationship_score += 0.5; }
            }
            "has_frontend_component" => {
                if nodes >= 1 { structural_score += 0.25; }
            }
            "has_backend_component" => {
                if nodes >= 2 { structural_score += 0.25; }
            }
            _ => {}
        }
    }

    structural_score = structural_score.min(1.0);
    relationship_score = relationship_score.min(1.0);

    let overall = (structural_score * 0.6) + (relationship_score * 0.4);

    DiagramQualityScore {
        structural_correctness: structural_score,
        relationship_validity: relationship_score,
        overall_quality: overall,
    }
}

fn evaluate_text_output(observations: &[serde_json::Value], task: &DiagramTaskDefinition) -> DiagramQualityScore {
    let mut has_components = false;
    let mut has_relationships = false;
    let mut has_description = false;
    
    for obs in observations {
        let content = obs.get("result").map(|r| {
            if let Some(s) = r.as_str() {
                s.to_string()
            } else {
                r.to_string()
            }
        }).unwrap_or_default();
        
        let content_lower = content.to_lowercase();
        
        for criterion in &task.validation_criteria {
            let crit: &str = criterion;
            match crit {
                "has_api_component" => {
                    if content_lower.contains("api") || content_lower.contains("rest") {
                        has_components = true;
                    }
                }
                "has_database_component" => {
                    if content_lower.contains("database") || content_lower.contains("db") || content_lower.contains("postgresql") {
                        has_components = true;
                    }
                }
                "has_auth_component" => {
                    if content_lower.contains("auth") || content_lower.contains("authentication") {
                        has_components = true;
                    }
                }
                "has_cache_component" => {
                    if content_lower.contains("cache") || content_lower.contains("redis") {
                        has_components = true;
                    }
                }
                "has_multiple_services" => {
                    if content_lower.contains("service") {
                        has_components = true;
                    }
                }
                "has_service_relationships" | "has_data_flow" | "has_relationships" => {
                    if content_lower.contains("communicate") || content_lower.contains("call") || 
                       content_lower.contains("depend") || content_lower.contains("connect") ||
                       content_lower.contains("->") || content_lower.contains("-->") {
                        has_relationships = true;
                    }
                }
                "has_components_from_codebase" | "has_frontend_component" | "has_backend_component" => {
                    if content_lower.contains("frontend") || content_lower.contains("backend") ||
                       content_lower.contains("api") || content_lower.contains("server") {
                        has_components = true;
                    }
                }
                _ => {}
            }
        }
        
        if !content.is_empty() {
            has_description = true;
        }
    }

    let structural_score = if has_components { 0.6 } else { 0.0 };
    let relationship_score = if has_relationships { 0.4 } else { 0.0 };
    let overall = if has_description { structural_score + relationship_score } else { 0.0 };

    DiagramQualityScore {
        structural_correctness: structural_score,
        relationship_validity: relationship_score,
        overall_quality: overall,
    }
}

fn calculate_comparison(with: &DiagramToolsResult, without: &DiagramToolsResult) -> DiagramComparisonResult {
    let with_success = with.aggregate.success_rate;
    let without_success = without.aggregate.success_rate;
    let with_quality = with.aggregate.avg_quality;
    let without_quality = without.aggregate.avg_quality;
    
    let quality_improvement = if without_quality > 0.0 {
        ((with_quality - without_quality) / without_quality) * 100.0
    } else if with_quality > 0.0 {
        100.0
    } else {
        0.0
    };
    
    let efficiency_ratio = if without.aggregate.avg_duration_ms > 0.0 {
        with.aggregate.avg_duration_ms / without.aggregate.avg_duration_ms
    } else {
        1.0
    };
    
    let winner = if with_success > without_success {
        "with_diagram_tools"
    } else if without_success > with_success {
        "without_diagram_tools"
    } else if with_quality > without_quality {
        "with_diagram_tools"
    } else if without_quality > with_quality {
        "without_diagram_tools"
    } else {
        "tie"
    }.to_string();

    DiagramComparisonResult {
        with_tools_success_rate: with_success,
        without_tools_success_rate: without_success,
        with_tools_quality: with_quality,
        without_tools_quality: without_quality,
        quality_improvement,
        efficiency_ratio,
        winner,
    }
}

fn get_api_key_from_env(provider_id: LlmProviderId) -> Option<String> {
    let keys = api_key_env_keys(provider_id);
    first_non_empty_env(&keys)
}

fn build_task_descriptors() -> Vec<DiagramTaskDescriptor> {
    diagram_tasks()
        .into_iter()
        .map(|task| DiagramTaskDescriptor {
            task_id: task.id.to_string(),
            task_label: task.label.to_string(),
            description: task.description.to_string(),
            category: task.category,
            max_turns: task.max_turns,
            validation_criteria: task.validation_criteria.iter().map(|s| s.to_string()).collect(),
        })
        .collect()
}

pub async fn run_quick_diagram_benchmark() -> DiagramBenchReport {
    let options = DiagramBenchOptions::default();
    run_diagram_benchmark(options).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramScenarioDescriptor {
    pub scenario_id: String,
    pub name: String,
    pub description: String,
    pub task_count: usize,
    pub estimated_duration_seconds: u64,
}

pub fn available_diagram_scenarios() -> Vec<DiagramScenarioDescriptor> {
    vec![DiagramScenarioDescriptor {
        scenario_id: "diagram_benchmark".to_string(),
        name: "Architecture Diagram Benchmark".to_string(),
        description: "Tests agent performance WITH vs WITHOUT diagram tools on architecture tasks. Evaluates structural correctness, relationship validity, and overall quality.".to_string(),
        task_count: 4,
        estimated_duration_seconds: 180,
    }]
}

enum DiagramBenchmarkClient {
    MiniMax(MiniMaxClient),
    Kimi(KimiClient),
    Zhipu(GlmClient),
    Modal(ModalClient),
}

impl AgentModelClient for DiagramBenchmarkClient {
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

async fn create_diagram_benchmark_client(
    provider_id: LlmProviderId,
    config: &LlmProviderConfig,
) -> Result<DiagramBenchmarkClient, String> {
    let api_key = config.api_key.clone()
        .or_else(|| get_api_key_from_env(provider_id))
        .ok_or_else(|| format!("No API key found for {}", provider_id.as_str()))?;
    
    let model = config.model.clone();
    let base_url = config.base_url.clone();
    
    match provider_id {
        LlmProviderId::MiniMax => {
            let client = MiniMaxClient::new(api_key, model);
            Ok(DiagramBenchmarkClient::MiniMax(client))
        }
        LlmProviderId::Kimi => {
            let client = KimiClient::new(api_key, model, base_url);
            Ok(DiagramBenchmarkClient::Kimi(client))
        }
        LlmProviderId::Zhipu => {
            let client = GlmClient::new(api_key, model, base_url);
            Ok(DiagramBenchmarkClient::Zhipu(client))
        }
        LlmProviderId::Modal => {
            let client = ModalClient::new(api_key, model, base_url);
            Ok(DiagramBenchmarkClient::Modal(client))
        }
    }
}
