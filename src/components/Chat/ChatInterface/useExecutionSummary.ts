import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { RunRow, SubAgentRow, ToolCallRow } from "@/types";

export type ExecutionSummary = {
  totalSteps: number;
  completedSteps: number;
  failedSteps: number;
  runningStep: number | null;
  runningTool: string | null;
};

export function useExecutionSummary(taskId: string, status: string) {
  const [executionSummary, setExecutionSummary] = useState<ExecutionSummary | null>(
    null
  );

  useEffect(() => {
    let disposed = false;
    let timer: number | null = null;

    const fetchExecutionSummary = async () => {
      if (status !== "executing") {
        if (!disposed) setExecutionSummary(null);
        return;
      }

      try {
        const run = await invoke<RunRow | null>("get_latest_run", { taskId });
        if (!run?.id) {
          if (!disposed) setExecutionSummary(null);
          return;
        }

        const [subAgents, toolCalls] = await Promise.all([
          invoke<SubAgentRow[]>("list_sub_agents", { runId: run.id }),
          invoke<ToolCallRow[]>("list_tool_calls", { runId: run.id }),
        ]);

        const completedSteps = subAgents.filter(
          (item) => item.status === "completed"
        ).length;
        const failedSteps = subAgents.filter((item) => item.status === "failed").length;
        const runningSubAgent =
          subAgents
            .filter((item) => item.status === "running")
            .sort((a, b) => a.step_idx - b.step_idx)[0] ?? null;
        const runningTools = toolCalls
          .filter((item) => item.status === "running")
          .sort((a, b) =>
            (a.started_at ?? "").localeCompare(b.started_at ?? "")
          );
        const runningTool =
          runningTools.length > 0 ? runningTools[runningTools.length - 1] : null;

        if (!disposed) {
          setExecutionSummary({
            totalSteps: subAgents.length,
            completedSteps,
            failedSteps,
            runningStep: runningSubAgent?.step_idx ?? null,
            runningTool: runningTool?.tool_name ?? null,
          });
        }
      } catch (error) {
        console.error("Failed to fetch execution summary", error);
      }
    };

    fetchExecutionSummary();
    if (status === "executing") {
      timer = window.setInterval(fetchExecutionSummary, 1500);
    }

    return () => {
      disposed = true;
      if (timer != null) {
        window.clearInterval(timer);
      }
    };
  }, [taskId, status]);

  return executionSummary;
}
