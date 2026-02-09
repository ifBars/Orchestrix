import { useState } from "react";
import { Bot, ChevronDown, ChevronRight, ListChecks, Loader2 } from "lucide-react";
import { Streamdown } from "streamdown";
import { code } from "@streamdown/code";
import { runtimeEventBuffer } from "@/runtime/eventBuffer";

type PlanMessageProps = {
  plan: ReturnType<typeof runtimeEventBuffer.getPlan>;
  planStream: string | null;
  assistantMessage: string | null;
  status: string;
};

export function PlanMessage({
  plan,
  planStream,
  assistantMessage,
  status,
}: PlanMessageProps) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="flex gap-3">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-accent text-muted-foreground">
        <Bot size={14} />
      </div>
      <div className="min-w-0 flex-1 pt-1">
        {assistantMessage && (
          <div className="prose prose-sm max-w-none text-sm leading-relaxed text-foreground dark:prose-invert prose-p:my-1 prose-headings:my-2 prose-code:text-xs">
            <Streamdown plugins={{ code }}>{assistantMessage}</Streamdown>
          </div>
        )}

        {!assistantMessage && planStream && (
          <div className="prose prose-sm max-w-none text-sm leading-relaxed text-foreground dark:prose-invert prose-p:my-1">
            <Streamdown plugins={{ code }}>{planStream}</Streamdown>
          </div>
        )}

        {plan && plan.steps.length > 0 && (
          <div className="mt-3">
            <button
              type="button"
              onClick={() => setExpanded(!expanded)}
              className="flex items-center gap-2 rounded-lg border border-border bg-muted/30 px-3 py-2 text-xs font-medium text-muted-foreground transition-colors hover:bg-muted/60"
            >
              <ListChecks size={14} />
              <span>{plan.steps.length} steps planned</span>
              {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            </button>

            {expanded && (
              <div className="mt-2 space-y-1.5 pl-1">
                {plan.steps.map((step, i) => (
                  <div
                    key={i}
                    className="flex items-start gap-2 rounded-md border border-border/60 bg-card/50 px-3 py-2"
                  >
                    <span className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-muted text-[10px] font-semibold text-muted-foreground">
                      {i + 1}
                    </span>
                    <div className="min-w-0">
                      <p className="text-xs font-medium text-foreground">{step.title}</p>
                      {step.description && (
                        <p className="mt-0.5 text-xs text-muted-foreground">{step.description}</p>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {status === "planning" && (
          <div className="mt-2 inline-flex items-center gap-1.5 rounded-full bg-info/10 px-2.5 py-1 text-xs text-info">
            <Loader2 size={10} className="animate-spin" />
            Planning
          </div>
        )}
      </div>
    </div>
  );
}
