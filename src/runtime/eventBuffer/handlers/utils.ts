/**
 * Shared helpers for event handlers.
 */

import type { BusEvent } from "@/types";
import type { ConversationItem } from "../types";

export function toDisplayText(value: unknown): string | undefined {
  if (value == null) return undefined;
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function appendThinkingDelta(
  items: ConversationItem[],
  event: BusEvent,
  taskId: string,
  delta: string,
  thinker?: string
): void {
  const lastItem = items[items.length - 1];
  const sameThinker = (lastItem?.subAgentId ?? "main") === (thinker ?? "main");
  if (lastItem?.type === "thinking" && sameThinker) {
    lastItem.content = `${lastItem.content ?? ""}${delta}`;
    lastItem.timestamp = event.created_at;
    lastItem.seq = event.seq;
    return;
  }
  const thinkingId = thinker ? `thinking-${taskId}-${thinker}-${event.id}` : `thinking-${taskId}-${event.id}`;
  items.push({
    id: thinkingId,
    type: "thinking",
    timestamp: event.created_at,
    seq: event.seq,
    content: delta,
    subAgentId: thinker,
  });
}
