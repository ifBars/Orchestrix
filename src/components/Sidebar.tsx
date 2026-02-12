import { MessageSquare, Plus, Settings2, Trash2 } from "lucide-react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { SETTINGS_SECTIONS, type SettingsSectionId } from "@/components/Settings/types";

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

const statusDot: Record<string, string> = {
  pending: "bg-muted-foreground/40",
  planning: "bg-info animate-pulse",
  awaiting_review: "bg-warning",
  executing: "bg-info animate-pulse",
  completed: "bg-success",
  failed: "bg-destructive",
  cancelled: "bg-warning",
};

const statusLabel: Record<string, string> = {
  pending: "Pending",
  planning: "Planning",
  awaiting_review: "Review",
  executing: "Executing",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
};

const statusTone: Record<string, string> = {
  pending: "bg-muted text-muted-foreground",
  planning: "bg-info/15 text-info",
  awaiting_review: "bg-warning/15 text-warning",
  executing: "bg-info/15 text-info",
  completed: "bg-success/15 text-success",
  failed: "bg-destructive/15 text-destructive",
  cancelled: "bg-warning/15 text-warning",
};

export function Sidebar({
  activeView,
  activeSettingsSection,
  onOpenChat,
  onOpenSettings,
}: SidebarProps) {
  const [tasks, selectedTaskId, selectTask, deleteTask] = useAppStore(
    useShallow((state) => [
      state.tasks,
      state.selectedTaskId,
      state.selectTask,
      state.deleteTask,
    ])
  );

  return (
    <div className="flex h-full flex-col px-2 pb-2 pt-3">
      <div className="rounded-lg border border-sidebar-border/80 bg-background/45 p-2">
        <Button
          className="h-9 w-full justify-start gap-2"
          onClick={() => {
            selectTask(null);
            onOpenChat();
          }}
        >
          <Plus size={14} />
          New Conversation
        </Button>

        <div className="mt-2 space-y-1">
          <button
            type="button"
            className={`flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-sm transition-colors ${
              activeView === "chat"
                ? "bg-accent text-foreground"
                : "text-muted-foreground hover:bg-accent/60 hover:text-foreground"
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
            className={`flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-sm transition-colors ${
              activeView === "settings"
                ? "bg-accent text-foreground"
                : "text-muted-foreground hover:bg-accent/60 hover:text-foreground"
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
        <div className="mt-2 rounded-lg border border-sidebar-border/80 bg-background/35 p-2">
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
                  className={`flex w-full items-center rounded-md px-2 py-1.5 text-[11px] transition-colors ${
                    isActive
                      ? "bg-primary/12 text-foreground"
                      : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
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

      <div className="mt-3 flex items-center px-2 pb-1.5">
        <span className="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/70">
          History
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-1 pb-1">
        {tasks.length === 0 ? (
          <div className="rounded-lg border border-dashed border-sidebar-border/80 bg-background/35 p-4 text-center text-xs text-muted-foreground/60">
            No conversations yet
          </div>
        ) : (
          <div className="space-y-1">
            {tasks.map((task) => {
              const selected = task.id === selectedTaskId;
              const status = statusLabel[task.status] ?? task.status;
              return (
                <article
                  key={task.id}
                  className={`group rounded-lg border px-2.5 py-2 transition-colors ${
                    selected
                      ? "border-primary/35 bg-accent/65 text-foreground"
                      : "border-transparent bg-background/35 text-muted-foreground hover:border-sidebar-border/80 hover:bg-accent/35 hover:text-foreground"
                  }`}
                >
                  <button
                    type="button"
                    onClick={() => {
                      selectTask(task.id);
                      onOpenChat();
                    }}
                    className="flex w-full items-start gap-2.5 text-left"
                  >
                    <span
                      className={`mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full ${statusDot[task.status] ?? "bg-muted-foreground/40"}`}
                    />
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-[13px] leading-snug text-foreground">{task.prompt}</p>
                      <div className="mt-1.5 flex items-center gap-2 text-[10px] text-muted-foreground/70">
                        <span>{taskAge(task.updated_at)}</span>
                        <span
                          className={`rounded-full px-1.5 py-0.5 font-medium ${statusTone[task.status] ?? "bg-muted text-muted-foreground"}`}
                        >
                          {status}
                        </span>
                      </div>
                    </div>
                  </button>

                  <div className="mt-1 flex justify-end opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
                    <button
                      type="button"
                      className="rounded p-1 text-muted-foreground/50 transition-colors hover:bg-destructive/10 hover:text-destructive"
                      title="Delete conversation"
                      onClick={(e) => {
                        e.stopPropagation();
                        if (window.confirm("Delete this conversation?")) {
                          deleteTask(task.id).catch(console.error);
                        }
                      }}
                    >
                      <Trash2 size={12} />
                    </button>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
