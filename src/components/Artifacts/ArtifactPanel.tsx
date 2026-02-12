import { useEffect, useState } from "react";
import { ExternalLink, FileText, X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Streamdown } from "streamdown";
import { code } from "@streamdown/code";
import { useAppStore } from "@/stores/appStore";
import type { ArtifactContentView, ArtifactRow } from "@/types";

const EMPTY: ArtifactRow[] = [];

type ArtifactPanelProps = {
  taskId: string;
  onOpenReview?: () => void;
};

export function ArtifactPanel({ taskId, onOpenReview }: ArtifactPanelProps) {
  const artifacts = useAppStore((state) => state.artifactsByTask[taskId] ?? EMPTY);
  const [active, setActive] = useState<ArtifactRow | null>(null);
  const [preview, setPreview] = useState<ArtifactContentView | null>(null);

  useEffect(() => {
    if (!active) {
      setPreview(null);
      return;
    }
    invoke<ArtifactContentView>("read_artifact_content", { path: active.uri_or_content })
      .then(setPreview)
      .catch(() =>
        setPreview({
          path: active.uri_or_content,
          content: "Failed to load artifact",
          is_markdown: false,
        })
      );
  }, [active]);

  return (
    <div className="flex h-full w-full flex-col bg-card/40">
      <div className="flex items-center justify-between border-b border-border/60 px-4 py-3">
        <span className="text-xs font-semibold uppercase tracking-widest text-muted-foreground/75">
          Artifacts
        </span>
        <span className="rounded-full bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground/80">
          {artifacts.length}
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        {artifacts.length === 0 ? (
          <div className="rounded-lg border border-dashed border-border/70 bg-background/55 p-6 text-center text-xs text-muted-foreground/60">
            No artifacts yet
          </div>
        ) : (
          <div className="space-y-1">
            {artifacts.map((artifact) => {
              const selected = active?.id === artifact.id;
              const fileName = artifact.uri_or_content.split(/[/\\]/).pop() ?? artifact.uri_or_content;
              const isMarkdown = artifact.uri_or_content.toLowerCase().endsWith('.md') ||
                artifact.uri_or_content.toLowerCase().endsWith('.markdown');
              // Clicking a markdown artifact opens the full review workspace if handler is provided
              const shouldOpenReview = isMarkdown && onOpenReview;

              return (
                <button
                  key={artifact.id}
                  type="button"
                  onClick={() => {
                    if (shouldOpenReview) {
                      onOpenReview!();
                    } else {
                      setActive(selected ? null : artifact);
                    }
                  }}
                  className={`flex w-full items-center gap-2 rounded-lg border px-3 py-2 text-left transition-colors ${
                    selected
                      ? "border-primary/35 bg-accent/60 text-foreground"
                      : "border-transparent bg-background/45 text-muted-foreground hover:border-border/70 hover:bg-accent/35 hover:text-foreground"
                  }`}
                >
                  <FileText size={13} className="shrink-0" />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-xs">{fileName}</p>
                    <p className="truncate text-[10px] text-muted-foreground/65">{artifact.kind}</p>
                  </div>
                </button>
              );
            })}
          </div>
        )}
      </div>

      {preview && (
        <div className="border-t border-border/60 bg-background/35">
          <div className="flex items-center justify-between px-3 py-2.5">
            <span className="truncate text-xs font-medium text-foreground">
              {preview.path.split(/[/\\]/).pop()}
            </span>
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => {
                  if (preview.path.startsWith("http://") || preview.path.startsWith("https://")) {
                    openUrl(preview.path).catch(console.error);
                    return;
                  }
                  invoke("open_local_path", { path: preview.path }).catch(console.error);
                }}
                className="rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                title="Open externally"
              >
                <ExternalLink size={12} />
              </button>
              <button
                type="button"
                onClick={() => {
                  setActive(null);
                  setPreview(null);
                }}
                className="rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              >
                <X size={12} />
              </button>
            </div>
          </div>
          <div className="max-h-64 overflow-auto border-t border-border/40 p-3 text-xs text-muted-foreground">
            {preview.is_markdown ? (
              <div className="prose prose-sm max-w-none text-foreground dark:prose-invert prose-p:my-2 prose-headings:my-2">
                <Streamdown plugins={{ code }}>{preview.content}</Streamdown>
              </div>
            ) : (
              <pre>
                <code>{preview.content}</code>
              </pre>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
