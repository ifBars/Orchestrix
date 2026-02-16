/**
 * Public types and handler contract for the event buffer.
 */

import type { BusEvent } from "@/types";

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
  content?: string;
  stepIndex?: number;
  stepTitle?: string;
  stepDescription?: string;
  toolName?: string;
  toolArgs?: Record<string, unknown>;
  toolRationale?: string;
  toolStatus?: "running" | "success" | "error";
  toolResult?: string;
  toolError?: string;
  toolDurationMs?: number;
  filePath?: string;
  fileAction?: "write" | "patch" | "delete";
  status?: string;
  previousStatus?: string;
  errorMessage?: string;
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

export interface AgentMessageStream {
  streamId: string;
  content: string;
  startedAt: string;
  updatedAt: string;
  seq: number;
  isStreaming: boolean;
  subAgentId?: string;
  stepIdx?: number;
  turn?: number;
}

export interface HandlerResult {
  planChanged: boolean;
  timelineChanged: boolean;
  agentStreamChanged?: boolean;
}

/** Context passed to each event handler. Handlers mutate state via these references. */
export interface HandlerContext {
  event: BusEvent;
  taskId: string;
  /** Mutable items array for this task; handler may push/splice. */
  items: ConversationItem[];
  getPlan: () => PlanData | null;
  setPlan: (plan: PlanData | null) => void;
  getPlanStream: () => string;
  appendPlanStream: (delta: string) => void;
  getAssistantMessage: () => string;
  setAssistantMessage: (msg: string) => void;
  setActiveToolCall: (toolCallId: string, taskId: string, itemIndex: number) => void;
  getActiveToolCall: (toolCallId: string) => { taskId: string; itemIndex: number } | undefined;
  deleteActiveToolCall: (toolCallId: string) => void;
  pushRawEvent: (event: BusEvent) => void;
  upsertAgentTodos: (agentId: string, updatedAt: string, output: unknown) => void;
  removeLastTransient: () => void;
  setLastTransientId: (id: string | null) => void;
  getLastTransientId: () => string | null;
  toDisplayText: (value: unknown) => string | undefined;
  appendThinkingDelta: (delta: string, thinker?: string) => void;
  startAgentMessageStream: (params: {
    streamId: string;
    createdAt: string;
    seq: number;
    subAgentId?: string;
    stepIdx?: number;
    turn?: number;
  }) => void;
  appendAgentMessageDelta: (params: {
    streamId?: string;
    delta: string;
    createdAt: string;
    seq: number;
    subAgentId?: string;
    stepIdx?: number;
    turn?: number;
  }) => void;
  completeAgentMessageStream: (streamId?: string, completedAt?: string, seq?: number) => void;
  clearAgentMessageStream: () => void;
  flushAgentMessageStream: () => AgentMessageStream | null;
}

export type EventHandler = (ctx: HandlerContext) => HandlerResult;
