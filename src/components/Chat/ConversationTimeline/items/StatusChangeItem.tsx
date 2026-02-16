import { AlertTriangle, CheckCircle2, Clock3, GitMerge, Loader2 } from "lucide-react";
import type { ConversationItem } from "@/runtime/eventBuffer";

type StatusChangeItemProps = {
  item: ConversationItem;
};

export function StatusChangeItem({ item }: StatusChangeItemProps) {
  // Transient states that will be auto-removed (more subtle appearance)
  const isTransient = item.status === "deciding" || item.status === "preparing";
  
  let icon = <Clock3 size={12} className="text-muted-foreground" />;
  if (item.status === "completed" || item.status === "merged") {
    icon = <CheckCircle2 size={12} className="text-success" />;
  } else if (item.status === "failed" || item.status === "retrying") {
    icon = <AlertTriangle size={12} className="text-warning" />;
  } else if (item.status === "executing") {
    icon = <Loader2 size={12} className="animate-spin text-info" />;
  } else if (item.status === "merged") {
    icon = <GitMerge size={12} className="text-success" />;
  } else if (item.status === "deciding") {
    icon = <Loader2 size={12} className="animate-spin text-muted-foreground/60" />;
  } else if (item.status === "preparing") {
    icon = <Loader2 size={12} className="animate-spin text-muted-foreground/70" />;
  }

  // Transient items get a more subtle, inline appearance
  if (isTransient) {
    return (
      <div className="ml-11 flex items-center gap-1.5 px-1 py-0.5 text-[11px] text-muted-foreground/70 animate-in fade-in duration-200">
        {icon}
        <span className="italic">{item.content ?? `Status: ${item.status}`}</span>
      </div>
    );
  }

  // Permanent status changes get the full card treatment
  return (
    <div className="ml-11 flex items-center gap-2 rounded-md border border-border/60 bg-background/45 px-2.5 py-1.5 text-xs text-muted-foreground animate-in fade-in slide-in-from-left-2 duration-300">
      {icon}
      <span>{item.content ?? `Status: ${item.status}`}</span>
    </div>
  );
}
