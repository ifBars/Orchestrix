import { XCircle } from "lucide-react";
import type { ConversationItem } from "@/runtime/eventBuffer";

type ErrorItemProps = {
  item: ConversationItem;
};

export function ErrorItem({ item }: ErrorItemProps) {
  return (
    <div className="ml-11 flex items-start gap-2 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2">
      <XCircle size={13} className="mt-0.5 shrink-0 text-destructive" />
      <p className="text-xs text-destructive">{item.errorMessage ?? "Unknown error"}</p>
    </div>
  );
}
