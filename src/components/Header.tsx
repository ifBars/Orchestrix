import { ChevronDown, Folder, Minus, Moon, PanelRight, Square, Sun, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { type MouseEvent } from "react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import appIcon from "../../src-tauri/icons/icon.png";

type HeaderProps = {
  darkMode: boolean;
  artifactsOpen: boolean;
  onToggleTheme: () => void;
  onToggleArtifacts: () => void;
};

export function Header({ darkMode, artifactsOpen, onToggleTheme, onToggleArtifacts }: HeaderProps) {
  const [workspaceRoot, setWorkspaceRoot] = useAppStore(
    useShallow((state) => [state.workspaceRoot, state.setWorkspaceRoot])
  );

  const startDrag = async (event: MouseEvent<HTMLElement>) => {
    const target = event.target as HTMLElement;
    if (target.closest(".no-drag") || event.button !== 0) return;
    try {
      await getCurrentWindow().startDragging();
    } catch (error) {
      console.error("Failed to start dragging", error);
    }
  };

  const handleWindow = async (action: "minimize" | "maximize" | "close") => {
    const win = getCurrentWindow();
    if (action === "minimize") await win.minimize();
    if (action === "maximize") await win.toggleMaximize();
    if (action === "close") await win.close();
  };

  const pickWorkspace = async () => {
    const selected = await openDialog({
      directory: true,
      title: "Select workspace folder",
      defaultPath: workspaceRoot || undefined,
    });
    if (typeof selected === "string" && selected.length > 0) {
      await setWorkspaceRoot(selected);
    }
  };

  const workspaceName = workspaceRoot ? workspaceRoot.split(/[/\\]/).pop() : "No workspace";

  return (
    <div
      data-tauri-drag-region
      onMouseDown={startDrag}
      className="flex h-full items-center justify-between gap-3 px-3"
    >
      <div className="flex min-w-0 items-center gap-3">
        <div className="flex items-center gap-2 rounded-md border border-border/70 bg-background/70 px-2.5 py-1">
          <div className="h-[22px] w-[22px]">
            <img src={appIcon} alt="Orchestrix" className="h-full w-full object-contain" />
          </div>
          <div>
            <p className="text-[11px] font-semibold leading-none tracking-tight text-foreground">Orchestrix</p>
            <p className="mt-0.5 text-[10px] leading-none text-muted-foreground">Agent Workspace</p>
          </div>
        </div>

        <button
          type="button"
          onClick={pickWorkspace}
          className="no-drag flex min-w-0 items-center gap-1.5 rounded-md border border-border/70 bg-background/55 px-2.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent/55 hover:text-foreground"
          title={workspaceRoot || "Select workspace"}
        >
          <Folder size={12} />
          <span className="max-w-40 truncate">{workspaceName}</span>
          <ChevronDown size={10} />
        </button>
      </div>

      <div className="no-drag flex items-center gap-2">
        <div className="flex items-center rounded-md border border-border/70 bg-background/55 p-0.5">
          <button
            type="button"
            onClick={onToggleArtifacts}
            className={`rounded-md p-1.5 transition-colors ${
              artifactsOpen
                ? "bg-accent text-foreground"
                : "text-muted-foreground hover:bg-accent/70 hover:text-foreground"
            }`}
            title="Toggle artifacts panel"
            aria-pressed={artifactsOpen}
          >
            <PanelRight size={14} />
          </button>
          <button
            type="button"
            onClick={onToggleTheme}
            className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent/70 hover:text-foreground"
            title="Toggle theme"
          >
            {darkMode ? <Sun size={14} /> : <Moon size={14} />}
          </button>
        </div>

        <div className="flex items-center rounded-md border border-border/70 bg-background/55 p-0.5">
          <button
            type="button"
            onClick={() => handleWindow("minimize")}
            className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent/70 hover:text-foreground"
            title="Minimize"
          >
            <Minus size={14} />
          </button>
          <button
            type="button"
            onClick={() => handleWindow("maximize")}
            className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent/70 hover:text-foreground"
            title="Maximize"
          >
            <Square size={12} />
          </button>
          <button
            type="button"
            onClick={() => handleWindow("close")}
            className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground"
            title="Close"
          >
            <X size={14} />
          </button>
        </div>
      </div>
    </div>
  );
}
