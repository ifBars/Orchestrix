import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "@/lib/queryKeys";
import type { ApprovalRequestView } from "@/types";

export function usePendingApprovals(taskId: string, status: string) {
  const { data = [] } = useQuery({
    queryKey: queryKeys.pendingApprovals(taskId),
    queryFn: () => invoke<ApprovalRequestView[]>("list_pending_approvals", { taskId }),
    enabled: taskId.length > 0,
    refetchInterval: status === "executing" ? 1200 : false,
  });

  return data;
}
