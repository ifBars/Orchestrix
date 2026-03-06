import {
  Download,
  ExternalLink,
  FileText,
  Folder,
  Github,
  PanelLeft,
  PanelRight,
  ScrollText,
  Sparkles,
} from "lucide-react";
import { SafeStreamdown } from "@/components/Chat/ConversationTimeline/messages/SafeStreamdown";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { ArtifactContentView, ArtifactRow, TaskRow, TaskStatus } from "@/types";
import { OrchestrixMark } from "@/components/landing/OrchestrixMark";

type ShellHeaderViewProps = {
  workspaceName: string;
  sidebarOpen: boolean;
  artifactsOpen: boolean;
  onToggleSidebar?: () => void;
  onToggleArtifacts?: () => void;
  repoUrl: string;
  className?: string;
};

type ShellSidebarViewProps = {
  tasks: TaskRow[];
  selectedTaskId: string;
  workspaceName: string;
  onSelectTask: (taskId: string) => void;
  className?: string;
};

type ShellArtifactRailViewProps = {
  artifacts: ArtifactRow[];
  activeArtifactPath: string | null;
  artifactContentsByPath: Record<string, ArtifactContentView>;
  onSelectArtifact: (path: string) => void;
  onOpenReview?: (path: string) => void;
  className?: string;
};

const STATUS_META: Record<TaskStatus, { dotClassName: string; label: string; badgeClassName: string }> = {
  pending: {
    dotClassName: "bg-muted-foreground/45",
    label: "Pending",
    badgeClassName: "bg-muted text-muted-foreground",
  },
  planning: {
    dotClassName: "bg-info animate-pulse",
    label: "Planning",
    badgeClassName: "bg-info/15 text-info",
  },
  awaiting_review: {
    dotClassName: "bg-warning",
    label: "Review",
    badgeClassName: "bg-warning/15 text-warning",
  },
  executing: {
    dotClassName: "bg-info animate-pulse",
    label: "Executing",
    badgeClassName: "bg-info/15 text-info",
  },
  completed: {
    dotClassName: "bg-success",
    label: "Completed",
    badgeClassName: "bg-success/15 text-success",
  },
  failed: {
    dotClassName: "bg-destructive",
    label: "Failed",
    badgeClassName: "bg-destructive/15 text-destructive",
  },
  cancelled: {
    dotClassName: "bg-warning",
    label: "Cancelled",
    badgeClassName: "bg-warning/15 text-warning",
  },
};

function taskAge(iso: string): string {
  const delta = Date.now() - new Date(iso).getTime();
  const minutes = Math.max(1, Math.floor(delta / 60000));
  if (minutes < 60) return String(minutes) + "m";
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return String(hours) + "h";
  return String(Math.floor(hours / 24)) + "d";
}

export function ShellHeaderView({
  workspaceName,
  sidebarOpen,
  artifactsOpen,
  onToggleSidebar,
  onToggleArtifacts,
  repoUrl,
  className,
}: ShellHeaderViewProps) {
  return (
    <div className={cn("flex h-full items-center justify-between gap-3 px-3", className)}>
      <div className="flex min-w-0 items-center gap-3">
        <div className="flex items-center gap-2 rounded-md border border-border/70 bg-background/70 px-2.5 py-1">
          <OrchestrixMark className="h-[22px] w-[22px]" />
          <div>
            <p className="text-[11px] font-semibold leading-none tracking-tight text-foreground">Orchestrix</p>
            <p className="mt-0.5 text-[10px] leading-none text-muted-foreground">Preview Workspace</p>
          </div>
        </div>

        <div className="flex min-w-0 items-center gap-1.5 rounded-md border border-border/70 bg-background/55 px-2.5 py-1 text-xs text-muted-foreground">
          <Folder size={12} />
          <span className="max-w-40 truncate">{workspaceName}</span>
        </div>

        <div className="hidden items-center gap-1.5 rounded-md border border-primary/20 bg-primary/10 px-2.5 py-1 text-[11px] text-primary md:flex">
          <Sparkles size={12} />
          <span>Preview mode</span>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <div className="hidden items-center rounded-md border border-border/70 bg-background/55 p-0.5 sm:flex">
          <button
            type="button"
            onClick={onToggleSidebar}
            className={cn(
              "rounded-md p-1.5 transition-colors",
              sidebarOpen ? "bg-accent text-foreground" : "text-muted-foreground hover:bg-accent/70 hover:text-foreground"
            )}
            aria-label="Toggle sidebar"
          >
            <PanelLeft size={14} />
          </button>
          <button
            type="button"
            onClick={onToggleArtifacts}
            className={cn(
              "rounded-md p-1.5 transition-colors",
              artifactsOpen ? "bg-accent text-foreground" : "text-muted-foreground hover:bg-accent/70 hover:text-foreground"
            )}
            aria-label="Toggle artifacts panel"
          >
            <PanelRight size={14} />
          </button>
        </div>

        <a
          href={repoUrl}
          target="_blank"
          rel="noreferrer"
          className="inline-flex h-7 items-center gap-1.5 rounded-md border border-border/70 bg-background/55 px-2.5 text-[11px] text-muted-foreground transition-colors hover:bg-accent/55 hover:text-foreground"
        >
          <Github size={12} />
          <span className="hidden sm:inline">GitHub</span>
        </a>

        <Button
          size="sm"
          className="h-7 gap-1.5 rounded-md px-2.5 text-[11px] font-medium"
          onClick={() => window.open(repoUrl, "_blank", "noopener,noreferrer")}
        >
          <Download size={12} />
          <span className="hidden sm:inline">Download</span>
        </Button>
      </div>
    </div>
  );
}

export function ShellSidebarView({
  tasks,
  selectedTaskId,
  workspaceName,
  onSelectTask,
  className,
}: ShellSidebarViewProps) {
  return (
    <div className={cn("flex h-full flex-col gap-2 px-2 pb-2 pt-3 text-sidebar-foreground", className)}>
      <div className="rounded-lg border border-sidebar-border/80 bg-sidebar/75 p-2 backdrop-blur-sm">
        <Button className="h-9 w-full justify-start gap-2 rounded-md" disabled>
          <Sparkles size={14} />
          Preview Conversations
        </Button>
      </div>

      <div className="mt-1 flex items-center justify-between px-2 pb-1.5 pt-1">
        <span className="truncate text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/70">
          {workspaceName}
        </span>
        <span className="text-[10px] font-medium text-muted-foreground/60">{tasks.length}</span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-1 pb-1">
        <div className="space-y-1">
          {tasks.map((task) => {
            const selected = task.id === selectedTaskId;
            const statusMeta = STATUS_META[task.status];

            return (
              <article
                key={task.id}
                className={cn(
                  "group relative overflow-hidden rounded-lg border px-2.5 py-2 transition-colors",
                  selected
                    ? "border-primary/40 bg-gradient-to-br from-card/95 via-card/90 to-accent/35 text-foreground shadow-sm"
                    : "border-transparent bg-sidebar/55 text-muted-foreground hover:border-sidebar-border/80 hover:bg-accent/45 hover:text-foreground"
                )}
              >
                <div
                  className={cn(
                    "pointer-events-none absolute inset-y-0 left-0 w-px bg-transparent transition-colors",
                    selected && "bg-primary/70"
                  )}
                />

                <button
                  type="button"
                  onClick={() => onSelectTask(task.id)}
                  className="flex w-full items-start gap-2.5 text-left"
                >
                  <span className={cn("mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full", statusMeta.dotClassName)} />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[13px] font-medium leading-snug text-foreground">{task.prompt}</p>
                    <div className="mt-1.5 flex items-center gap-2 text-[10px] text-muted-foreground/70">
                      <span>{taskAge(task.updated_at)}</span>
                      <span className={cn("rounded-full px-1.5 py-0.5 font-medium", statusMeta.badgeClassName)}>
                        {statusMeta.label}
                      </span>
                    </div>
                  </div>
                </button>
              </article>
            );
          })}
        </div>
      </div>

      <div className="rounded-lg border border-sidebar-border/70 bg-sidebar/60 px-3 py-2">
        <p className="text-[10px] font-semibold uppercase tracking-[0.18em] text-muted-foreground/65">
          Preview only
        </p>
        <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
          Real shell components. Fixture data. No AI requests, tool calls, or file writes.
        </p>
      </div>
    </div>
  );
}

export function ShellArtifactRailView({
  artifacts,
  activeArtifactPath,
  artifactContentsByPath,
  onSelectArtifact,
  onOpenReview,
  className,
}: ShellArtifactRailViewProps) {
  const activePreview = activeArtifactPath ? artifactContentsByPath[activeArtifactPath] ?? null : null;

  return (
    <div className={cn("flex h-full w-full flex-col bg-card/40", className)}>
      <div className="flex items-center justify-between border-b border-border/60 px-4 py-3">
        <span className="text-xs font-semibold uppercase tracking-widest text-muted-foreground/75">
          Artifacts
        </span>
        <span className="rounded-full bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground/80">
          {artifacts.length}
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        <div className="space-y-1">
          {artifacts.map((artifact) => {
            const fileName = artifact.uri_or_content.split(/[/\\\\]/).pop() ?? artifact.uri_or_content;
            const isMarkdown = /\\.(md|markdown|mdx)$/i.test(fileName);
            const selected = artifact.uri_or_content === activeArtifactPath;

            return (
              <button
                key={artifact.id}
                type="button"
                onClick={() => {
                  if (isMarkdown && onOpenReview) {
                    onOpenReview(artifact.uri_or_content);
                    return;
                  }
                  onSelectArtifact(artifact.uri_or_content);
                }}
                className={cn(
                  "flex w-full items-center gap-2 rounded-lg border px-3 py-2 text-left transition-colors",
                  selected
                    ? "border-primary/35 bg-accent/60 text-foreground"
                    : "border-transparent bg-background/45 text-muted-foreground hover:border-border/70 hover:bg-accent/35 hover:text-foreground"
                )}
              >
                {isMarkdown ? <ScrollText size={13} className="shrink-0" /> : <FileText size={13} className="shrink-0" />}
                <div className="min-w-0 flex-1">
                  <p className="truncate text-xs">{fileName}</p>
                  <p className="truncate text-[10px] text-muted-foreground/65">{artifact.kind}</p>
                </div>
                {isMarkdown && onOpenReview ? <ExternalLink size={12} className="shrink-0 opacity-70" /> : null}
              </button>
            );
          })}
        </div>
      </div>

      {activePreview && (
        <div className="border-t border-border/60 bg-background/35">
          <div className="flex items-center justify-between px-3 py-2.5">
            <span className="truncate text-xs font-medium text-foreground">
              {activePreview.path.split(/[/\\\\]/).pop()}
            </span>
            <span className="rounded-full border border-border/60 bg-background/70 px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
              preview
            </span>
          </div>
          <div className="max-h-64 overflow-auto border-t border-border/40">
            {activePreview.is_markdown ? (
              <div className="prose prose-sm max-w-none p-3 text-foreground dark:prose-invert prose-p:my-2 prose-headings:my-2">
                <SafeStreamdown content={activePreview.content} mermaid />
              </div>
            ) : (
              <pre className="overflow-x-auto p-3 text-xs leading-relaxed text-muted-foreground">
                <code>{activePreview.content}</code>
              </pre>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

