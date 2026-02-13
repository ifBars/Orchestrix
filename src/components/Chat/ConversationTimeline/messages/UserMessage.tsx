import { ArrowRight } from "lucide-react";
import type { TaskRow } from "@/types";
import { PromptWithMentions } from "./PromptWithMentions";

type UserMessageProps = {
  prompt: string | null;
  relatedTasks: TaskRow[];
  onSelectTask: (id: string) => void;
};

export function UserMessage({ prompt, relatedTasks, onSelectTask }: UserMessageProps) {
  const hasPrompt = Boolean(prompt && prompt.trim().length > 0);

  return (
    <div className="flex gap-3">
      {hasPrompt ? (
        <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-primary/25 bg-primary/12 text-primary">
          <span className="text-xs font-semibold">You</span>
        </div>
      ) : (
        <div className="w-8 shrink-0" />
      )}
      <div className="min-w-0 flex-1">
        {hasPrompt && (
          <div className="rounded-xl border border-border/70 bg-background/55 px-3 py-2.5">
            <PromptWithMentions content={prompt ?? ""} className="text-sm leading-relaxed text-foreground" />
          </div>
        )}
        {relatedTasks.length > 0 && (
          <div className={`${hasPrompt ? "mt-2" : ""} flex flex-wrap gap-1.5`}>
            {!hasPrompt && (
              <span className="mr-1 inline-flex items-center text-[11px] text-muted-foreground">
                Forked context from
              </span>
            )}
            {relatedTasks.map((related) => (
              <button
                key={related.id}
                type="button"
                className="inline-flex items-center gap-1 rounded-full border border-border/70 bg-background/75 px-2.5 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-accent/70 hover:text-accent-foreground"
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
