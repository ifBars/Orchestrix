/**
 * RuntimeEventBuffer — converts raw BusEvents into structured conversation
 * items inspired by OpenAI Codex's ThreadItem model.
 *
 * Item types:
 *  - userMessage     : the original task prompt
 *  - planStep        : a step from the planner's plan
 *  - agentMessage    : assistant text (plan summary, completion message)
 *  - toolCall        : a tool invocation with optional result
 *  - fileChange      : a file write/patch (extracted from tool calls)
 *  - statusChange    : task/agent status transitions
 *  - error           : errors from execution
 */

import type { BusEvent } from "@/types";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export type ConversationItemType =
  | "userMessage"
  | "agentMessage"
  | "planStep"
  | "toolCall"
  | "fileChange"
  | "statusChange"
  | "error"
  | "thinking";

export interface ConversationItem {
  id: string;
  type: ConversationItemType;
  timestamp: string;
  seq: number;

  // userMessage / agentMessage / thinking
  content?: string;

  // planStep
  stepIndex?: number;
  stepTitle?: string;
  stepDescription?: string;

  // toolCall
  toolName?: string;
  toolArgs?: Record<string, unknown>;
  toolRationale?: string;
  toolStatus?: "running" | "success" | "error";
  toolResult?: string;
  toolError?: string;
  toolDurationMs?: number;

  // fileChange
  filePath?: string;
  fileAction?: "write" | "patch" | "delete";

  // statusChange
  status?: string;
  previousStatus?: string;

  // error
  errorMessage?: string;

  // sub-agent info
  subAgentId?: string;
}

export interface PlanData {
  goalSummary: string;
  steps: Array<{
    title: string;
    description: string;
    tool_intents?: string[];
  }>;
  completionCriteria?: string;
}

export interface AgentTodoItem {
  id: string;
  content: string;
  status: string;
  priority?: string;
}

export interface AgentTodoList {
  agentId: string;
  todos: AgentTodoItem[];
  updatedAt: string;
}

// ---------------------------------------------------------------------------
// Internal tracking
// ---------------------------------------------------------------------------

const MAX_ITEMS_PER_TASK = 500;

class RuntimeEventBuffer {
  private runToTask = new Map<string, string>();
  private itemsByTask = new Map<string, ConversationItem[]>();
  private planByTask = new Map<string, PlanData>();
  private assistantMessageByTask = new Map<string, string>();
  // Track tool calls in progress to update them when finished
  private activeToolCalls = new Map<string, { taskId: string; itemIndex: number }>();
  // Track streaming plan text
  private planStreamByTask = new Map<string, string>();
  private seenEventIdsByTask = new Map<string, Set<string>>();
  private rawEventsByTask = new Map<string, BusEvent[]>();
  private agentTodosByTask = new Map<string, Map<string, AgentTodoList>>();

  private normalizeTodoItem(item: unknown, idx: number): AgentTodoItem | null {
    if (!item || typeof item !== "object") return null;

    const record = item as Record<string, unknown>;
    const contentCandidate =
      (typeof record.content === "string" && record.content) ||
      (typeof record.title === "string" && record.title) ||
      (typeof record.text === "string" && record.text) ||
      (typeof record.name === "string" && record.name) ||
      "";

    const content = contentCandidate.trim();
    if (!content) return null;

    const idCandidate =
      (typeof record.id === "string" && record.id) ||
      (typeof record.key === "string" && record.key) ||
      `todo-${idx}`;
    const status =
      typeof record.status === "string" && record.status.trim()
        ? record.status.trim()
        : "pending";
    const priority =
      typeof record.priority === "string" && record.priority.trim()
        ? record.priority.trim()
        : undefined;

    return {
      id: idCandidate,
      content,
      status,
      priority,
    };
  }

  private parseTodoItems(output: unknown): AgentTodoItem[] {
    let source: unknown = output;

    if (typeof source === "string") {
      try {
        source = JSON.parse(source);
      } catch {
        return [];
      }
    }

    if (!source || typeof source !== "object") return [];

    const obj = source as Record<string, unknown>;
    const direct = Array.isArray(obj.todos) ? obj.todos : null;
    const nested =
      !direct &&
      obj.data &&
      typeof obj.data === "object" &&
      Array.isArray((obj.data as Record<string, unknown>).todos)
        ? ((obj.data as Record<string, unknown>).todos as unknown[])
        : null;

    const todos = (direct ?? nested ?? []) as unknown[];
    return todos
      .map((item, idx) => this.normalizeTodoItem(item, idx))
      .filter((item): item is AgentTodoItem => item != null);
  }

  private upsertAgentTodos(taskId: string, agentId: string, updatedAt: string, output: unknown) {
    const todos = this.parseTodoItems(output);
    const byAgent = this.agentTodosByTask.get(taskId) ?? new Map<string, AgentTodoList>();
    byAgent.set(agentId, {
      agentId,
      todos,
      updatedAt,
    });
    this.agentTodosByTask.set(taskId, byAgent);
  }

  private appendThinkingDelta(
    items: ConversationItem[],
    event: BusEvent,
    taskId: string,
    delta: string,
    thinker?: string,
  ) {
    const lastItem = items[items.length - 1];
    const sameThinker = (lastItem?.subAgentId ?? "main") === (thinker ?? "main");

    if (lastItem?.type === "thinking" && sameThinker) {
      lastItem.content = `${lastItem.content ?? ""}${delta}`;
      lastItem.timestamp = event.created_at;
      lastItem.seq = event.seq;
      return;
    }

    const thinkingId = thinker ? `thinking-${taskId}-${thinker}-${event.id}` : `thinking-${taskId}-${event.id}`;
    items.push({
      id: thinkingId,
      type: "thinking",
      timestamp: event.created_at,
      seq: event.seq,
      content: delta,
      subAgentId: thinker,
    });
  }

  private toDisplayText(value: unknown): string | undefined {
    if (value == null) return undefined;
    if (typeof value === "string") return value;
    try {
      return JSON.stringify(value, null, 2);
    } catch {
      return String(value);
    }
  }

  resolveTaskId(event: BusEvent): string | null {
    const payloadTask = event.payload?.task_id || event.payload?.taskId;
    if (typeof payloadTask === "string") {
      if (event.run_id) this.runToTask.set(event.run_id, payloadTask);
      return payloadTask;
    }
    if (event.run_id) {
      return this.runToTask.get(event.run_id) ?? null;
    }
    return null;
  }

  ingest(event: BusEvent, taskId: string): { planChanged: boolean; timelineChanged: boolean } {
    let planChanged = false;
    let timelineChanged = false;

    const items = this.itemsByTask.get(taskId) ?? [];
    const seen = this.seenEventIdsByTask.get(taskId) ?? new Set<string>();

    if (seen.has(event.id)) {
      return { planChanged, timelineChanged };
    }
    seen.add(event.id);
    this.seenEventIdsByTask.set(taskId, seen);

    const rawEvents = this.rawEventsByTask.get(taskId) ?? [];
    rawEvents.push(event);
    if (rawEvents.length > 400) {
      rawEvents.splice(0, rawEvents.length - 400);
    }
    this.rawEventsByTask.set(taskId, rawEvents);

    switch (event.event_type) {
      // ---------------------------------------------------------------
      // Planning events
      // ---------------------------------------------------------------
      case "agent.plan_ready": {
        const plan = event.payload?.plan;
        if (plan && typeof plan === "object") {
          const planObj = plan as Record<string, unknown>;
          const steps = Array.isArray(planObj.steps)
            ? (planObj.steps as Array<Record<string, unknown>>).map((s) => ({
                title: String(s.title ?? ""),
                description: String(s.description ?? ""),
                tool_intents: Array.isArray(s.tool_intents)
                  ? (s.tool_intents as string[])
                  : undefined,
              }))
            : [];
          const pd: PlanData = {
            goalSummary: String(planObj.goal_summary ?? ""),
            steps,
            completionCriteria: typeof planObj.completion_criteria === "string"
              ? planObj.completion_criteria
              : undefined,
          };
          this.planByTask.set(taskId, pd);

          // Add plan steps as items
          for (let i = 0; i < pd.steps.length; i++) {
            const step = pd.steps[i];
            if (!items.some((it) => it.type === "planStep" && it.stepIndex === i)) {
              items.push({
                id: `${event.id}-step-${i}`,
                type: "planStep",
                timestamp: event.created_at,
                seq: event.seq + i * 0.001,
                stepIndex: i,
                stepTitle: step.title,
                stepDescription: step.description,
              });
              timelineChanged = true;
            }
          }
          planChanged = true;
        }
        break;
      }

      case "agent.plan_message": {
        const content = event.payload?.content;
        if (typeof content === "string" && content.trim().length > 0) {
          const trimmed = content.trim();
          this.assistantMessageByTask.set(taskId, trimmed);
          planChanged = true;
        }
        break;
      }

      // ---------------------------------------------------------------
      // User events
      // ---------------------------------------------------------------
      case "user.message_sent": {
        const content = event.payload?.content;
        if (typeof content === "string" && content.trim().length > 0) {
          items.push({
            id: event.id,
            type: "userMessage",
            timestamp: event.created_at,
            seq: event.seq,
            content: content.trim(),
          });
          timelineChanged = true;
        }
        break;
      }

      case "agent.plan_delta": {
        const delta = event.payload?.content;
        if (typeof delta === "string") {
          const prev = this.planStreamByTask.get(taskId) ?? "";
          this.planStreamByTask.set(taskId, prev + delta);
          planChanged = true;
        }
        break;
      }

      case "agent.plan_thinking_delta": {
        const delta = event.payload?.content;
        if (typeof delta === "string") {
          this.appendThinkingDelta(items, event, taskId, delta);
          timelineChanged = true;
        }
        break;
      }

      case "agent.thinking_delta": {
        const delta = event.payload?.content;
        if (typeof delta === "string") {
          const thinker = (event.payload?.sub_agent_id as string | undefined) ?? "main";
          this.appendThinkingDelta(items, event, taskId, delta, thinker);
          timelineChanged = true;
        }
        break;
      }

      case "agent.message": {
        const content = event.payload?.content;
        if (typeof content === "string" && content.trim()) {
          const trimmed = content.trim();

          // Deduplication logic:
          // 1. Check if the exact same content exists in the last 5 items (to handle interleaved status changes)
          // 2. This catches duplicate "Plan mode finished" messages sent as both plan_message and regular message
          const isDuplicate = items.slice(-5).some(
            (it) => it.type === "agentMessage" && it.content === trimmed
          );

          if (!isDuplicate) {
            items.push({
              id: event.id,
              type: "agentMessage",
              timestamp: event.created_at,
              seq: event.seq,
              content: trimmed,
              subAgentId: (event.payload?.sub_agent_id as string | undefined) ?? undefined,
            });
            timelineChanged = true;
          }
        }
        break;
      }

      case "agent.message_delta": {
        const delta = event.payload?.content;
        if (typeof delta === "string") {
          const msgId = `agent-delta-${taskId}`;
          const existingIdx = items.findIndex(
            (it) => it.type === "agentMessage" && it.id === msgId
          );
          const prevContent = existingIdx >= 0 ? (items[existingIdx].content ?? "") : "";
          const nextItem: ConversationItem = {
            id: msgId,
            type: "agentMessage",
            timestamp: event.created_at,
            seq: event.seq,
            content: `${prevContent}${delta}`,
          };
          if (existingIdx >= 0) {
            items[existingIdx] = nextItem;
          } else {
            items.push(nextItem);
          }
          timelineChanged = true;
        }
        break;
      }

      // ---------------------------------------------------------------
      // Tool call events
      // ---------------------------------------------------------------
      case "tool.call_started": {
        const toolName = event.payload?.tool_name as string | undefined;
        const rationale = event.payload?.rationale as string | undefined;
        const toolCallId = event.payload?.tool_call_id as string | undefined;
        const subAgentId = event.payload?.sub_agent_id as string | undefined;

        const item: ConversationItem = {
          id: event.id,
          type: "toolCall",
          timestamp: event.created_at,
          seq: event.seq,
          toolName: toolName ?? "unknown",
          toolArgs: (event.payload?.tool_args as Record<string, unknown> | undefined) ?? undefined,
          toolRationale: rationale,
          toolStatus: "running",
          subAgentId,
        };

        items.push(item);
        timelineChanged = true;

        // Track for later update
        if (toolCallId) {
          this.activeToolCalls.set(toolCallId, {
            taskId,
            itemIndex: items.length - 1,
          });
        }

        if (toolName === "git.apply_patch") {
          items.push({
            id: `${event.id}-patch`,
            type: "fileChange",
            timestamp: event.created_at,
            seq: event.seq + 0.0001,
            fileAction: "patch",
            subAgentId,
          });
        }
        break;
      }

      case "tool.call_finished": {
        const toolCallId = event.payload?.tool_call_id as string | undefined;
        const ok = event.payload?.status === "succeeded";
        const data = this.toDisplayText(event.payload?.output);
        const error = this.toDisplayText(event.payload?.error);

        if (toolCallId && this.activeToolCalls.has(toolCallId)) {
          const ref = this.activeToolCalls.get(toolCallId)!;
          const item = items[ref.itemIndex];
          if (item && item.type === "toolCall") {
            item.toolStatus = ok ? "success" : "error";
            item.toolResult = data;
            item.toolError = error;

            if (item.toolName === "agent.todo" && ok) {
              const agentId = item.subAgentId ?? "main";
              this.upsertAgentTodos(taskId, agentId, event.created_at, event.payload?.output);
            }
          }
          this.activeToolCalls.delete(toolCallId);
        } else {
          // Fallback: find by matching tool_call_id in items or add as new
          items.push({
            id: event.id,
            type: "toolCall",
            timestamp: event.created_at,
            seq: event.seq,
            toolName: (event.payload?.tool_name as string) ?? "unknown",
            toolStatus: ok ? "success" : "error",
            toolResult: data,
            toolError: error,
          });
        }
        timelineChanged = true;
        break;
      }

      // ---------------------------------------------------------------
      // Sub-agent events
      // ---------------------------------------------------------------
      case "agent.subagent_created": {
        const subAgentId = event.payload?.sub_agent_id as string | undefined;
        const stepIdx = event.payload?.step_idx as number | undefined;
        items.push({
          id: event.id,
          type: "statusChange",
          timestamp: event.created_at,
          seq: event.seq,
          status: "created",
          subAgentId,
          content: `Step ${(stepIdx ?? 0) + 1} delegated`,
        });
        timelineChanged = true;
        break;
      }

      case "agent.subagent_started": {
        const subAgentId = event.payload?.sub_agent_id as string | undefined;
        const stepIdx = event.payload?.step_idx as number | undefined;
        items.push({
          id: event.id,
          type: "statusChange",
          timestamp: event.created_at,
          seq: event.seq,
          status: "executing",
          subAgentId,
          content: `Step ${(stepIdx ?? 0) + 1} started`,
        });
        timelineChanged = true;
        break;
      }

      case "agent.subagent_completed": {
        const subAgentId = event.payload?.sub_agent_id as string | undefined;
        const output = event.payload?.output_path as string | undefined;
        items.push({
          id: event.id,
          type: "statusChange",
          timestamp: event.created_at,
          seq: event.seq,
          status: "completed",
          subAgentId,
          content: output ? `Completed. Output: ${output}` : "Step completed",
        });
        timelineChanged = true;
        break;
      }

      case "agent.subagent_waiting_for_merge": {
        const subAgentId = event.payload?.sub_agent_id as string | undefined;
        items.push({
          id: event.id,
          type: "statusChange",
          timestamp: event.created_at,
          seq: event.seq,
          status: "waiting_for_merge",
          subAgentId,
          content: "Waiting for parent integration",
        });
        timelineChanged = true;
        break;
      }

      case "agent.subagent_failed": {
        const subAgentId = event.payload?.sub_agent_id as string | undefined;
        const error = event.payload?.error as string | undefined;
        items.push({
          id: event.id,
          type: "error",
          timestamp: event.created_at,
          seq: event.seq,
          errorMessage: error ?? "Sub-agent failed",
          subAgentId,
        });
        timelineChanged = true;
        break;
      }

      case "agent.subagent_attempt": {
        // Retry attempt — don't clutter conversation, just update status
        const attempt = event.payload?.attempt as number | undefined;
        const subAgentId = event.payload?.sub_agent_id as string | undefined;
        if (attempt && attempt > 1) {
          items.push({
            id: event.id,
            type: "statusChange",
            timestamp: event.created_at,
            seq: event.seq,
            status: "retrying",
            subAgentId,
            content: `Retry attempt ${attempt}`,
          });
          timelineChanged = true;
        }
        break;
      }

      case "agent.worktree_merged": {
        items.push({
          id: event.id,
          type: "statusChange",
          timestamp: event.created_at,
          seq: event.seq,
          status: "merged",
          content: "Changes merged into main branch",
        });
        timelineChanged = true;
        break;
      }

      case "agent.subagent_closed": {
        const subAgentId = event.payload?.sub_agent_id as string | undefined;
        const finalStatus = event.payload?.final_status as string | undefined;
        const closeReason = event.payload?.close_reason as string | undefined;
        items.push({
          id: event.id,
          type: "statusChange",
          timestamp: event.created_at,
          seq: event.seq,
          status: "closed",
          subAgentId,
          content: finalStatus
            ? `Closed (${finalStatus}${closeReason ? `: ${closeReason}` : ""})`
            : "Sub-agent closed",
        });
        timelineChanged = true;
        break;
      }

      // ---------------------------------------------------------------
      // Status changes
      // ---------------------------------------------------------------
      case "task.status_changed": {
        const status = event.payload?.status as string | undefined;
        if (status) {
          items.push({
            id: event.id,
            type: "statusChange",
            timestamp: event.created_at,
            seq: event.seq,
            status,
          });
          timelineChanged = true;
        }
        break;
      }

      case "agent.planning_started": {
        items.push({
          id: event.id,
          type: "statusChange",
          timestamp: event.created_at,
          seq: event.seq,
          status: "planning",
          content: "Generating execution plan...",
        });
        timelineChanged = true;
        break;
      }

      case "agent.subagents_scheduled": {
        const count = event.payload?.count as number | undefined;
        items.push({
          id: event.id,
          type: "statusChange",
          timestamp: event.created_at,
          seq: event.seq,
          status: "scheduled",
          content: count ? `${count} sub-agents scheduled` : "Sub-agents scheduled",
        });
        timelineChanged = true;
        break;
      }

      // ---------------------------------------------------------------
      // Artifacts
      // ---------------------------------------------------------------
      case "artifact.created": {
        const kind = event.payload?.kind as string | undefined;
        const uri = event.payload?.uri as string | undefined;
        if (kind && uri) {
          items.push({
            id: event.id,
            type: "fileChange",
            timestamp: event.created_at,
            seq: event.seq,
            filePath: uri,
            fileAction: "write",
            content: `Artifact created: ${kind}`,
          });
          timelineChanged = true;
        }
        break;
      }

      default:
        // Unknown event — skip, don't pollute the conversation
        break;
    }

    // Trim to max size
    if (items.length > MAX_ITEMS_PER_TASK) {
      items.splice(0, items.length - MAX_ITEMS_PER_TASK);
    }
    this.itemsByTask.set(taskId, items);

    return { planChanged, timelineChanged };
  }

  // -----------------------------------------------------------------------
  // Public accessors
  // -----------------------------------------------------------------------

  getItems(taskId: string): ConversationItem[] {
    // Return a shallow copy so React detects a new array reference
    return [...(this.itemsByTask.get(taskId) ?? [])];
  }

  getPlan(taskId: string): PlanData | null {
    return this.planByTask.get(taskId) ?? null;
  }

  getAssistantMessage(taskId: string): string | null {
    return this.assistantMessageByTask.get(taskId) ?? null;
  }

  getPlanStream(taskId: string): string | null {
    return this.planStreamByTask.get(taskId) ?? null;
  }

  getRawEvents(taskId: string): BusEvent[] {
    return this.rawEventsByTask.get(taskId) ?? [];
  }

  getAgentTodos(taskId: string): AgentTodoList[] {
    const byAgent = this.agentTodosByTask.get(taskId);
    if (!byAgent) return [];

    return [...byAgent.values()].sort((a, b) => {
      if (a.agentId === "main") return -1;
      if (b.agentId === "main") return 1;
      return a.agentId.localeCompare(b.agentId);
    });
  }

  clearTask(taskId: string) {
    this.itemsByTask.delete(taskId);
    this.planByTask.delete(taskId);
    this.assistantMessageByTask.delete(taskId);
    this.planStreamByTask.delete(taskId);
    this.seenEventIdsByTask.delete(taskId);
    this.rawEventsByTask.delete(taskId);
    this.agentTodosByTask.delete(taskId);
  }

  // Legacy compat — kept so store.ts event listener still works
  getTimeline(taskId: string): ConversationItem[] {
    return this.getItems(taskId);
  }

  getPlanMarkdown(taskId: string, prompt: string, status: string): string {
    const plan = this.planByTask.get(taskId);
    const assistantMessage = this.assistantMessageByTask.get(taskId);

    if (plan) {
      return [
        assistantMessage ?? "I drafted an execution plan for this task.",
        "",
        `**Goal:** ${plan.goalSummary}`,
        `**Steps:** ${plan.steps.length}`,
        `**Status:** ${status}`,
      ].join("\n");
    }

    const stream = this.planStreamByTask.get(taskId);
    if (stream) {
      return stream;
    }

    return assistantMessage ?? `Working on: ${prompt}`;
  }
}

export const runtimeEventBuffer = new RuntimeEventBuffer();
