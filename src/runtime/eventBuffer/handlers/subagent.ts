import type { HandlerContext, HandlerResult } from "../types";

export function handleSubagentCreated(ctx: HandlerContext): HandlerResult {
  const subAgentId = ctx.event.payload?.sub_agent_id as string | undefined;
  const stepIdx = ctx.event.payload?.step_idx as number | undefined;
  ctx.items.push({
    id: ctx.event.id,
    type: "statusChange",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    status: "created",
    subAgentId,
    content: `Step ${(stepIdx ?? 0) + 1} delegated`,
  });
  return { planChanged: false, timelineChanged: true };
}

export function handleSubagentStarted(ctx: HandlerContext): HandlerResult {
  const subAgentId = ctx.event.payload?.sub_agent_id as string | undefined;
  const stepIdx = ctx.event.payload?.step_idx as number | undefined;
  ctx.items.push({
    id: ctx.event.id,
    type: "statusChange",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    status: "executing",
    subAgentId,
    content: `Step ${(stepIdx ?? 0) + 1} started`,
  });
  return { planChanged: false, timelineChanged: true };
}

export function handleSubagentCompleted(ctx: HandlerContext): HandlerResult {
  const subAgentId = ctx.event.payload?.sub_agent_id as string | undefined;
  const output = ctx.event.payload?.output_path as string | undefined;
  ctx.items.push({
    id: ctx.event.id,
    type: "statusChange",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    status: "completed",
    subAgentId,
    content: output ? `Completed. Output: ${output}` : "Step completed",
  });
  return { planChanged: false, timelineChanged: true };
}

export function handleSubagentWaitingForMerge(ctx: HandlerContext): HandlerResult {
  const subAgentId = ctx.event.payload?.sub_agent_id as string | undefined;
  ctx.items.push({
    id: ctx.event.id,
    type: "statusChange",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    status: "waiting_for_merge",
    subAgentId,
    content: "Waiting for parent integration",
  });
  return { planChanged: false, timelineChanged: true };
}

export function handleSubagentFailed(ctx: HandlerContext): HandlerResult {
  const subAgentId = ctx.event.payload?.sub_agent_id as string | undefined;
  const error = ctx.event.payload?.error as string | undefined;
  ctx.items.push({
    id: ctx.event.id,
    type: "error",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    errorMessage: error ?? "Sub-agent failed",
    subAgentId,
  });
  return { planChanged: false, timelineChanged: true };
}

export function handleSubagentAttempt(ctx: HandlerContext): HandlerResult {
  const attempt = ctx.event.payload?.attempt as number | undefined;
  const subAgentId = ctx.event.payload?.sub_agent_id as string | undefined;
  if (attempt && attempt > 1) {
    ctx.items.push({
      id: ctx.event.id,
      type: "statusChange",
      timestamp: ctx.event.created_at,
      seq: ctx.event.seq,
      status: "retrying",
      subAgentId,
      content: `Retry attempt ${attempt}`,
    });
    return { planChanged: false, timelineChanged: true };
  }
  return { planChanged: false, timelineChanged: false };
}

export function handleWorktreeMerged(ctx: HandlerContext): HandlerResult {
  ctx.items.push({
    id: ctx.event.id,
    type: "statusChange",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    status: "merged",
    content: "Changes merged into main branch",
  });
  return { planChanged: false, timelineChanged: true };
}

export function handleSubagentClosed(ctx: HandlerContext): HandlerResult {
  const subAgentId = ctx.event.payload?.sub_agent_id as string | undefined;
  const finalStatus = ctx.event.payload?.final_status as string | undefined;
  const closeReason = ctx.event.payload?.close_reason as string | undefined;
  ctx.items.push({
    id: ctx.event.id,
    type: "statusChange",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    status: "closed",
    subAgentId,
    content: finalStatus
      ? `Closed (${finalStatus}${closeReason ? `: ${closeReason}` : ""})`
      : "Sub-agent closed",
  });
  return { planChanged: false, timelineChanged: true };
}
