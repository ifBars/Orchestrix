export const queryKeys = {
  pendingApprovals: (taskId: string) => ["task", taskId, "pending-approvals"] as const,
  pendingQuestions: (taskId: string) => ["task", taskId, "pending-questions"] as const,
  executionSummary: (taskId: string, status: string) =>
    ["task", taskId, "execution-summary", status] as const,
  taskContextSnapshot: (taskId: string | null, timelineTick: number) =>
    ["task", taskId, "context-snapshot", timelineTick] as const,
};
