import { CheckCircle2, Edit3, Eye, FileCode2, Loader2, X } from "lucide-react";
import { SafeStreamdown } from "@/components/Chat/ConversationTimeline/messages/SafeStreamdown";
import { Suspense, lazy, useCallback, useEffect, useRef, useState } from "react";
import type { ArtifactRow } from "@/types";
import type { ReviewComment } from "@/hooks/useArtifactReview";
import { useCommentHover } from "./useCommentHover";
import { FloatingCommentButton } from "./FloatingCommentButton";
import { CommentRail } from "./CommentRail";
import { DraftCommentEditor } from "./DraftCommentEditor";

const LazyCodeEditor = lazy(async () => {
  const module = await import("@/components/ui/CodeEditor");
  return { default: module.CodeEditor };
});

type ReviewWorkspaceProps = {
  markdownArtifacts: ArtifactRow[];
  selectedArtifactPath: string | null;
  onSelectArtifact: (path: string) => void;
  previewText: string;
  activeComments: ReviewComment[];
  draftLine: number | null;
  draftText: string;
  onDraftTextChange: (value: string) => void;
  draftAnchorRef: React.RefObject<HTMLElement | null>;
  draftTextareaRef: React.RefObject<HTMLTextAreaElement | null>;
  onOpenCommentEditor: (line: number, anchorElement?: HTMLElement) => void;
  onSaveComment: () => void;
  onCancelDraft: () => void;
  onEditComment: (id: string) => void;
  onDeleteComment: (id: string) => void;
  onBackToChat: () => void;
  onSubmitReview: () => Promise<void>;
  onBuild: () => Promise<void>;
  submittingReview: boolean;
  approving: boolean;
  showGeneralReviewInput: boolean;
  generalReviewText: string;
  onGeneralReviewTextChange: (value: string) => void;
  onPreviewTextChange?: (text: string) => void;
  interactionMode?: "default" | "preview";
};

export function ReviewWorkspace(props: ReviewWorkspaceProps) {
  const hasCommentRail = props.activeComments.length > 0;
  const proseRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [isEditing, setIsEditing] = useState(false);
  const [editedText, setEditedText] = useState(props.previewText);
  const isPreviewMode = props.interactionMode === "preview";

  useEffect(() => {
    setEditedText(props.previewText);
  }, [props.previewText]);

  const {
    hoverState,
    setHoverState,
    handleButtonMouseEnter,
    handleButtonMouseLeave,
    getButtonStyle,
  } = useCommentHover(proseRef, buttonRef, props.draftLine, props.previewText);

  const handleAddComment = useCallback(() => {
    if (hoverState) {
      props.onOpenCommentEditor(hoverState.line, hoverState.element);
      setHoverState(null);
    }
  }, [hoverState, props.onOpenCommentEditor, setHoverState]);

  const handleContentChange = useCallback(
    (content: string) => {
      setEditedText(content);
      props.onPreviewTextChange?.(content);
    },
    [props.onPreviewTextChange]
  );

  const handleSubmitReview = () => {
    if (isPreviewMode) return;
    props.onSubmitReview().catch(console.error);
  };

  const handleBuild = () => {
    if (isPreviewMode) return;
    props.onBuild().catch(console.error);
  };

  return (
    <div className="flex h-full w-full flex-col pb-2">
      <div className="sticky top-0 z-10 flex items-center justify-between border-b border-border/60 bg-card/80 px-4 py-2.5 backdrop-blur supports-[backdrop-filter]:bg-card/72">
        <div className="flex min-w-0 items-center gap-2">
          <span className="text-sm font-semibold text-foreground">Implementation Plan</span>
          {isPreviewMode ? (
            <span className="inline-flex shrink-0 items-center rounded-full border border-primary/20 bg-primary/10 px-2 py-0.5 text-[10px] font-medium uppercase tracking-[0.18em] text-primary">
              Preview only
            </span>
          ) : null}
          <select
            value={props.selectedArtifactPath ?? ""}
            onChange={(event) => props.onSelectArtifact(event.target.value)}
            className="h-8 min-w-0 rounded-md border border-border/80 bg-background/85 px-2 text-xs text-muted-foreground outline-none focus-visible:border-ring/70"
          >
            {props.markdownArtifacts.map((artifact) => (
              <option key={artifact.id} value={artifact.uri_or_content}>
                {artifact.uri_or_content.split(/[/\\]/).pop()}
              </option>
            ))}
          </select>
        </div>

        <div className="relative flex items-center gap-2">
          {isPreviewMode ? (
            <div className="hidden rounded-lg border border-primary/15 bg-primary/8 px-3 py-1.5 text-[11px] text-primary md:flex">
              Review actions are disabled in the landing preview.
            </div>
          ) : null}

          <button
            type="button"
            onClick={() => setIsEditing((current) => !current)}
            className={`inline-flex items-center gap-1.5 rounded-lg border border-border/80 px-3 py-1.5 text-xs font-medium transition-colors ${
              isEditing
                ? "bg-accent text-foreground"
                : "bg-background/75 text-muted-foreground hover:bg-accent/70 hover:text-foreground"
            }`}
            title={isEditing ? "Switch to view mode" : "Switch to edit mode"}
          >
            {isEditing ? (
              <>
                <Eye size={12} />
                View
              </>
            ) : (
              <>
                <Edit3 size={12} />
                Edit
              </>
            )}
          </button>

          <button
            type="button"
            onClick={handleSubmitReview}
            disabled={isPreviewMode || props.submittingReview}
            className="inline-flex items-center gap-2 rounded-lg border border-border/80 bg-background/75 px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-50"
            title={isPreviewMode ? "Disabled in preview mode" : "Submit review"}
          >
            {props.submittingReview ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <FileCode2 size={12} />
            )}
            Submit Review
          </button>

          <button
            type="button"
            disabled={isPreviewMode || props.approving}
            onClick={handleBuild}
            className="inline-flex items-center gap-2 rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
            title={isPreviewMode ? "Disabled in preview mode" : "Approve and build"}
          >
            {props.approving ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <CheckCircle2 size={12} />
            )}
            Build
          </button>

          <button
            type="button"
            onClick={props.onBackToChat}
            className="rounded-lg border border-border/80 px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-accent/70 hover:text-foreground"
          >
            <span className="inline-flex items-center gap-1.5">
              <X size={12} />
              Back To Chat
            </span>
          </button>

          {props.showGeneralReviewInput && props.activeComments.length === 0 && !isPreviewMode ? (
            <div className="absolute right-0 top-10 z-20 w-80 rounded-lg border border-border/70 bg-card/95 p-2.5 elevation-2 backdrop-blur">
              <div className="mb-1 text-[10px] uppercase tracking-wider text-muted-foreground">
                General review feedback
              </div>
              <textarea
                value={props.generalReviewText}
                onChange={(event) => props.onGeneralReviewTextChange(event.target.value)}
                className="min-h-20 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs outline-none focus:border-primary"
                placeholder="Add overall feedback if you do not want line comments"
              />
            </div>
          ) : null}
        </div>
      </div>

      <div className="relative min-h-0 flex-1 overflow-hidden">
        <div className="flex h-full overflow-auto">
          <div className={`relative min-w-0 flex-1 p-6 ${hasCommentRail ? "pr-64" : "pr-6"}`}>
            {isEditing ? (
              <div className="h-full overflow-hidden rounded-xl border border-border/60 bg-background/55">
                <Suspense
                  fallback={
                    <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
                      Loading editor...
                    </div>
                  }
                >
                  <LazyCodeEditor
                    value={editedText}
                    onChange={handleContentChange}
                    language="markdown"
                    className="h-full border-0"
                    minHeight="100%"
                  />
                </Suspense>
              </div>
            ) : (
              <div
                ref={proseRef}
                className="prose prose-sm max-w-none rounded-xl border border-border/60 bg-background/55 p-5 text-foreground dark:prose-invert"
              >
                <SafeStreamdown content={props.previewText} mermaid />
              </div>
            )}

            {!isEditing ? (
              <FloatingCommentButton
                hoverState={hoverState}
                buttonRef={buttonRef}
                draftLine={props.draftLine}
                onMouseEnter={handleButtonMouseEnter}
                onMouseLeave={handleButtonMouseLeave}
                onClick={handleAddComment}
                getButtonStyle={getButtonStyle}
              />
            ) : null}
          </div>

          <CommentRail
            comments={props.activeComments}
            onEdit={props.onEditComment}
            onDelete={props.onDeleteComment}
          />
        </div>

        {props.draftLine != null && !isEditing ? (
          <DraftCommentEditor
            draftLine={props.draftLine}
            draftText={props.draftText}
            anchorRef={props.draftAnchorRef as React.RefObject<HTMLDivElement>}
            textareaRef={props.draftTextareaRef}
            onTextChange={props.onDraftTextChange}
            onCancel={props.onCancelDraft}
            onSave={props.onSaveComment}
          />
        ) : null}
      </div>
    </div>
  );
}
