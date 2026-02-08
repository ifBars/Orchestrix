import { CheckCircle2, FileCode2, Loader2, MessageSquarePlus, Pencil, Trash2, X } from "lucide-react";
import { Streamdown } from "streamdown";
import { code } from "@streamdown/code";
import type { MutableRefObject, RefObject } from "react";
import type { ArtifactRow } from "@/types";
import type { ReviewComment } from "@/hooks/useArtifactReview";

type ReviewWorkspaceProps = {
  markdownArtifacts: ArtifactRow[];
  selectedArtifactPath: string | null;
  onSelectArtifact: (path: string) => void;
  previewText: string;
  previewLines: string[];
  activeComments: ReviewComment[];
  draftLine: number | null;
  draftText: string;
  onDraftTextChange: (value: string) => void;
  draftAnchorTop: number;
  reviewViewportRef: RefObject<HTMLDivElement | null>;
  lineButtonRefs: MutableRefObject<Record<number, HTMLButtonElement | null>>;
  draftTextareaRef: RefObject<HTMLTextAreaElement | null>;
  onOpenCommentEditor: (line: number) => void;
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

  return (
    <div className="flex h-full w-full flex-col pb-2">
      <div className="flex items-center justify-between border-b border-border/40 px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-foreground">Implementation Plan</span>
          <select
            value={props.selectedArtifactPath ?? ""}
            onChange={(e) => props.onSelectArtifact(e.target.value)}
            className="h-8 rounded-md border border-border bg-background px-2 text-xs text-muted-foreground"
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
            className="inline-flex items-center gap-2 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {props.submittingReview ? <Loader2 size={12} className="animate-spin" /> : <FileCode2 size={12} />}
            Submit Review
          </button>
          <button
            type="button"
            disabled={props.approving}
            onClick={() => props.onBuild().catch(console.error)}
            className="inline-flex items-center gap-2 rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {props.approving ? <Loader2 size={12} className="animate-spin" /> : <CheckCircle2 size={12} />}
            Build
          </button>
          <button
            type="button"
            onClick={props.onBackToChat}
            className="rounded-lg border border-border px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            <span className="inline-flex items-center gap-1.5">
              <X size={12} />
              Back To Chat
            </span>
          </button>

          {props.showGeneralReviewInput && props.activeComments.length === 0 && (
            <div className="absolute right-0 top-10 z-20 w-80 rounded-lg border border-border/60 bg-card/95 p-2.5 shadow-lg backdrop-blur">
              <div className="mb-1 text-[10px] uppercase tracking-wider text-muted-foreground">General review feedback</div>
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
        <div ref={props.reviewViewportRef} className="flex h-full overflow-auto">
          <div className="w-16 shrink-0 border-r border-border/30 px-2 py-4">
            <div className="space-y-1.5">
              {props.previewLines.map((_, idx) => (
                <button
                  key={`line-btn-${idx + 1}`}
                  ref={(el) => {
                    props.lineButtonRefs.current[idx + 1] = el;
                  }}
                  type="button"
                  onClick={() => props.onOpenCommentEditor(idx + 1)}
                  className={`flex w-full items-center justify-center gap-1 rounded px-1 py-1 text-[10px] transition-colors ${
                    props.activeComments.some((comment) => comment.line === idx + 1)
                      ? "bg-primary/15 text-primary"
                      : "text-muted-foreground hover:bg-accent hover:text-foreground"
                  }`}
                  title={`Comment on line ${idx + 1}`}
                >
                  <span className="font-mono">L{idx + 1}</span>
                  <span className="text-[11px]">+</span>
                </button>
              ))}
            </div>
          </div>

          <div className={`relative min-w-0 flex-1 p-6 ${hasCommentRail ? "pr-80" : "pr-6"}`}>
            <div className="prose prose-sm max-w-none text-foreground dark:prose-invert">
              <Streamdown plugins={{ code }}>{props.previewText}</Streamdown>
            </div>
          </div>

          {hasCommentRail && (
            <div className="sticky top-4 z-10 w-72 self-start p-4">
              <div className="space-y-3">
                {props.activeComments.map((comment) => (
                  <div key={comment.id} className="rounded-lg border border-border/60 bg-card/90 p-2.5 shadow-sm backdrop-blur">
                    <div className="mb-1 text-[10px] uppercase tracking-wider text-muted-foreground">Line {comment.line}</div>
                    <p className="text-xs text-foreground">{comment.text}</p>
                    <div className="mt-2 flex items-center justify-end gap-1">
                      <button
                        type="button"
                        onClick={() => props.onEditComment(comment.id)}
                        className="rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                        title="Edit comment"
                      >
                        <Pencil size={12} />
                      </button>
                      <button
                        type="button"
                        onClick={() => props.onDeleteComment(comment.id)}
                        className="rounded p-1 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                        title="Delete comment"
                      >
                        <Trash2 size={12} />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        {props.draftLine != null && (
          <div
            className="absolute left-24 z-20 w-96 rounded-lg border border-border/60 bg-card/95 p-2.5 shadow-lg backdrop-blur"
            style={{ top: props.draftAnchorTop }}
          >
            <div className="mb-1 text-[10px] uppercase tracking-wider text-muted-foreground">Comment on line {props.draftLine}</div>
            <textarea
              ref={props.draftTextareaRef}
              value={props.draftText}
              onChange={(e) => props.onDraftTextChange(e.target.value)}
              className="min-h-20 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs outline-none focus:border-primary"
              placeholder="Add a line-specific comment..."
            />
            <div className="mt-2 flex items-center justify-end gap-2">
              <button
                type="button"
                onClick={props.onCancelDraft}
                className="rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={props.onSaveComment}
                disabled={!props.draftText.trim()}
                className="inline-flex items-center gap-1 rounded-md bg-primary px-2 py-1 text-xs font-medium text-primary-foreground disabled:opacity-60"
              >
                <MessageSquarePlus size={11} />
                Add
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
