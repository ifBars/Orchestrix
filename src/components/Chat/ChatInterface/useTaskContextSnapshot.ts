import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "@/lib/queryKeys";
import { useTaskTimelineTick } from "@/stores/streamStore";
import type { TaskContextSnapshotView } from "@/types";

export function useTaskContextSnapshot(taskId: string | null) {
  const timelineTick = useTaskTimelineTick(taskId);

  const { data = null } = useQuery({
    queryKey: queryKeys.taskContextSnapshot(taskId, timelineTick),
    queryFn: async (): Promise<TaskContextSnapshotView | null> => {
      if (!taskId) {
        return null;
      }

      return invoke<TaskContextSnapshotView>("get_task_context_snapshot", { taskId });
    },
    enabled: taskId !== null,
  });

  return data;
}
