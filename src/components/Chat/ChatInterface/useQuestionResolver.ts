import { useCallback } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "@/lib/queryKeys";
import type { UserQuestionAnswer } from "@/types";

export function useQuestionResolver(
  taskId: string,
  setResolvingQuestionId: (id: string | null) => void
) {
  const queryClient = useQueryClient();
  const resolveQuestionMutation = useMutation({
    mutationFn: async ({
      questionId,
      answer,
    }: {
      questionId: string;
      answer: UserQuestionAnswer;
    }) => {
      setResolvingQuestionId(questionId);
      await invoke("resolve_question", { questionId, answer });
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.pendingQuestions(taskId) });
    },
    onError: (error) => {
      console.error("Failed to resolve user question", error);
    },
    onSettled: () => {
      setResolvingQuestionId(null);
    },
  });

  const resolveQuestion = useCallback(
    async (questionId: string, answer: UserQuestionAnswer) => {
      await resolveQuestionMutation.mutateAsync({ questionId, answer });
    },
    [resolveQuestionMutation]
  );

  return resolveQuestion;
}
