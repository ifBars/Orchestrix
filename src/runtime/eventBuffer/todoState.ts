/**
 * Agent todo state per task.
 */

import type { AgentTodoItem, AgentTodoList } from "./types";

function normalizeTodoItem(item: unknown, idx: number): AgentTodoItem | null {
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
  return { id: idCandidate, content, status, priority };
}

function parseTodoItems(output: unknown): AgentTodoItem[] {
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
    .map((item, idx) => normalizeTodoItem(item, idx))
    .filter((item): item is AgentTodoItem => item != null);
}

export class TodoState {
  private agentTodosByTask = new Map<string, Map<string, AgentTodoList>>();

  upsertAgentTodos(taskId: string, agentId: string, updatedAt: string, output: unknown): void {
    const todos = parseTodoItems(output);
    const byAgent = this.agentTodosByTask.get(taskId) ?? new Map<string, AgentTodoList>();
    byAgent.set(agentId, { agentId, todos, updatedAt });
    this.agentTodosByTask.set(taskId, byAgent);
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

  clearTask(taskId: string): void {
    this.agentTodosByTask.delete(taskId);
  }
}
