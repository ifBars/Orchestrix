import type { ReactNode } from "react";
import { PanelLeft } from "lucide-react";

type IdeShellProps = {
  header: ReactNode;
  sidebar: ReactNode;
  main: ReactNode;
  composer?: ReactNode;
  artifacts?: ReactNode;
  isArtifactsOpen: boolean;
  isSidebarOpen?: boolean;
  onToggleSidebar?: () => void;
  /** When true, the main area fills height without scroll/padding (e.g. canvas) */
  fillMain?: boolean;
  /** Optional strip rendered between the header and the main scroll area (e.g. tab bar) */
  subheader?: ReactNode;
};

export function IdeShell({ 
  header, 
  sidebar, 
  main, 
  composer, 
  artifacts, 
  isArtifactsOpen, 
  isSidebarOpen = true,
  onToggleSidebar,
  fillMain, 
  subheader 
}: IdeShellProps) {
  const hasComposer = composer != null;

  return (
    <div className="relative flex h-full flex-col overflow-hidden bg-background/70">
      <header
        data-tauri-drag-region
        className="elevation-1 h-10 shrink-0 border-b border-border/80 bg-card/75 backdrop-blur-md"
      >
        {header}
      </header>

      <div className="flex min-h-0 flex-1 bg-background/20">
        {/* Sidebar with smooth width transition */}
        <aside 
          className={`elevation-1 shrink-0 border-r border-sidebar-border/90 bg-sidebar/92 transition-all duration-200 ease-out ${
            isSidebarOpen ? "w-64 opacity-100" : "w-0 overflow-hidden opacity-0"
          }`}
        >
          {sidebar}
        </aside>

        {!isSidebarOpen && onToggleSidebar && (
          <div className="elevation-1 flex w-11 shrink-0 items-start justify-center border-r border-border/70 bg-card/35 pt-2">
            <button
              type="button"
              onClick={onToggleSidebar}
              className="rounded-md border border-border/70 bg-card/90 p-1.5 text-muted-foreground shadow-sm backdrop-blur-sm transition-all hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              title="Show sidebar (Ctrl+B)"
              aria-label="Show sidebar"
            >
              <PanelLeft size={15} />
            </button>
          </div>
        )}

        <div className="relative flex min-w-0 flex-1 flex-col transition-all duration-200">

          {/* Optional subheader (e.g. tab strip) */}
          {subheader && (
            <div className="shrink-0">
              {subheader}
            </div>
          )}

          {/* Main scrollable content */}
          {fillMain ? (
            <div className="min-h-0 flex-1 overflow-hidden">
              {main}
            </div>
          ) : (
            <div className="flex-1 overflow-y-auto scroll-smooth px-6 pt-6">
              <div className="w-full pb-40">{main}</div>
            </div>
          )}

          {/* Composer */}
          {hasComposer && (
            <div className="pointer-events-none absolute inset-x-0 bottom-0 z-10 px-6 pt-2">
              <div className="pointer-events-auto w-full">{composer}</div>
            </div>
          )}
        </div>

        {artifacts && (
          <aside
            className={[
              "elevation-2 shrink-0 overflow-hidden bg-card/88 backdrop-blur-md transition-all duration-200 ease-out",
              isArtifactsOpen
                ? "w-80 border-l border-border/80 opacity-100"
                : "w-0 border-l-0 opacity-0 pointer-events-none",
            ].join(" ")}
            aria-hidden={!isArtifactsOpen}
          >
            {artifacts}
          </aside>
        )}
      </div>
    </div>
  );
}
