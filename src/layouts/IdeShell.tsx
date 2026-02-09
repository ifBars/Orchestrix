import type { ReactNode } from "react";

type IdeShellProps = {
  header: ReactNode;
  sidebar: ReactNode;
  main: ReactNode;
  composer: ReactNode;
  artifacts?: ReactNode;
  isArtifactsOpen: boolean;
};

export function IdeShell({ header, sidebar, main, composer, artifacts, isArtifactsOpen }: IdeShellProps) {
  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Title bar */}
      <header
        data-tauri-drag-region
        className="elevation-1 h-10 shrink-0 border-b border-border bg-sidebar/85 backdrop-blur-sm"
      >
        {header}
      </header>

      <div className="flex min-h-0 flex-1">
        {/* Sidebar */}
        <aside className="elevation-1 w-64 shrink-0 border-r border-sidebar-border bg-sidebar/90">
          {sidebar}
        </aside>

        {/* Main chat area — always centered */}
        <div className="relative flex min-w-0 flex-1 flex-col">
          {/* Scrollable chat content */}
          <div className="flex-1 overflow-y-auto px-6 py-6 pb-48">
            {main}
          </div>

          {/* Composer pinned to bottom */}
          <div className="absolute inset-x-0 bottom-0 border-t border-border/50 bg-background/80 px-6 py-4 backdrop-blur-md">
            {composer}
          </div>
        </div>

        {/* Artifact panel — slide-out overlay */}
        {isArtifactsOpen && artifacts && (
          <aside className="elevation-2 w-80 shrink-0 border-l border-border bg-card/80 backdrop-blur-sm">
            {artifacts}
          </aside>
        )}
      </div>
    </div>
  );
}
