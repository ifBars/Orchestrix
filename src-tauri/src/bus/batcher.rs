use std::time::Duration;

use tauri::Emitter;
use tokio::sync::broadcast;
use tokio::time;

use super::BusEvent;

const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_MAX_BATCH: usize = 50;

pub struct EventBatcher;

impl EventBatcher {
    /// Spawn a background task that batches events and emits them to the Tauri
    /// frontend via `orchestrix://events`.
    ///
    /// - "Immediate" events (category starts with `task` or event_type starts
    ///   with `agent.step_`) are flushed instantly as a single-element batch.
    /// - All other events are buffered and flushed every 100ms or when the
    ///   buffer reaches 50 events.
    pub fn start(
        mut rx: broadcast::Receiver<BusEvent>,
        app_handle: tauri::AppHandle,
    ) -> tauri::async_runtime::JoinHandle<()> {
        tauri::async_runtime::spawn(async move {
            let mut buffer: Vec<BusEvent> = Vec::with_capacity(DEFAULT_MAX_BATCH);
            let mut interval = time::interval(DEFAULT_FLUSH_INTERVAL);

            loop {
                tokio::select! {
                    result = rx.recv() => {
                        match result {
                            Ok(event) => {
                                if is_immediate(&event) {
                                    // Flush buffer first so ordering is preserved
                                    if !buffer.is_empty() {
                                        flush(&app_handle, &mut buffer);
                                    }
                                    let _ = app_handle.emit("orchestrix://events", vec![&event]);
                                } else {
                                    buffer.push(event);
                                    if buffer.len() >= DEFAULT_MAX_BATCH {
                                        flush(&app_handle, &mut buffer);
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                tracing::warn!("event batcher lagged, dropped {n} events");
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                // Bus shut down — flush remainder and exit.
                                if !buffer.is_empty() {
                                    flush(&app_handle, &mut buffer);
                                }
                                break;
                            }
                        }
                    }
                    _ = interval.tick() => {
                        if !buffer.is_empty() {
                            flush(&app_handle, &mut buffer);
                        }
                    }
                }
            }
        })
    }
}

fn is_immediate(event: &BusEvent) -> bool {
    event.category == "task" || event.event_type.starts_with("agent.step_")
}

fn flush(app_handle: &tauri::AppHandle, buffer: &mut Vec<BusEvent>) {
    if let Err(e) = app_handle.emit("orchestrix://events", &*buffer) {
        tracing::warn!("failed to emit event batch to frontend: {e}");
    }
    buffer.clear();
}
