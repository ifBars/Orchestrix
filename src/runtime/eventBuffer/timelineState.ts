/**
 * Timeline state: items per task, active tool calls, raw events, transient id.
 */

import type { BusEvent } from "@/types";
import type { ConversationItem } from "./types";

const MAX_RAW_EVENTS = 400;

export class TimelineState {
  private itemsByTask = new Map<string, ConversationItem[]>();
  private activeToolCalls = new Map<string, { taskId: string; itemIndex: number }>();
  private seenEventIdsByTask = new Map<string, Set<string>>();
  private rawEventsByTask = new Map<string, BusEvent[]>();
  private lastTransientIdByTask = new Map<string, string | null>();

  getItems(taskId: string): ConversationItem[] {
    let items = this.itemsByTask.get(taskId);
    if (!items) {
      items = [];
      this.itemsByTask.set(taskId, items);
    }
    return items;
  }

  hasSeen(taskId: string, eventId: string): boolean {
    return this.seenEventIdsByTask.get(taskId)?.has(eventId) ?? false;
  }

  addSeen(taskId: string, eventId: string): void {
    let set = this.seenEventIdsByTask.get(taskId);
    if (!set) {
      set = new Set();
      this.seenEventIdsByTask.set(taskId, set);
    }
    set.add(eventId);
  }

  pushRawEvent(taskId: string, event: BusEvent): void {
    const raw = this.rawEventsByTask.get(taskId) ?? [];
    raw.push(event);
    if (raw.length > MAX_RAW_EVENTS) raw.splice(0, raw.length - MAX_RAW_EVENTS);
    this.rawEventsByTask.set(taskId, raw);
  }

  getRawEvents(taskId: string): BusEvent[] {
    return this.rawEventsByTask.get(taskId) ?? [];
  }

  setActiveToolCall(toolCallId: string, taskId: string, itemIndex: number): void {
    this.activeToolCalls.set(toolCallId, { taskId, itemIndex });
  }

  getActiveToolCall(toolCallId: string): { taskId: string; itemIndex: number } | undefined {
    return this.activeToolCalls.get(toolCallId);
  }

  deleteActiveToolCall(toolCallId: string): void {
    this.activeToolCalls.delete(toolCallId);
  }

  getLastTransientId(taskId: string): string | null {
    return this.lastTransientIdByTask.get(taskId) ?? null;
  }

  setLastTransientId(taskId: string, id: string | null): void {
    this.lastTransientIdByTask.set(taskId, id);
  }

  removeLastTransient(taskId: string): boolean {
    const items = this.itemsByTask.get(taskId);
    const lastId = this.lastTransientIdByTask.get(taskId);
    if (!items?.length || !lastId) return false;
    const idx = items.findIndex((it) => it.id === lastId);
    if (idx >= 0) {
      items.splice(idx, 1);
      this.lastTransientIdByTask.set(taskId, null);
      return true;
    }
    return false;
  }

  trimToMax(taskId: string, max: number): void {
    const items = this.itemsByTask.get(taskId);
    if (items && items.length > max) items.splice(0, items.length - max);
  }

  clearTask(taskId: string): void {
    this.itemsByTask.delete(taskId);
    this.seenEventIdsByTask.delete(taskId);
    this.rawEventsByTask.delete(taskId);
    this.lastTransientIdByTask.delete(taskId);
    for (const [id, ref] of this.activeToolCalls.entries()) {
      if (ref.taskId === taskId) this.activeToolCalls.delete(id);
    }
  }
}
