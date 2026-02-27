import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { UserQuestionRequestView } from "@/types";

export function usePendingQuestions(taskId: string, status: string) {
  const [pendingQuestions, setPendingQuestions] = useState<UserQuestionRequestView[]>([]);

  useEffect(() => {
    let disposed = false;
    let timer: number | null = null;

    const fetchPendingQuestions = async () => {
      try {
        const questions = await invoke<UserQuestionRequestView[]>(
          "list_pending_questions",
          { taskId }
        );
        if (!disposed) {
          setPendingQuestions(questions);
        }
      } catch (error) {
        console.error("Failed to fetch pending questions", error);
      }
    };

    fetchPendingQuestions();
    if (status === "executing" || status === "planning") {
      timer = window.setInterval(fetchPendingQuestions, 1200);
    }

    return () => {
      disposed = true;
      if (timer != null) {
        window.clearInterval(timer);
      }
    };
  }, [taskId, status]);

  return pendingQuestions;
}
