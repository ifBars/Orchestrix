import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "@/lib/queryKeys";
import type { UserQuestionRequestView } from "@/types";

export function usePendingQuestions(taskId: string, status: string) {
  const { data = [] } = useQuery({
    queryKey: queryKeys.pendingQuestions(taskId),
    queryFn: () => invoke<UserQuestionRequestView[]>("list_pending_questions", { taskId }),
    enabled: taskId.length > 0,
    refetchInterval: status === "executing" || status === "planning" ? 1200 : false,
  });

  return data;
}
