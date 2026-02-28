import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTaskTimelineTick } from "@/stores/streamStore";
import type { TaskContextSnapshotView } from "@/types";

export function useTaskContextSnapshot(taskId: string | null) {
  const [snapshot, setSnapshot] = useState<TaskContextSnapshotView | null>(null);
  const timelineTick = useTaskTimelineTick(taskId);
  const lastSnapshotRef = useRef<TaskContextSnapshotView | null>(null);

  useEffect(() => {
    let disposed = false;

    const fetchSnapshot = async () => {
      if (!taskId) {
        if (!disposed) {
          setSnapshot(null);
          lastSnapshotRef.current = null;
        }
        return;
      }

      try {
        const next = await invoke<TaskContextSnapshotView>("get_task_context_snapshot", {
          taskId,
        });
        if (!disposed) {
          setSnapshot(next);
          lastSnapshotRef.current = next;
        }
      } catch (error) {
        if (!disposed) {
          setSnapshot(null);
          lastSnapshotRef.current = null;
        }
        console.error("Failed to load task context snapshot", error);
      }
    };

    fetchSnapshot();

    return () => {
      disposed = true;
    };
  }, [taskId, timelineTick]);

  return snapshot;
}
