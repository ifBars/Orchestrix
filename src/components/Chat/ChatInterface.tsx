import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useShallow } from "zustand/shallow";
import { runtimeEventBuffer, type ConversationItem } from "@/runtime/eventBuffer";
import { useTaskPlanTick, useTaskTimelineTick } from "@/stores/streamStore";
import type { ApprovalRequestView, RunRow, SubAgentRow, TaskRow, ToolCallRow } from "@/types";
import { useAppStore } from "@/stores/appStore";
import { ConversationTimeline } from "./ConversationTimeline";
import { ReviewWorkspace } from "./ReviewWorkspace";
import { useArtifactReview } from "@/hooks/useArtifactReview";

type ChatInterfaceProps = {
  task: TaskRow;
};

export function ChatInterface({ task }: ChatInterfaceProps) {
  const [tasks, taskLinksByTask, artifactsByTask, selectTask, approvePlan, submitPlanFeedback, cancelTask] = useAppStore(
    useShallow((state) => [
      state.tasks,
      state.taskLinksByTask,
      state.artifactsByTask,
      state.selectTask,
      state.approvePlan,
      state.submitPlanFeedback,
      state.cancelTask,
    ])
  );

  const [submittingReview, setSubmittingReview] = useState(false);
  const [approving, setApproving] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [activeTab, setActiveTab] = useState<"chat" | "review">("chat");
  const [executionSummary, setExecutionSummary] = useState<{
    totalSteps: number;
    completedSteps: number;
    failedSteps: number;
    runningStep: number | null;
    runningTool: string | null;
  } | null>(null);
  const [pendingApprovals, setPendingApprovals] = useState<ApprovalRequestView[]>([]);
  const [resolvingApprovalId, setResolvingApprovalId] = useState<string | null>(null);

  const planTick = useTaskPlanTick(task.id);
  const timelineTick = useTaskTimelineTick(task.id);

  const items = useMemo(() => runtimeEventBuffer.getItems(task.id), [task.id, timelineTick, planTick]);
  const plan = useMemo(() => runtimeEventBuffer.getPlan(task.id), [task.id, planTick]);
  const assistantMessage = useMemo(() => runtimeEventBuffer.getAssistantMessage(task.id), [task.id, planTick]);
  const planStream = useMemo(() => runtimeEventBuffer.getPlanStream(task.id), [task.id, planTick]);
  const rawEvents = useMemo(() => runtimeEventBuffer.getRawEvents(task.id), [task.id, timelineTick, planTick]);
  const agentTodos = useMemo(() => runtimeEventBuffer.getAgentTodos(task.id), [task.id, timelineTick, planTick]);

  const review = useArtifactReview(task.id, task.status, artifactsByTask);

  useEffect(() => {
    if (task.status === "awaiting_review" && review.markdownArtifacts.length > 0) {
      setActiveTab("review");
    }
  }, [task.status, review.markdownArtifacts.length]);

  useEffect(() => {
    let disposed = false;
    let timer: number | null = null;

    const fetchExecutionSummary = async () => {
      if (task.status !== "executing") {
        if (!disposed) setExecutionSummary(null);
        return;
      }

      try {
        const run = await invoke<RunRow | null>("get_latest_run", { taskId: task.id });
        if (!run?.id) {
          if (!disposed) setExecutionSummary(null);
          return;
        }

        const [subAgents, toolCalls] = await Promise.all([
          invoke<SubAgentRow[]>("list_sub_agents", { runId: run.id }),
          invoke<ToolCallRow[]>("list_tool_calls", { runId: run.id }),
        ]);

        const completedSteps = subAgents.filter((item) => item.status === "completed").length;
        const failedSteps = subAgents.filter((item) => item.status === "failed").length;
        const runningSubAgent =
          subAgents
            .filter((item) => item.status === "running")
            .sort((a, b) => a.step_idx - b.step_idx)[0] ?? null;
        const runningTools = toolCalls
          .filter((item) => item.status === "running")
          .sort((a, b) => (a.started_at ?? "").localeCompare(b.started_at ?? ""));
        const runningTool = runningTools.length > 0 ? runningTools[runningTools.length - 1] : null;

        if (!disposed) {
          setExecutionSummary({
            totalSteps: subAgents.length,
            completedSteps,
            failedSteps,
            runningStep: runningSubAgent?.step_idx ?? null,
            runningTool: runningTool?.tool_name ?? null,
          });
        }
      } catch (error) {
        console.error("Failed to fetch execution summary", error);
      }
    };

    fetchExecutionSummary();
    if (task.status === "executing") {
      timer = window.setInterval(fetchExecutionSummary, 1500);
    }

    return () => {
      disposed = true;
      if (timer != null) {
        window.clearInterval(timer);
      }
    };
  }, [task.id, task.status]);

  useEffect(() => {
    let disposed = false;
    let timer: number | null = null;

    const fetchPendingApprovals = async () => {
      try {
        const approvals = await invoke<ApprovalRequestView[]>("list_pending_approvals", { taskId: task.id });
        if (!disposed) {
          setPendingApprovals(approvals);
        }
      } catch (error) {
        console.error("Failed to fetch pending approvals", error);
      }
    };

    fetchPendingApprovals();
    if (task.status === "executing") {
      timer = window.setInterval(fetchPendingApprovals, 1200);
    }

    return () => {
      disposed = true;
      if (timer != null) {
        window.clearInterval(timer);
      }
    };
  }, [task.id, task.status]);

  const resolveApproval = async (approvalId: string, approve: boolean) => {
    try {
      setResolvingApprovalId(approvalId);
      await invoke("resolve_approval_request", { approvalId, approve });
      const approvals = await invoke<ApprovalRequestView[]>("list_pending_approvals", { taskId: task.id });
      setPendingApprovals(approvals);
    } catch (error) {
      console.error("Failed to resolve approval request", error);
    } finally {
      setResolvingApprovalId(null);
    }
  };

  const relatedTasks = useMemo(() => {
    const linkedIds = taskLinksByTask[task.id] ?? [];
    return linkedIds
      .map((id) => tasks.find((entry) => entry.id === id))
      .filter((entry): entry is TaskRow => !!entry);
  }, [task.id, taskLinksByTask, tasks]);

  const visibleItems = useMemo(() => {
    return items.filter((item) => {
      if (item.type === "statusChange" && !item.content && !item.subAgentId) return false;
      return true;
    });
  }, [items]);

  const isWorking = task.status === "planning" || task.status === "executing";
  const renderKey = (item: ConversationItem, idx: number) => `${item.id}-${item.seq}-${idx}`;

  const submitReview = async () => {
    const submission = review.buildReviewSubmission();
    if (!submission) {
      review.setShowGeneralReviewInput(true);
      return;
    }

    try {
      setSubmittingReview(true);
      await submitPlanFeedback(task.id, submission);
      review.setGeneralReviewText("");
      review.setShowGeneralReviewInput(false);
    } catch (error) {
      console.error("Failed to submit plan feedback", error);
    } finally {
      setSubmittingReview(false);
    }
  };

  const build = async () => {
    try {
      setApproving(true);
      await approvePlan(task.id);
    } catch (error) {
      console.error("Failed to approve plan", error);
    } finally {
      setApproving(false);
    }
  };

  const stop = async () => {
    try {
      setStopping(true);
      await cancelTask(task.id);
    } catch (error) {
      console.error("Failed to cancel task", error);
    } finally {
      setStopping(false);
    }
  };

  if (activeTab === "review") {
    return (
      <ReviewWorkspace
        markdownArtifacts={review.markdownArtifacts}
        selectedArtifactPath={review.selectedArtifactPath}
        onSelectArtifact={review.setSelectedArtifactPath}
        previewText={review.previewText}
        previewLines={review.previewLines}
        activeComments={review.activeComments}
        draftLine={review.draftLine}
        draftText={review.draftText}
        onDraftTextChange={review.setDraftText}
        draftAnchorTop={review.draftAnchorTop}
        reviewViewportRef={review.reviewViewportRef}
        lineButtonRefs={review.lineButtonRefs}
        draftTextareaRef={review.draftTextareaRef}
        onOpenCommentEditor={review.openCommentEditor}
        onSaveComment={review.saveComment}
        onCancelDraft={review.cancelDraft}
        onEditComment={review.startEditingComment}
        onDeleteComment={review.deleteComment}
        onBackToChat={() => setActiveTab("chat")}
        onSubmitReview={submitReview}
        onBuild={build}
        submittingReview={submittingReview}
        approving={approving}
        showGeneralReviewInput={review.showGeneralReviewInput}
        generalReviewText={review.generalReviewText}
        onGeneralReviewTextChange={review.setGeneralReviewText}
      />
    );
  }

  return (
    <ConversationTimeline
      task={task}
      relatedTasks={relatedTasks}
      onSelectTask={selectTask}
      plan={plan}
      planStream={planStream}
      assistantMessage={assistantMessage}
      visibleItems={visibleItems}
      renderKey={renderKey}
      isWorking={isWorking}
      onBuild={build}
      approving={approving}
      stopping={stopping}
      markdownArtifactCount={review.markdownArtifacts.length}
      executionSummary={executionSummary}
      rawEvents={rawEvents}
      agentTodos={agentTodos}
      pendingApprovals={pendingApprovals}
      resolvingApprovalId={resolvingApprovalId}
      onResolveApproval={resolveApproval}
      onStop={stop}
    />
  );
}
