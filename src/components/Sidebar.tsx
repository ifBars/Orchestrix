import { Bot, Plus, Settings, Sparkles, Trash2 } from "lucide-react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";

type SidebarProps = {
  onOpenSettings: () => void;
  onOpenSkills: () => void;
  onOpenAgents: () => void;
  onOpenChat: () => void;
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

export function Sidebar({ onOpenSettings, onOpenSkills, onOpenAgents, onOpenChat }: SidebarProps) {
  const [tasks, selectedTaskId, selectTask, providerConfigs, deleteTask] = useAppStore(
    useShallow((state) => [
      state.tasks,
      state.selectedTaskId,
      state.selectTask,
      state.providerConfigs,
      state.deleteTask,
    ])
  );

  const configured = providerConfigs.filter((item) => item.configured).length;

  return (
    <div className="flex h-full flex-col">
      {/* New run button */}
      <div className="p-3">
        <Button
          className="w-full justify-center gap-2"
          onClick={() => {
            selectTask(null);
            onOpenChat();
          }}
        >
          <Plus size={14} />
          New Conversation
        </Button>
      </div>

      {/* Navigation */}
      <div className="space-y-0.5 px-3 pb-3">
        <button
          type="button"
          className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground"
          onClick={onOpenSkills}
        >
          <Sparkles size={14} />
          Skills
        </button>
        <button
          type="button"
          className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground"
          onClick={onOpenAgents}
        >
          <Bot size={14} />
          Agents
        </button>
        <button
          type="button"
          className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground"
          onClick={onOpenSettings}
        >
          <Settings size={14} />
          Providers
          {configured > 0 && (
            <span className="ml-auto rounded-full bg-success/15 px-1.5 py-0.5 text-[10px] font-medium text-success">
              {configured}
            </span>
          )}
        </button>
      </div>

      {/* Divider */}
      <div className="mx-3 border-t border-sidebar-border" />

      {/* Conversation list */}
      <div className="flex items-center px-4 pt-3 pb-1.5">
        <span className="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/70">
          History
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        {tasks.length === 0 ? (
          <div className="p-4 text-center text-xs text-muted-foreground/60">
            No conversations yet
          </div>
        ) : (
          <div className="space-y-0.5">
            {tasks.map((task) => {
              const selected = task.id === selectedTaskId;
              return (
                <div
                  key={task.id}
                  className={`group rounded-lg px-3 py-2.5 transition-colors ${
                    selected
                      ? "bg-accent/60 text-foreground"
                      : "text-muted-foreground hover:bg-accent/30 hover:text-foreground"
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
                      <p className="truncate text-sm leading-snug">{task.prompt}</p>
                      <div className="mt-1 flex items-center gap-2 text-[10px] text-muted-foreground/60">
                        <span>{taskAge(task.updated_at)}</span>
                        <span className="capitalize">{task.status}</span>
                      </div>
                    </div>
                  </button>

                  {/* Delete button — only visible on hover */}
                  <div className="mt-1 flex justify-end opacity-0 transition-opacity group-hover:opacity-100">
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
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
