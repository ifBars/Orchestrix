import type { ConversationItem } from "../types";
import type { HandlerContext, HandlerResult } from "../types";

export function handleThinkingDelta(ctx: HandlerContext): HandlerResult {
  const delta = ctx.event.payload?.content;
  if (typeof delta === "string") {
    const thinker = (ctx.event.payload?.sub_agent_id as string | undefined) ?? "main";
    ctx.appendThinkingDelta(delta, thinker);
    return { planChanged: false, timelineChanged: true };
  }
  return { planChanged: false, timelineChanged: false };
}

export function handleMessage(ctx: HandlerContext): HandlerResult {
  const content = ctx.event.payload?.content;
  if (typeof content === "string" && content.trim()) {
    const trimmed = content.trim();
    const isDuplicate = ctx.items
      .slice(-5)
      .some((it) => it.type === "agentMessage" && it.content === trimmed);
    if (!isDuplicate) {
      ctx.removeLastTransient();
      ctx.items.push({
        id: ctx.event.id,
        type: "agentMessage",
        timestamp: ctx.event.created_at,
        seq: ctx.event.seq,
        content: trimmed,
        subAgentId: (ctx.event.payload?.sub_agent_id as string | undefined) ?? undefined,
      });
      return { planChanged: false, timelineChanged: true };
    }
  }
  return { planChanged: false, timelineChanged: false };
}

export function handleMessageDelta(ctx: HandlerContext): HandlerResult {
  const delta = ctx.event.payload?.content;
  if (typeof delta === "string") {
    const msgId = `agent-delta-${ctx.taskId}`;
    const existingIdx = ctx.items.findIndex((it) => it.type === "agentMessage" && it.id === msgId);
    const prevContent = existingIdx >= 0 ? (ctx.items[existingIdx].content ?? "") : "";
    const nextItem: ConversationItem = {
      id: msgId,
      type: "agentMessage",
      timestamp: ctx.event.created_at,
      seq: ctx.event.seq,
      content: `${prevContent}${delta}`,
    };
    if (existingIdx >= 0) ctx.items[existingIdx] = nextItem;
    else ctx.items.push(nextItem);
    return { planChanged: false, timelineChanged: true };
  }
  return { planChanged: false, timelineChanged: false };
}

export function handleDeciding(ctx: HandlerContext): HandlerResult {
  const id = `deciding-${ctx.taskId}`;
  ctx.setLastTransientId(id);
  ctx.items.push({
    id,
    type: "statusChange",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    status: "deciding",
    content: "Thinking…",
  });
  return { planChanged: false, timelineChanged: true };
}

export function handleToolCallsPreparing(ctx: HandlerContext): HandlerResult {
  const toolNames = (ctx.event.payload?.tool_names as string[] | undefined) ?? [];
  const content = toolNames.length > 0 ? `Preparing: ${toolNames.join(", ")}` : "Preparing tool calls…";
  const id = `preparing-${ctx.taskId}`;
  ctx.setLastTransientId(id);
  ctx.items.push({
    id,
    type: "statusChange",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    status: "preparing",
    content,
  });
  return { planChanged: false, timelineChanged: true };
}

export function handleSubagentsScheduled(ctx: HandlerContext): HandlerResult {
  const count = ctx.event.payload?.count as number | undefined;
  ctx.items.push({
    id: ctx.event.id,
    type: "statusChange",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    status: "scheduled",
    content: count ? `${count} sub-agents scheduled` : "Sub-agents scheduled",
  });
  return { planChanged: false, timelineChanged: true };
}
