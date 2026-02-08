import type { ConversationItem } from "@/runtime/eventBuffer";

export type TimelineBlock =
  | { kind: "item"; item: ConversationItem }
  | { kind: "toolBatch"; id: string; items: ConversationItem[] };

function isToolCall(item: ConversationItem): boolean {
  return item.type === "toolCall";
}

export function groupConversationItems(items: ConversationItem[]): TimelineBlock[] {
  const blocks: TimelineBlock[] = [];
  let idx = 0;

  while (idx < items.length) {
    const current = items[idx];
    if (!isToolCall(current)) {
      blocks.push({ kind: "item", item: current });
      idx += 1;
      continue;
    }

    const batch: ConversationItem[] = [current];
    let cursor = idx + 1;
    while (cursor < items.length && isToolCall(items[cursor])) {
      batch.push(items[cursor]);
      cursor += 1;
    }

    if (batch.length === 1) {
      blocks.push({ kind: "item", item: current });
    } else {
      blocks.push({ kind: "toolBatch", id: `tool-batch-${current.id}-${batch.length}`, items: batch });
    }

    idx = cursor;
  }

  return blocks;
}
