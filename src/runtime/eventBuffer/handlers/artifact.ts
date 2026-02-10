import type { HandlerContext, HandlerResult } from "../types";

export function handleArtifactCreated(ctx: HandlerContext): HandlerResult {
  const kind = ctx.event.payload?.kind as string | undefined;
  const uri = ctx.event.payload?.uri as string | undefined;
  if (kind && uri) {
    ctx.items.push({
      id: ctx.event.id,
      type: "fileChange",
      timestamp: ctx.event.created_at,
      seq: ctx.event.seq,
      filePath: uri,
      fileAction: "write",
      content: `Artifact created: ${kind}`,
    });
    return { planChanged: false, timelineChanged: true };
  }
  return { planChanged: false, timelineChanged: false };
}
