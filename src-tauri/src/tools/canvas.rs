use crate::bus::EventBus;
use crate::core::tool::ToolDescriptor;
use crate::db::{queries, Database};
use crate::policy::PolicyEngine;
use crate::tools::types::{Tool, ToolCallOutput, ToolError};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramState {
    pub nodes: Vec<DiagramNode>,
    pub edges: Vec<DiagramEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub description: String,
    pub position: Option<NodePosition>,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: String,
    pub edge_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramRevision {
    pub version: u64,
    pub updated_at: String,
    pub operations: Vec<DiagramOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum DiagramOp {
    #[serde(rename = "addNode")]
    AddNode {
        id: String,
        label: String,
        node_type: String,
        description: String,
        position: Option<NodePosition>,
        data: serde_json::Value,
    },
    #[serde(rename = "updateNode")]
    UpdateNode {
        id: String,
        label: Option<String>,
        description: Option<String>,
        node_type: Option<String>,
        position: Option<NodePosition>,
        data: Option<serde_json::Value>,
    },
    #[serde(rename = "removeNode")]
    RemoveNode { id: String },
    #[serde(rename = "addEdge")]
    AddEdge {
        id: String,
        source: String,
        target: String,
        label: String,
        edge_type: String,
    },
    #[serde(rename = "updateEdge")]
    UpdateEdge {
        id: String,
        label: Option<String>,
        edge_type: Option<String>,
    },
    #[serde(rename = "removeEdge")]
    RemoveEdge { id: String },
    #[serde(rename = "setViewport")]
    SetViewport { x: f64, y: f64, zoom: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramOpBatch {
    pub base_version: u64,
    pub author: String,
    pub operations: Vec<DiagramOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpResult {
    pub success: bool,
    pub new_version: u64,
    pub applied_ops: Vec<String>,
    pub conflicts: Vec<OpConflict>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpConflict {
    pub op_id: String,
    pub reason: String,
    pub can_rebase: bool,
}

impl Default for DiagramState {
    fn default() -> Self {
        Self {
            nodes: vec![],
            edges: vec![],
        }
    }
}

impl DiagramState {
    pub fn from_json(json: &serde_json::Value) -> Self {
        let nodes: Vec<DiagramNode> = json
            .get("nodes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|n| {
                        Some(DiagramNode {
                            id: n.get("id")?.as_str()?.to_string(),
                            label: n
                                .get("label")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            node_type: n
                                .get("kind")
                                .and_then(|v| v.as_str())
                                .unwrap_or("concept")
                                .to_string(),
                            description: n
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            position: n.get("position").and_then(|p| {
                                Some(NodePosition {
                                    x: p.get("x")?.as_f64()?,
                                    y: p.get("y")?.as_f64()?,
                                })
                            }),
                            data: n.get("data").cloned().unwrap_or(serde_json::json!({})),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let edges: Vec<DiagramEdge> = json
            .get("edges")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        Some(DiagramEdge {
                            id: e.get("id")?.as_str()?.to_string(),
                            source: e.get("source")?.as_str()?.to_string(),
                            target: e.get("target")?.as_str()?.to_string(),
                            label: e
                                .get("label")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            edge_type: e
                                .get("edge_type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("default")
                                .to_string(),
                            data: e.get("data").cloned().unwrap_or(serde_json::json!({})),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self { nodes, edges }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "nodes": self.nodes.iter().map(|n| serde_json::json!({
                "id": n.id,
                "label": n.label,
                "kind": n.node_type,
                "description": n.description,
                "position": n.position,
                "data": n.data,
            })).collect::<Vec<_>>(),
            "edges": self.edges.iter().map(|e| serde_json::json!({
                "id": e.id,
                "source": e.source,
                "target": e.target,
                "label": e.label,
                "edge_type": e.edge_type,
                "data": e.data,
            })).collect::<Vec<_>>(),
        })
    }

    pub fn validate_op(&self, op: &DiagramOp) -> Result<(), String> {
        match op {
            DiagramOp::AddNode { id, label, .. } => {
                if id.is_empty() {
                    return Err("Node ID cannot be empty".to_string());
                }
                if label.is_empty() {
                    return Err("Node label cannot be empty".to_string());
                }
                if self.nodes.iter().any(|n| n.id == *id) {
                    return Err(format!("Node with ID '{}' already exists", id));
                }
                Ok(())
            }
            DiagramOp::UpdateNode { id, data, .. } => {
                if id.is_empty() {
                    return Err("Node ID cannot be empty".to_string());
                }
                if !self.nodes.iter().any(|n| n.id == *id) {
                    return Err(format!("Node with ID '{}' not found", id));
                }
                if let Some(d) = data {
                    if let Some(obj) = d.as_object() {
                        for key in obj.keys() {
                            if ["id", "source", "target"].contains(&key.as_str()) {
                                return Err(format!("Cannot update reserved field '{}'", key));
                            }
                        }
                    }
                }
                Ok(())
            }
            DiagramOp::RemoveNode { id } => {
                if id.is_empty() {
                    return Err("Node ID cannot be empty".to_string());
                }
                if !self.nodes.iter().any(|n| n.id == *id) {
                    return Err(format!("Node with ID '{}' not found", id));
                }
                Ok(())
            }
            DiagramOp::AddEdge {
                id, source, target, ..
            } => {
                if id.is_empty() {
                    return Err("Edge ID cannot be empty".to_string());
                }
                if source.is_empty() || target.is_empty() {
                    return Err("Edge source and target cannot be empty".to_string());
                }
                if !self.nodes.iter().any(|n| n.id == *source) {
                    return Err(format!("Source node '{}' not found", source));
                }
                if !self.nodes.iter().any(|n| n.id == *target) {
                    return Err(format!("Target node '{}' not found", target));
                }
                if self.edges.iter().any(|e| e.id == *id) {
                    return Err(format!("Edge with ID '{}' already exists", id));
                }
                Ok(())
            }
            DiagramOp::UpdateEdge { id, .. } => {
                if id.is_empty() {
                    return Err("Edge ID cannot be empty".to_string());
                }
                if !self.edges.iter().any(|e| e.id == *id) {
                    return Err(format!("Edge with ID '{}' not found", id));
                }
                Ok(())
            }
            DiagramOp::RemoveEdge { id } => {
                if id.is_empty() {
                    return Err("Edge ID cannot be empty".to_string());
                }
                if !self.edges.iter().any(|e| e.id == *id) {
                    return Err(format!("Edge with ID '{}' not found", id));
                }
                Ok(())
            }
            DiagramOp::SetViewport { .. } => Ok(()),
        }
    }

    pub fn apply_op(&mut self, op: &DiagramOp) -> Result<(), String> {
        self.validate_op(op)?;

        match op {
            DiagramOp::AddNode {
                id,
                label,
                node_type,
                description,
                position,
                data,
            } => {
                self.nodes.push(DiagramNode {
                    id: id.clone(),
                    label: label.clone(),
                    node_type: node_type.clone(),
                    description: description.clone(),
                    position: position.clone(),
                    data: data.clone(),
                });
            }
            DiagramOp::UpdateNode {
                id,
                label,
                description,
                node_type,
                position,
                data,
            } => {
                if let Some(node) = self.nodes.iter_mut().find(|n| n.id == *id) {
                    if let Some(l) = label {
                        node.label = l.clone();
                    }
                    if let Some(d) = description {
                        node.description = d.clone();
                    }
                    if let Some(t) = node_type {
                        node.node_type = t.clone();
                    }
                    if let Some(p) = position {
                        node.position = Some(p.clone());
                    }
                    if let Some(d) = data {
                        node.data = d.clone();
                    }
                }
            }
            DiagramOp::RemoveNode { id } => {
                self.nodes.retain(|n| n.id != *id);
                self.edges.retain(|e| e.source != *id && e.target != *id);
            }
            DiagramOp::AddEdge {
                id,
                source,
                target,
                label,
                edge_type,
            } => {
                self.edges.push(DiagramEdge {
                    id: id.clone(),
                    source: source.clone(),
                    target: target.clone(),
                    label: label.clone(),
                    edge_type: edge_type.clone(),
                    data: serde_json::json!({}),
                });
            }
            DiagramOp::UpdateEdge {
                id,
                label,
                edge_type,
            } => {
                if let Some(edge) = self.edges.iter_mut().find(|e| e.id == *id) {
                    if let Some(l) = label {
                        edge.label = l.clone();
                    }
                    if let Some(t) = edge_type {
                        edge.edge_type = t.clone();
                    }
                }
            }
            DiagramOp::RemoveEdge { id } => {
                self.edges.retain(|e| e.id != *id);
            }
            DiagramOp::SetViewport { .. } => {}
        }

        Ok(())
    }
}

pub fn handle_read_graph(db: &Database, task_id: &str) -> Result<ToolCallOutput, ToolError> {
    let row = queries::get_task_canvas(db, task_id)
        .map_err(|e| ToolError::Execution(format!("failed to read canvas state: {e}")))?;

    let (state, version) = if let Some(r) = row {
        let state = DiagramState::from_json(
            &serde_json::from_str(&r.state_json)
                .unwrap_or_else(|_| serde_json::json!({ "nodes": [], "edges": [] })),
        );
        (state, r.version)
    } else {
        (DiagramState::default(), 0)
    };

    Ok(ToolCallOutput {
        ok: true,
        data: serde_json::json!({
            "state": state.to_json(),
            "version": version,
        }),
        error: None,
    })
}

pub fn handle_apply_ops(
    db: &Database,
    task_id: &str,
    batch: DiagramOpBatch,
) -> Result<ToolCallOutput, ToolError> {
    let row = queries::get_task_canvas(db, task_id)
        .map_err(|e| ToolError::Execution(format!("failed to read canvas state: {e}")))?;

    let current_version = row.as_ref().map(|r| r.version).unwrap_or(0);
    let mut state = if let Some(r) = row {
        DiagramState::from_json(
            &serde_json::from_str(&r.state_json)
                .unwrap_or_else(|_| serde_json::json!({ "nodes": [], "edges": [] })),
        )
    } else {
        DiagramState::default()
    };

    let mut applied_ops = Vec::new();
    let mut conflicts = Vec::new();
    let mut errors = Vec::new();

    for op in &batch.operations {
        match state.validate_op(op) {
            Ok(()) => {
                if let Err(e) = state.apply_op(op) {
                    errors.push(format!("{:?}: {}", op, e));
                } else {
                    applied_ops.push(format!("{:?}", op));
                }
            }
            Err(e) => {
                let can_rebase = match op {
                    DiagramOp::UpdateNode { id, .. } => state.nodes.iter().any(|n| n.id == *id),
                    DiagramOp::RemoveNode { id } => !state.nodes.iter().any(|n| n.id == *id),
                    DiagramOp::UpdateEdge { id, .. } => state.edges.iter().any(|e| e.id == *id),
                    DiagramOp::RemoveEdge { id } => !state.edges.iter().any(|e| e.id == *id),
                    _ => false,
                };
                conflicts.push(OpConflict {
                    op_id: format!("{:?}", op),
                    reason: e,
                    can_rebase,
                });
            }
        }
    }

    let new_version = current_version + applied_ops.len() as u64;
    let success = conflicts.is_empty() && errors.is_empty();

    let result = OpResult {
        success,
        new_version,
        applied_ops,
        conflicts,
        errors,
    };

    if success || !result.applied_ops.is_empty() {
        let state_json = serde_json::to_string(&state.to_json())
            .map_err(|e| ToolError::Execution(format!("failed to serialize state: {e}")))?;
        let now = Utc::now().to_rfc3339();

        queries::upsert_task_canvas(db, task_id, &state_json, &now)
            .map_err(|e| ToolError::Execution(format!("failed to save canvas state: {e}")))?;
    }

    Ok(ToolCallOutput {
        ok: success,
        data: serde_json::to_value(result).unwrap_or(serde_json::json!({})),
        error: if !success {
            Some("Operation had conflicts or errors".to_string())
        } else {
            None
        },
    })
}

pub struct CanvasReadStateTool;

impl Tool for CanvasReadStateTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "diagram.read_graph".into(),
            description: r#"Read the current architecture diagram state.

Returns the current diagram with:
- nodes: Array of components (id, label, kind, description, position, data)
- edges: Array of relationships (id, source, target, label, edge_type)
- version: Current revision number for optimistic locking

Always read the graph before making changes to ensure you have the latest state."#
                .into(),
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

pub struct CanvasApplyOpsTool;

impl Tool for CanvasApplyOpsTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "diagram.apply_ops".into(),
            description: r#"Apply operations to the architecture diagram.

Operations are validated and applied atomically. Each operation includes:
- op: Operation type
- id: Unique identifier
- Required fields based on operation type

Operations:
- addNode: Add a component {id, label, node_type, description, position?, data?}
- updateNode: Modify component {id, label?, description?, node_type?, position?, data?}
- removeNode: Delete component {id} (also removes connected edges)
- addEdge: Add relationship {id, source, target, label?, edge_type?}
- updateEdge: Modify relationship {id, label?, edge_type?}
- removeEdge: Delete relationship {id}
- setViewport: Set view {x, y, zoom}

Batch multiple operations together for atomic application.
Include base_version to enable conflict detection.

Example:
{
  "base_version": 3,
  "author": "ai",
  "operations": [
    {"op": "addNode", "id": "api", "label": "API Server", "node_type": "service"},
    {"op": "addNode", "id": "db", "label": "Database", "node_type": "database"},
    {"op": "addEdge", "id": "api-db", "source": "api", "target": "db", "label": "calls"}
  ]
}"#
            .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "base_version": {
                        "type": "integer",
                        "description": "Version number from last read_graph to enable conflict detection"
                    },
                    "author": {
                        "type": "string",
                        "enum": ["ai", "human"],
                        "default": "ai"
                    },
                    "operations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "oneOf": [
                                {
                                    "properties": {
                                        "op": {"const": "addNode"},
                                        "id": {"type": "string"},
                                        "label": {"type": "string"},
                                        "node_type": {"type": "string"},
                                        "description": {"type": "string"},
                                        "position": {"type": "object", "properties": {"x": {"type": "number"}, "y": {"type": "number"}}},
                                        "data": {"type": "object"}
                                    },
                                    "required": ["op", "id", "label", "node_type"]
                                },
                                {
                                    "properties": {
                                        "op": {"const": "updateNode"},
                                        "id": {"type": "string"},
                                        "label": {"type": "string"},
                                        "description": {"type": "string"},
                                        "node_type": {"type": "string"},
                                        "position": {"type": "object"},
                                        "data": {"type": "object"}
                                    },
                                    "required": ["op", "id"]
                                },
                                {
                                    "properties": {
                                        "op": {"const": "removeNode"},
                                        "id": {"type": "string"}
                                    },
                                    "required": ["op", "id"]
                                },
                                {
                                    "properties": {
                                        "op": {"const": "addEdge"},
                                        "id": {"type": "string"},
                                        "source": {"type": "string"},
                                        "target": {"type": "string"},
                                        "label": {"type": "string"},
                                        "edge_type": {"type": "string"}
                                    },
                                    "required": ["op", "id", "source", "target"]
                                },
                                {
                                    "properties": {
                                        "op": {"const": "updateEdge"},
                                        "id": {"type": "string"},
                                        "label": {"type": "string"},
                                        "edge_type": {"type": "string"}
                                    },
                                    "required": ["op", "id"]
                                },
                                {
                                    "properties": {
                                        "op": {"const": "removeEdge"},
                                        "id": {"type": "string"}
                                    },
                                    "required": ["op", "id"]
                                },
                                {
                                    "properties": {
                                        "op": {"const": "setViewport"},
                                        "x": {"type": "number"},
                                        "y": {"type": "number"},
                                        "zoom": {"type": "number"}
                                    },
                                    "required": ["op", "x", "y", "zoom"]
                                }
                            ]
                        }
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
            "diagram.apply_ops must be intercepted by the orchestrator".into(),
        ))
    }
}
