use std::sync::atomic::{AtomicI64, Ordering};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

const BUS_CAPACITY: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusEvent {
    pub id: String,
    pub run_id: Option<String>,
    pub seq: i64,
    pub category: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

pub struct EventBus {
    tx: broadcast::Sender<BusEvent>,
    seq: AtomicI64,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BUS_CAPACITY);
        Self {
            tx,
            seq: AtomicI64::new(0),
        }
    }

    /// Publish a pre-built event onto the bus.
    pub fn publish(&self, event: BusEvent) {
        if let Err(e) = self.tx.send(event) {
            tracing::warn!("event bus publish failed (no receivers?): {e}");
        }
    }

    /// Convenience: build and publish an event in one call.
    pub fn emit(
        &self,
        category: impl Into<String>,
        event_type: impl Into<String>,
        run_id: Option<String>,
        payload: serde_json::Value,
    ) -> BusEvent {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let event = BusEvent {
            id: Uuid::new_v4().to_string(),
            run_id,
            seq,
            category: category.into(),
            event_type: event_type.into(),
            payload,
            created_at: Utc::now().to_rfc3339(),
        };
        self.publish(event.clone());
        event
    }

    /// Get a new receiver for this bus.
    pub fn subscribe(&self) -> broadcast::Receiver<BusEvent> {
        self.tx.subscribe()
    }
}
