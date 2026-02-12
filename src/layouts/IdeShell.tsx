import type { ReactNode } from "react";

type IdeShellProps = {
  header: ReactNode;
  sidebar: ReactNode;
  main: ReactNode;
  composer?: ReactNode;
  artifacts?: ReactNode;
  isArtifactsOpen: boolean;
};

export function IdeShell({ header, sidebar, main, composer, artifacts, isArtifactsOpen }: IdeShellProps) {
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
        <aside className="elevation-1 w-64 shrink-0 border-r border-sidebar-border/90 bg-sidebar/92">
          {sidebar}
        </aside>

        <div className="relative flex min-w-0 flex-1 flex-col">
          {/* Main scrollable content */}
          <div className="min-h-0 flex-1 overflow-y-auto scroll-smooth px-6 pt-6 pb-6">
            <div className="w-full">{main}</div>
          </div>

          {/* Composer - no longer absolute, part of flex layout */}
          {hasComposer && (
            <div className="shrink-0 border-t border-border/70 bg-background/88 px-6 pb-4 pt-3 backdrop-blur-xl">
              <div className="w-full">{composer}</div>
            </div>
          )}
        </div>

        {isArtifactsOpen && artifacts && (
          <aside className="elevation-2 w-80 shrink-0 border-l border-border/80 bg-card/88 backdrop-blur-md">
            {artifacts}
          </aside>
        )}
      </div>
    </div>
  );
}
