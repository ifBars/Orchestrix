import { useMemo } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Loader2,
  XCircle,
} from "lucide-react";
import { runtimeEventBuffer, type ConversationItem } from "@/runtime/eventBuffer";
import type { ApprovalRequestView, BusEvent, TaskRow } from "@/types";
import { groupConversationItems } from "@/lib/groupConversationItems";
import { AgentTodoPanel } from "./AgentTodoPanel";
import { DebugEvents } from "./DebugEvents";
import { UserMessage, PlanMessage } from "./messages";
import { ConversationItemView, ToolCallBatchItem } from "./items";

type ConversationTimelineProps = {
  task: TaskRow;
  relatedTasks: TaskRow[];
  onSelectTask: (id: string) => void;
  plan: ReturnType<typeof runtimeEventBuffer.getPlan>;
  planStream: string | null;
  assistantMessage: string | null;
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

export function ConversationTimeline(props: ConversationTimelineProps) {
  const timelineBlocks = useMemo(
    () => groupConversationItems(props.visibleItems),
    [props.visibleItems]
  );

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-4 pb-4">
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

      {timelineBlocks.map((block, idx) => {
        if (block.kind === "toolBatch") {
          return <ToolCallBatchItem key={block.id} items={block.items} />;
        }
        return (
          <ConversationItemView
            key={props.renderKey(block.item, idx)}
            item={block.item}
          />
        );
      })}

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

      {props.task.status === "awaiting_review" && (
        <div className="rounded-xl border border-info/30 bg-info/5 p-4">
          <div className="mb-2 flex items-center gap-2 text-sm font-medium text-info">
            <Clock3 size={14} />
            Awaiting plan review
          </div>
          <p className="mb-3 text-xs text-muted-foreground">
            Review the artifact in full-screen mode and add line comments. Build starts execution.
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={props.approving}
              onClick={() => props.onBuild().catch(console.error)}
              className="inline-flex items-center gap-2 rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {props.approving ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <CheckCircle2 size={12} />
              )}
              Build
            </button>
          </div>
        </div>
      )}

      {props.task.status === "completed" && (
        <div className="flex items-center gap-3 rounded-xl border border-success/30 bg-success/5 px-4 py-3">
          <CheckCircle2 size={16} className="text-success" />
          <span className="text-sm text-success">Task completed successfully</span>
        </div>
      )}

      {props.task.status === "failed" && (
        <div className="flex items-center gap-3 rounded-xl border border-destructive/30 bg-destructive/5 px-4 py-3">
          <XCircle size={16} className="text-destructive" />
          <span className="text-sm text-destructive">Task failed</span>
        </div>
      )}

      <DebugEvents rawEvents={props.rawEvents} />
    </div>
  );
}

export { AgentTodoPanel, DebugEvents };
export * from "./messages";
export * from "./items";
export * from "./utils";
