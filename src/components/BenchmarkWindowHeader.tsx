import { Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type MouseEvent } from "react";
import appIcon from "../../src-tauri/icons/icon.png";

type BenchmarkWindowHeaderProps = {
  onExit?: () => void;
};

export function BenchmarkWindowHeader({ onExit }: BenchmarkWindowHeaderProps) {
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

  return (
    <div
      data-tauri-drag-region
      onMouseDown={startDrag}
      className="flex h-full items-center justify-between gap-3 px-3"
    >
      <div className="flex min-w-0 items-center gap-2 rounded-md border border-border/70 bg-background/70 px-2.5 py-1">
        <img src={appIcon} alt="Orchestrix" className="h-[18px] w-[18px] object-contain" />
        <div>
          <p className="text-[11px] font-semibold leading-none tracking-tight text-foreground">Orchestrix Benchmarks</p>
          <p className="mt-0.5 text-[10px] leading-none text-muted-foreground">Benchmark Workspace</p>
        </div>
      </div>

      <div className="no-drag flex items-center gap-2">
        {onExit ? (
          <button
            type="button"
            onClick={onExit}
            className="rounded-md border border-border/70 bg-background/55 px-2.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent/70 hover:text-foreground"
            title="Back to regular app"
          >
            Exit benchmark mode
          </button>
        ) : null}

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
