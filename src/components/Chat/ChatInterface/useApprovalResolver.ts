import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ApprovalRequestView } from "@/types";

export function useApprovalResolver(
  taskId: string,
  setPendingApprovals: (approvals: ApprovalRequestView[]) => void,
  setResolvingApprovalId: (id: string | null) => void
) {
  const resolveApproval = useCallback(
    async (approvalId: string, approve: boolean) => {
      try {
        setResolvingApprovalId(approvalId);
        await invoke("resolve_approval_request", { approvalId, approve });
        const approvals = await invoke<ApprovalRequestView[]>(
          "list_pending_approvals",
          { taskId }
        );
        setPendingApprovals(approvals);
      } catch (error) {
        console.error("Failed to resolve approval request", error);
      } finally {
        setResolvingApprovalId(null);
      }
    },
    [taskId, setPendingApprovals, setResolvingApprovalId]
  );

  return resolveApproval;
}
