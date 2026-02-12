import { Bot, ChevronDown, ChevronRight, Loader2 } from "lucide-react";
import type { AgentMessageStream, ConversationItem } from "@/runtime/eventBuffer";
import type { BusEvent } from "@/types";

type SubAgentActivityPanelProps = {
  items: ConversationItem[];
  rawEvents: BusEvent[];
  activeAgentStream: AgentMessageStream | null;
  delegatedSubAgentIds: ReadonlySet<string>;
};

type SubAgentState = "running" | "completed" | "failed" | "closed";

type SubAgentMeta = {
  id: string;
  stepIdx?: number;
  name?: string;
  objective?: string;
  state: SubAgentState;
};

type SubAgentView = {
  id: string;
  subAgentIds: string[];
  attemptCount: number;
  failedAttempts: number;
  stepIdx?: number;
  name?: string;
  objective?: string;
  state: SubAgentState;
  items: ConversationItem[];
};

export function SubAgentActivityPanel({
  items,
  rawEvents,
  activeAgentStream,
  delegatedSubAgentIds,
}: SubAgentActivityPanelProps) {
  if (delegatedSubAgentIds.size === 0) return null;

  const views = buildSubAgentViews(items, rawEvents, delegatedSubAgentIds);
  if (views.length === 0) return null;

  return (
    <section className="space-y-2">
      {views.map((view) => {
        const objectiveSnippet = truncate(view.objective ?? "No objective captured", 96);
        const stepLabel = typeof view.stepIdx === "number" ? `Step ${view.stepIdx + 1}` : "Delegated";
        const stream = activeAgentStream?.subAgentId === view.id ? activeAgentStream : null;
        const [stateLabel, stateTone] = statusMeta(view.state);

        return (
          <details key={view.id} className="group rounded-xl border border-border bg-card/55">
            <summary className="flex cursor-pointer list-none items-start gap-2 px-3 py-2.5">
              <span className="mt-0.5 text-muted-foreground group-open:hidden">
                <ChevronRight size={13} />
              </span>
              <span className="mt-0.5 text-muted-foreground group-open:block hidden">
                <ChevronDown size={13} />
              </span>
              <span className="mt-0.5 rounded-md border border-border/60 bg-background/70 p-1 text-muted-foreground">
                <Bot size={12} />
              </span>
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium text-foreground">Sub-agent started - {objectiveSnippet}</p>
                <div className="mt-1 flex items-center gap-2 text-[11px] text-muted-foreground">
                  <span>{stepLabel}</span>
                  {view.name ? <span>- {view.name}</span> : null}
                  <span className={`rounded-full px-1.5 py-0.5 font-medium ${stateTone}`}>{stateLabel}</span>
                  {view.attemptCount > 1 ? <span>{view.attemptCount} attempts</span> : null}
                  {view.failedAttempts > 0 && view.state === "completed" ? <span>{view.failedAttempts} failed before success</span> : null}
                  <span>{view.items.length} update{view.items.length === 1 ? "" : "s"}</span>
                </div>
              </div>
            </summary>

            <div className="border-t border-border/70 px-3 py-3">
              <div className="rounded-lg border border-border bg-background/60 p-2.5">
                <p className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/70">Objective</p>
                <p className="mt-1 text-xs leading-relaxed text-foreground">{view.objective ?? "No objective captured"}</p>
              </div>

              <div className="mt-2 space-y-1.5">
                {stream && stream.content.trim().length > 0 ? (
                  <div className="rounded-lg border border-info/25 bg-info/8 px-2.5 py-2 text-xs text-foreground">
                    <div className="mb-1 inline-flex items-center gap-1 text-[11px] text-info">
                      <Loader2 size={10} className="animate-spin" />
                      In progress
                    </div>
                    <p className="line-clamp-4 whitespace-pre-wrap">{stream.content}</p>
                  </div>
                ) : null}

                {view.items.map((item) => (
                  <div key={item.id} className="rounded-md border border-border/70 bg-background/55 px-2.5 py-1.5 text-xs">
                    <p className="text-foreground">{itemSummary(item)}</p>
                  </div>
                ))}
              </div>
            </div>
          </details>
        );
      })}
    </section>
  );
}

function buildSubAgentViews(
  items: ConversationItem[],
  rawEvents: BusEvent[],
  delegatedSubAgentIds: ReadonlySet<string>
): SubAgentView[] {
  const groupedItems = new Map<string, ConversationItem[]>();

  for (const item of items) {
    if (!item.subAgentId || !delegatedSubAgentIds.has(item.subAgentId)) continue;
    const bucket = groupedItems.get(item.subAgentId) ?? [];
    bucket.push(item);
    groupedItems.set(item.subAgentId, bucket);
  }

  const groupedMeta = new Map<string, SubAgentMeta>();

  for (const event of rawEvents) {
    const subAgentId = typeof event.payload?.sub_agent_id === "string" ? (event.payload.sub_agent_id as string) : null;
    if (!subAgentId || !delegatedSubAgentIds.has(subAgentId)) continue;

    const prev = groupedMeta.get(subAgentId) ?? { id: subAgentId, state: "running" as SubAgentState };
    const stepIdx = typeof event.payload?.step_idx === "number" ? (event.payload.step_idx as number) : prev.stepIdx;
    const name = typeof event.payload?.name === "string" ? (event.payload.name as string) : prev.name;
    const objective =
      typeof event.payload?.objective === "string" ? (event.payload.objective as string) : prev.objective;

    let state = prev.state;
    if (event.event_type === "agent.subagent_completed") state = "completed";
    if (event.event_type === "agent.subagent_failed") state = "failed";
    if (event.event_type === "agent.subagent_closed") {
      const finalStatus =
        typeof event.payload?.final_status === "string" ? (event.payload.final_status as string) : "";
      state = finalStatus === "failed" ? "failed" : "closed";
    }

    groupedMeta.set(subAgentId, { id: subAgentId, stepIdx, name, objective, state });
  }

  const validIds = [...new Set<string>([...groupedItems.keys(), ...groupedMeta.keys()])].filter(
    (id) => !id.startsWith("parent-")
  );

  const byObjective = new Map<
    string,
    {
      objective: string;
      metas: SubAgentMeta[];
      ids: string[];
    }
  >();

  for (const id of validIds) {
    const meta = groupedMeta.get(id) ?? { id, state: "running" as SubAgentState };
    const objective = meta.objective?.trim() || `Sub-agent ${id.slice(0, 8)}`;
    const key = normalizeObjectiveKey(objective);
    const bucket = byObjective.get(key) ?? { objective, metas: [], ids: [] };
    bucket.metas.push(meta);
    bucket.ids.push(id);
    byObjective.set(key, bucket);
  }

  return [...byObjective.entries()]
    .map(([key, group]) => {
      const completedAgents = group.metas.filter((m) => m.state === "completed");
      const runningAgents = group.metas.filter((m) => m.state === "running");
      const failedAgents = group.metas.filter((m) => m.state === "failed");

      const selectedAgentId = pickPrimaryAgent(group.ids, groupedItems, completedAgents.map((m) => m.id));
      const selectedItems = (groupedItems.get(selectedAgentId) ?? []).slice().sort((a, b) => a.seq - b.seq);
      const selectedMeta = group.metas.find((m) => m.id === selectedAgentId);

      let state: SubAgentState = "closed";
      if (completedAgents.length > 0) {
        state = "completed";
      } else if (runningAgents.length > 0) {
        state = "running";
      } else if (failedAgents.length > 0) {
        state = "failed";
      }

      return {
        id: key,
        subAgentIds: group.ids,
        attemptCount: group.ids.length,
        failedAttempts: failedAgents.length,
        stepIdx: selectedMeta?.stepIdx,
        name: selectedMeta?.name,
        objective: group.objective,
        state,
        items: selectedItems,
      };
    })
    .sort((a, b) => {
      const aSeq = a.items[0]?.seq ?? Number.MAX_SAFE_INTEGER;
      const bSeq = b.items[0]?.seq ?? Number.MAX_SAFE_INTEGER;
      return aSeq - bSeq;
    });
}

function pickPrimaryAgent(
  ids: string[],
  groupedItems: Map<string, ConversationItem[]>,
  preferredIds: string[]
): string {
  const sourceIds = preferredIds.length > 0 ? preferredIds : ids;
  let bestId = sourceIds[0] ?? ids[0] ?? "";
  let bestSeq = -1;

  for (const id of sourceIds) {
    const items = groupedItems.get(id) ?? [];
    const seq = items.length > 0 ? items[items.length - 1]?.seq ?? -1 : -1;
    if (seq > bestSeq) {
      bestSeq = seq;
      bestId = id;
    }
  }

  return bestId;
}

function normalizeObjectiveKey(objective: string): string {
  return objective
    .toLowerCase()
    .replace(/\s+/g, " ")
    .replace(/\.+$/g, "")
    .trim();
}

function itemSummary(item: ConversationItem): string {
  if (item.type === "toolCall") {
    const toolStatus = item.toolStatus ?? "running";
    return `${toolStatus === "running" ? "Running" : toolStatus === "success" ? "Completed" : "Failed"}: ${item.toolName ?? "tool"}`;
  }
  if (item.type === "agentMessage") {
    return `Response: ${truncate(item.content ?? "", 180)}`;
  }
  if (item.type === "error") {
    return `Error: ${item.errorMessage ?? "Unknown failure"}`;
  }
  if (item.type === "statusChange") {
    return item.content ?? `Status: ${item.status ?? "updated"}`;
  }
  if (item.type === "fileChange") {
    const action = item.fileAction ?? "write";
    return `${action} ${item.filePath ?? "(path unavailable)"}`;
  }
  if (item.type === "thinking") {
    return `Reasoning: ${truncate(item.content ?? "", 180)}`;
  }
  return truncate(item.content ?? "Update", 180);
}

function statusMeta(state: SubAgentState): [string, string] {
  if (state === "running") {
    return ["Running", "bg-info/15 text-info"];
  }
  if (state === "completed") {
    return ["Completed", "bg-success/15 text-success"];
  }
  if (state === "failed") {
    return ["Failed", "bg-destructive/15 text-destructive"];
  }
  return ["Closed", "bg-muted text-muted-foreground"];
}

function truncate(value: string, max: number): string {
  if (value.length <= max) return value;
  return `${value.slice(0, max).trimEnd()}...`;
}
