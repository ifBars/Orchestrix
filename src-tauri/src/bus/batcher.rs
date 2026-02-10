use std::time::Duration;

use tauri::Emitter;
use tokio::sync::broadcast;
use tokio::time;

use super::event_bus::BusEvent;
use super::event_types::should_flush_immediately;

const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_MAX_BATCH: usize = 50;

pub struct EventBatcher;

impl EventBatcher {
    /// Spawn a background task that batches events and emits them to the Tauri
    /// frontend via `orchestrix://events`.
    ///
    /// Events for which [should_flush_immediately] returns true are sent
    /// immediately (after flushing the current buffer to preserve order).
    /// All other events are buffered and flushed every 100ms or when the
    /// buffer reaches 50 events.
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
                                if should_flush_immediately(&event) {
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

fn flush(app_handle: &tauri::AppHandle, buffer: &mut Vec<BusEvent>) {
    if let Err(e) = app_handle.emit("orchestrix://events", &*buffer) {
        tracing::warn!("failed to emit event batch to frontend: {e}");
    }
    buffer.clear();
}
