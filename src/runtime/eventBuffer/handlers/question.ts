import type { HandlerContext, HandlerResult } from "../types";

export function handleQuestionRequired(ctx: HandlerContext): HandlerResult {
  const question = ctx.event.payload?.question;
  const text = typeof question === "string" ? question : "User input requested";

  ctx.removeLastTransient();
  ctx.items.push({
    id: ctx.event.id,
    type: "statusChange",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    status: "question_required",
    content: text,
    subAgentId:
      typeof ctx.event.payload?.sub_agent_id === "string"
        ? (ctx.event.payload.sub_agent_id as string)
        : undefined,
  });

  return { planChanged: false, timelineChanged: true };
}

export function handleQuestionAnswered(ctx: HandlerContext): HandlerResult {
  const answer = ctx.event.payload?.answer as Record<string, unknown> | undefined;
  const finalText =
    typeof answer?.final_text === "string" && answer.final_text.trim().length > 0
      ? answer.final_text.trim()
      : "Question answered";

  ctx.items.push({
    id: ctx.event.id,
    type: "statusChange",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    status: "question_answered",
    content: `Answered: ${finalText}`,
    subAgentId:
      typeof ctx.event.payload?.sub_agent_id === "string"
        ? (ctx.event.payload.sub_agent_id as string)
        : undefined,
  });

  return { planChanged: false, timelineChanged: true };
}
