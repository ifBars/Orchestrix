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

pub mod catalog;
pub mod provider;
pub mod traits;
pub mod types;

// Provider implementations
pub mod providers;

// Re-export commonly used types
pub use catalog::ModelCatalog;
pub use provider::ProviderId;
pub use shared::strip_tool_call_markup;
pub use traits::AgentModelClient;
pub use types::{
    ModelError, StreamDelta, WorkerAction, WorkerActionRequest, WorkerDecision, WorkerToolCall,
};

// Re-export provider clients for convenience
pub use providers::glm::GlmClient;
pub use providers::kimi::KimiClient;
pub use providers::minimax::MiniMaxClient;
pub use providers::modal::ModalClient;
