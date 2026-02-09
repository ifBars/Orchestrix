import { Pencil, Trash2 } from "lucide-react";
import type { ReviewComment } from "@/hooks/useArtifactReview";

type CommentRailProps = {
  comments: ReviewComment[];
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
};

export function CommentRail({ comments, onEdit, onDelete }: CommentRailProps) {
  if (comments.length === 0) return null;

  return (
    <div className="sticky top-4 z-10 w-56 shrink-0 self-start py-4 pl-2 pr-4">
      <div className="space-y-2">
        {comments.map((comment) => (
          <div
            key={comment.id}
            className="rounded border border-border/60 bg-card/90 p-2 shadow-sm backdrop-blur"
          >
            <div className="mb-1 text-[10px] uppercase tracking-wider text-muted-foreground">
              Line {comment.line}
            </div>
            <p className="text-xs text-foreground">{comment.text}</p>
            <div className="mt-2 flex items-center justify-end gap-1">
              <button
                type="button"
                onClick={() => onEdit(comment.id)}
                className="rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                title="Edit comment"
              >
                <Pencil size={12} />
              </button>
              <button
                type="button"
                onClick={() => onDelete(comment.id)}
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
  );
}
