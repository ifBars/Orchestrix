import { useState } from "react";
import { CheckCircle2, ChevronDown, ChevronRight, Loader2, Terminal, XCircle } from "lucide-react";
import type { ConversationItem } from "@/runtime/eventBuffer";
import { toDisplay } from "../utils";

type ToolCallItemProps = {
  item: ConversationItem;
  compact?: boolean;
};

export function ToolCallItem({ item, compact = false }: ToolCallItemProps) {
  const [expanded, setExpanded] = useState(false);
  const isRunning = item.toolStatus === "running";
  const isError = item.toolStatus === "error";

  const statusIcon = isRunning ? (
    <Loader2 size={12} className="animate-spin text-info" />
  ) : isError ? (
    <XCircle size={12} className="text-destructive" />
  ) : (
    <CheckCircle2 size={12} className="text-success" />
  );

  return (
    <div className={compact ? "" : "ml-11"}>
      <button
        type="button"
        aria-expanded={expanded}
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 rounded-lg border border-border/70 bg-background/55 px-3 py-2 text-left transition-colors hover:bg-accent/45"
      >
        <Terminal size={13} className="shrink-0 text-muted-foreground" />
        <div className="min-w-0 flex-1">
          <span className="block truncate text-xs font-medium text-foreground">{item.toolName}</span>
          {item.toolRationale && (
            <span className="block truncate text-[11px] text-muted-foreground">{item.toolRationale}</span>
          )}
        </div>
        {statusIcon}
        {expanded ? (
          <ChevronDown size={12} className="shrink-0 text-muted-foreground" />
        ) : (
          <ChevronRight size={12} className="shrink-0 text-muted-foreground" />
        )}
      </button>

      {expanded && (
        <div className="mt-1 rounded-lg border border-border/60 bg-card/45 p-3">
          {item.toolArgs && Object.keys(item.toolArgs).length > 0 && (
            <div className="mb-2">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Arguments</span>
              <pre className="mt-1 max-h-32 overflow-auto rounded-md border border-border/50 bg-background/70 p-2 text-xs text-muted-foreground">
                {JSON.stringify(item.toolArgs, null, 2)}
              </pre>
            </div>
          )}
          {item.toolResult && (
            <div className="mb-2">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Result</span>
              <pre className="mt-1 max-h-40 overflow-auto rounded-md border border-border/50 bg-background/70 p-2 text-xs text-muted-foreground">
                {toDisplay(item.toolResult)}
              </pre>
            </div>
          )}
          {item.toolError && (
            <div>
              <span className="text-[10px] font-semibold uppercase tracking-wider text-destructive/80">Error</span>
              <pre className="mt-1 max-h-24 overflow-auto rounded-md border border-destructive/30 bg-destructive/8 p-2 text-xs text-destructive">
                {toDisplay(item.toolError)}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
