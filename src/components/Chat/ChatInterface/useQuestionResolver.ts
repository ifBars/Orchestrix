import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { UserQuestionAnswer, UserQuestionRequestView } from "@/types";

export function useQuestionResolver(
  taskId: string,
  setPendingQuestions: (questions: UserQuestionRequestView[]) => void,
  setResolvingQuestionId: (id: string | null) => void
) {
  const resolveQuestion = useCallback(
    async (questionId: string, answer: UserQuestionAnswer) => {
      try {
        setResolvingQuestionId(questionId);
        await invoke("resolve_question", { questionId, answer });
        const questions = await invoke<UserQuestionRequestView[]>(
          "list_pending_questions",
          { taskId }
        );
        setPendingQuestions(questions);
      } catch (error) {
        console.error("Failed to resolve user question", error);
      } finally {
        setResolvingQuestionId(null);
      }
    },
    [taskId, setPendingQuestions, setResolvingQuestionId]
  );

  return resolveQuestion;
}
