import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useEffect, useState, type MouseEvent } from "react";
import {
  ActivitySquare,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Folder,
  FolderOpen,
  GitBranch,
  MessageSquare,
  Plus,
  Settings2,
  Trash2,
  X,
  type LucideIcon,
} from "lucide-react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { SETTINGS_SECTIONS, type SettingsSectionId } from "@/components/Settings/types";
import { cn } from "@/lib/utils";
import type { TaskRow, TaskStatus } from "@/types";

const SIDEBAR_WORKSPACES_STORAGE_KEY = "orchestrix.sidebar.workspace-roots";

type SidebarProps = {
  activeView: "chat" | "settings" | "benchmarks";
  activeSettingsSection: SettingsSectionId;
  showBenchmarks?: boolean;
  onToggleSidebar?: () => void;
  onOpenChat: () => void;
  onOpenSettings: (section?: SettingsSectionId) => void;
  onOpenBenchmarks: () => void;
};

type SidebarUtilityButtonProps = {
  icon: LucideIcon;
  label: string;
  shortcut?: string;
  active?: boolean;
  onClick: () => void;
};

type ChatHistoryEntryProps = {
  task: TaskRow;
  isSelected: boolean;
  canManage: boolean;
  onOpenTask: (taskId: string) => void;
  onForkTask: (taskId: string) => void;
  onDeleteTask: (taskId: string) => void;
};

function taskAge(iso: string): string {
  const delta = Date.now() - new Date(iso).getTime();
  const minutes = Math.max(1, Math.floor(delta / 60000));
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  return `${Math.floor(hours / 24)}d`;
}

function workspaceLabel(root: string): string {
  return root.replace(/\\/g, "/").split("/").filter(Boolean).pop() ?? root;
}

function normalizeWorkspaceRoots(roots: Array<string | null | undefined>): string[] {
  const seen = new Set<string>();
  const normalized: string[] = [];

  for (const root of roots) {
    const value = root?.trim();
    if (!value || seen.has(value)) continue;
    seen.add(value);
    normalized.push(value);
  }

  return normalized;
}

function sameWorkspaceRoots(left: string[], right: string[]): boolean {
  if (left.length !== right.length) return false;
  return left.every((value, index) => value === right[index]);
}

function readStoredWorkspaceRoots(): string[] {
  if (typeof window === "undefined") return [];

  try {
    const raw = window.localStorage.getItem(SIDEBAR_WORKSPACES_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return normalizeWorkspaceRoots(parsed.filter((value): value is string => typeof value === "string"));
  } catch {
    return [];
  }
}

function writeStoredWorkspaceRoots(roots: string[]): void {
  if (typeof window === "undefined") return;

  try {
    window.localStorage.setItem(SIDEBAR_WORKSPACES_STORAGE_KEY, JSON.stringify(roots));
  } catch {
    console.error("Failed to persist sidebar workspaces");
  }
}

function isWorkspaceExpanded(
  expandedRoots: Record<string, boolean>,
  workspaceRootKey: string,
  activeWorkspaceRoot: string
): boolean {
  return expandedRoots[workspaceRootKey] ?? workspaceRootKey === activeWorkspaceRoot;
}

const STATUS_META: Record<TaskStatus, { dotClassName: string; label: string }> = {
  pending: {
    dotClassName: "bg-muted-foreground/45",
    label: "Pending",
  },
  planning: {
    dotClassName: "bg-info animate-pulse",
    label: "Planning",
  },
  awaiting_review: {
    dotClassName: "bg-warning",
    label: "Awaiting review",
  },
  executing: {
    dotClassName: "bg-info animate-pulse",
    label: "Executing",
  },
  completed: {
    dotClassName: "bg-success",
    label: "Completed",
  },
  failed: {
    dotClassName: "bg-destructive",
    label: "Failed",
  },
  cancelled: {
    dotClassName: "bg-warning",
    label: "Cancelled",
  },
};

function SidebarUtilityButton({
  icon: Icon,
  label,
  shortcut,
  active = false,
  onClick,
}: SidebarUtilityButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-3 rounded-none px-2 py-1.5 text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-sidebar",
        active
          ? "bg-accent/55 text-foreground"
          : "text-muted-foreground hover:bg-accent/35 hover:text-foreground"
      )}
    >
      <Icon size={14} className="shrink-0" />
      <span className="flex-1 text-left">{label}</span>
      {shortcut ? (
        <kbd className="hidden rounded-sm bg-muted/80 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground lg:inline">
          {shortcut}
        </kbd>
      ) : null}
    </button>
  );
}

function ChatHistoryEntry({
  task,
  isSelected,
  canManage,
  onOpenTask,
  onForkTask,
  onDeleteTask,
}: ChatHistoryEntryProps) {
  const statusMeta = STATUS_META[task.status];
  const age = taskAge(task.updated_at);

  const handleForkTask = (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    onForkTask(task.id);
  };

  const handleDeleteTask = (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    if (window.confirm("Delete this conversation?")) {
      onDeleteTask(task.id);
    }
  };

  return (
    <article
      className={cn(
        "group flex items-center gap-2 px-2 py-1.5 transition-colors focus-within:ring-2 focus-within:ring-ring focus-within:ring-inset",
        isSelected ? "bg-accent/55 text-foreground" : "text-muted-foreground hover:bg-accent/35 hover:text-foreground"
      )}
    >
      <button
        type="button"
        onClick={() => onOpenTask(task.id)}
        className="flex min-w-0 flex-1 items-center gap-2 text-left focus-visible:outline-none"
        aria-label={`${task.prompt}, ${statusMeta.label}, updated ${age} ago`}
        title={`${statusMeta.label} - ${age} ago`}
      >
        <span className={cn("h-1.5 w-1.5 shrink-0 rounded-full", statusMeta.dotClassName)} />
        <p
          className={cn(
            "min-w-0 flex-1 truncate text-[13px] leading-5",
            isSelected ? "font-semibold text-foreground" : "font-medium text-sidebar-foreground/88 group-hover:text-foreground"
          )}
        >
          {task.prompt}
        </p>
      </button>

      <div className="grid w-14 shrink-0 justify-items-end">
        <span
          className={cn(
            "col-start-1 row-start-1 text-[10px] font-medium text-muted-foreground/55 transition-opacity",
            canManage && "group-hover:opacity-0 group-focus-within:opacity-0"
          )}
        >
          {age}
        </span>

        {canManage ? (
          <div className="col-start-1 row-start-1 flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
            <button
              type="button"
              className="rounded-none p-1 text-muted-foreground/60 transition-colors hover:bg-primary/10 hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              title="Fork conversation"
              aria-label="Fork conversation"
              onClick={handleForkTask}
            >
              <GitBranch size={12} />
            </button>
            <button
              type="button"
              className="rounded-none p-1 text-muted-foreground/60 transition-colors hover:bg-destructive/10 hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              title="Delete conversation"
              aria-label="Delete conversation"
              onClick={handleDeleteTask}
            >
              <Trash2 size={12} />
            </button>
          </div>
        ) : null}
      </div>
    </article>
  );
}

export function Sidebar({
  activeView,
  activeSettingsSection,
  showBenchmarks = false,
  onToggleSidebar,
  onOpenChat,
  onOpenSettings,
  onOpenBenchmarks,
}: SidebarProps) {
  const [expandedWorkspaceRoots, setExpandedWorkspaceRoots] = useState<Record<string, boolean>>({});
  const [knownWorkspaceRoots, setKnownWorkspaceRoots] = useState<string[]>(() => readStoredWorkspaceRoots());
  const [workspaceTasksByRoot, setWorkspaceTasksByRoot] = useState<Record<string, TaskRow[]>>({});
  const [loadingWorkspaceRoots, setLoadingWorkspaceRoots] = useState<Record<string, boolean>>({});
  const [tasks, selectedTaskId, workspaceRoot, selectTask, branchTask, deleteTask, setWorkspaceRoot] = useAppStore(
    useShallow((state) => [
      state.tasks,
      state.selectedTaskId,
      state.workspaceRoot,
      state.selectTask,
      state.branchTask,
      state.deleteTask,
      state.setWorkspaceRoot,
    ])
  );

  const persistWorkspaceRoots = (nextValue: string[] | ((current: string[]) => string[])) => {
    setKnownWorkspaceRoots((current) => {
      const resolved = typeof nextValue === "function" ? nextValue(current) : nextValue;
      const normalized = normalizeWorkspaceRoots(resolved);
      if (sameWorkspaceRoots(current, normalized)) return current;
      writeStoredWorkspaceRoots(normalized);
      return normalized;
    });
  };

  useEffect(() => {
    if (!workspaceRoot) return;
    persistWorkspaceRoots((current) => [workspaceRoot, ...current]);
    setExpandedWorkspaceRoots((current) => ({ ...current, [workspaceRoot]: true }));
  }, [workspaceRoot]);

  useEffect(() => {
    if (!workspaceRoot) return;
    setWorkspaceTasksByRoot((current) => ({ ...current, [workspaceRoot]: tasks }));
  }, [workspaceRoot, tasks]);

  useEffect(() => {
    let cancelled = false;
    const rootsToFetch = knownWorkspaceRoots.filter(
      (root) => root && root !== workspaceRoot && workspaceTasksByRoot[root] === undefined && !loadingWorkspaceRoots[root]
    );

    if (rootsToFetch.length === 0) return;

    const loadWorkspaceTasks = async () => {
      for (const root of rootsToFetch) {
        setLoadingWorkspaceRoots((current) => ({ ...current, [root]: true }));

        try {
          const projectTasks = await invoke<TaskRow[]>("list_tasks", { workspaceRoot: root || null });
          if (!cancelled) {
            setWorkspaceTasksByRoot((current) => ({ ...current, [root]: projectTasks }));
          }
        } catch (error) {
          console.error("Failed to load workspace tasks", error);
          if (!cancelled) {
            setWorkspaceTasksByRoot((current) => ({ ...current, [root]: [] }));
          }
        } finally {
          if (!cancelled) {
            setLoadingWorkspaceRoots((current) => ({ ...current, [root]: false }));
          }
        }
      }
    };

    void loadWorkspaceTasks();

    return () => {
      cancelled = true;
    };
  }, [knownWorkspaceRoots, loadingWorkspaceRoots, workspaceRoot, workspaceTasksByRoot]);

  const handleAddWorkspace = async () => {
    const selected = await openDialog({
      directory: true,
      title: "Add workspace folder",
      defaultPath: workspaceRoot || undefined,
    });

    if (typeof selected !== "string" || selected.length === 0) return;

    persistWorkspaceRoots((current) => [selected, ...current]);
    setExpandedWorkspaceRoots((current) => ({ ...current, [selected]: true }));
    await setWorkspaceRoot(selected);
    selectTask(null);
    onOpenChat();
  };

  const handleSelectWorkspace = async (workspaceRootKey: string) => {
    const expanded = isWorkspaceExpanded(expandedWorkspaceRoots, workspaceRootKey, workspaceRoot);

    if (workspaceRootKey === workspaceRoot) {
      setExpandedWorkspaceRoots((current) => ({ ...current, [workspaceRootKey]: !expanded }));
      onOpenChat();
      return;
    }

    setExpandedWorkspaceRoots((current) => ({ ...current, [workspaceRootKey]: true }));
    await setWorkspaceRoot(workspaceRootKey);
    onOpenChat();
  };

  const handleCreateConversation = async (workspaceRootKey: string) => {
    if (workspaceRootKey !== workspaceRoot) {
      await setWorkspaceRoot(workspaceRootKey);
    }

    selectTask(null);
    onOpenChat();
  };

  const handleOpenTask = async (workspaceRootKey: string, taskId: string) => {
    if (workspaceRootKey !== workspaceRoot) {
      await setWorkspaceRoot(workspaceRootKey);
    }

    selectTask(taskId);
    onOpenChat();
  };

  const handleForkTask = (taskId: string) => {
    branchTask(taskId).catch(console.error);
  };

  const handleDeleteTask = (taskId: string) => {
    deleteTask(taskId).catch(console.error);
  };

  const handleRemoveWorkspace = async (workspaceRootKey: string) => {
    if (knownWorkspaceRoots.length <= 1) return;
    if (!window.confirm(`Remove ${workspaceLabel(workspaceRootKey)} from the sidebar?`)) return;

    const remainingRoots = knownWorkspaceRoots.filter((root) => root !== workspaceRootKey);
    persistWorkspaceRoots(remainingRoots);

    setWorkspaceTasksByRoot((current) => {
      const next = { ...current };
      delete next[workspaceRootKey];
      return next;
    });

    setLoadingWorkspaceRoots((current) => {
      const next = { ...current };
      delete next[workspaceRootKey];
      return next;
    });

    setExpandedWorkspaceRoots((current) => {
      const next = { ...current };
      delete next[workspaceRootKey];
      return next;
    });

    if (workspaceRootKey === workspaceRoot && remainingRoots[0]) {
      await setWorkspaceRoot(remainingRoots[0]);
      selectTask(null);
      onOpenChat();
    }
  };

  const visibleWorkspaceRoots = normalizeWorkspaceRoots([workspaceRoot, ...knownWorkspaceRoots]);

  return (
    <div data-sidebar="true" className="flex h-full min-h-0 flex-col text-sidebar-foreground">
      <div className="border-b border-sidebar-border/70 px-2 pb-1.5 pt-2">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-1">
            {onToggleSidebar ? (
              <Button
                variant="ghost"
                size="icon"
                className="size-7 rounded-none text-muted-foreground/65 hover:bg-accent/30 hover:text-foreground"
                onClick={onToggleSidebar}
                title="Collapse sidebar"
                aria-label="Collapse sidebar"
              >
                <ChevronLeft size={15} />
              </Button>
            ) : null}
            <p className="text-[10px] font-semibold uppercase tracking-[0.22em] text-muted-foreground/60">
              Projects
            </p>
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="size-7 rounded-none text-muted-foreground/75 hover:bg-accent/35 hover:text-foreground"
            onClick={() => {
              void handleAddWorkspace();
            }}
            title="Add workspace"
            aria-label="Add workspace"
          >
            <Plus size={15} />
          </Button>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-1 py-2">
        <div className="space-y-1">
          {visibleWorkspaceRoots.map((workspaceRootKey) => {
            const projectTasks = workspaceRootKey === workspaceRoot ? tasks : workspaceTasksByRoot[workspaceRootKey] ?? [];
            const isExpanded = isWorkspaceExpanded(expandedWorkspaceRoots, workspaceRootKey, workspaceRoot);
            const isActiveWorkspace = workspaceRootKey === workspaceRoot;
            const isLoading = workspaceRootKey !== workspaceRoot && loadingWorkspaceRoots[workspaceRootKey];
            const canRemoveWorkspace = visibleWorkspaceRoots.length > 1;
            const WorkspaceIcon = isExpanded ? FolderOpen : Folder;

            return (
              <section key={workspaceRootKey}>
                <div className="group flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() => {
                      void handleSelectWorkspace(workspaceRootKey);
                    }}
                    className={cn(
                      "flex min-w-0 flex-1 items-center gap-2 rounded-none px-1.5 py-1.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-sidebar",
                      isActiveWorkspace
                        ? "bg-accent/40 text-foreground"
                        : "text-muted-foreground hover:bg-accent/25 hover:text-foreground"
                    )}
                    aria-expanded={isExpanded}
                    title={workspaceRootKey}
                  >
                    {isExpanded ? (
                      <ChevronDown size={14} className="shrink-0 text-muted-foreground/70" />
                    ) : (
                      <ChevronRight size={14} className="shrink-0 text-muted-foreground/70" />
                    )}
                    <WorkspaceIcon size={14} className="shrink-0 text-muted-foreground/70" />
                    <span
                      className={cn(
                        "min-w-0 flex-1 truncate text-[13px] leading-5",
                        isActiveWorkspace ? "font-semibold tracking-tight text-sidebar-foreground" : "font-medium"
                      )}
                    >
                      {workspaceLabel(workspaceRootKey)}
                    </span>
                    <span className="text-[10px] font-medium text-muted-foreground/50">{projectTasks.length}</span>
                  </button>

                  <Button
                    variant="ghost"
                    size="icon"
                    className={cn(
                      "size-6 rounded-none text-muted-foreground/70 hover:bg-accent/25 hover:text-foreground",
                      !isActiveWorkspace && "opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
                    )}
                    onClick={() => {
                      void handleCreateConversation(workspaceRootKey);
                    }}
                    data-sidebar-action={isActiveWorkspace ? "new-conversation" : undefined}
                    title={`New conversation in ${workspaceLabel(workspaceRootKey)}`}
                    aria-label={`New conversation in ${workspaceLabel(workspaceRootKey)}`}
                  >
                    <Plus size={14} />
                  </Button>

                  {canRemoveWorkspace ? (
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-6 rounded-none text-muted-foreground/60 opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive group-hover:opacity-100 group-focus-within:opacity-100"
                      onClick={() => {
                        void handleRemoveWorkspace(workspaceRootKey);
                      }}
                      title={`Remove ${workspaceLabel(workspaceRootKey)}`}
                      aria-label={`Remove ${workspaceLabel(workspaceRootKey)}`}
                    >
                      <X size={13} />
                    </Button>
                  ) : null}
                </div>

                {isExpanded ? (
                  <div
                    className={cn(
                      "mt-1",
                      projectTasks.length > 0 && "relative ml-2.5 pl-3 before:absolute before:bottom-1 before:left-[3px] before:top-1 before:w-px before:bg-sidebar-border/70"
                    )}
                  >
                    {isLoading ? (
                      <p className="px-2 py-1 text-xs leading-5 text-muted-foreground/60">Loading conversations...</p>
                    ) : projectTasks.length === 0 ? (
                      <div className="px-2 py-1">
                        <p className="text-xs leading-5 text-muted-foreground/60">No conversations yet.</p>
                        {isActiveWorkspace ? (
                          <button
                            type="button"
                            onClick={() => {
                              void handleCreateConversation(workspaceRootKey);
                            }}
                            className="mt-1 text-xs font-medium text-foreground/85 transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-sidebar"
                          >
                            Start a conversation
                          </button>
                        ) : null}
                      </div>
                    ) : (
                      <div className="space-y-1">
                        {projectTasks.map((task) => (
                          <ChatHistoryEntry
                            key={task.id}
                            task={task}
                            isSelected={isActiveWorkspace && task.id === selectedTaskId}
                            canManage={isActiveWorkspace}
                            onOpenTask={(taskId) => {
                              void handleOpenTask(workspaceRootKey, taskId);
                            }}
                            onForkTask={handleForkTask}
                            onDeleteTask={handleDeleteTask}
                          />
                        ))}
                      </div>
                    )}
                  </div>
                ) : null}
              </section>
            );
          })}
        </div>
      </div>

      {activeView === "settings" ? (
        <div className="border-t border-sidebar-border/70 px-2 py-2">
          <p className="px-1 pb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-muted-foreground/60">
            Settings Sections
          </p>
          <div className="space-y-1">
            {SETTINGS_SECTIONS.map((section, idx) => {
              const Icon = section.icon;
              const isActiveSection = activeSettingsSection === section.id;

              return (
                <button
                  key={section.id}
                  type="button"
                  onClick={() => onOpenSettings(section.id)}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-none px-2 py-1.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-sidebar",
                    isActiveSection
                      ? "bg-accent/55 text-foreground"
                      : "text-muted-foreground hover:bg-accent/35 hover:text-foreground"
                  )}
                >
                  <Icon size={14} className="shrink-0" />
                  <span className="flex-1 truncate text-[13px] font-medium leading-5">{section.label}</span>
                  <kbd className="hidden rounded-sm bg-muted/80 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground lg:inline">
                    Shift+{idx + 1}
                  </kbd>
                </button>
              );
            })}
          </div>
        </div>
      ) : null}

      <div className="border-t border-sidebar-border/70 px-2 py-2">
        {activeView !== "chat" ? (
          <SidebarUtilityButton
            icon={MessageSquare}
            label="Conversations"
            shortcut="Ctrl+1"
            onClick={onOpenChat}
          />
        ) : null}

        {showBenchmarks ? (
          <SidebarUtilityButton
            icon={ActivitySquare}
            label="Benchmarks"
            shortcut="Ctrl+3"
            active={activeView === "benchmarks"}
            onClick={onOpenBenchmarks}
          />
        ) : null}

        <SidebarUtilityButton
          icon={Settings2}
          label="Settings"
          shortcut="Ctrl+2"
          active={activeView === "settings"}
          onClick={() => onOpenSettings()}
        />
      </div>
    </div>
  );
}
