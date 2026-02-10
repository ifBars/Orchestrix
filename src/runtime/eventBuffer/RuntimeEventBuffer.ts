/**
 * Orchestrator: owns state, resolves task id, builds handler context, and runs the registry.
 */

import type { BusEvent } from "@/types";
import { resolveTaskId } from "./taskResolution";
import { getHandler } from "./handlers";
import type { HandlerContext } from "./types";
import { TimelineState } from "./timelineState";
import { PlanState } from "./planState";
import { TodoState } from "./todoState";
import { toDisplayText, appendThinkingDelta } from "./handlers/utils";
import type { ConversationItem, PlanData, AgentTodoList } from "./types";

const MAX_ITEMS_PER_TASK = 500;

export class RuntimeEventBuffer {
  private runToTask = new Map<string, string>();
  private timelineState = new TimelineState();
  private planState = new PlanState();
  private todoState = new TodoState();

  resolveTaskId(event: BusEvent): string | null {
    return resolveTaskId(event, this.runToTask);
  }

  ingest(event: BusEvent, taskId: string): { planChanged: boolean; timelineChanged: boolean } {
    let planChanged = false;
    let timelineChanged = false;

    if (this.timelineState.hasSeen(taskId, event.id)) {
      return { planChanged, timelineChanged };
    }
    this.timelineState.addSeen(taskId, event.id);
    this.timelineState.pushRawEvent(taskId, event);

    const items = this.timelineState.getItems(taskId);
    const handler = getHandler(event.event_type);
    if (!handler) {
      this.timelineState.trimToMax(taskId, MAX_ITEMS_PER_TASK);
      return { planChanged, timelineChanged };
    }

    const ctx = this.buildContext(event, taskId, items);
    const result = handler(ctx);
    planChanged = result.planChanged;
    timelineChanged = result.timelineChanged;

    this.timelineState.trimToMax(taskId, MAX_ITEMS_PER_TASK);
    return { planChanged, timelineChanged };
  }

  private buildContext(
    event: BusEvent,
    taskId: string,
    items: ConversationItem[]
  ): HandlerContext {
    const self = this;
    return {
      event,
      taskId,
      items,
      getPlan: () => self.planState.getPlan(taskId),
      setPlan: (plan) => self.planState.setPlan(taskId, plan),
      getPlanStream: () => self.planState.getPlanStream(taskId),
      appendPlanStream: (delta) => self.planState.appendPlanStream(taskId, delta),
      getAssistantMessage: () => self.planState.getAssistantMessage(taskId),
      setAssistantMessage: (msg) => self.planState.setAssistantMessage(taskId, msg),
      setActiveToolCall: (toolCallId, tid, itemIndex) =>
        self.timelineState.setActiveToolCall(toolCallId, tid, itemIndex),
      getActiveToolCall: (id) => self.timelineState.getActiveToolCall(id),
      deleteActiveToolCall: (id) => self.timelineState.deleteActiveToolCall(id),
      pushRawEvent: (ev) => self.timelineState.pushRawEvent(taskId, ev),
      upsertAgentTodos: (agentId, updatedAt, output) =>
        self.todoState.upsertAgentTodos(taskId, agentId, updatedAt, output),
      removeLastTransient: () => self.timelineState.removeLastTransient(taskId),
      setLastTransientId: (id) => self.timelineState.setLastTransientId(taskId, id),
      getLastTransientId: () => self.timelineState.getLastTransientId(taskId),
      toDisplayText,
      appendThinkingDelta: (delta, thinker) =>
        appendThinkingDelta(items, event, taskId, delta, thinker),
    };
  }

  getItems(taskId: string): ConversationItem[] {
    return [...this.timelineState.getItems(taskId)];
  }

  getPlan(taskId: string): PlanData | null {
    return this.planState.getPlan(taskId);
  }

  getAssistantMessage(taskId: string): string | null {
    const msg = this.planState.getAssistantMessage(taskId);
    return msg || null;
  }

  getPlanStream(taskId: string): string | null {
    const stream = this.planState.getPlanStream(taskId);
    return stream || null;
  }

  getRawEvents(taskId: string): BusEvent[] {
    return this.timelineState.getRawEvents(taskId);
  }

  getAgentTodos(taskId: string): AgentTodoList[] {
    return this.todoState.getAgentTodos(taskId);
  }

  clearTask(taskId: string): void {
    this.timelineState.clearTask(taskId);
    this.planState.clearTask(taskId);
    this.todoState.clearTask(taskId);
  }

  getTimeline(taskId: string): ConversationItem[] {
    return this.getItems(taskId);
  }

  getPlanMarkdown(taskId: string, prompt: string, status: string): string {
    const plan = this.planState.getPlan(taskId);
    const assistantMessage = this.planState.getAssistantMessage(taskId);

    if (plan) {
      return [
        assistantMessage || "I drafted an execution plan for this task.",
        "",
        `**Goal:** ${plan.goalSummary}`,
        `**Steps:** ${plan.steps.length}`,
        `**Status:** ${status}`,
      ].join("\n");
    }

    const stream = this.planState.getPlanStream(taskId);
    if (stream) return stream;

    return assistantMessage || `Working on: ${prompt}`;
  }
}
