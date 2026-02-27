import { memo, useEffect, useMemo, useRef, useState } from "react";
import { AlertTriangle } from "lucide-react";
import { runtimeEventBuffer, type ConversationItem } from "@/runtime/eventBuffer";
import type { AgentMessageStream } from "@/runtime/eventBuffer";
import type {
  ApprovalRequestView,
  BusEvent,
  TaskRow,
  UserQuestionAnswer,
  UserQuestionRequestView,
} from "@/types";
import { groupConversationItems } from "@/lib/groupConversationItems";
import { AgentTodoPanel } from "./AgentTodoPanel";
import { DebugEvents } from "./DebugEvents";
import { SubAgentActivityPanel } from "./SubAgentActivityPanel";
import { AgentStreamItem, PlanMessage, UserMessage } from "./messages";
import { ConversationItemView, ToolCallBatchItem } from "./items";

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
  pendingQuestions: UserQuestionRequestView[];
  resolvingApprovalId: string | null;
  resolvingQuestionId: string | null;
  onResolveApproval: (approvalId: string, approve: boolean) => Promise<void>;
  onResolveQuestion: (questionId: string, answer: UserQuestionAnswer) => Promise<void>;
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
  const hasUserMessagesInTimeline = useMemo(
    () => mainTimelineItems.some((item) => item.type === "userMessage"),
    [mainTimelineItems]
  );
  const isBranchPrompt = props.task.prompt.startsWith("Branch:");
  const introPrompt = hasUserMessagesInTimeline || isBranchPrompt ? null : props.task.prompt;
  const bottomRef = useRef<HTMLDivElement>(null);
  const [selectedOptionIdsByQuestion, setSelectedOptionIdsByQuestion] = useState<
    Record<string, string[]>
  >({});
  const [customTextByQuestion, setCustomTextByQuestion] = useState<Record<string, string>>({});

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

      {(introPrompt || props.relatedTasks.length > 0) && (
        <UserMessage
          prompt={introPrompt}
          relatedTasks={props.relatedTasks}
          onSelectTask={props.onSelectTask}
        />
      )}

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
                <p className="mt-1 font-mono text-xs text-muted-foreground">
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

      {props.pendingQuestions.length > 0 && (
        <div className="rounded-xl border border-primary/30 bg-primary/5 p-4">
          <div className="mb-2 flex items-center gap-2 text-sm font-medium text-primary">
            <AlertTriangle size={14} />
            Question for you
          </div>
          <div className="space-y-3">
            {props.pendingQuestions.map((question) => {
              const selected = selectedOptionIdsByQuestion[question.id] ?? [];
              const customText = customTextByQuestion[question.id] ?? "";
              return (
                <div
                  key={question.id}
                  className="rounded-lg border border-primary/25 bg-background/60 p-3"
                >
                  <p className="text-sm text-foreground">{question.question}</p>
                  <div className="mt-2 space-y-2">
                    {question.options.map((option) => {
                      const checked = selected.includes(option.id);
                      return (
                        <label key={option.id} className="flex items-start gap-2 text-xs">
                          <input
                            type={question.multiple ? "checkbox" : "radio"}
                            name={`question-${question.id}`}
                            checked={checked}
                            onChange={() => {
                              setSelectedOptionIdsByQuestion((prev) => {
                                if (question.multiple) {
                                  const current = prev[question.id] ?? [];
                                  const next = current.includes(option.id)
                                    ? current.filter((id) => id !== option.id)
                                    : [...current, option.id];
                                  return { ...prev, [question.id]: next };
                                }
                                return { ...prev, [question.id]: [option.id] };
                              });
                            }}
                            className="mt-0.5"
                          />
                          <span>
                            <span className="font-medium text-foreground">{option.label}</span>
                            {option.description ? (
                              <span className="ml-1 text-muted-foreground">
                                {option.description}
                              </span>
                            ) : null}
                          </span>
                        </label>
                      );
                    })}
                  </div>
                  <textarea
                    value={customText}
                    onChange={(event) => {
                      const value = event.target.value;
                      setCustomTextByQuestion((prev) => ({
                        ...prev,
                        [question.id]: value,
                      }));
                    }}
                    className="mt-3 w-full rounded-md border border-input bg-background px-2 py-1.5 text-xs"
                    rows={3}
                    placeholder={
                      question.allow_custom
                        ? "Optional custom answer"
                        : "Add optional notes"
                    }
                  />
                  <div className="mt-2 flex justify-end">
                    <button
                      type="button"
                      disabled={props.resolvingQuestionId === question.id}
                      onClick={() => {
                        const pickedLabels = question.options
                          .filter((option) => selected.includes(option.id))
                          .map((option) => option.label);
                        const finalText =
                          customText.trim() ||
                          (pickedLabels.length > 0
                            ? pickedLabels.join(question.multiple ? ", " : "")
                            : "");
                        props
                          .onResolveQuestion(question.id, {
                            selected_option_ids: selected,
                            custom_text: customText.trim() ? customText.trim() : null,
                            final_text: finalText,
                          })
                          .catch(console.error);
                      }}
                      className="inline-flex items-center gap-1 rounded-lg bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-60"
                    >
                      Submit
                    </button>
                  </div>
                </div>
              );
            })}
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
