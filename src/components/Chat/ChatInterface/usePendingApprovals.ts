import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ApprovalRequestView } from "@/types";

export function usePendingApprovals(taskId: string, status: string) {
  const [pendingApprovals, setPendingApprovals] = useState<ApprovalRequestView[]>(
    []
  );

  useEffect(() => {
    let disposed = false;
    let timer: number | null = null;

    const fetchPendingApprovals = async () => {
      try {
        const approvals = await invoke<ApprovalRequestView[]>(
          "list_pending_approvals",
          { taskId }
        );
        if (!disposed) {
          setPendingApprovals(approvals);
        }
      } catch (error) {
        console.error("Failed to fetch pending approvals", error);
      }
    };

    fetchPendingApprovals();
    if (status === "executing") {
      timer = window.setInterval(fetchPendingApprovals, 1200);
    }

    return () => {
      disposed = true;
      if (timer != null) {
        window.clearInterval(timer);
      }
    };
  }, [taskId, status]);

  return pendingApprovals;
}
