export function toDisplay(value: unknown): string {
  if (value == null) return "";
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function todoStatusClass(status: string): string {
  if (status === "completed") {
    return "rounded-full border border-success/30 bg-success/10 px-1.5 py-0.5 text-[10px] text-success";
  }
  if (status === "in_progress") {
    return "rounded-full border border-info/30 bg-info/10 px-1.5 py-0.5 text-[10px] text-info";
  }
  if (status === "cancelled") {
    return "rounded-full border border-warning/30 bg-warning/10 px-1.5 py-0.5 text-[10px] text-warning";
  }
  return "rounded-full border border-border/60 bg-background/70 px-1.5 py-0.5 text-[10px] text-muted-foreground";
}

export type PromptSegment =
  | {
      type: "text";
      value: string;
    }
  | {
      type: "mention";
      value: string;
      mentionKind: "agent" | "reference";
    };

export function parsePromptSegments(content: string): PromptSegment[] {
  if (!content) return [];

  const rawParts = content.split(/(@[A-Za-z0-9._:/-]+)/g).filter((part) => part.length > 0);
  return rawParts.map((part) => {
    if (!part.startsWith("@")) {
      return {
        type: "text",
        value: part,
      };
    }

    const mentionValue = part;
    const mentionKind = mentionValue.startsWith("@agent:") ? "agent" : "reference";
    return {
      type: "mention",
      value: mentionValue,
      mentionKind,
    };
  });
}
