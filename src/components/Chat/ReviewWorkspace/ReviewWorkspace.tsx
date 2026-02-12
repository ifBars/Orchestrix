import { CheckCircle2, FileCode2, Loader2, X } from "lucide-react";
import { Streamdown } from "streamdown";
import { code } from "@streamdown/code";
import { useRef, useCallback } from "react";
import type { ArtifactRow } from "@/types";
import type { ReviewComment } from "@/hooks/useArtifactReview";
import { useCommentHover } from "./useCommentHover";
import { FloatingCommentButton } from "./FloatingCommentButton";
import { CommentRail } from "./CommentRail";
import { DraftCommentEditor } from "./DraftCommentEditor";

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
};

export function ReviewWorkspace(props: ReviewWorkspaceProps) {
  const hasCommentRail = props.activeComments.length > 0;
  const proseRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

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

  return (
    <div className="flex h-full w-full flex-col pb-2">
      <div className="sticky top-0 z-10 flex items-center justify-between border-b border-border/60 bg-card/80 px-4 py-2.5 backdrop-blur supports-[backdrop-filter]:bg-card/72">
        <div className="flex items-center gap-2">
          <span className="text-sm font-semibold text-foreground">Implementation Plan</span>
          <select
            value={props.selectedArtifactPath ?? ""}
            onChange={(e) => props.onSelectArtifact(e.target.value)}
            className="h-8 rounded-md border border-border/80 bg-background/85 px-2 text-xs text-muted-foreground outline-none focus-visible:border-ring/70"
          >
            {props.markdownArtifacts.map((artifact) => (
              <option key={artifact.id} value={artifact.uri_or_content}>
                {artifact.uri_or_content.split(/[/\\]/).pop()}
              </option>
            ))}
          </select>
        </div>
        <div className="relative flex items-center gap-2">
          <button
            type="button"
            onClick={() => props.onSubmitReview().catch(console.error)}
            disabled={props.submittingReview}
            className="inline-flex items-center gap-2 rounded-lg border border-border/80 bg-background/75 px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-50"
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
            disabled={props.approving}
            onClick={() => props.onBuild().catch(console.error)}
            className="inline-flex items-center gap-2 rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
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

          {props.showGeneralReviewInput && props.activeComments.length === 0 && (
            <div className="absolute right-0 top-10 z-20 w-80 rounded-lg border border-border/70 bg-card/95 p-2.5 elevation-2 backdrop-blur">
              <div className="mb-1 text-[10px] uppercase tracking-wider text-muted-foreground">
                General review feedback
              </div>
              <textarea
                value={props.generalReviewText}
                onChange={(e) => props.onGeneralReviewTextChange(e.target.value)}
                className="min-h-20 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs outline-none focus:border-primary"
                placeholder="Add overall feedback if you do not want line comments"
              />
            </div>
          )}
        </div>
      </div>

      <div className="relative min-h-0 flex-1 overflow-hidden">
        <div className="flex h-full overflow-auto">
          <div
            className={`relative min-w-0 flex-1 p-6 ${hasCommentRail ? "pr-64" : "pr-6"}`}
          >
            <div
              ref={proseRef}
              className="prose prose-sm max-w-none rounded-xl border border-border/60 bg-background/55 p-5 text-foreground dark:prose-invert"
            >
              <Streamdown plugins={{ code }}>{props.previewText}</Streamdown>
            </div>

            <FloatingCommentButton
              hoverState={hoverState}
              buttonRef={buttonRef}
              draftLine={props.draftLine}
              onMouseEnter={handleButtonMouseEnter}
              onMouseLeave={handleButtonMouseLeave}
              onClick={handleAddComment}
              getButtonStyle={getButtonStyle}
            />
          </div>

          <CommentRail
            comments={props.activeComments}
            onEdit={props.onEditComment}
            onDelete={props.onDeleteComment}
          />
        </div>

        {props.draftLine != null && (
          <DraftCommentEditor
            draftLine={props.draftLine}
            draftText={props.draftText}
            anchorRef={props.draftAnchorRef as React.RefObject<HTMLDivElement>}
            textareaRef={props.draftTextareaRef}
            onTextChange={props.onDraftTextChange}
            onCancel={props.onCancelDraft}
            onSave={props.onSaveComment}
          />
        )}
      </div>
    </div>
  );
}
