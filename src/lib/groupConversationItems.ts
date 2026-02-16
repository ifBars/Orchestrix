import type { ConversationItem } from "@/runtime/eventBuffer";

export type TimelineBlock =
  | { kind: "item"; item: ConversationItem }
  | { kind: "toolBatch"; id: string; items: ConversationItem[] };

function isToolCall(item: ConversationItem): boolean {
  return item.type === "toolCall";
}

function isTransientStatus(item: ConversationItem): boolean {
  return item.type === "statusChange" && (item.status === "deciding" || item.status === "preparing");
}

function shouldSkipItem(item: ConversationItem, nextItem: ConversationItem | undefined): boolean {
  // Skip transient status items if they're immediately followed by content
  if (isTransientStatus(item) && nextItem) {
    // Hide "Thinking..." if followed by actual content (tool calls, messages, etc.)
    return (
      nextItem.type === "toolCall" ||
      nextItem.type === "agentMessage" ||
      nextItem.type === "thinking" ||
      nextItem.type === "fileChange"
    );
  }
  return false;
}

export function groupConversationItems(items: ConversationItem[]): TimelineBlock[] {
  const blocks: TimelineBlock[] = [];
  let idx = 0;

  while (idx < items.length) {
    const current = items[idx];
    const next = items[idx + 1];

    // Skip transient items that are superseded by actual content
    if (shouldSkipItem(current, next)) {
      idx += 1;
      continue;
    }

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
