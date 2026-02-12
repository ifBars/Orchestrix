import { AlertTriangle, CheckCircle2, Clock3, GitMerge, Loader2 } from "lucide-react";
import type { ConversationItem } from "@/runtime/eventBuffer";

type StatusChangeItemProps = {
  item: ConversationItem;
};

export function StatusChangeItem({ item }: StatusChangeItemProps) {
  let icon = <Clock3 size={12} className="text-muted-foreground" />;
  if (item.status === "completed" || item.status === "merged") {
    icon = <CheckCircle2 size={12} className="text-success" />;
  } else if (item.status === "failed" || item.status === "retrying") {
    icon = <AlertTriangle size={12} className="text-warning" />;
  } else if (item.status === "executing") {
    icon = <Loader2 size={12} className="animate-spin text-info" />;
  } else if (item.status === "merged") {
    icon = <GitMerge size={12} className="text-success" />;
  }

  return (
    <div className="ml-11 flex items-center gap-2 rounded-md border border-border/60 bg-background/45 px-2.5 py-1.5 text-xs text-muted-foreground">
      {icon}
      <span>{item.content ?? `Status: ${item.status}`}</span>
    </div>
  );
}
