import { useRef, useState } from "react";
import { Brain, ChevronDown, ChevronRight } from "lucide-react";
import type { ConversationItem } from "@/runtime/eventBuffer";
import { SafeStreamdown } from "../messages/SafeStreamdown";

const collapsedStates = new Map<string, boolean>();

type ThinkingItemProps = {
  item: ConversationItem;
};

export function ThinkingItem({ item }: ThinkingItemProps) {
  const text = item.content ?? "";

  const collapsedRef = useRef(collapsedStates.get(item.id) ?? true);
  const [, forceUpdate] = useState({});

  const toggleCollapsed = () => {
    const newState = !collapsedRef.current;
    collapsedRef.current = newState;
    collapsedStates.set(item.id, newState);
    forceUpdate({});
  };

  const collapsed = collapsedRef.current;

  return (
    <div className="ml-11">
      <button
        type="button"
        onClick={toggleCollapsed}
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
