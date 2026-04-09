import { useEffect } from "react";
import {
  MessageSquare,
  Settings,
  FolderOpen,
  Plus,
  Moon,
  Sun,
  PanelRight,
  Command,
  ActivitySquare,
  Trash2,
  GitBranch,
  FileText,
} from "lucide-react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "@/components/ui/command";
import type { TaskRow } from "@/types";

type CommandPaletteProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onOpenChat: () => void;
  onOpenSettings: () => void;
  onOpenBenchmarks: () => void;
  onNewConversation: () => void;
  onSelectWorkspace: () => void;
  darkMode: boolean;
  onToggleTheme: () => void;
  artifactsOpen: boolean;
  onToggleArtifacts: () => void;
};

export function CommandPalette({
  open,
  onOpenChange,
  onOpenChat,
  onOpenSettings,
  onOpenBenchmarks,
  onNewConversation,
  onSelectWorkspace,
  darkMode,
  onToggleTheme,
  artifactsOpen,
  onToggleArtifacts,
}: CommandPaletteProps) {
  const [tasks, selectedTaskId, selectTask, deleteTask, branchTask] = useAppStore(
    useShallow((state) => [
      state.tasks,
      state.selectedTaskId,
      state.selectTask,
      state.deleteTask,
      state.branchTask,
    ])
  );

  // Handle keyboard shortcut (Ctrl+Shift+P or Cmd+Shift+P)
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() === "p" && (e.metaKey || e.ctrlKey) && e.shiftKey) {
        e.preventDefault();
        onOpenChange(!open);
      }
    };

    document.addEventListener("keydown", down);
    return () => document.removeEventListener("keydown", down);
  }, [open, onOpenChange]);

  const handleSelectTask = (taskId: string) => {
    selectTask(taskId);
    onOpenChat();
    onOpenChange(false);
  };

  const handleDeleteTask = async (taskId: string) => {
    await deleteTask(taskId);
  };

  const handleForkTask = async (taskId: string) => {
    await branchTask(taskId);
  };

  const handleNewConversation = () => {
    onNewConversation();
    onOpenChange(false);
  };

  const handleOpenSettings = () => {
    onOpenSettings();
    onOpenChange(false);
  };

  const handleOpenChat = () => {
    onOpenChat();
    onOpenChange(false);
  };

  const handleOpenBenchmarks = () => {
    onOpenBenchmarks();
    onOpenChange(false);
  };

  const handleSelectWorkspace = () => {
    onSelectWorkspace();
    onOpenChange(false);
  };

  const handleToggleTheme = () => {
    onToggleTheme();
    onOpenChange(false);
  };

  const handleToggleArtifacts = () => {
    onToggleArtifacts();
    onOpenChange(false);
  };

  return (
    <CommandDialog open={open} onOpenChange={onOpenChange}>
      <CommandInput placeholder="Type a command or search..." />
      <CommandList>
        <CommandEmpty>No results found.</CommandEmpty>

        <CommandGroup heading="Actions">
      <CommandItem onSelect={handleNewConversation}>
        <Plus className="mr-2 h-4 w-4" />
        <span>New Conversation</span>
      </CommandItem>
          <CommandItem onSelect={handleOpenChat}>
            <MessageSquare className="mr-2 h-4 w-4" />
            <span>Open Chat</span>
            <CommandShortcut>Ctrl+1</CommandShortcut>
          </CommandItem>
          <CommandItem onSelect={handleOpenSettings}>
            <Settings className="mr-2 h-4 w-4" />
            <span>Open Settings</span>
            <CommandShortcut>Ctrl+2</CommandShortcut>
          </CommandItem>
          <CommandItem onSelect={handleOpenBenchmarks}>
            <ActivitySquare className="mr-2 h-4 w-4" />
            <span>Open Benchmarks</span>
            <CommandShortcut>Ctrl+3</CommandShortcut>
          </CommandItem>
        </CommandGroup>

        <CommandSeparator />

        <CommandGroup heading="Workspace">
          <CommandItem onSelect={handleSelectWorkspace}>
            <FolderOpen className="mr-2 h-4 w-4" />
            <span>Change Workspace</span>
          </CommandItem>
          <CommandItem onSelect={handleToggleArtifacts}>
            <PanelRight className="mr-2 h-4 w-4" />
            <span>{artifactsOpen ? "Hide" : "Show"} Artifacts Panel</span>
          </CommandItem>
        </CommandGroup>

        <CommandSeparator />

        <CommandGroup heading="Appearance">
          <CommandItem onSelect={handleToggleTheme}>
            {darkMode ? (
              <>
                <Sun className="mr-2 h-4 w-4" />
                <span>Switch to Light Theme</span>
              </>
            ) : (
              <>
                <Moon className="mr-2 h-4 w-4" />
                <span>Switch to Dark Theme</span>
              </>
            )}
            <CommandShortcut>Ctrl+Shift+L</CommandShortcut>
          </CommandItem>
        </CommandGroup>

        {tasks.length > 0 && (
          <>
            <CommandSeparator />
            <CommandGroup heading="Recent Conversations">
              {tasks.slice(0, 10).map((task: TaskRow) => (
                <CommandItem
                  key={task.id}
                  onSelect={() => handleSelectTask(task.id)}
                  className="group"
                >
                  <FileText className="mr-2 h-4 w-4 shrink-0" />
                  <span className="truncate">{task.prompt}</span>
                  {task.id === selectedTaskId && (
                    <span className="ml-2 text-xs text-muted-foreground">(current)</span>
                  )}
                  <div className="ml-auto flex items-center gap-1 opacity-0 group-hover:opacity-100">
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleForkTask(task.id);
                      }}
                      className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
                      title="Fork"
                    >
                      <GitBranch size={12} />
                    </button>
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDeleteTask(task.id);
                      }}
                      className="rounded p-1 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                      title="Delete"
                    >
                      <Trash2 size={12} />
                    </button>
                  </div>
                </CommandItem>
              ))}
            </CommandGroup>
          </>
        )}
      </CommandList>
    </CommandDialog>
  );
}

export function CommandPaletteTrigger({
  onClick,
}: {
  onClick?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex h-8 items-center gap-2 rounded-md border border-border/70 bg-background/55 px-2.5 text-xs text-muted-foreground transition-colors hover:bg-accent/55 hover:text-foreground"
    >
      <Command size={12} />
      <span>Command Palette</span>
      <kbd className="ml-2 hidden rounded bg-muted px-1.5 py-0.5 font-mono text-[10px] lg:inline">
        Ctrl+K
      </kbd>
    </button>
  );
}
