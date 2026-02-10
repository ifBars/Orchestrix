import { useEffect, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/shallow";
import { runtimeEventBuffer } from "@/runtime/eventBuffer";
import { useTaskPlanTick, useTaskTimelineTick } from "@/stores/streamStore";
import { useAppStore } from "@/stores/appStore";
import { useArtifactReview } from "@/hooks/useArtifactReview";
import { useExecutionSummary } from "./useExecutionSummary";
import { usePendingApprovals } from "./usePendingApprovals";
import { useApprovalResolver } from "./useApprovalResolver";
import { ConversationTimeline } from "../ConversationTimeline";
import { ReviewWorkspace } from "../ReviewWorkspace";
import type { TaskRow } from "@/types";

type ChatInterfaceProps = {
  task: TaskRow;
  activeTab?: "chat" | "review";
  onActiveTabChange?: (tab: "chat" | "review") => void;
};

export function ChatInterface({
  task,
  activeTab: controlledActiveTab,
  onActiveTabChange,
}: ChatInterfaceProps) {
  const [tasks, taskLinksByTask, artifactsByTask, selectTask, approvePlan, submitPlanFeedback, cancelTask] =
    useAppStore(
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
  const [internalActiveTab, setInternalActiveTab] = useState<"chat" | "review">("chat");
  const [resolvingApprovalId, setResolvingApprovalId] = useState<string | null>(
    null
  );

  const draftAnchorRef = useRef<HTMLDivElement | null>(null);

  // Use controlled state if provided, otherwise use internal state
  const activeTab = controlledActiveTab ?? internalActiveTab;
  const setActiveTab = onActiveTabChange ?? setInternalActiveTab;

  const planTick = useTaskPlanTick(task.id);
  const timelineTick = useTaskTimelineTick(task.id);

  const items = useMemo(
    () => runtimeEventBuffer.getItems(task.id),
    [task.id, timelineTick, planTick]
  );
  const plan = useMemo(
    () => runtimeEventBuffer.getPlan(task.id),
    [task.id, planTick]
  );
  const assistantMessage = useMemo(
    () => runtimeEventBuffer.getAssistantMessage(task.id),
    [task.id, planTick]
  );
  const planStream = useMemo(
    () => runtimeEventBuffer.getPlanStream(task.id),
    [task.id, planTick]
  );
  const rawEvents = useMemo(
    () => runtimeEventBuffer.getRawEvents(task.id),
    [task.id, timelineTick, planTick]
  );
  const agentTodos = useMemo(
    () => runtimeEventBuffer.getAgentTodos(task.id),
    [task.id, timelineTick, planTick]
  );

  const review = useArtifactReview(task.id, task.status, artifactsByTask);
  const executionSummary = useExecutionSummary(task.id, task.status);
  const pendingApprovals = usePendingApprovals(task.id, task.status);
  const resolveApproval = useApprovalResolver(
    task.id,
    () => {},
    setResolvingApprovalId
  );

  useEffect(() => {
    if (task.status === "awaiting_review" && review.markdownArtifacts.length > 0) {
      setActiveTab("review");
    }
  }, [task.status, review.markdownArtifacts.length]);

  const relatedTasks = useMemo(() => {
    const linkedIds = taskLinksByTask[task.id] ?? [];
    return linkedIds
      .map((id) => tasks.find((entry) => entry.id === id))
      .filter((entry): entry is TaskRow => !!entry);
  }, [task.id, taskLinksByTask, tasks]);

  const visibleItems = useMemo(() => {
    return items.filter((item) => {
      if (item.type === "statusChange" && !item.content && !item.subAgentId)
        return false;
      // Avoid duplicating the initial user message: we already show task.prompt in the timeline header
      if (item.type === "userMessage" && item.content === task.prompt) return false;
      return true;
    });
  }, [items, task.prompt]);

  const isWorking = task.status === "planning" || task.status === "executing";
  const renderKey = (item: { id: string; seq: number }, idx: number) =>
    `${item.id}-${item.seq}-${idx}`;

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
        activeComments={review.activeComments}
        draftLine={review.draftLine}
        draftText={review.draftText}
        onDraftTextChange={review.setDraftText}
        draftTextareaRef={review.draftTextareaRef}
        draftAnchorRef={draftAnchorRef}
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

export { useExecutionSummary, usePendingApprovals, useApprovalResolver };
