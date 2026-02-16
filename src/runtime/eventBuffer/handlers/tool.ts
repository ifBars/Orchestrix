import type { HandlerContext, HandlerResult } from "../types";

function completionMessageFromOutput(output: unknown): string | null {
  if (!output || typeof output !== "object") return null;

  const record = output as Record<string, unknown>;
  const summary = typeof record.summary === "string" ? record.summary.trim() : "";
  const outputs = Array.isArray(record.outputs)
    ? record.outputs.filter((value): value is string => typeof value === "string" && value.trim().length > 0)
    : [];

  if (!summary && outputs.length === 0) return null;
  if (summary && outputs.length === 0) return summary;

  const outputLines = outputs.map((value) => `- ${value}`).join("\n");
  if (!summary) return `Completed outputs:\n${outputLines}`;
  return `${summary}\n\nOutputs:\n${outputLines}`;
}

function pushAgentCompletionMessage(ctx: HandlerContext, content: string, subAgentId?: string): void {
  const trimmed = content.trim();
  if (!trimmed) return;

  const duplicate = ctx.items
    .slice(-5)
    .some((item) => item.type === "agentMessage" && item.content?.trim() === trimmed);
  if (duplicate) return;

  ctx.items.push({
    id: `${ctx.event.id}-completion`,
    type: "agentMessage",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq + 0.0002,
    content: trimmed,
    subAgentId,
  });
}

export function handleToolCallStarted(ctx: HandlerContext): HandlerResult {
  // Flush any text stream before showing the tool call
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
  const payloadToolName = ctx.event.payload?.tool_name as string | undefined;
  const payloadSubAgentId = ctx.event.payload?.sub_agent_id as string | undefined;
  let completedToolName = payloadToolName;
  let completedSubAgentId = payloadSubAgentId;

  if (toolCallId && ctx.getActiveToolCall(toolCallId)) {
    const ref = ctx.getActiveToolCall(toolCallId)!;
    const item = ctx.items[ref.itemIndex];
    if (item && item.type === "toolCall") {
      completedToolName = item.toolName;
      completedSubAgentId = item.subAgentId;
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

  if (ok && completedToolName === "agent.complete") {
    const completionMessage = completionMessageFromOutput(ctx.event.payload?.output) ?? "Objective completed.";
    pushAgentCompletionMessage(ctx, completionMessage, completedSubAgentId);
  }

  return { planChanged: false, timelineChanged: true };
}
