//! Model clients for AI providers.
//!
//! This module provides a unified interface for different AI model providers.
//!
//! ## Structure
//!
//! - `types`: Core types (WorkerAction, WorkerDecision, etc.)
//! - `traits`: Client trait definitions (AgentModelClient)
//! - `provider`: Provider ID enum and parsing
//! - `catalog`: Model metadata and defaults
//! - `factory`: Client construction
//! - `prompts`: System prompt builders
//! - `sanitize`: Output text utilities
//! - `providers/`: Provider-specific implementations
//! - `adapters/`: API format adapters

// Core types and traits
mod shared;

pub mod types;
pub mod traits;
pub mod provider;
pub mod catalog;

// Provider implementations
pub mod providers;

// Re-export commonly used types
pub use types::{
    ModelError, StreamDelta, WorkerAction, WorkerActionRequest, WorkerDecision, WorkerToolCall,
};
pub use traits::AgentModelClient;
pub use provider::ProviderId;
pub use catalog::ModelCatalog;
pub use shared::strip_tool_call_markup;

// Re-export provider clients for convenience
pub use providers::minimax::MiniMaxClient;
pub use providers::kimi::KimiClient;
