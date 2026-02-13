import { Bot } from "lucide-react";
import { parsePromptSegments } from "../utils";

type PromptWithMentionsProps = {
  content: string;
  className?: string;
};

export function PromptWithMentions({ content, className }: PromptWithMentionsProps) {
  const segments = parsePromptSegments(content);

  return (
    <p className={className}>
      {segments.map((segment, idx) => {
        if (segment.type === "text") {
          return <span key={`${segment.type}-${idx}`}>{segment.value}</span>;
        }

        if (segment.mentionKind === "agent") {
          return (
            <span
              key={`${segment.type}-${segment.value}-${idx}`}
              className="mx-0.5 inline-flex items-center gap-1 rounded-md border border-info/35 bg-info/12 px-1.5 py-0.5 text-[0.82em] font-medium text-info"
            >
              <Bot size={11} />
              <span>{segment.value}</span>
            </span>
          );
        }

        return (
          <span
            key={`${segment.type}-${segment.value}-${idx}`}
            className="mx-0.5 inline-flex items-center rounded-md border border-border/70 bg-accent/45 px-1.5 py-0.5 text-[0.82em] font-medium text-foreground"
          >
            {segment.value}
          </span>
        );
      })}
    </p>
  );
}
