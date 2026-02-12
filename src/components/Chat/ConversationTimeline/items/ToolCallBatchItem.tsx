import { useState } from "react";
import { ChevronDown, ChevronRight, Terminal } from "lucide-react";
import type { ConversationItem } from "@/runtime/eventBuffer";
import { ToolCallItem } from "./ToolCallItem";

type ToolCallBatchItemProps = {
  items: ConversationItem[];
};

export function ToolCallBatchItem({ items }: ToolCallBatchItemProps) {
  const [expanded, setExpanded] = useState(false);

  const runningCount = items.filter((item) => item.toolStatus === "running").length;
  const errorCount = items.filter((item) => item.toolStatus === "error").length;
  const successCount = items.filter((item) => item.toolStatus === "success").length;

  return (
    <div className="ml-11 rounded-lg border border-border/70 bg-background/55">
      <button
        type="button"
        aria-expanded={expanded}
        aria-controls={`tool-batch-${items[0]?.id ?? "unknown"}`}
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition-colors hover:bg-accent/45"
      >
        <Terminal size={13} className="shrink-0 text-muted-foreground" />
        <span className="min-w-0 flex-1 text-xs font-medium text-foreground">{items.length} tool calls</span>
        {runningCount > 0 && (
          <span className="text-[11px] text-info">{runningCount} running</span>
        )}
        {errorCount > 0 && (
          <span className="text-[11px] text-destructive">{errorCount} failed</span>
        )}
        {successCount > 0 && errorCount === 0 && runningCount === 0 && (
          <span className="text-[11px] text-success">{successCount} done</span>
        )}
        {expanded ? (
          <ChevronDown size={12} className="shrink-0 text-muted-foreground" />
        ) : (
          <ChevronRight size={12} className="shrink-0 text-muted-foreground" />
        )}
      </button>

      {expanded && (
        <div
          id={`tool-batch-${items[0]?.id ?? "unknown"}`}
          className="space-y-2 border-t border-border/60 px-2.5 py-2.5"
        >
          {items.map((toolItem) => (
            <ToolCallItem key={toolItem.id} item={toolItem} compact />
          ))}
        </div>
      )}
    </div>
  );
}
