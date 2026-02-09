import { MessageSquarePlus } from "lucide-react";

type DraftCommentEditorProps = {
  draftLine: number;
  draftText: string;
  anchorRef: React.RefObject<HTMLDivElement | null>;
  textareaRef: React.RefObject<HTMLTextAreaElement | null>;
  onTextChange: (value: string) => void;
  onCancel: () => void;
  onSave: () => void;
};

export function DraftCommentEditor({
  draftLine,
  draftText,
  anchorRef,
  textareaRef,
  onTextChange,
  onCancel,
  onSave,
}: DraftCommentEditorProps) {
  return (
    <div
      ref={anchorRef}
      className="absolute left-24 z-20 w-96 rounded-lg border border-border/60 bg-card/95 p-2.5 shadow-lg backdrop-blur"
    >
      <div className="mb-1 text-[10px] uppercase tracking-wider text-muted-foreground">
        Comment on block {draftLine}
      </div>
      <textarea
        ref={textareaRef}
        value={draftText}
        onChange={(e) => onTextChange(e.target.value)}
        className="min-h-20 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs outline-none focus:border-primary"
        placeholder="Add a comment..."
      />
      <div className="mt-2 flex items-center justify-end gap-2">
        <button
          type="button"
          onClick={onCancel}
          className="rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          Cancel
        </button>
        <button
          type="button"
          onClick={onSave}
          disabled={!draftText.trim()}
          className="inline-flex items-center gap-1 rounded-md bg-primary px-2 py-1 text-xs font-medium text-primary-foreground disabled:opacity-60"
        >
          <MessageSquarePlus size={11} />
          Add
        </button>
      </div>
    </div>
  );
}
