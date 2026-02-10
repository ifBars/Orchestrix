import type { EventHandler } from "../types";
import * as planning from "./planning";
import * as agent from "./agent";
import * as tool from "./tool";
import * as subagent from "./subagent";
import * as task from "./task";
import * as artifact from "./artifact";

export const eventHandlers: Record<string, EventHandler> = {
  "agent.plan_ready": planning.handlePlanReady,
  "agent.plan_message": planning.handlePlanMessage,
  "agent.plan_delta": planning.handlePlanDelta,
  "agent.plan_thinking_delta": planning.handlePlanThinkingDelta,
  "agent.planning_started": planning.handlePlanningStarted,

  "agent.thinking_delta": agent.handleThinkingDelta,
  "agent.message": agent.handleMessage,
  "agent.message_delta": agent.handleMessageDelta,
  "agent.deciding": agent.handleDeciding,
  "agent.tool_calls_preparing": agent.handleToolCallsPreparing,
  "agent.subagents_scheduled": agent.handleSubagentsScheduled,

  "tool.call_started": tool.handleToolCallStarted,
  "tool.call_finished": tool.handleToolCallFinished,

  "agent.subagent_created": subagent.handleSubagentCreated,
  "agent.subagent_started": subagent.handleSubagentStarted,
  "agent.subagent_completed": subagent.handleSubagentCompleted,
  "agent.subagent_waiting_for_merge": subagent.handleSubagentWaitingForMerge,
  "agent.subagent_failed": subagent.handleSubagentFailed,
  "agent.subagent_attempt": subagent.handleSubagentAttempt,
  "agent.worktree_merged": subagent.handleWorktreeMerged,
  "agent.subagent_closed": subagent.handleSubagentClosed,

  "task.status_changed": task.handleTaskStatusChanged,
  "user.message_sent": task.handleUserMessageSent,

  "artifact.created": artifact.handleArtifactCreated,
};

export function getHandler(eventType: string): EventHandler | undefined {
  return eventHandlers[eventType];
}
