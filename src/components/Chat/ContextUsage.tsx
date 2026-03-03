import { Gauge, Sparkles } from "lucide-react";
import { memo, useMemo } from "react";
import { cn } from "@/lib/utils";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import type { TaskContextSnapshotView } from "@/types";

const SEGMENT_DOT_CLASS: Record<string, string> = {
  system_prompt: "bg-info/70",
  tool_definitions: "bg-primary/70",
  mcp_tools: "bg-destructive/60",
  messages: "bg-warning/75",
  compaction_buffer: "bg-muted-foreground/60",
  free_space: "bg-success/65",
};

interface ContextUsageChipProps {
  snapshot: TaskContextSnapshotView;
  className?: string;
}

export const ContextUsageChip = memo(function ContextUsageChip({
  snapshot,
  className,
}: ContextUsageChipProps) {
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          className={cn(
            "inline-flex h-7 items-center gap-1.5 rounded-lg border px-2 text-xs transition-colors",
            "border-border/70 bg-background/70 text-muted-foreground hover:bg-accent/60 hover:text-foreground",
            className
          )}
          title="View context window usage"
        >
          <Gauge size={13} />
          <span className="font-mono text-[11px] text-foreground">
            {formatTokenCompact(snapshot.used_tokens)} / {formatTokenCompact(snapshot.context_window)}
          </span>
          <span className="rounded-md bg-accent/70 px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
            {formatPercent(snapshot.usage_percentage)}
          </span>
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[320px] p-0">
        <ContextUsagePopoverContent snapshot={snapshot} />
      </PopoverContent>
    </Popover>
  );
});

interface ContextUsagePopoverContentProps {
  snapshot: TaskContextSnapshotView;
}

function ContextUsagePopoverContent({ snapshot }: ContextUsagePopoverContentProps) {
  const segments = useMemo(() => {
    return [...snapshot.segments].sort((a, b) => {
      if (a.key === "free_space") return 1;
      if (b.key === "free_space") return -1;
      return b.tokens - a.tokens;
    });
  }, [snapshot.segments]);

  return (
    <div className="p-3">
      <div className="mb-2 flex items-center justify-between gap-2">
        <span className="text-sm font-semibold text-foreground">Context Window</span>
        <span className="text-sm font-medium text-muted-foreground">
          {formatPercent(snapshot.usage_percentage)}
        </span>
      </div>

      <div className="mb-3 flex items-center gap-2 text-[11px] text-muted-foreground">
        <span className="truncate">{snapshot.model ?? "Unknown model"}</span>
        <span className="text-muted-foreground/55">•</span>
        <span>{formatTokenCompact(snapshot.used_tokens)} used</span>
        {snapshot.estimated && (
          <span className="inline-flex items-center gap-1 rounded-full border border-border/70 bg-background/65 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground/80">
            <Sparkles size={10} />
            Est.
          </span>
        )}
      </div>

      <div className="mb-3 h-1.5 overflow-hidden rounded-full bg-accent/65">
        <div
          className="h-full rounded-full bg-primary/80 transition-all"
          style={{ width: `${Math.min(100, Math.max(0, snapshot.usage_percentage))}%` }}
        />
      </div>

      <div className="space-y-1.5">
        {segments.map((segment) => (
          <div key={segment.key} className="flex items-center gap-2 text-xs">
            <span
              className={cn(
                "h-2 w-2 shrink-0 rounded-full",
                SEGMENT_DOT_CLASS[segment.key] ?? "bg-muted-foreground/60"
              )}
            />
            <span className="truncate text-muted-foreground">{segment.label}</span>
            <span className="ml-auto font-mono text-[11px] text-foreground">
              {formatTokenCompact(segment.tokens)}
            </span>
            <span className="w-10 text-right text-[11px] text-muted-foreground/80">
              {formatPercent(segment.percentage)}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

interface ContextUsageInlineProps {
  snapshot: TaskContextSnapshotView;
  className?: string;
}

export const ContextUsageInline = memo(function ContextUsageInline({
  snapshot,
  className,
}: ContextUsageInlineProps) {
  return (
    <div
      className={cn(
        "inline-flex items-center gap-1.5 rounded-lg border border-border/70 bg-card/55 px-2 py-1 text-[11px] text-muted-foreground",
        className
      )}
    >
      <Gauge size={12} />
      <span className="font-mono text-foreground">
        {formatTokenCompact(snapshot.used_tokens)} / {formatTokenCompact(snapshot.context_window)}
      </span>
      <span>{formatPercent(snapshot.usage_percentage)}</span>
    </div>
  );
});

function formatTokenCompact(value: number): string {
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(1)}M`;
  }
  if (value >= 10_000) {
    return `${(value / 1_000).toFixed(0)}k`;
  }
  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(1)}k`;
  }
  return `${value}`;
}

function formatPercent(value: number): string {
  const normalized = Number.isFinite(value) ? Math.max(0, value) : 0;
  if (normalized >= 10) {
    return `${normalized.toFixed(0)}%`;
  }
  return `${normalized.toFixed(1)}%`;
}

export { ContextUsagePopoverContent };