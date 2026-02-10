/**
 * Plan-related state: plan, plan stream, assistant message per task.
 */

import type { PlanData } from "./types";

export class PlanState {
  private planByTask = new Map<string, PlanData>();
  private planStreamByTask = new Map<string, string>();
  private assistantMessageByTask = new Map<string, string>();

  getPlan(taskId: string): PlanData | null {
    return this.planByTask.get(taskId) ?? null;
  }

  setPlan(taskId: string, plan: PlanData | null): void {
    if (plan) this.planByTask.set(taskId, plan);
    else this.planByTask.delete(taskId);
  }

  getPlanStream(taskId: string): string {
    return this.planStreamByTask.get(taskId) ?? "";
  }

  appendPlanStream(taskId: string, delta: string): void {
    const prev = this.planStreamByTask.get(taskId) ?? "";
    this.planStreamByTask.set(taskId, prev + delta);
  }

  getAssistantMessage(taskId: string): string {
    return this.assistantMessageByTask.get(taskId) ?? "";
  }

  setAssistantMessage(taskId: string, msg: string): void {
    this.assistantMessageByTask.set(taskId, msg);
  }

  clearTask(taskId: string): void {
    this.planByTask.delete(taskId);
    this.planStreamByTask.delete(taskId);
    this.assistantMessageByTask.delete(taskId);
  }
}
