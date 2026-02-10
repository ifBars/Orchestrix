/**
 * Resolve task id from an event (payload.task_id or run_id lookup).
 */

import type { BusEvent } from "@/types";

export function resolveTaskId(
  event: BusEvent,
  runToTask: Map<string, string>
): string | null {
  const payloadTask = event.payload?.task_id ?? event.payload?.taskId;
  if (typeof payloadTask === "string") {
    if (event.run_id) runToTask.set(event.run_id, payloadTask);
    return payloadTask;
  }
  if (event.run_id) return runToTask.get(event.run_id) ?? null;
  return null;
}
