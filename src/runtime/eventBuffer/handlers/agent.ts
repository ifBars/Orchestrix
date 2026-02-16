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
    ctx.clearAgentMessageStream();
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
  if (typeof delta === "string" && delta.length > 0) {
    const streamId =
      typeof ctx.event.payload?.stream_id === "string" ? (ctx.event.payload.stream_id as string) : undefined;
    const subAgentId =
      typeof ctx.event.payload?.sub_agent_id === "string"
        ? (ctx.event.payload.sub_agent_id as string)
        : undefined;
    const stepIdx =
      typeof ctx.event.payload?.step_idx === "number"
        ? (ctx.event.payload.step_idx as number)
        : undefined;
    const turn = typeof ctx.event.payload?.turn === "number" ? (ctx.event.payload.turn as number) : undefined;

    ctx.appendAgentMessageDelta({
      streamId,
      delta,
      createdAt: ctx.event.created_at,
      seq: ctx.event.seq,
      subAgentId,
      stepIdx,
      turn,
    });

    return { planChanged: false, timelineChanged: false, agentStreamChanged: true };
  }
  return { planChanged: false, timelineChanged: false };
}

export function handleMessageStreamStarted(ctx: HandlerContext): HandlerResult {
  ctx.removeLastTransient();
  const streamId =
    typeof ctx.event.payload?.stream_id === "string"
      ? (ctx.event.payload.stream_id as string)
      : `stream-${ctx.taskId}-${ctx.event.id}`;
  const subAgentId =
    typeof ctx.event.payload?.sub_agent_id === "string"
      ? (ctx.event.payload.sub_agent_id as string)
      : undefined;
  const stepIdx =
    typeof ctx.event.payload?.step_idx === "number"
      ? (ctx.event.payload.step_idx as number)
      : undefined;
  const turn = typeof ctx.event.payload?.turn === "number" ? (ctx.event.payload.turn as number) : undefined;

  ctx.startAgentMessageStream({
    streamId,
    createdAt: ctx.event.created_at,
    seq: ctx.event.seq,
    subAgentId,
    stepIdx,
    turn,
  });
  return { planChanged: false, timelineChanged: false, agentStreamChanged: true };
}

export function handleMessageStreamCompleted(ctx: HandlerContext): HandlerResult {
  const streamId =
    typeof ctx.event.payload?.stream_id === "string" ? (ctx.event.payload.stream_id as string) : undefined;
  ctx.completeAgentMessageStream(streamId, ctx.event.created_at, ctx.event.seq);
  return { planChanged: false, timelineChanged: false, agentStreamChanged: true };
}

export function handleMessageStreamCancelled(ctx: HandlerContext): HandlerResult {
  // When the backend cancels a stream (e.g. model chose tool calls instead of
  // a text response), preserve any content that was already streamed so the
  // user doesn't see text appear and then vanish.
  const stream = ctx.flushAgentMessageStream();
  if (stream && stream.content.trim()) {
    ctx.items.push({
      id: stream.streamId,
      type: "agentMessage",
      timestamp: stream.updatedAt,
      seq: stream.seq,
      content: stream.content.trim(),
      subAgentId: stream.subAgentId,
    });
    return { planChanged: false, timelineChanged: true, agentStreamChanged: true };
  }
  return { planChanged: false, timelineChanged: false, agentStreamChanged: true };
}

export function handleDeciding(ctx: HandlerContext): HandlerResult {
  // If we have a message stream (text) from before, flush it now
  // so it doesn't get lost or look out of order.
  // Preserve it even if still streaming, since we're moving to the next turn.
  const completedStream = ctx.flushAgentMessageStream();
  if (completedStream && completedStream.content.trim()) {
    ctx.items.push({
      id: completedStream.streamId,
      type: "agentMessage",
      timestamp: completedStream.updatedAt,
      seq: completedStream.seq,
      content: completedStream.content.trim(),
      subAgentId: completedStream.subAgentId,
    });
  }

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
  // Flush any text stream before showing "Preparing tool calls"
  // Preserve it even if still streaming, since we're moving to tool execution.
  const completedStream = ctx.flushAgentMessageStream();
  if (completedStream && completedStream.content.trim()) {
    ctx.items.push({
      id: completedStream.streamId,
      type: "agentMessage",
      timestamp: completedStream.updatedAt,
      seq: completedStream.seq,
      content: completedStream.content.trim(),
      subAgentId: completedStream.subAgentId,
    });
  }

  ctx.removeLastTransient();
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
  // Flush any text stream before showing "Sub-agents scheduled"
  // Preserve it even if still streaming, since we're moving to sub-agent execution.
  const completedStream = ctx.flushAgentMessageStream();
  if (completedStream && completedStream.content.trim()) {
    ctx.items.push({
      id: completedStream.streamId,
      type: "agentMessage",
      timestamp: completedStream.updatedAt,
      seq: completedStream.seq,
      content: completedStream.content.trim(),
      subAgentId: completedStream.subAgentId,
    });
  }

  ctx.removeLastTransient();
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
