import { useEffect, useRef, useState } from "react";
import { BadgeCheck, Binary, Eye, EyeOff, Sparkles } from "lucide-react";
import { ConversationTimeline } from "@/components/Chat/ConversationTimeline";
import { ReviewWorkspace } from "@/components/Chat/ReviewWorkspace";
import { ORCHESTRIX_REPO_URL } from "@/components/landing/constants";
import {
  ShellArtifactRailView,
  ShellHeaderView,
  ShellSidebarView,
} from "@/components/landing/preview/PreviewChrome";
import {
  getScenarioForTask,
  LANDING_PREVIEW_SCENARIOS,
  LANDING_PREVIEW_TASKS,
  type LandingPreviewScenarioId,
} from "@/components/landing/preview/previewData";
import type { ReviewComment } from "@/hooks/useArtifactReview";
import { IdeShell } from "@/layouts/IdeShell";
import { cn } from "@/lib/utils";
import type { ArtifactContentView } from "@/types";

type PreviewWorkbenchProps = {
  variant?: "hero" | "full";
  initialScenario?: LandingPreviewScenarioId;
  interactive?: boolean;
  className?: string;
};

function cloneCommentsByArtifact(input: Record<string, ReviewComment[]>) {
  return Object.fromEntries(
    Object.entries(input).map(([path, comments]) => [path, comments.map((comment) => ({ ...comment }))])
  );
}

function buildInitialArtifactContentState() {
  const initial = {} as Record<LandingPreviewScenarioId, Record<string, ArtifactContentView>>;
  const scenarioIds = Object.keys(LANDING_PREVIEW_SCENARIOS) as LandingPreviewScenarioId[];

  scenarioIds.forEach((scenarioId) => {
    initial[scenarioId] = Object.fromEntries(
      Object.entries(LANDING_PREVIEW_SCENARIOS[scenarioId].artifactContentsByPath).map(([path, content]) => [path, { ...content }])
    );
  });

  return initial;
}

function buildInitialCommentState() {
  const initial = {} as Record<LandingPreviewScenarioId, Record<string, ReviewComment[]>>;
  const scenarioIds = Object.keys(LANDING_PREVIEW_SCENARIOS) as LandingPreviewScenarioId[];

  scenarioIds.forEach((scenarioId) => {
    initial[scenarioId] = cloneCommentsByArtifact(LANDING_PREVIEW_SCENARIOS[scenarioId].initialCommentsByArtifact);
  });

  return initial;
}

export function PreviewWorkbench({
  variant = "full",
  initialScenario = "awaiting_review",
  interactive = true,
  className,
}: PreviewWorkbenchProps) {
  const heroMode = variant === "hero";
  const initialData = LANDING_PREVIEW_SCENARIOS[initialScenario];
  const [scenarioId, setScenarioId] = useState<LandingPreviewScenarioId>(initialScenario);
  const [selectedTaskId, setSelectedTaskId] = useState<string>(initialData.task.id);
  const [activeTab, setActiveTab] = useState<"chat" | "review">(initialData.defaultTab);
  const [sidebarOpen, setSidebarOpen] = useState(!heroMode);
  const [artifactsOpen, setArtifactsOpen] = useState(variant === "full");
  const [artifactContentsByScenario, setArtifactContentsByScenario] = useState(buildInitialArtifactContentState);
  const [commentsByScenario, setCommentsByScenario] = useState(buildInitialCommentState);
  const [selectedArtifactPath, setSelectedArtifactPath] = useState<string | null>(initialData.initialSelectedArtifactPath);
  const [selectedRailArtifactPath, setSelectedRailArtifactPath] = useState<string | null>(initialData.initialRailArtifactPath);
  const [draftLine, setDraftLine] = useState<number | null>(null);
  const [draftText, setDraftText] = useState("");
  const [editingCommentId, setEditingCommentId] = useState<string | null>(null);
  const [generalReviewText, setGeneralReviewText] = useState("");

  const draftAnchorRef = useRef<HTMLDivElement | null>(null);
  const draftTextareaRef = useRef<HTMLTextAreaElement | null>(null);

  const scenario = LANDING_PREVIEW_SCENARIOS[scenarioId];
  const artifactContentsByPath = artifactContentsByScenario[scenarioId];
  const commentsByArtifact = commentsByScenario[scenarioId];
  const reviewArtifacts = scenario.markdownArtifacts;
  const previewText = selectedArtifactPath ? (artifactContentsByPath[selectedArtifactPath]?.content ?? "") : "";
  const activeComments = selectedArtifactPath ? commentsByArtifact[selectedArtifactPath] ?? [] : [];

  useEffect(() => {
    const nextScenario = LANDING_PREVIEW_SCENARIOS[scenarioId];
    setSelectedTaskId(nextScenario.task.id);
    setActiveTab(nextScenario.defaultTab);
    setSelectedArtifactPath(nextScenario.initialSelectedArtifactPath);
    setSelectedRailArtifactPath(nextScenario.initialRailArtifactPath);
    setDraftLine(null);
    setDraftText("");
    setEditingCommentId(null);
    setGeneralReviewText("");
    setSidebarOpen(!heroMode);
    setArtifactsOpen(variant === "full" && nextScenario.defaultTab === "chat");
  }, [heroMode, scenarioId, variant]);

  const disabledComposer = (
    <div className="landing-preview-composer">
      <div className="landing-preview-composer__copy">
        <span className="landing-preview-composer__label">Preview mode</span>
        <p>Composer is intentionally disabled. Explore the shell, timeline, artifacts, and review flow locally.</p>
      </div>
      <button type="button" disabled className="landing-preview-composer__button">
        Send
      </button>
    </div>
  );

  const renderKey = (item: { id: string; seq: number }, idx: number) => item.id + "-" + String(item.seq) + "-" + String(idx);

  const workbenchMain = activeTab === "review" ? (
    <ReviewWorkspace
      markdownArtifacts={reviewArtifacts}
      selectedArtifactPath={selectedArtifactPath}
      onSelectArtifact={setSelectedArtifactPath}
      previewText={previewText}
      activeComments={activeComments}
      draftLine={draftLine}
      draftText={draftText}
      onDraftTextChange={setDraftText}
      draftAnchorRef={draftAnchorRef}
      draftTextareaRef={draftTextareaRef}
      onOpenCommentEditor={(line) => {
        const existing = activeComments.find((comment) => comment.line === line);
        setDraftLine(line);
        setDraftText(existing ? existing.text : "");
        setEditingCommentId(existing ? existing.id : null);
      }}
      onSaveComment={() => {
        if (!selectedArtifactPath || draftLine == null || !draftText.trim()) return;

        setCommentsByScenario((prev) => {
          const nextScenarioComments = cloneCommentsByArtifact(prev[scenarioId]);
          const currentComments = nextScenarioComments[selectedArtifactPath] ?? [];

          if (editingCommentId) {
            nextScenarioComments[selectedArtifactPath] = currentComments
              .map((comment) =>
                comment.id === editingCommentId ? { ...comment, line: draftLine, text: draftText.trim() } : comment
              )
              .sort((a, b) => a.line - b.line);
          } else {
            nextScenarioComments[selectedArtifactPath] = [
              ...currentComments,
              {
                id: "comment-" + String(Date.now()),
                line: draftLine,
                text: draftText.trim(),
              },
            ].sort((a, b) => a.line - b.line);
          }

          return {
            ...prev,
            [scenarioId]: nextScenarioComments,
          };
        });

        setDraftLine(null);
        setDraftText("");
        setEditingCommentId(null);
      }}
      onCancelDraft={() => {
        setDraftLine(null);
        setDraftText("");
        setEditingCommentId(null);
      }}
      onEditComment={(commentId) => {
        const comment = activeComments.find((entry) => entry.id === commentId);
        if (!comment) return;
        setDraftLine(comment.line);
        setDraftText(comment.text);
        setEditingCommentId(comment.id);
      }}
      onDeleteComment={(commentId) => {
        if (!selectedArtifactPath) return;
        setCommentsByScenario((prev) => ({
          ...prev,
          [scenarioId]: {
            ...prev[scenarioId],
            [selectedArtifactPath]: (prev[scenarioId][selectedArtifactPath] ?? []).filter(
              (comment) => comment.id !== commentId
            ),
          },
        }));
      }}
      onBackToChat={() => setActiveTab("chat")}
      onSubmitReview={async () => undefined}
      onBuild={async () => undefined}
      submittingReview={false}
      approving={false}
      showGeneralReviewInput={false}
      generalReviewText={generalReviewText}
      onGeneralReviewTextChange={setGeneralReviewText}
      onPreviewTextChange={(text) => {
        if (!selectedArtifactPath) return;
        setArtifactContentsByScenario((prev) => ({
          ...prev,
          [scenarioId]: {
            ...prev[scenarioId],
            [selectedArtifactPath]: {
              ...(prev[scenarioId][selectedArtifactPath] ?? {
                path: selectedArtifactPath,
                content: text,
                is_markdown: true,
              }),
              content: text,
            },
          },
        }));
      }}
      interactionMode="preview"
    />
  ) : (
    <ConversationTimeline
      task={scenario.task}
      relatedTasks={[]}
      onSelectTask={() => undefined}
      plan={scenario.plan}
      planStream={scenario.planStream}
      assistantMessage={scenario.assistantMessage}
      activeAgentStream={scenario.activeAgentStream}
      visibleItems={scenario.visibleItems}
      renderKey={renderKey}
      isWorking={scenario.task.status === "planning" || scenario.task.status === "executing"}
      onBuild={async () => undefined}
      approving={false}
      onStop={async () => undefined}
      stopping={false}
      markdownArtifactCount={reviewArtifacts.length}
      executionSummary={scenario.executionSummary}
      contextSnapshot={scenario.contextSnapshot}
      rawEvents={scenario.rawEvents}
      agentTodos={scenario.agentTodos}
      pendingApprovals={[]}
      pendingQuestions={[]}
      resolvingApprovalId={null}
      resolvingQuestionId={null}
      onResolveApproval={async () => undefined}
      onResolveQuestion={async () => undefined}
    />
  );

  const shellHeightClass = heroMode ? "h-[320px] sm:h-[340px] xl:h-[360px]" : "h-[860px] max-h-[80vh] min-h-[720px]";

  const controlBar = interactive ? (
    <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
      <div className="flex flex-wrap items-center gap-2">
        {(Object.keys(LANDING_PREVIEW_SCENARIOS) as LandingPreviewScenarioId[]).map((entryId) => {
          const entry = LANDING_PREVIEW_SCENARIOS[entryId];
          return (
            <button
              key={entry.id}
              type="button"
              onClick={() => setScenarioId(entry.id)}
              className={cn(
                "landing-scenario-pill",
                scenarioId === entry.id && "landing-scenario-pill--active"
              )}
            >
              {entry.label}
            </button>
          );
        })}
      </div>

      <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
        <span className="landing-micro-chip">
          <Binary size={11} />
          app-adjacent shell
        </span>
        <span className="landing-micro-chip">
          <BadgeCheck size={11} />
          {String(reviewArtifacts.length) + " review artifacts"}
        </span>
        <span className="landing-micro-chip">
          {artifactsOpen ? <Eye size={11} /> : <EyeOff size={11} />}
          {String(scenario.artifacts.length) + " preview files"}
        </span>
      </div>
    </div>
  ) : null;

  return (
    <div className={cn("space-y-4", heroMode && "pointer-events-none")}>
      {controlBar}
      <div
        className={cn(
          "landing-shell-window overflow-hidden",
          heroMode && "landing-shell-window--hero landing-shell-window--hero-preview",
          className
        )}
      >
        <div className={cn("relative", shellHeightClass)}>
          <IdeShell
            isArtifactsOpen={!heroMode && activeTab === "chat" && artifactsOpen}
            isSidebarOpen={sidebarOpen}
            onToggleSidebar={interactive ? () => setSidebarOpen((prev) => !prev) : undefined}
            header={
              <ShellHeaderView
                workspaceName="Orchestrix"
                sidebarOpen={sidebarOpen}
                artifactsOpen={activeTab === "chat" && artifactsOpen}
                onToggleSidebar={interactive ? () => setSidebarOpen((prev) => !prev) : undefined}
                onToggleArtifacts={interactive ? () => setArtifactsOpen((prev) => !prev) : undefined}
                repoUrl={ORCHESTRIX_REPO_URL}
              />
            }
            sidebar={
              <ShellSidebarView
                tasks={LANDING_PREVIEW_TASKS}
                selectedTaskId={selectedTaskId}
                workspaceName="Workspace history"
                onSelectTask={(taskId) => {
                  if (!interactive) return;
                  setSelectedTaskId(taskId);
                  setScenarioId(getScenarioForTask(taskId));
                }}
              />
            }
            subheader={
              <div className="flex items-center gap-0 border-b border-border/70 bg-card/60 px-4 backdrop-blur-md">
                {(["chat", "review"] as const).map((tab) => (
                  <button
                    key={tab}
                    type="button"
                    onClick={() => interactive && setActiveTab(tab)}
                    className={cn(
                      "relative px-3 py-2 text-xs font-medium transition-colors",
                      activeTab === tab
                        ? "text-foreground after:absolute after:inset-x-0 after:bottom-0 after:h-0.5 after:bg-primary"
                        : "text-muted-foreground hover:text-foreground"
                    )}
                  >
                    {tab === "chat" ? "Timeline" : "Review"}
                  </button>
                ))}
                <div className="ml-auto hidden items-center gap-2 text-[10px] font-medium uppercase tracking-[0.18em] text-muted-foreground/70 lg:flex">
                  <Sparkles size={11} />
                  local-only preview
                </div>
              </div>
            }
            main={workbenchMain}
            composer={activeTab === "chat" ? disabledComposer : null}
            artifacts={
              <ShellArtifactRailView
                artifacts={scenario.artifacts}
                activeArtifactPath={selectedRailArtifactPath}
                artifactContentsByPath={artifactContentsByPath}
                onSelectArtifact={setSelectedRailArtifactPath}
                onOpenReview={(path) => {
                  setSelectedArtifactPath(path);
                  setActiveTab("review");
                }}
              />
            }
          />
        </div>
      </div>
      {interactive ? (
        <p className="text-sm leading-relaxed text-muted-foreground">
          The shell is fully interactive inside the browser. Switch scenarios, inspect artifacts, expand tool batches,
          and review the fake plan without invoking any real models or tools.
        </p>
      ) : null}
    </div>
  );
}

