import { ArrowRight } from "lucide-react";
import type { TaskRow } from "@/types";

type UserMessageProps = {
  prompt: string;
  relatedTasks: TaskRow[];
  onSelectTask: (id: string) => void;
};

export function UserMessage({ prompt, relatedTasks, onSelectTask }: UserMessageProps) {
  return (
    <div className="flex gap-3">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/15 text-primary">
        <span className="text-xs font-semibold">You</span>
      </div>
      <div className="min-w-0 flex-1 pt-1">
        <p className="text-sm leading-relaxed text-foreground">{prompt}</p>
        {relatedTasks.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {relatedTasks.map((related) => (
              <button
                key={related.id}
                type="button"
                className="inline-flex items-center gap-1 rounded-full border border-border bg-muted/50 px-2.5 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                onClick={() => onSelectTask(related.id)}
              >
                <ArrowRight size={10} />
                <span className="max-w-32 truncate">{related.prompt}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
