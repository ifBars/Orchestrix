//! Event system for real-time frontend-backend communication.
//!
//! The event bus provides:
//! - Publish-subscribe pattern for real-time updates
//! - Event batching to reduce frontend re-renders
//! - Persistent event storage for recovery
//!
//! # Architecture
//!
//! Events flow from backend → EventBus → EventBatcher → Frontend:
//! - `EventBus`: In-memory broadcast channel for immediate distribution
//! - `EventBatcher`: Buffers events (100ms/50 events) before sending to UI
//! - Events are also persisted to SQLite for crash recovery

mod event_bus;
mod batcher;

pub use event_bus::{BusEvent, EventBus};
pub use batcher::EventBatcher;
