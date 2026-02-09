import { useState } from "react";
import { Brain, ChevronDown, ChevronRight } from "lucide-react";
import { Streamdown } from "streamdown";
import { code } from "@streamdown/code";
import type { ConversationItem } from "@/runtime/eventBuffer";

type ThinkingItemProps = {
  item: ConversationItem;
};

export function ThinkingItem({ item }: ThinkingItemProps) {
  const [collapsed, setCollapsed] = useState(true);
  const text = item.content ?? "";
  if (!text) return null;
  const preview = text.replace(/\s+/g, " ").trim();

  return (
    <div className="ml-11">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="flex items-center gap-2 text-xs text-muted-foreground/70 transition-colors hover:text-muted-foreground"
      >
        <Brain size={12} />
        <span>Reasoning</span>
        {collapsed ? <ChevronRight size={10} /> : <ChevronDown size={10} />}
      </button>
      {collapsed && (
        <p className="mt-1 overflow-hidden text-xs text-muted-foreground/80 [display:-webkit-box] [-webkit-box-orient:vertical] [-webkit-line-clamp:2]">
          {preview}
        </p>
      )}
      {!collapsed && (
        <div className="mt-1 rounded-lg border border-border/30 bg-muted/10 p-3">
          <div className="prose prose-sm max-w-none text-xs italic text-muted-foreground/80 dark:prose-invert prose-p:my-1 prose-headings:my-2 prose-code:text-[11px]">
            <Streamdown plugins={{ code }}>{text}</Streamdown>
          </div>
        </div>
      )}
    </div>
  );
}
