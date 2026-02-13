import { GitBranch, MessageSquare, Plus, Settings2, Trash2 } from "lucide-react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { SETTINGS_SECTIONS, type SettingsSectionId } from "@/components/Settings/types";
import { cn } from "@/lib/utils";
import type { TaskRow, TaskStatus } from "@/types";

type SidebarProps = {
  activeView: "chat" | "settings";
  activeSettingsSection: SettingsSectionId;
  onOpenChat: () => void;
  onOpenSettings: (section?: SettingsSectionId) => void;
};

function taskAge(iso: string): string {
  const delta = Date.now() - new Date(iso).getTime();
  const minutes = Math.max(1, Math.floor(delta / 60000));
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  return `${Math.floor(hours / 24)}d`;
}

const STATUS_META: Record<TaskStatus, { dotClassName: string; label: string; badgeClassName: string }> = {
  pending: {
    dotClassName: "bg-muted-foreground/45",
    label: "Pending",
    badgeClassName: "bg-muted text-muted-foreground",
  },
  planning: {
    dotClassName: "bg-info animate-pulse",
    label: "Planning",
    badgeClassName: "bg-info/15 text-info",
  },
  awaiting_review: {
    dotClassName: "bg-warning",
    label: "Review",
    badgeClassName: "bg-warning/15 text-warning",
  },
  executing: {
    dotClassName: "bg-info animate-pulse",
    label: "Executing",
    badgeClassName: "bg-info/15 text-info",
  },
  completed: {
    dotClassName: "bg-success",
    label: "Completed",
    badgeClassName: "bg-success/15 text-success",
  },
  failed: {
    dotClassName: "bg-destructive",
    label: "Failed",
    badgeClassName: "bg-destructive/15 text-destructive",
  },
  cancelled: {
    dotClassName: "bg-warning",
    label: "Cancelled",
    badgeClassName: "bg-warning/15 text-warning",
  },
};

type ChatHistoryEntryProps = {
  task: TaskRow;
  isSelected: boolean;
  onOpenTask: (taskId: string) => void;
  onForkTask: (taskId: string) => void;
  onDeleteTask: (taskId: string) => void;
};

function ChatHistoryEntry({ task, isSelected, onOpenTask, onForkTask, onDeleteTask }: ChatHistoryEntryProps) {
  const statusMeta = STATUS_META[task.status];

  const handleOpenTask = () => {
    onOpenTask(task.id);
  };

  const handleForkTask = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    onForkTask(task.id);
  };

  const handleDeleteTask = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    if (window.confirm("Delete this conversation?")) {
      onDeleteTask(task.id);
    }
  };

  return (
    <article
      className={cn(
        "group relative overflow-hidden rounded-lg border px-2.5 py-2 transition-colors",
        isSelected
          ? "border-primary/40 bg-gradient-to-br from-card/95 via-card/90 to-accent/35 text-foreground shadow-sm"
          : "border-transparent bg-sidebar/55 text-muted-foreground hover:border-sidebar-border/80 hover:bg-accent/45 hover:text-foreground"
      )}
    >
      <div
        className={cn(
          "pointer-events-none absolute inset-y-0 left-0 w-px bg-transparent transition-colors",
          isSelected && "bg-primary/70"
        )}
      />

      <button
        type="button"
        onClick={handleOpenTask}
        className="flex w-full items-start gap-2.5 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-sidebar"
      >
        <span className={cn("mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full", statusMeta.dotClassName)} />
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-medium leading-snug text-foreground">{task.prompt}</p>
          <div className="mt-1.5 flex items-center gap-2 text-[10px] text-muted-foreground/70">
            <span>{taskAge(task.updated_at)}</span>
            <span className={cn("rounded-full px-1.5 py-0.5 font-medium", statusMeta.badgeClassName)}>
              {statusMeta.label}
            </span>
          </div>
        </div>
      </button>

      <div
        className={cn(
          "mt-1 flex justify-end gap-1 transition-opacity",
          isSelected ? "opacity-100" : "opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
        )}
      >
        <button
          type="button"
          className="rounded p-1 text-muted-foreground/55 transition-colors hover:bg-primary/10 hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          title="Fork conversation"
          aria-label="Fork conversation"
          onClick={handleForkTask}
        >
          <GitBranch size={12} />
        </button>
        <button
          type="button"
          className="rounded p-1 text-muted-foreground/55 transition-colors hover:bg-destructive/10 hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          title="Delete conversation"
          aria-label="Delete conversation"
          onClick={handleDeleteTask}
        >
          <Trash2 size={12} />
        </button>
      </div>
    </article>
  );
}

export function Sidebar({
  activeView,
  activeSettingsSection,
  onOpenChat,
  onOpenSettings,
}: SidebarProps) {
  const [tasks, selectedTaskId, selectTask, branchTask, deleteTask] = useAppStore(
    useShallow((state) => [
      state.tasks,
      state.selectedTaskId,
      state.selectTask,
      state.branchTask,
      state.deleteTask,
    ])
  );

  const handleCreateConversation = () => {
    selectTask(null);
    onOpenChat();
  };

  const handleOpenTask = (taskId: string) => {
    selectTask(taskId);
    onOpenChat();
  };

  const handleForkTask = (taskId: string) => {
    branchTask(taskId).catch(console.error);
  };

  const handleDeleteTask = (taskId: string) => {
    deleteTask(taskId).catch(console.error);
  };

  return (
    <div className="flex h-full flex-col gap-2 px-2 pb-2 pt-3 text-sidebar-foreground">
      <div className="rounded-lg border border-sidebar-border/80 bg-sidebar/75 p-2 backdrop-blur-sm">
        <Button
          className="h-9 w-full justify-start gap-2 rounded-md"
          onClick={handleCreateConversation}
        >
          <Plus size={14} />
          New Conversation
        </Button>

        <div className="mt-2 space-y-1">
          <button
            type="button"
            className={`flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-sidebar ${
              activeView === "chat"
                ? "bg-accent text-foreground"
                : "text-muted-foreground hover:bg-accent/70 hover:text-foreground"
            }`}
            onClick={onOpenChat}
          >
            <MessageSquare size={14} />
            <span className="flex-1 text-left">Chat</span>
            <kbd className="hidden rounded bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground lg:inline">
              Ctrl+1
            </kbd>
          </button>

          <button
            type="button"
            className={`flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-sidebar ${
              activeView === "settings"
                ? "bg-accent text-foreground"
                : "text-muted-foreground hover:bg-accent/70 hover:text-foreground"
            }`}
            onClick={() => onOpenSettings()}
          >
            <Settings2 size={14} />
            <span className="flex-1 text-left">Settings</span>
            <kbd className="hidden rounded bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground lg:inline">
              Ctrl+2
            </kbd>
          </button>
        </div>
      </div>

      {activeView === "settings" && (
        <div className="rounded-lg border border-sidebar-border/80 bg-sidebar/65 p-2 backdrop-blur-sm">
          <p className="px-1 pb-1 text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/70">
            Settings Sections
          </p>
          <div className="space-y-0.5">
            {SETTINGS_SECTIONS.map((section, idx) => {
              const isActive = activeSettingsSection === section.id;
              const shortcutNum = idx + 1;
              return (
                <button
                  key={section.id}
                  type="button"
                  className={`flex w-full items-center rounded-md px-2 py-1.5 text-[11px] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-sidebar ${
                    isActive
                      ? "bg-primary/12 text-foreground"
                      : "text-muted-foreground hover:bg-accent/60 hover:text-foreground"
                  }`}
                  onClick={() => onOpenSettings(section.id)}
                >
                  <span className="flex-1 text-left">{section.label}</span>
                  <kbd className="hidden rounded bg-muted px-1 py-0.5 font-mono text-[9px] text-muted-foreground/70 lg:inline">
                    Shift+{shortcutNum}
                  </kbd>
                </button>
              );
            })}
          </div>
        </div>
      )}

      <div className="mt-1 flex items-center justify-between px-2 pb-1.5 pt-1">
        <span className="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/70">
          History
        </span>
        <span className="text-[10px] font-medium text-muted-foreground/60">{tasks.length}</span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-1 pb-1">
        {tasks.length === 0 ? (
          <div className="rounded-lg border border-dashed border-sidebar-border/80 bg-sidebar/55 p-4 text-center text-xs text-muted-foreground/70">
            No conversation history
          </div>
        ) : (
          <div className="space-y-1">
            {tasks.map((task) => {
              return (
                <ChatHistoryEntry
                  key={task.id}
                  task={task}
                  isSelected={task.id === selectedTaskId}
                  onOpenTask={handleOpenTask}
                  onForkTask={handleForkTask}
                  onDeleteTask={handleDeleteTask}
                />
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
