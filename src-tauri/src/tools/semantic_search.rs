use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::core::tool::ToolDescriptor;
use crate::embeddings::SemanticIndexService;
use crate::policy::{PolicyDecision, PolicyEngine};
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

static SEMANTIC_INDEX_SERVICE: std::sync::OnceLock<Arc<SemanticIndexService>> =
    std::sync::OnceLock::new();

pub fn set_semantic_index_service(service: Arc<SemanticIndexService>) {
    let _ = SEMANTIC_INDEX_SERVICE.set(service);
}

fn semantic_index_service() -> Result<&'static Arc<SemanticIndexService>, ToolError> {
    SEMANTIC_INDEX_SERVICE.get().ok_or_else(|| {
        ToolError::Execution("semantic index service is not initialized".to_string())
    })
}

pub struct SearchEmbeddingsTool;

impl Tool for SearchEmbeddingsTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "search.embeddings".into(),
            description: concat!(
                "Semantic code search using workspace embeddings. ",
                "Returns ranked code/document chunks by semantic similarity. ",
                "If embeddings are not built yet, it starts background indexing and returns indexing status."
            )
            .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Semantic query text"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of result chunks to return (default: 8, max: 50)"
                    }
                },
                "required": ["query"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let query = input
            .get("query")
            .and_then(|value| value.as_str())
            .ok_or_else(|| ToolError::InvalidInput("query required".to_string()))?
            .trim()
            .to_string();
        if query.is_empty() {
            return Err(ToolError::InvalidInput(
                "query must not be empty".to_string(),
            ));
        }

        let limit = input
            .get("limit")
            .and_then(|value| value.as_u64())
            .unwrap_or(8)
            .clamp(1, 50) as usize;

        let workspace_root = resolve_workspace_root(cwd);
        match policy.evaluate_path(&workspace_root) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }

        let service = Arc::clone(semantic_index_service()?);
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|error| ToolError::Execution(format!("no async runtime: {error}")))?;

        let response = runtime
            .block_on(async move { service.semantic_search(workspace_root, query, limit).await })
            .map_err(|error| ToolError::Execution(error.to_string()))?;

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::to_value(response).unwrap_or_else(|_| serde_json::json!({})),
            error: None,
        })
    }
}

fn resolve_workspace_root(cwd: &Path) -> PathBuf {
    cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf())
}
