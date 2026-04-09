import { useCallback } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "@/lib/queryKeys";

export function useApprovalResolver(
  taskId: string,
  setResolvingApprovalId: (id: string | null) => void
) {
  const queryClient = useQueryClient();
  const resolveApprovalMutation = useMutation({
    mutationFn: async ({ approvalId, approve }: { approvalId: string; approve: boolean }) => {
      setResolvingApprovalId(approvalId);
      await invoke("resolve_approval_request", { approvalId, approve });
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.pendingApprovals(taskId) });
    },
    onError: (error) => {
      console.error("Failed to resolve approval request", error);
    },
    onSettled: () => {
      setResolvingApprovalId(null);
    },
  });

  const resolveApproval = useCallback(
    async (approvalId: string, approve: boolean) => {
      await resolveApprovalMutation.mutateAsync({ approvalId, approve });
    },
    [resolveApprovalMutation]
  );

  return resolveApproval;
}
