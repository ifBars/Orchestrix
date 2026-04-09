import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "@/lib/queryKeys";
import type { RunRow, SubAgentRow, ToolCallRow } from "@/types";

export type ExecutionSummary = {
  totalSteps: number;
  completedSteps: number;
  failedSteps: number;
  runningStep: number | null;
  runningTool: string | null;
};

export function useExecutionSummary(taskId: string, status: string) {
  const { data = null } = useQuery({
    queryKey: queryKeys.executionSummary(taskId, status),
    queryFn: async (): Promise<ExecutionSummary | null> => {
      if (status !== "executing") {
        return null;
      }

      const run = await invoke<RunRow | null>("get_latest_run", { taskId });
      if (!run?.id) {
        return null;
      }

      const [subAgents, toolCalls] = await Promise.all([
        invoke<SubAgentRow[]>("list_sub_agents", { runId: run.id }),
        invoke<ToolCallRow[]>("list_tool_calls", { runId: run.id }),
      ]);

      const completedSteps = subAgents.filter((item) => item.status === "completed").length;
      const failedSteps = subAgents.filter((item) => item.status === "failed").length;
      const runningSubAgent =
        subAgents
          .filter((item) => item.status === "running")
          .sort((a, b) => a.step_idx - b.step_idx)[0] ?? null;
      const runningTools = toolCalls
        .filter((item) => item.status === "running")
        .sort((a, b) => (a.started_at ?? "").localeCompare(b.started_at ?? ""));
      const runningTool = runningTools.length > 0 ? runningTools[runningTools.length - 1] : null;

      return {
        totalSteps: subAgents.length,
        completedSteps,
        failedSteps,
        runningStep: runningSubAgent?.step_idx ?? null,
        runningTool: runningTool?.tool_name ?? null,
      };
    },
    enabled: taskId.length > 0,
    refetchInterval: status === "executing" ? 1500 : false,
  });

  return data;
}
