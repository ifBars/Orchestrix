import { useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  ArrowRight,
  Bot,
  Brain,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Clock3,
  FileCode2,
  GitMerge,
  ListChecks,
  Loader2,
  Terminal,
  XCircle,
} from "lucide-react";
import { Streamdown } from "streamdown";
import { code } from "@streamdown/code";
import { runtimeEventBuffer, type ConversationItem } from "@/runtime/eventBuffer";
import type { ApprovalRequestView, BusEvent, TaskRow } from "@/types";
import { groupConversationItems } from "@/lib/groupConversationItems";

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
  const timelineBlocks = useMemo(() => groupConversationItems(props.visibleItems), [props.visibleItems]);

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-4 pb-4">
      <UserMessage prompt={props.task.prompt} relatedTasks={props.relatedTasks} onSelectTask={props.onSelectTask} />

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
        return <ConversationItemView key={props.renderKey(block.item, idx)} item={block.item} />;
      })}

      {props.pendingApprovals.length > 0 && (
        <div className="rounded-xl border border-warning/40 bg-warning/5 p-4">
          <div className="mb-2 flex items-center gap-2 text-sm font-medium text-warning">
            <AlertTriangle size={14} />
            Approval required
          </div>
          <div className="space-y-3">
            {props.pendingApprovals.map((approval) => (
              <div key={approval.id} className="rounded-lg border border-warning/30 bg-background/60 p-3">
                <p className="text-xs text-foreground">
                  Tool <span className="font-medium">{approval.tool_name}</span> requested access to:
                </p>
                <p className="mt-1 text-xs font-mono text-muted-foreground">{approval.scope}</p>
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
              {props.approving ? <Loader2 size={12} className="animate-spin" /> : <CheckCircle2 size={12} />}
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

function AgentTodoPanel({
  agentTodos,
  isWorking,
}: {
  agentTodos: ReturnType<typeof runtimeEventBuffer.getAgentTodos>;
  isWorking: boolean;
}) {
  const [expanded, setExpanded] = useState(isWorking);

  useEffect(() => {
    setExpanded(isWorking);
  }, [isWorking]);

  const total = agentTodos.reduce((sum, list) => sum + list.todos.length, 0);
  const completed = agentTodos.reduce(
    (sum, list) => sum + list.todos.filter((todo) => todo.status === "completed").length,
    0,
  );
  const inProgress = agentTodos.reduce(
    (sum, list) => sum + list.todos.filter((todo) => todo.status === "in_progress").length,
    0,
  );

  return (
    <div className="rounded-xl border border-border/70 bg-background/90 p-3 shadow-sm backdrop-blur supports-[backdrop-filter]:bg-background/80">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 text-left"
      >
        <ListChecks size={14} className="text-muted-foreground" />
        <span className="text-xs font-medium text-foreground">
          Todo lists ({total})
        </span>
        <span className="ml-auto text-[11px] text-muted-foreground">
          {completed} completed{inProgress > 0 ? `, ${inProgress} in progress` : ""}
        </span>
        {expanded ? (
          <ChevronDown size={12} className="text-muted-foreground" />
        ) : (
          <ChevronRight size={12} className="text-muted-foreground" />
        )}
      </button>

      {expanded && (
        <div className="mt-3 space-y-2">
          {agentTodos.map((list) => (
            <div key={list.agentId} className="rounded-lg border border-border/60 bg-card/40 p-2.5">
              <div className="mb-2 flex items-center justify-between gap-2">
                <span className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
                  {list.agentId === "main" ? "Main agent" : `Sub-agent ${list.agentId.slice(0, 8)}`}
                </span>
                <span className="text-[11px] text-muted-foreground">
                  {list.todos.length} items
                </span>
              </div>

              <div className="space-y-1.5">
                {list.todos.map((todo) => (
                  <div key={`${list.agentId}-${todo.id}`} className="flex items-start gap-2">
                    <span className={todoStatusClass(todo.status)}>{todo.status.replace(/_/g, " ")}</span>
                    <p className="min-w-0 flex-1 text-xs text-foreground">{todo.content}</p>
                    {todo.priority && (
                      <span className="rounded-full border border-border/60 bg-background/70 px-1.5 py-0.5 text-[10px] text-muted-foreground">
                        {todo.priority}
                      </span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function todoStatusClass(status: string): string {
  if (status === "completed") {
    return "rounded-full border border-success/30 bg-success/10 px-1.5 py-0.5 text-[10px] text-success";
  }
  if (status === "in_progress") {
    return "rounded-full border border-info/30 bg-info/10 px-1.5 py-0.5 text-[10px] text-info";
  }
  if (status === "cancelled") {
    return "rounded-full border border-warning/30 bg-warning/10 px-1.5 py-0.5 text-[10px] text-warning";
  }
  return "rounded-full border border-border/60 bg-background/70 px-1.5 py-0.5 text-[10px] text-muted-foreground";
}

function DebugEvents({ rawEvents }: { rawEvents: BusEvent[] }) {
  if (rawEvents.length === 0) return null;

  const recent = rawEvents.slice(-80).reverse();
  return (
    <details className="rounded-xl border border-border/70 bg-muted/10 p-3">
      <summary className="cursor-pointer text-xs font-medium text-muted-foreground">
        Debug Timeline ({rawEvents.length} events)
      </summary>
      <div className="mt-3 max-h-72 space-y-2 overflow-auto">
        {recent.map((event) => (
          <div key={event.id} className="rounded-md border border-border/50 bg-background/70 p-2">
            <div className="mb-1 flex items-center justify-between gap-2 text-[11px] text-muted-foreground">
              <span className="truncate">{event.event_type}</span>
              <span>{new Date(event.created_at).toLocaleTimeString()}</span>
            </div>
            <pre className="overflow-auto text-[11px] text-foreground/80">
              <code>{JSON.stringify(event.payload, null, 2)}</code>
            </pre>
          </div>
        ))}
      </div>
    </details>
  );
}

function UserMessage({
  prompt,
  relatedTasks,
  onSelectTask,
}: {
  prompt: string;
  relatedTasks: TaskRow[];
  onSelectTask: (id: string) => void;
}) {
  return (
    <div className="flex gap-3">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/15 text-primary">
        <span className="text-xs font-semibold">You</span>
      </div>
      <div className="min-w-0 flex-1 pt-1">
        <p className="text-sm leading-relaxed text-foreground">{prompt}</p>
        {relatedTasks.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {relatedTasks.map((related) => (
              <button
                key={related.id}
                type="button"
                className="inline-flex items-center gap-1 rounded-full border border-border bg-muted/50 px-2.5 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                onClick={() => onSelectTask(related.id)}
              >
                <ArrowRight size={10} />
                <span className="max-w-32 truncate">{related.prompt}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function UserMessageItem({ content }: { content: string }) {
  return (
    <div className="flex gap-3">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/15 text-primary">
        <span className="text-xs font-semibold">You</span>
      </div>
      <div className="min-w-0 flex-1 pt-1">
        <p className="text-sm leading-relaxed text-foreground">{content}</p>
      </div>
    </div>
  );
}

function PlanMessage({
  plan,
  planStream,
  assistantMessage,
  status,
}: {
  plan: ReturnType<typeof runtimeEventBuffer.getPlan>;
  planStream: string | null;
  assistantMessage: string | null;
  status: string;
}) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="flex gap-3">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-accent text-muted-foreground">
        <Bot size={14} />
      </div>
      <div className="min-w-0 flex-1 pt-1">
        {assistantMessage && (
          <div className="prose prose-sm max-w-none text-sm leading-relaxed text-foreground dark:prose-invert prose-p:my-1 prose-headings:my-2 prose-code:text-xs">
            <Streamdown plugins={{ code }}>{assistantMessage}</Streamdown>
          </div>
        )}

        {!assistantMessage && planStream && (
          <div className="prose prose-sm max-w-none text-sm leading-relaxed text-foreground dark:prose-invert prose-p:my-1">
            <Streamdown plugins={{ code }}>{planStream}</Streamdown>
          </div>
        )}

        {plan && plan.steps.length > 0 && (
          <div className="mt-3">
            <button
              type="button"
              onClick={() => setExpanded(!expanded)}
              className="flex items-center gap-2 rounded-lg border border-border bg-muted/30 px-3 py-2 text-xs font-medium text-muted-foreground transition-colors hover:bg-muted/60"
            >
              <ListChecks size={14} />
              <span>{plan.steps.length} steps planned</span>
              {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            </button>

            {expanded && (
              <div className="mt-2 space-y-1.5 pl-1">
                {plan.steps.map((step, i) => (
                  <div key={i} className="flex items-start gap-2 rounded-md border border-border/60 bg-card/50 px-3 py-2">
                    <span className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-muted text-[10px] font-semibold text-muted-foreground">
                      {i + 1}
                    </span>
                    <div className="min-w-0">
                      <p className="text-xs font-medium text-foreground">{step.title}</p>
                      {step.description && <p className="mt-0.5 text-xs text-muted-foreground">{step.description}</p>}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {status === "planning" && (
          <div className="mt-2 inline-flex items-center gap-1.5 rounded-full bg-info/10 px-2.5 py-1 text-xs text-info">
            <Loader2 size={10} className="animate-spin" />
            Planning
          </div>
        )}
      </div>
    </div>
  );
}

function ConversationItemView({ item }: { item: ConversationItem }) {
  switch (item.type) {
    case "userMessage":
      return <UserMessageItem content={item.content ?? ""} />;
    case "agentMessage":
      return <AgentMessageItem item={item} />;
    case "toolCall":
      return <ToolCallItem item={item} />;
    case "fileChange":
      return <FileChangeItem item={item} />;
    case "statusChange":
      return <StatusChangeItem item={item} />;
    case "error":
      return <ErrorItem item={item} />;
    case "thinking":
      return <ThinkingItem item={item} />;
    default:
      return null;
  }
}

function ToolCallBatchItem({ items }: { items: ConversationItem[] }) {
  const [expanded, setExpanded] = useState(false);

  const runningCount = items.filter((item) => item.toolStatus === "running").length;
  const errorCount = items.filter((item) => item.toolStatus === "error").length;
  const successCount = items.filter((item) => item.toolStatus === "success").length;

  return (
    <div className="ml-11 rounded-lg border border-border/60 bg-muted/20">
      <button
        type="button"
        aria-expanded={expanded}
        aria-controls={`tool-batch-${items[0]?.id ?? "unknown"}`}
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition-colors hover:bg-muted/40"
      >
        <Terminal size={13} className="shrink-0 text-muted-foreground" />
        <span className="min-w-0 flex-1 text-xs font-medium text-foreground">
          {items.length} tool calls
        </span>
        {runningCount > 0 && <span className="text-[11px] text-info">{runningCount} running</span>}
        {errorCount > 0 && <span className="text-[11px] text-destructive">{errorCount} failed</span>}
        {successCount > 0 && errorCount === 0 && runningCount === 0 && (
          <span className="text-[11px] text-success">{successCount} done</span>
        )}
        {expanded ? <ChevronDown size={12} className="shrink-0 text-muted-foreground" /> : <ChevronRight size={12} className="shrink-0 text-muted-foreground" />}
      </button>

      {expanded && (
        <div id={`tool-batch-${items[0]?.id ?? "unknown"}`} className="space-y-2 border-t border-border/60 px-2 py-2">
          {items.map((toolItem) => (
            <ToolCallItem key={toolItem.id} item={toolItem} compact />
          ))}
        </div>
      )}
    </div>
  );
}

function AgentMessageItem({ item }: { item: ConversationItem }) {
  return (
    <div className="flex gap-3">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-accent text-muted-foreground">
        <Bot size={14} />
      </div>
      <div className="min-w-0 flex-1 pt-1">
        <div className="prose prose-sm max-w-none text-sm leading-relaxed text-foreground dark:prose-invert prose-p:my-1 prose-code:text-xs">
          <Streamdown plugins={{ code }}>{item.content ?? ""}</Streamdown>
        </div>
      </div>
    </div>
  );
}

function ToolCallItem({ item, compact = false }: { item: ConversationItem; compact?: boolean }) {
  const [expanded, setExpanded] = useState(false);
  const isRunning = item.toolStatus === "running";
  const isError = item.toolStatus === "error";

  const statusIcon = isRunning ? (
    <Loader2 size={12} className="animate-spin text-info" />
  ) : isError ? (
    <XCircle size={12} className="text-destructive" />
  ) : (
    <CheckCircle2 size={12} className="text-success" />
  );

  return (
    <div className={compact ? "" : "ml-11"}>
      <button
        type="button"
        aria-expanded={expanded}
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 rounded-lg border border-border/60 bg-muted/20 px-3 py-2 text-left transition-colors hover:bg-muted/40"
      >
        <Terminal size={13} className="shrink-0 text-muted-foreground" />
        <div className="min-w-0 flex-1">
          <span className="block truncate text-xs font-medium text-foreground">{item.toolName}</span>
          {item.toolRationale && (
            <span className="block truncate text-[11px] text-muted-foreground">{item.toolRationale}</span>
          )}
        </div>
        {statusIcon}
        {expanded ? <ChevronDown size={12} className="shrink-0 text-muted-foreground" /> : <ChevronRight size={12} className="shrink-0 text-muted-foreground" />}
      </button>

      {expanded && (
        <div className="mt-1 rounded-lg border border-border/40 bg-card/30 p-3">
          {item.toolArgs && Object.keys(item.toolArgs).length > 0 && (
            <div className="mb-2">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Arguments</span>
              <pre className="mt-1 max-h-32 overflow-auto rounded-md bg-background/60 p-2 text-xs text-muted-foreground">{JSON.stringify(item.toolArgs, null, 2)}</pre>
            </div>
          )}
          {item.toolResult && (
            <div className="mb-2">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Result</span>
              <pre className="mt-1 max-h-40 overflow-auto rounded-md bg-background/60 p-2 text-xs text-muted-foreground">{toDisplay(item.toolResult)}</pre>
            </div>
          )}
          {item.toolError && (
            <div>
              <span className="text-[10px] font-semibold uppercase tracking-wider text-destructive/80">Error</span>
              <pre className="mt-1 max-h-24 overflow-auto rounded-md bg-destructive/5 p-2 text-xs text-destructive">{toDisplay(item.toolError)}</pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function FileChangeItem({ item }: { item: ConversationItem }) {
  if (!item.filePath) return null;
  const fileName = item.filePath.split(/[/\\]/).pop() ?? item.filePath;

  return (
    <div className="ml-11 flex items-center gap-2 rounded-lg border border-border/40 bg-muted/10 px-3 py-1.5">
      <FileCode2 size={13} className="shrink-0 text-info" />
      <span className="min-w-0 truncate text-xs text-foreground">{fileName}</span>
      <span className="shrink-0 rounded-full bg-info/10 px-1.5 py-0.5 text-[10px] text-info">{item.fileAction ?? "modified"}</span>
    </div>
  );
}

function StatusChangeItem({ item }: { item: ConversationItem }) {
  let icon = <Clock3 size={12} className="text-muted-foreground" />;
  if (item.status === "completed" || item.status === "merged") {
    icon = <CheckCircle2 size={12} className="text-success" />;
  } else if (item.status === "failed" || item.status === "retrying") {
    icon = <AlertTriangle size={12} className="text-warning" />;
  } else if (item.status === "executing") {
    icon = <Loader2 size={12} className="animate-spin text-info" />;
  } else if (item.status === "merged") {
    icon = <GitMerge size={12} className="text-success" />;
  }

  return (
    <div className="ml-11 flex items-center gap-2 py-1 text-xs text-muted-foreground">
      {icon}
      <span>{item.content ?? `Status: ${item.status}`}</span>
    </div>
  );
}

function ErrorItem({ item }: { item: ConversationItem }) {
  return (
    <div className="ml-11 flex items-start gap-2 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2">
      <XCircle size={13} className="mt-0.5 shrink-0 text-destructive" />
      <p className="text-xs text-destructive">{item.errorMessage ?? "Unknown error"}</p>
    </div>
  );
}

function ThinkingItem({ item }: { item: ConversationItem }) {
  const [collapsed, setCollapsed] = useState(true);
  const text = item.content ?? "";
  if (!text) return null;
  const preview = text.replace(/\s+/g, " ").trim();

  return (
    <div className="ml-11">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="flex items-center gap-2 text-xs text-muted-foreground/70 transition-colors hover:text-muted-foreground"
      >
        <Brain size={12} />
        <span>Reasoning</span>
        {collapsed ? <ChevronRight size={10} /> : <ChevronDown size={10} />}
      </button>
      {collapsed && (
        <p className="mt-1 overflow-hidden text-xs text-muted-foreground/80 [display:-webkit-box] [-webkit-box-orient:vertical] [-webkit-line-clamp:2]">
          {preview}
        </p>
      )}
      {!collapsed && (
        <div className="mt-1 rounded-lg border border-border/30 bg-muted/10 p-3">
          <div className="prose prose-sm max-w-none text-xs italic text-muted-foreground/80 dark:prose-invert prose-p:my-1 prose-headings:my-2 prose-code:text-[11px]">
            <Streamdown plugins={{ code }}>{text}</Streamdown>
          </div>
        </div>
      )}
    </div>
  );
}

function toDisplay(value: unknown): string {
  if (value == null) return "";
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}
