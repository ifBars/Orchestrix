//! Tool registry and implementations for AI agent operations.
//!
//! This module provides:
//! - Tool registry for dynamic tool discovery
//! - Built-in tools: filesystem, commands, search, git
//! - Skill-based tools loaded from `.agents/skills/`
//! - Policy enforcement for all tool invocations
//!
//! # Tool Lifecycle
//!
//! 1. Tool is invoked by the orchestrator
//! 2. Policy engine evaluates permission
//! 3. Tool executes with sandboxing
//! 4. Result is recorded and returned
//!
//! # Module Structure
//!
//! - `types`: Core types (Tool trait, ToolCallInput/Output, ToolError)
//! - `registry`: ToolRegistry for managing and invoking tools
//! - `fs`: Filesystem tools (read, write, list)
//! - `search`: Search tools (ripgrep)
//! - `cmd`: Command execution tools
//! - `git`: Git repository tools
//! - `agent`: Agent management tools (todo, mode switching)
//! - `skills`: Skills management tools
//!
//! # Adding New Tools
//!
//! 1. Implement the tool logic in the appropriate submodule
//! 2. Register in the ToolRegistry (in `registry.rs`)
//! 3. Add to policy allowlist if needed
//! 4. Update tool descriptors for LLM

// Public exports
pub use registry::ToolRegistry;
#[allow(unused_imports)]
pub use types::{ToolCallInput, ToolCallOutput, ToolError};

// Submodules
mod agent;
mod cmd;
mod fs;
mod git;
mod registry;
mod search;
mod skills;
mod types;

/// Infer a tool call from step title and optional tool intent.
/// Used as a fallback when the model doesn't explicitly return a tool call.
pub fn infer_tool_call(title: &str, tool_intent: Option<&str>) -> Option<ToolCallInput> {
    // If there's explicit tool intent, try to parse it
    if let Some(intent) = tool_intent {
        // Try to extract tool name and arguments from intent
        let intent_lower = intent.to_lowercase();

        // Pattern: "tool_name: args" or "tool_name(args)"
        if let Some(colon_idx) = intent.find(':') {
            let tool_name = &intent[..colon_idx].trim();
            let args_str = &intent[colon_idx + 1..].trim();

            // Try to parse args as JSON
            if let Ok(args) = serde_json::from_str(args_str) {
                return Some(ToolCallInput {
                    name: tool_name.to_string(),
                    args,
                });
            }

            // If not valid JSON, treat as string content for common tools
            let args = match *tool_name {
                "fs.read" | "fs.write" | "fs.list" => {
                    serde_json::json!({ "path": args_str })
                }
                "search.rg" => {
                    serde_json::json!({ "pattern": args_str })
                }
                "cmd.exec" => {
                    serde_json::json!({ "command": args_str })
                }
                _ => serde_json::json!({"content": args_str}),
            };

            return Some(ToolCallInput {
                name: tool_name.to_string(),
                args,
            });
        }

        // Check for common patterns in the title/intent
        if intent_lower.contains("read ") || intent_lower.contains("read file") {
            // Try to extract filename
            let words: Vec<&str> = title.split_whitespace().collect();
            if let Some(filename) = words.last() {
                if !filename.starts_with("read") {
                    return Some(ToolCallInput {
                        name: "fs.read".to_string(),
                        args: serde_json::json!({ "path": filename }),
                    });
                }
            }
        }

        if intent_lower.contains("write ") || intent_lower.contains("create ") {
            // Try to extract filename
            let words: Vec<&str> = title.split_whitespace().collect();
            if let Some(filename) = words.last() {
                return Some(ToolCallInput {
                    name: "fs.write".to_string(),
                    args: serde_json::json!({ "path": filename, "content": "" }),
                });
            }
        }

        if intent_lower.contains("search") || intent_lower.contains("find") {
            // Try to extract search pattern
            let words: Vec<&str> = title.split_whitespace().collect();
            if words.len() >= 2 {
                let pattern = words[words.len() - 2..].join(" ");
                return Some(ToolCallInput {
                    name: "search.rg".to_string(),
                    args: serde_json::json!({ "pattern": pattern }),
                });
            }
        }
    }

    // Try to infer from title alone
    let title_lower = title.to_lowercase();

    if title_lower.contains("read ") {
        let words: Vec<&str> = title.split_whitespace().collect();
        if let Some(filename) = words.last() {
            return Some(ToolCallInput {
                name: "fs.read".to_string(),
                args: serde_json::json!({ "path": filename }),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests;
