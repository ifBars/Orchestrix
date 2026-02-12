import { useState } from "react";
import { Brain, ChevronDown, ChevronRight } from "lucide-react";
import type { ConversationItem } from "@/runtime/eventBuffer";
import { SafeStreamdown } from "../messages/SafeStreamdown";

type ThinkingItemProps = {
  item: ConversationItem;
};

export function ThinkingItem({ item }: ThinkingItemProps) {
  const [collapsed, setCollapsed] = useState(true);
  const text = item.content ?? "";
  if (!text) return null;

  return (
    <div className="ml-11">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="flex items-center gap-2 rounded-md border border-border/60 bg-background/45 px-2.5 py-1 text-xs text-muted-foreground/80 transition-colors hover:text-muted-foreground"
      >
        <Brain size={12} />
        <span>Reasoning</span>
        {collapsed ? <ChevronRight size={10} /> : <ChevronDown size={10} />}
      </button>
      {!collapsed && (
        <div className="mt-1 rounded-lg border border-border/50 bg-card/45 p-3">
          <div className="prose prose-sm max-w-none text-xs text-muted-foreground/85 dark:prose-invert prose-p:my-1 prose-headings:my-2 prose-code:text-[11px]">
            <SafeStreamdown content={text} />
          </div>
        </div>
      )}
    </div>
  );
}
