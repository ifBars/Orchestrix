//! Event type and category constants and flush policy.
//!
//! Single source of truth for which events are "immediate" (flushed to the
//! frontend without batching) vs batched.

use super::event_bus::BusEvent;

// ---------------------------------------------------------------------------
// Categories
// ---------------------------------------------------------------------------

pub const CATEGORY_TASK: &str = "task";
pub const CATEGORY_AGENT: &str = "agent";
#[allow(dead_code)]
pub const CATEGORY_TOOL: &str = "tool";
#[allow(dead_code)]
pub const CATEGORY_USER: &str = "user";
#[allow(dead_code)]
pub const CATEGORY_ARTIFACT: &str = "artifact";
#[allow(dead_code)]
pub const CATEGORY_LOG: &str = "log";

// ---------------------------------------------------------------------------
// Event types (UX and high-priority)
// ---------------------------------------------------------------------------

pub const EVENT_AGENT_DECIDING: &str = "agent.deciding";
pub const EVENT_AGENT_TOOL_CALLS_PREPARING: &str = "agent.tool_calls_preparing";
pub const EVENT_AGENT_MESSAGE_STREAM_STARTED: &str = "agent.message_stream_started";
pub const EVENT_AGENT_MESSAGE_DELTA: &str = "agent.message_delta";
pub const EVENT_AGENT_MESSAGE_STREAM_COMPLETED: &str = "agent.message_stream_completed";
pub const EVENT_AGENT_MESSAGE_STREAM_CANCELLED: &str = "agent.message_stream_cancelled";

// ---------------------------------------------------------------------------
// Flush policy
// ---------------------------------------------------------------------------

/// Returns true if this event should be sent to the frontend immediately
/// instead of being buffered. Immediate events preserve ordering with the
/// current buffer before being sent.
pub fn should_flush_immediately(event: &BusEvent) -> bool {
    if event.category == CATEGORY_TASK {
        return true;
    }
    if event.event_type.starts_with("agent.step_") {
        return true;
    }
    if event.event_type == EVENT_AGENT_DECIDING {
        return true;
    }
    if event.event_type == EVENT_AGENT_TOOL_CALLS_PREPARING {
        return true;
    }
    false
}
