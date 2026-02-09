import { Bot } from "lucide-react";
import { Streamdown } from "streamdown";
import { code } from "@streamdown/code";
import type { ConversationItem } from "@/runtime/eventBuffer";

type AgentMessageItemProps = {
  item: ConversationItem;
};

export function AgentMessageItem({ item }: AgentMessageItemProps) {
  return (
    <div className="flex gap-3">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-accent text-muted-foreground">
        <Bot size={14} />
      </div>
      <div className="min-w-0 flex-1 pt-1">
        <div className="prose prose-sm max-w-none text-sm leading-relaxed text-foreground dark:prose-invert prose-p:my-1 prose-code:text-xs">
          <Streamdown plugins={{ code }}>{item.content ?? ""}</Streamdown>
        </div>
      </div>
    </div>
  );
}
