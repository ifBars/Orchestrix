import type { PlanData } from "../types";
import type { HandlerContext, HandlerResult } from "../types";

export function handlePlanReady(ctx: HandlerContext): HandlerResult {
  const plan = ctx.event.payload?.plan;
  let planChanged = false;
  let timelineChanged = false;
  if (plan && typeof plan === "object") {
    const planObj = plan as Record<string, unknown>;
    const steps = Array.isArray(planObj.steps)
      ? (planObj.steps as Array<Record<string, unknown>>).map((s) => ({
          title: String(s.title ?? ""),
          description: String(s.description ?? ""),
          tool_intents: Array.isArray(s.tool_intents) ? (s.tool_intents as string[]) : undefined,
        }))
      : [];
    const pd: PlanData = {
      goalSummary: String(planObj.goal_summary ?? ""),
      steps,
      completionCriteria:
        typeof planObj.completion_criteria === "string" ? planObj.completion_criteria : undefined,
    };
    ctx.setPlan(pd);
    for (let i = 0; i < pd.steps.length; i++) {
      const step = pd.steps[i];
      if (!ctx.items.some((it) => it.type === "planStep" && it.stepIndex === i)) {
        ctx.items.push({
          id: `${ctx.event.id}-step-${i}`,
          type: "planStep",
          timestamp: ctx.event.created_at,
          seq: ctx.event.seq + i * 0.001,
          stepIndex: i,
          stepTitle: step.title,
          stepDescription: step.description,
        });
        timelineChanged = true;
      }
    }
    planChanged = true;
  }
  return { planChanged, timelineChanged };
}

export function handlePlanMessage(ctx: HandlerContext): HandlerResult {
  const content = ctx.event.payload?.content;
  if (typeof content === "string" && content.trim().length > 0) {
    ctx.setAssistantMessage(content.trim());
    return { planChanged: true, timelineChanged: false };
  }
  return { planChanged: false, timelineChanged: false };
}

export function handlePlanDelta(ctx: HandlerContext): HandlerResult {
  const delta = ctx.event.payload?.content;
  if (typeof delta === "string") {
    ctx.appendPlanStream(delta);
    return { planChanged: true, timelineChanged: false };
  }
  return { planChanged: false, timelineChanged: false };
}

export function handlePlanThinkingDelta(ctx: HandlerContext): HandlerResult {
  const delta = ctx.event.payload?.content;
  if (typeof delta === "string") {
    ctx.appendThinkingDelta(delta);
    return { planChanged: false, timelineChanged: true };
  }
  return { planChanged: false, timelineChanged: false };
}

export function handlePlanningStarted(ctx: HandlerContext): HandlerResult {
  ctx.items.push({
    id: ctx.event.id,
    type: "statusChange",
    timestamp: ctx.event.created_at,
    seq: ctx.event.seq,
    status: "planning",
    content: "Generating execution plan...",
  });
  return { planChanged: false, timelineChanged: true };
}
