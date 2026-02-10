/**
 * Runtime event buffer: converts raw BusEvents into structured conversation items.
 * Public API preserved for @/runtime/eventBuffer imports.
 */

import { RuntimeEventBuffer } from "./RuntimeEventBuffer";

export const runtimeEventBuffer = new RuntimeEventBuffer();
export { RuntimeEventBuffer };
export type {
  ConversationItem,
  ConversationItemType,
  PlanData,
  AgentTodoItem,
  AgentTodoList,
  HandlerResult,
  HandlerContext,
  EventHandler,
} from "./types";
