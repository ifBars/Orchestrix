use crate::bus::EventBus;
use crate::core::tool::ToolDescriptor;
use crate::db::{queries, Database};
use crate::policy::PolicyEngine;
use crate::tools::types::{Tool, ToolCallOutput, ToolError};
use chrono::Utc;
use std::path::Path;

/// Helper to execute diagram.read_graph
pub fn handle_read_graph(db: &Database, task_id: &str) -> Result<ToolCallOutput, ToolError> {
    let row = queries::get_task_canvas(db, task_id)
        .map_err(|e| ToolError::Execution(format!("failed to read canvas state: {e}")))?;

    let state = if let Some(r) = row {
        serde_json::from_str(&r.state_json)
            .unwrap_or_else(|_| serde_json::json!({ "nodes": [], "edges": [] }))
    } else {
        serde_json::json!({ "nodes": [], "edges": [] })
    };

    Ok(ToolCallOutput {
        ok: true,
        data: state,
        error: None,
    })
}

/// Helper to execute diagram.mutate_graph
pub fn handle_mutate_graph(
    db: &Database,
    bus: &EventBus,
    task_id: &str,
    args: &serde_json::Value,
) -> Result<ToolCallOutput, ToolError> {
    let operations = args
        .get("operations")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ToolError::InvalidInput("operations array is required".to_string()))?;

    // Read current state
    let mut state = handle_read_graph(db, task_id)?.data;
    let mut nodes = state
        .get("nodes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut edges = state
        .get("edges")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for op in operations {
        let op_type = op.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let id = op
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }

        match op_type {
            "add_node" => {
                let kind = op
                    .get("node_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("concept");
                let label = op.get("label").and_then(|v| v.as_str()).unwrap_or("");
                let description = op.get("description").and_then(|v| v.as_str()).unwrap_or("");

                // Auto-generate label from id if missing (kebab-case -> Title Case)
                let final_label = if label.is_empty() {
                    id.split('-')
                        .map(|word| {
                            let mut chars = word.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(first) => {
                                    first.to_uppercase().collect::<String>() + chars.as_str()
                                }
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    label.to_string()
                };

                // Store in the flat CanvasNode format that the frontend expects:
                // { id, label, kind, description }  (no nested "data" wrapper)
                let new_node = serde_json::json!({
                    "id": id,
                    "kind": kind,
                    "label": final_label,
                    "description": description
                });
                // Remove if exists then add
                nodes.retain(|n| n.get("id").and_then(|v| v.as_str()) != Some(&id));
                nodes.push(new_node);
            }
            "update_node" => {
                let label = op.get("label").and_then(|v| v.as_str());
                let description = op.get("description").and_then(|v| v.as_str());
                let kind = op.get("node_type").and_then(|v| v.as_str());

                for node in nodes.iter_mut() {
                    if node.get("id").and_then(|v| v.as_str()) == Some(&id) {
                        if let Some(obj) = node.as_object_mut() {
                            if let Some(l) = label {
                                obj.insert(
                                    "label".to_string(),
                                    serde_json::Value::String(l.to_string()),
                                );
                            }
                            if let Some(d) = description {
                                obj.insert(
                                    "description".to_string(),
                                    serde_json::Value::String(d.to_string()),
                                );
                            }
                            if let Some(k) = kind {
                                obj.insert(
                                    "kind".to_string(),
                                    serde_json::Value::String(k.to_string()),
                                );
                            }
                        }
                        break;
                    }
                }
            }
            "remove_node" => {
                nodes.retain(|n| n.get("id").and_then(|v| v.as_str()) != Some(&id));
                edges.retain(|e| {
                    e.get("source").and_then(|v| v.as_str()) != Some(&id)
                        && e.get("target").and_then(|v| v.as_str()) != Some(&id)
                });
            }
            "add_edge" => {
                let source = op.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let target = op.get("target").and_then(|v| v.as_str()).unwrap_or("");
                let label = op.get("label").and_then(|v| v.as_str());
                if !source.is_empty() && !target.is_empty() {
                    edges.retain(|e| e.get("id").and_then(|v| v.as_str()) != Some(&id));
                    let mut edge = serde_json::json!({
                        "id": id,
                        "source": source,
                        "target": target,
                        "animated": true
                    });
                    if let Some(l) = label {
                        edge.as_object_mut().unwrap().insert(
                            "label".to_string(),
                            serde_json::Value::String(l.to_string()),
                        );
                    }
                    edges.push(edge);
                }
            }
            "remove_edge" => {
                edges.retain(|e| e.get("id").and_then(|v| v.as_str()) != Some(&id));
            }
            _ => {}
        }
    }

    state
        .as_object_mut()
        .unwrap()
        .insert("nodes".to_string(), serde_json::Value::Array(nodes));
    state
        .as_object_mut()
        .unwrap()
        .insert("edges".to_string(), serde_json::Value::Array(edges));

    let state_json = serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string());
    let now = Utc::now().to_rfc3339();

    queries::upsert_task_canvas(db, task_id, &state_json, &now)
        .map_err(|e| ToolError::Execution(format!("failed to save canvas state: {e}")))?;

    let _ = crate::runtime::planner::emit_and_record(
        db,
        bus,
        "canvas",
        "canvas.updated",
        None,
        serde_json::json!({
            "task_id": task_id,
            "state_json": state_json,
        }),
    );

    Ok(ToolCallOutput {
        ok: true,
        data: serde_json::json!({
            "success": true,
            "message": "Canvas mutations applied successfully."
        }),
        error: None,
    })
}

/// Tool for Plan-Mode agents to read the current graph state.
pub struct CanvasReadStateTool;

impl Tool for CanvasReadStateTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "diagram.read_graph".into(),
            description: "Read the current architecture diagram graph (nodes and edges) from the shared canvas. Use this to ingest the human's architectural context before planning.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        Err(ToolError::Execution(
            "diagram.read_graph must be intercepted by the orchestrator".into(),
        ))
    }
}

/// Tool for Plan-Mode agents to mutate the graph state.
pub struct CanvasMutateTool;

impl Tool for CanvasMutateTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "diagram.mutate_graph".into(),
            description: r#"Mutate the architecture canvas by adding/updating/removing nodes and edges.

Nodes represent components (services, databases, UI modules, etc.). Edges represent relationships between them (calls, depends-on, owns, etc.).

IMPORTANT: Always connect related nodes with edges. A diagram with disconnected nodes is architecturally meaningless. Every component should have at least one relationship to another component unless it is truly isolated.

Operations:
- add_node: Create a component (id, node_type, label, description)
- add_edge: Connect two components (id, source, target). Use this to show: service A calls service B, component X depends on database Y, etc.
- update_node: Modify existing component metadata
- remove_node: Delete a component (also removes connected edges)
- remove_edge: Delete a relationship

When adding multiple related components, always include add_edge operations in the same batch to establish their relationships immediately."#.into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": {
                                    "type": "string",
                                    "enum": ["add_node", "update_node", "remove_node", "add_edge", "remove_edge"]
                                },
                                "id": {"type": "string", "description": "Unique identifier for the node or edge"},
                                "node_type": {"type": "string", "description": "For add_node/update_node: Type of component, e.g., 'service', 'database', 'ui', 'api', 'concept'"},
                                "label": {"type": "string", "description": "For add_node/update_node: Display name shown on the node (REQUIRED for nodes). For add_edge: Optional relationship label (e.g., 'HTTP', 'depends on')"},
                                "description": {"type": "string", "description": "For add_node/update_node: Detailed explanation of the component"},
                                "source": {"type": "string", "description": "For add_edge: ID of the source node"},
                                "target": {"type": "string", "description": "For add_edge: ID of the target node"}
                            },
                            "required": ["type", "id"],
                            "allOf": [
                                {
                                    "if": {"properties": {"type": {"const": "add_node"}}},
                                    "then": {"required": ["label"]}
                                },
                                {
                                    "if": {"properties": {"type": {"const": "add_edge"}}},
                                    "then": {"required": ["source", "target"]}
                                }
                            ]
                        },
                        "description": "Batch of mutations to apply to the canvas"
                    }
                },
                "required": ["operations"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        Err(ToolError::Execution(
            "diagram.mutate_graph must be intercepted by the orchestrator".into(),
        ))
    }
}
