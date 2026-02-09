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
      className="flex h-full items-center justify-between px-3"
    >
      {/* Left section: logo + workspace */}
      <div className="flex min-w-0 items-center gap-3">
        <div className="flex items-center gap-2">
          <div className="h-[22px] w-[22px]">
            <img src={appIcon} alt="Orchestrix" className="h-full w-full object-contain" />
          </div>
          <span className="text-sm font-semibold tracking-tight">Orchestrix</span>
        </div>

        <div className="h-4 w-px bg-border/50" />

        <button
          type="button"
          onClick={pickWorkspace}
          className="no-drag flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <Folder size={12} />
          <span className="max-w-36 truncate">{workspaceName}</span>
          <ChevronDown size={10} />
        </button>
      </div>

      {/* Right section: controls */}
      <div className="no-drag flex items-center gap-0.5">
        <button
          type="button"
          onClick={onToggleArtifacts}
          className={`rounded-md p-1.5 transition-colors ${
            artifactsOpen
              ? "bg-accent text-foreground"
              : "text-muted-foreground hover:bg-accent hover:text-foreground"
          }`}
          title="Toggle artifacts panel"
        >
          <PanelRight size={14} />
        </button>
        <button
          type="button"
          onClick={onToggleTheme}
          className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          title="Toggle theme"
        >
          {darkMode ? <Sun size={14} /> : <Moon size={14} />}
        </button>

        <div className="mx-1 h-4 w-px bg-border/30" />

        <button
          type="button"
          onClick={() => handleWindow("minimize")}
          className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <Minus size={14} />
        </button>
        <button
          type="button"
          onClick={() => handleWindow("maximize")}
          className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <Square size={12} />
        </button>
        <button
          type="button"
          onClick={() => handleWindow("close")}
          className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground"
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
