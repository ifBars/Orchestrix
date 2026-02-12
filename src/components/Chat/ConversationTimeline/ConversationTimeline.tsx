import { memo, useEffect, useMemo, useRef } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Loader2,
  XCircle,
} from "lucide-react";
import { runtimeEventBuffer, type ConversationItem } from "@/runtime/eventBuffer";
import type { AgentMessageStream } from "@/runtime/eventBuffer";
import type { ApprovalRequestView, BusEvent, TaskRow } from "@/types";
import { groupConversationItems } from "@/lib/groupConversationItems";
import { AgentTodoPanel } from "./AgentTodoPanel";
import { DebugEvents } from "./DebugEvents";
import { SubAgentActivityPanel } from "./SubAgentActivityPanel";
import { AgentStreamItem, PlanMessage, UserMessage } from "./messages";
import { ConversationItemView, ToolCallBatchItem } from "./items";

const phaseVisual: Record<
  string,
  { label: string; tone: string; badgeTone: string }
> = {
  pending: {
    label: "Pending",
    tone: "border-border/70 bg-background/70 text-muted-foreground",
    badgeTone: "bg-muted text-muted-foreground",
  },
  planning: {
    label: "Planning",
    tone: "border-info/35 bg-info/8 text-info",
    badgeTone: "bg-info/15 text-info",
  },
  awaiting_review: {
    label: "Awaiting review",
    tone: "border-warning/35 bg-warning/8 text-warning",
    badgeTone: "bg-warning/15 text-warning",
  },
  executing: {
    label: "Executing",
    tone: "border-info/35 bg-info/8 text-info",
    badgeTone: "bg-info/15 text-info",
  },
  completed: {
    label: "Completed",
    tone: "border-success/35 bg-success/8 text-success",
    badgeTone: "bg-success/15 text-success",
  },
  failed: {
    label: "Failed",
    tone: "border-destructive/35 bg-destructive/8 text-destructive",
    badgeTone: "bg-destructive/15 text-destructive",
  },
  cancelled: {
    label: "Cancelled",
    tone: "border-warning/35 bg-warning/8 text-warning",
    badgeTone: "bg-warning/15 text-warning",
  },
};

type ConversationTimelineProps = {
  task: TaskRow;
  relatedTasks: TaskRow[];
  onSelectTask: (id: string) => void;
  plan: ReturnType<typeof runtimeEventBuffer.getPlan>;
  planStream: string | null;
  assistantMessage: string | null;
  activeAgentStream: AgentMessageStream | null;
  visibleItems: ConversationItem[];
  renderKey: (item: ConversationItem, idx: number) => string;
  isWorking: boolean;
  onBuild: () => Promise<void>;
  approving: boolean;
  onStop: () => Promise<void>;
  stopping: boolean;
  markdownArtifactCount: number;
  executionSummary: {
    totalSteps: number;
    completedSteps: number;
    failedSteps: number;
    runningStep: number | null;
    runningTool: string | null;
  } | null;
  rawEvents: BusEvent[];
  agentTodos: ReturnType<typeof runtimeEventBuffer.getAgentTodos>;
  pendingApprovals: ApprovalRequestView[];
  resolvingApprovalId: string | null;
  onResolveApproval: (approvalId: string, approve: boolean) => Promise<void>;
};

type TimelineBlocksViewProps = {
  blocks: ReturnType<typeof groupConversationItems>;
  renderKey: (item: ConversationItem, idx: number) => string;
};

const TimelineBlocksView = memo(function TimelineBlocksView({ blocks, renderKey }: TimelineBlocksViewProps) {
  return (
    <>
      {blocks.map((block, idx) => {
        if (block.kind === "toolBatch") {
          return <ToolCallBatchItem key={block.id} items={block.items} />;
        }
        return <ConversationItemView key={renderKey(block.item, idx)} item={block.item} />;
      })}
    </>
  );
});

export function ConversationTimeline(props: ConversationTimelineProps) {
  const delegatedSubAgentIds = useMemo(
    () => collectDelegatedSubAgentIds(props.rawEvents, props.visibleItems),
    [props.rawEvents, props.visibleItems]
  );

  const mainTimelineItems = useMemo(
    () =>
      props.visibleItems.filter(
        (item) => !item.subAgentId || !delegatedSubAgentIds.has(item.subAgentId)
      ),
    [props.visibleItems, delegatedSubAgentIds]
  );
  const timelineBlocks = useMemo(
    () => groupConversationItems(mainTimelineItems),
    [mainTimelineItems]
  );
  const phase = phaseVisual[props.task.status] ?? phaseVisual.pending;
  const isRunning = props.task.status === "planning" || props.task.status === "executing";
  const hasExecutionProgress =
    props.executionSummary && props.executionSummary.totalSteps > 0 && props.task.status === "executing";
  const bottomRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when new content arrives
  useEffect(() => {
    if (bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: "smooth", block: "end" });
    }
  }, [
    props.visibleItems.length,
    props.activeAgentStream?.content,
    props.planStream,
    props.agentTodos.length,
  ]);

  return (
    <div className="mr-auto flex w-full max-w-[1180px] flex-col gap-3 pb-4">
      <div className={`rounded-xl border px-3.5 py-2.5 ${phase.tone}`}>
        <div className="flex flex-wrap items-center gap-2">
          <div className="inline-flex items-center gap-2">
            {isRunning ? (
              <Loader2 size={13} className="animate-spin" />
            ) : props.task.status === "completed" ? (
              <CheckCircle2 size={13} />
            ) : props.task.status === "failed" ? (
              <XCircle size={13} />
            ) : props.task.status === "awaiting_review" ? (
              <Clock3 size={13} />
            ) : (
              <Clock3 size={13} />
            )}
            <span className="text-xs font-semibold">{phase.label}</span>
          </div>

          <span className={`rounded-full px-1.5 py-0.5 text-[10px] font-medium ${phase.badgeTone}`}>
            {props.task.status.replace(/_/g, " ")}
          </span>

          {hasExecutionProgress && props.executionSummary && (
            <span className="text-[11px] text-muted-foreground">
              Step {props.executionSummary.completedSteps + 1}/{props.executionSummary.totalSteps}
              {props.executionSummary.runningTool ? ` - ${props.executionSummary.runningTool}` : ""}
            </span>
          )}

          {props.markdownArtifactCount > 0 && (
            <span className="text-[11px] text-muted-foreground">
              {props.markdownArtifactCount} review artifact{props.markdownArtifactCount === 1 ? "" : "s"}
            </span>
          )}

          {props.task.status === "awaiting_review" && (
            <button
              type="button"
              disabled={props.approving}
              onClick={() => props.onBuild().catch(console.error)}
              className="ml-auto inline-flex items-center gap-1.5 rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-60"
            >
              {props.approving ? <Loader2 size={11} className="animate-spin" /> : <CheckCircle2 size={11} />}
              Build
            </button>
          )}

          {isRunning && (
            <button
              type="button"
              disabled={props.stopping}
              onClick={() => props.onStop().catch(console.error)}
              className="ml-auto inline-flex items-center gap-1.5 rounded-md border border-destructive/40 bg-destructive/10 px-2.5 py-1 text-xs font-medium text-destructive transition-colors hover:bg-destructive/20 disabled:opacity-60"
            >
              {props.stopping ? <Loader2 size={11} className="animate-spin" /> : <XCircle size={11} />}
              Stop
            </button>
          )}
        </div>
      </div>

      <UserMessage
        prompt={props.task.prompt}
        relatedTasks={props.relatedTasks}
        onSelectTask={props.onSelectTask}
      />

      {(props.plan || props.planStream || props.assistantMessage) && (
        <PlanMessage
          plan={props.plan}
          planStream={props.planStream}
          assistantMessage={props.assistantMessage}
          status={props.task.status}
        />
      )}

      {props.agentTodos.length > 0 && (
        <div className="sticky top-2 z-20">
          <AgentTodoPanel agentTodos={props.agentTodos} isWorking={props.isWorking} />
        </div>
      )}

      <TimelineBlocksView blocks={timelineBlocks} renderKey={props.renderKey} />

      <SubAgentActivityPanel
        items={props.visibleItems}
        rawEvents={props.rawEvents}
        activeAgentStream={props.activeAgentStream}
        delegatedSubAgentIds={delegatedSubAgentIds}
      />

      {props.activeAgentStream &&
        (!props.activeAgentStream.subAgentId ||
          !delegatedSubAgentIds.has(props.activeAgentStream.subAgentId)) &&
        props.activeAgentStream.content.length > 0 && (
        <AgentStreamItem stream={props.activeAgentStream} />
      )}

      {props.pendingApprovals.length > 0 && (
        <div className="rounded-xl border border-warning/40 bg-warning/5 p-4">
          <div className="mb-2 flex items-center gap-2 text-sm font-medium text-warning">
            <AlertTriangle size={14} />
            Approval required
          </div>
          <div className="space-y-3">
            {props.pendingApprovals.map((approval) => (
              <div
                key={approval.id}
                className="rounded-lg border border-warning/30 bg-background/60 p-3"
              >
                <p className="text-xs text-foreground">
                  Tool <span className="font-medium">{approval.tool_name}</span> requested access to:
                </p>
                <p className="mt-1 text-xs font-mono text-muted-foreground">
                  {approval.scope}
                </p>
                <p className="mt-1 text-xs text-muted-foreground">{approval.reason}</p>
                <div className="mt-3 flex items-center gap-2">
                  <button
                    type="button"
                    disabled={props.resolvingApprovalId === approval.id}
                    onClick={() => props.onResolveApproval(approval.id, true).catch(console.error)}
                    className="inline-flex items-center gap-1 rounded-lg bg-success px-2.5 py-1 text-xs font-medium text-success-foreground transition-colors hover:bg-success/90 disabled:opacity-60"
                  >
                    Approve
                  </button>
                  <button
                    type="button"
                    disabled={props.resolvingApprovalId === approval.id}
                    onClick={() => props.onResolveApproval(approval.id, false).catch(console.error)}
                    className="inline-flex items-center gap-1 rounded-lg border border-destructive/40 bg-destructive/10 px-2.5 py-1 text-xs font-medium text-destructive transition-colors hover:bg-destructive/20 disabled:opacity-60"
                  >
                    Deny
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      <DebugEvents rawEvents={props.rawEvents} />

      {/* Invisible marker for auto-scroll */}
      <div ref={bottomRef} className="h-1" />
    </div>
  );
}

export { AgentTodoPanel, DebugEvents };
export * from "./messages";
export * from "./items";
export * from "./utils";

function collectDelegatedSubAgentIds(rawEvents: BusEvent[], visibleItems: ConversationItem[]): Set<string> {
  const ids = new Set<string>();

  for (const item of visibleItems) {
    if (!item.subAgentId) continue;
    if (item.type !== "statusChange") continue;
    if (!isDelegatedSubAgentStatus(item.status)) continue;
    ids.add(item.subAgentId);
  }

  for (const event of rawEvents) {
    if (!isSubAgentLifecycleEvent(event.event_type)) continue;
    const subAgentId =
      typeof event.payload?.sub_agent_id === "string"
        ? (event.payload.sub_agent_id as string)
        : null;
    if (subAgentId && subAgentId.trim().length > 0) {
      ids.add(subAgentId);
    }
  }
  return ids;
}

function isDelegatedSubAgentStatus(status?: string): boolean {
  return (
    status === "created" ||
    status === "waiting_for_merge" ||
    status === "closed" ||
    status === "retrying"
  );
}

function isSubAgentLifecycleEvent(eventType: string): boolean {
  return eventType.startsWith("agent.subagent_") || eventType === "agent.worktree_merged";
}
