import { FileText, Folder, Bot, Sparkles, Terminal, GitBranch, Plus } from "lucide-react";
import type { SlashMatch } from "./slashCommands";
import type { WorkspaceReferenceCandidate } from "@/types";

interface MentionGroup {
  group: string;
  items: WorkspaceReferenceCandidate[];
}

interface ComposerSuggestionPopupProps {
  type: "mention" | "slash";
  isOpen: boolean;
  items: MentionGroup[] | SlashMatch[];
  activeIndex: number;
  onSelect: (index: number) => void;
}

function getIconForKind(kind: WorkspaceReferenceCandidate["kind"]) {
  switch (kind) {
    case "file":
      return <FileText size={12} />;
    case "directory":
      return <Folder size={12} />;
    case "agent":
      return <Bot size={12} />;
    default:
      return <Sparkles size={12} />;
  }
}

function getIconForSlashCommand(commandId: SlashMatch["command"]["id"]) {
  switch (commandId) {
    case "compact":
      return <Terminal size={12} />;
    case "fork":
      return <GitBranch size={12} />;
    case "new":
      return <Plus size={12} />;
    default:
      return <Terminal size={12} />;
  }
}

export function ComposerSuggestionPopup({
  type,
  isOpen,
  items,
  activeIndex,
  onSelect,
}: ComposerSuggestionPopupProps) {
  if (!isOpen || items.length === 0) return null;

  if (type === "mention") {
    return (
      <div className="mx-3 mb-2 rounded-xl border border-border/80 bg-background/95 p-1.5 elevation-2">
        {(() => {
          const groups = items as MentionGroup[];
          let flatIndex = 0;
          return groups.map((group) => (
            <div key={group.group} className="mb-1.5 last:mb-0">
              <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground/60">
                {group.group}
              </div>
              {group.items.map((item) => {
                const idx = flatIndex++;
                const isActive = idx === activeIndex;
                const isAgent = item.kind === "agent";
                return (
                  <button
                    key={`${item.kind}:${item.value}`}
                    type="button"
                    onClick={() => onSelect(idx)}
                    className={`flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-xs transition-colors ${
                      isActive
                        ? isAgent
                          ? "bg-info/14 text-foreground"
                          : "bg-accent text-foreground"
                        : isAgent
                        ? "text-muted-foreground hover:bg-info/10 hover:text-foreground"
                        : "text-muted-foreground hover:bg-accent/60"
                    }`}
                  >
                    {getIconForKind(item.kind)}
                    <span className="truncate font-medium">@{item.value}</span>
                    {isAgent && (
                      <span className="rounded-full border border-info/35 bg-info/10 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-info">
                        Preset
                      </span>
                    )}
                    <span className="ml-auto truncate text-[10px] text-muted-foreground/70">
                      {item.description}
                    </span>
                  </button>
                );
              })}
            </div>
          ));
        })()}
      </div>
    );
  }

  // Slash command popup
  const slashItems = items as SlashMatch[];
  return (
    <div className="mx-3 mb-2 rounded-xl border border-border/80 bg-background/95 p-1.5 elevation-2">
      <div className="mb-1.5 last:mb-0">
        <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground/60">
          Commands
        </div>
        {slashItems.map((match, idx) => {
          const isActive = idx === activeIndex;
          return (
            <button
              key={match.command.id}
              type="button"
              onClick={() => onSelect(idx)}
              className={`flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-xs transition-colors ${
                isActive
                  ? "bg-accent text-foreground"
                  : "text-muted-foreground hover:bg-accent/60"
              }`}
            >
              {getIconForSlashCommand(match.command.id)}
              <span className="truncate font-medium">{match.command.command}</span>
              {match.aliasMatched && (
                <span className="rounded-full border border-info/35 bg-info/10 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-info">
                  {match.aliasMatched}
                </span>
              )}
              <span className="ml-auto truncate text-[10px] text-muted-foreground/70">
                {match.command.description}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
