import { FileCode2 } from "lucide-react";
import type { ConversationItem } from "@/runtime/eventBuffer";

type FileChangeItemProps = {
  item: ConversationItem;
};

export function FileChangeItem({ item }: FileChangeItemProps) {
  if (!item.filePath) return null;
  const fileName = item.filePath.split(/[/\\]/).pop() ?? item.filePath;

  return (
    <div className="ml-11 flex items-center gap-2 rounded-lg border border-border/60 bg-background/50 px-3 py-1.5">
      <FileCode2 size={13} className="shrink-0 text-info" />
      <span className="min-w-0 truncate text-xs text-foreground">{fileName}</span>
      <span className="shrink-0 rounded-full bg-info/10 px-1.5 py-0.5 text-[10px] text-info">{item.fileAction ?? "modified"}</span>
    </div>
  );
}
