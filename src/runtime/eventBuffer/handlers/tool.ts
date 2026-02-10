import type { HandlerContext, HandlerResult } from "../types";

export function handleToolCallStarted(ctx: HandlerContext): HandlerResult {
  ctx.removeLastTransient();
  const toolName = ctx.event.payload?.tool_name as string | undefined;
  const rationale = ctx.event.payload?.rationale as string | undefined;
  const toolCallId = ctx.event.payload?.tool_call_id as string | undefined;
  const subAgentId = ctx.event.payload?.sub_agent_id as string | undefined;

  ctx.items.push({
    id: ctx.event.id,
    type: "toolCall",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    toolName: toolName ?? "unknown",
    toolArgs: (ctx.event.payload?.tool_args as Record<string, unknown> | undefined) ?? undefined,
    toolRationale: rationale,
    toolStatus: "running",
    subAgentId,
  });

  if (toolCallId) {
    ctx.setActiveToolCall(toolCallId, ctx.taskId, ctx.items.length - 1);
  }

  if (toolName === "git.apply_patch") {
    ctx.items.push({
      id: `${ctx.event.id}-patch`,
      type: "fileChange",
      timestamp: ctx.event.created_at,
      seq: ctx.event.seq + 0.0001,
      fileAction: "patch",
      subAgentId,
    });
  }
  return { planChanged: false, timelineChanged: true };
}

export function handleToolCallFinished(ctx: HandlerContext): HandlerResult {
  const toolCallId = ctx.event.payload?.tool_call_id as string | undefined;
  const ok = ctx.event.payload?.status === "succeeded";
  const data = ctx.toDisplayText(ctx.event.payload?.output);
  const error = ctx.toDisplayText(ctx.event.payload?.error);

  if (toolCallId && ctx.getActiveToolCall(toolCallId)) {
    const ref = ctx.getActiveToolCall(toolCallId)!;
    const item = ctx.items[ref.itemIndex];
    if (item && item.type === "toolCall") {
      item.toolStatus = ok ? "success" : "error";
      item.toolResult = data;
      item.toolError = error;
      if (item.toolName === "agent.todo" && ok) {
        const agentId = item.subAgentId ?? "main";
        ctx.upsertAgentTodos(agentId, ctx.event.created_at, ctx.event.payload?.output);
      }
    }
    ctx.deleteActiveToolCall(toolCallId);
  } else {
    ctx.items.push({
      id: ctx.event.id,
      type: "toolCall",
      timestamp: ctx.event.created_at,
      seq: ctx.event.seq,
      toolName: (ctx.event.payload?.tool_name as string) ?? "unknown",
      toolStatus: ok ? "success" : "error",
      toolResult: data,
      toolError: error,
    });
  }
  return { planChanged: false, timelineChanged: true };
}
