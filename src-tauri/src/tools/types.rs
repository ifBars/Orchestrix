//! Shared types and traits for tool system.
//!
//! This module defines the core abstractions for tools:
//! - Tool input/output types
//! - Tool trait for implementing new tools
//! - Error types for tool execution

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::tool::ToolDescriptor;
use crate::policy::PolicyEngine;

/// Input to a tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInput {
    pub name: String,
    pub args: serde_json::Value,
}

/// Output from a tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallOutput {
    pub ok: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
}

/// Errors that can occur during tool execution.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("policy denied: {0}")]
    PolicyDenied(String),
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("approval required for scope '{scope}': {reason}")]
    ApprovalRequired { scope: String, reason: String },
}

/// Trait for implementing tools.
///
/// Tools are invoked by the orchestrator and must be Send + Sync for use
/// across async boundaries.
pub trait Tool: Send + Sync {
    /// Returns the descriptor for this tool, including name, description,
    /// and JSON schema for inputs.
    fn descriptor(&self) -> ToolDescriptor;

    /// Invokes the tool with the given policy, working directory, and input.
    ///
    /// # Arguments
    /// * `policy` - The policy engine for permission checks
    /// * `cwd` - The current working directory for relative paths
    /// * `input` - The tool arguments as JSON
    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError>;
}
