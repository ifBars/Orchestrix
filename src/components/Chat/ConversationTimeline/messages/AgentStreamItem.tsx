import { memo } from "react";
import { Bot } from "lucide-react";
import type { AgentMessageStream } from "@/runtime/eventBuffer";
import { SafeStreamdown } from "./SafeStreamdown";
import { TypingCursor } from "./TypingCursor";

type AgentStreamItemProps = {
  stream: AgentMessageStream;
};

export const AgentStreamItem = memo(function AgentStreamItem({ stream }: AgentStreamItemProps) {
  return (
    <div className="flex gap-3">
      <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-border/70 bg-accent/50 text-muted-foreground">
        <Bot size={14} />
      </div>
      <div className="min-w-0 flex-1 rounded-xl border border-border/70 bg-card/55 px-3 py-2.5">
        <div className="text-sm leading-relaxed text-foreground">
          <div className="prose prose-sm max-w-none dark:prose-invert prose-p:my-1 prose-code:text-xs [&>*:not(:last-child)]:block [&>*:last-child]:inline">
            <SafeStreamdown content={stream.content} isStreaming={stream.isStreaming} />
          </div>
          {stream.isStreaming && <TypingCursor />}
        </div>
      </div>
    </div>
  );
});
