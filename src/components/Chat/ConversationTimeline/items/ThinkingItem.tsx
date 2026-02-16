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
  
  // If the thinking content is empty or very short, don't show it
  if (!text || text.trim().length < 10) {
    return null;
  }

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
    <div className="ml-11 animate-in fade-in slide-in-from-left-1 duration-300">
      <button
        type="button"
        onClick={toggleCollapsed}
        className="flex items-center gap-1.5 rounded-md border border-border/40 bg-background/30 px-2 py-0.5 text-[11px] text-muted-foreground/70 transition-all hover:border-border/60 hover:bg-background/50 hover:text-muted-foreground"
      >
        <Brain size={11} />
        <span>Reasoning</span>
        {collapsed ? <ChevronRight size={9} /> : <ChevronDown size={9} />}
      </button>
      {!collapsed && (
        <div className="mt-1.5 rounded-lg border border-border/40 bg-card/30 p-2.5 animate-in fade-in slide-in-from-top-1 duration-200">
          <div className="prose prose-sm max-w-none text-[11px] text-muted-foreground/80 dark:prose-invert prose-p:my-0.5 prose-headings:my-1.5 prose-code:text-[10px]">
            <SafeStreamdown content={text} />
          </div>
        </div>
      )}
    </div>
  );
}
