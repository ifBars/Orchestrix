import type { HandlerContext, HandlerResult } from "../types";

export function handleTaskStatusChanged(ctx: HandlerContext): HandlerResult {
  const status = ctx.event.payload?.status as string | undefined;
  if (status) {
    ctx.items.push({
      id: ctx.event.id,
      type: "statusChange",
      timestamp: ctx.event.created_at,
      seq: ctx.event.seq,
      status,
    });
    return { planChanged: false, timelineChanged: true };
  }
  return { planChanged: false, timelineChanged: false };
}

export function handleUserMessageSent(ctx: HandlerContext): HandlerResult {
  const content = ctx.event.payload?.content;
  if (typeof content === "string" && content.trim().length > 0) {
    ctx.items.push({
      id: ctx.event.id,
      type: "userMessage",
      timestamp: ctx.event.created_at,
      seq: ctx.event.seq,
      content: content.trim(),
    });
    return { planChanged: false, timelineChanged: true };
  }
  return { planChanged: false, timelineChanged: false };
}
