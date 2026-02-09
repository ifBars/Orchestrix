import { ArrowUp, FileText, Folder, Loader2, MessageCircle, Paperclip, Sparkles, XCircle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import type { WorkspaceReferenceCandidate } from "@/types";

export function Composer() {
  const [prompt, setPrompt] = useState("");
  const [attachments, setAttachments] = useState<string[]>([]);
  const [mode, setMode] = useState<"plan" | "build">("plan");
  const [stopping, setStopping] = useState(false);
  const [sending, setSending] = useState(false);
  const [mentionOpen, setMentionOpen] = useState(false);
  const [mentionQuery, setMentionQuery] = useState("");
  const [mentionStart, setMentionStart] = useState<number | null>(null);
  const [mentionItems, setMentionItems] = useState<WorkspaceReferenceCandidate[]>([]);
  const [mentionIndex, setMentionIndex] = useState(0);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const mentionRequestSeq = useRef(0);

  const [
    createTask,
    sendMessageToTask,
    cancelTask,
    selectedTask,
    modelCatalog,
    selectedProvider,
    selectedModel,
    selectProviderModel,
  ] = useAppStore(
    useShallow((state) => [
      state.createTask,
      state.sendMessageToTask,
      state.cancelTask,
      state.tasks.find((t) => t.id === state.selectedTaskId) ?? null,
      state.modelCatalog,
      state.selectedProvider,
      state.selectedModel,
      state.selectProviderModel,
    ])
  );

  const models = modelCatalog.find((item) => item.provider === selectedProvider)?.models ?? [];

  // Determine if we can continue chatting with the selected task
  const canContinueChat =
    selectedTask &&
    (selectedTask.status === "completed" ||
      selectedTask.status === "failed" ||
      selectedTask.status === "cancelled");
  const isWorking = selectedTask && (selectedTask.status === "planning" || selectedTask.status === "executing");

  const pickFiles = async () => {
    const result = await openDialog({ multiple: true, title: "Attach files" });
    if (Array.isArray(result)) setAttachments(result);
    if (typeof result === "string") setAttachments([result]);
  };

  const submit = async () => {
    const value = prompt.trim();
    if (!value) return;
    setMentionOpen(false);
    setPrompt("");
    setAttachments([]);

    if (canContinueChat && selectedTask) {
      // Send as follow-up message to existing task
      setSending(true);
      try {
        await sendMessageToTask(selectedTask.id, value);
      } finally {
        setSending(false);
      }
    } else {
      // Create new task
      await createTask(value, { mode });
    }
  };

  // Auto-resize textarea
  const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setPrompt(e.target.value);
    const el = e.target;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;

    const ctx = getMentionContext(e.target.value, e.target.selectionStart ?? e.target.value.length);
    if (!ctx) {
      setMentionOpen(false);
      setMentionQuery("");
      setMentionStart(null);
      return;
    }

    setMentionOpen(true);
    setMentionQuery(ctx.query);
    setMentionStart(ctx.start);
    setMentionIndex(0);
  };

  useEffect(() => {
    if (!mentionOpen) return;
    const seq = ++mentionRequestSeq.current;
    const timer = setTimeout(() => {
      invoke<WorkspaceReferenceCandidate[]>("search_workspace_references", {
        query: mentionQuery,
        limit: 8,
      })
        .then((items) => {
          if (mentionRequestSeq.current !== seq) return;
          setMentionItems(items);
          setMentionIndex(0);
          if (items.length === 0) setMentionOpen(false);
        })
        .catch(() => {
          if (mentionRequestSeq.current !== seq) return;
          setMentionItems([]);
          setMentionOpen(false);
        });
    }, 120);

    return () => clearTimeout(timer);
  }, [mentionOpen, mentionQuery]);

  const insertMention = (item: WorkspaceReferenceCandidate) => {
    if (mentionStart == null) return;
    const el = textareaRef.current;
    const cursor = el?.selectionStart ?? prompt.length;
    const before = prompt.slice(0, mentionStart);
    const after = prompt.slice(cursor);
    const next = `${before}@${item.value} ${after}`;
    setPrompt(next);
    setMentionOpen(false);
    setMentionItems([]);
    setMentionQuery("");
    setMentionStart(null);

    requestAnimationFrame(() => {
      if (!el) return;
      const nextCursor = mentionStart + item.value.length + 2;
      el.focus();
      el.setSelectionRange(nextCursor, nextCursor);
      el.style.height = "auto";
      el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
    });
  };

  const canSubmit = prompt.trim().length > 0 && !sending;

  const mentionGroups = useMemo(() => {
    const grouped = new Map<string, WorkspaceReferenceCandidate[]>();
    for (const item of mentionItems) {
      const key = item.kind === "skill" ? "Skills" : item.group || "(root)";
      const bucket = grouped.get(key);
      if (bucket) {
        bucket.push(item);
      } else {
        grouped.set(key, [item]);
      }
    }
    return [...grouped.entries()].map(([group, items]) => ({ group, items }));
  }, [mentionItems]);

  const handleStop = async () => {
    if (!selectedTask) return;
    setStopping(true);
    try {
      await cancelTask(selectedTask.id);
    } finally {
      setStopping(false);
    }
  };

  return (
    <div className="mx-auto w-full max-w-3xl">
      {/* Attached files */}
      {attachments.length > 0 && (
        <div className="mb-2 flex flex-wrap gap-1.5">
          {attachments.slice(0, 5).map((file) => (
            <span
              key={file}
              className="inline-flex items-center rounded-full border border-border bg-muted/50 px-2.5 py-0.5 text-xs text-muted-foreground"
            >
              {file.split(/[/\\]/).pop() ?? file}
            </span>
          ))}
          {attachments.length > 5 && (
            <span className="text-xs text-muted-foreground">+{attachments.length - 5} more</span>
          )}
        </div>
      )}

      {/* Continue chat indicator */}
      {canContinueChat && (
        <div className="mb-2 flex items-center gap-2 text-xs text-muted-foreground">
          <MessageCircle size={12} />
          <span>Continuing conversation with previous task</span>
        </div>
      )}

      {/* Input container */}
      <div className="rounded-2xl border border-border bg-card/90 shadow-sm transition-shadow focus-within:shadow-md focus-within:border-ring/30">
        <textarea
          ref={textareaRef}
          value={prompt}
          onChange={handleInput}
          onKeyDown={(e) => {
            if (mentionOpen && mentionItems.length > 0) {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setMentionIndex((idx) => (idx + 1) % mentionItems.length);
                return;
              }
              if (e.key === "ArrowUp") {
                e.preventDefault();
                setMentionIndex((idx) => (idx - 1 + mentionItems.length) % mentionItems.length);
                return;
              }
              if (e.key === "Enter" || e.key === "Tab") {
                e.preventDefault();
                insertMention(mentionItems[mentionIndex] ?? mentionItems[0]);
                return;
              }
              if (e.key === "Escape") {
                e.preventDefault();
                setMentionOpen(false);
                return;
              }
            }

            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              submit().catch(console.error);
            }
          }}
          className="block w-full resize-none bg-transparent px-4 pt-3 pb-2 text-sm text-foreground outline-none placeholder:text-muted-foreground/60"
          placeholder={
            canContinueChat
              ? "Send a follow-up message..."
              : "Describe what you want to build..."
          }
          rows={1}
          style={{ minHeight: "42px", maxHeight: "200px" }}
        />

        {mentionOpen && mentionItems.length > 0 && (
          <div className="mx-3 mb-1 rounded-xl border border-border bg-background/95 p-1">
            {(() => {
              let flatIndex = 0;
              return mentionGroups.map((group) => (
                <div key={group.group} className="mb-1 last:mb-0">
                  <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground/60">
                    {group.group}
                  </div>
                  {group.items.map((item) => {
                    const idx = flatIndex++;
                    return (
                      <button
                        key={`${item.kind}:${item.value}`}
                        type="button"
                        onClick={() => insertMention(item)}
                        className={`flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-xs transition-colors ${
                          idx === mentionIndex ? "bg-accent text-foreground" : "text-muted-foreground hover:bg-accent/60"
                        }`}
                      >
                        {item.kind === "file" ? (
                          <FileText size={12} />
                        ) : item.kind === "directory" ? (
                          <Folder size={12} />
                        ) : (
                          <Sparkles size={12} />
                        )}
                        <span className="truncate">@{item.value}</span>
                        <span className="ml-auto truncate text-[10px] text-muted-foreground/70">{item.description}</span>
                      </button>
                    );
                  })}
                </div>
              ));
            })()}
          </div>
        )}

        {/* Bottom bar */}
        <div className="flex items-center justify-between gap-2 px-3 pb-2.5">
          <div className="flex items-center gap-1">
            {/* Plan/Build mode toggle - only show when creating new tasks */}
            {!canContinueChat && !isWorking && (
              <div className="mr-1 inline-flex items-center rounded-md border border-border bg-muted/30 p-0.5">
                <button
                  type="button"
                  onClick={() => setMode("plan")}
                  className={`rounded px-2 py-1 text-[11px] ${
                    mode === "plan" ? "bg-card text-foreground" : "text-muted-foreground"
                  }`}
                >
                  Plan
                </button>
                <button
                  type="button"
                  onClick={() => setMode("build")}
                  className={`rounded px-2 py-1 text-[11px] ${
                    mode === "build" ? "bg-card text-foreground" : "text-muted-foreground"
                  }`}
                >
                  Build
                </button>
              </div>
            )}

            {/* Attach button - only for new tasks */}
            {!canContinueChat && (
              <button
                type="button"
                onClick={pickFiles}
                className="rounded-lg p-1.5 text-muted-foreground/60 transition-colors hover:bg-accent hover:text-foreground"
                title="Attach files"
              >
                <Paperclip size={14} />
              </button>
            )}

            {/* Model selector */}
            <div className="flex items-center gap-1 pl-1">
              <select
                value={selectedProvider}
                onChange={(e) => {
                  const provider = e.target.value;
                  const fallback =
                    modelCatalog.find((item) => item.provider === provider)?.models[0] ?? "";
                  selectProviderModel(provider, fallback);
                }}
                className="h-6 rounded-md border-0 bg-muted/40 px-2 text-[11px] text-muted-foreground outline-none transition-colors hover:bg-muted/70"
              >
                <option value="minimax">MiniMax</option>
                <option value="kimi">Kimi</option>
              </select>
              <select
                value={selectedModel}
                onChange={(e) => selectProviderModel(selectedProvider, e.target.value)}
                className="h-6 rounded-md border-0 bg-muted/40 px-2 text-[11px] text-muted-foreground outline-none transition-colors hover:bg-muted/70"
              >
                {models.map((model) => (
                  <option key={model} value={model}>
                    {model}
                  </option>
                ))}
              </select>
            </div>
          </div>

          <span className="pr-1 text-[10px] text-muted-foreground/70">Use @ to reference files, folders, and skills</span>

          {/* Stop button - shown when a task is running */}
          {isWorking ? (
            <button
              type="button"
              onClick={() => handleStop().catch(console.error)}
              disabled={stopping}
              className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-destructive/40 bg-destructive/10 px-3 text-xs font-medium text-destructive transition-colors hover:bg-destructive/20 disabled:cursor-not-allowed disabled:opacity-60"
              title="Stop current task"
            >
              {stopping ? <Loader2 size={14} className="animate-spin" /> : <XCircle size={14} />}
              <span>Stop</span>
            </button>
          ) : (
            /* Submit button */
            <button
              type="button"
              onClick={() => submit().catch(console.error)}
              disabled={!canSubmit}
              className={`flex h-8 w-8 items-center justify-center rounded-lg transition-all ${
                canSubmit
                  ? "bg-primary text-primary-foreground hover:brightness-110"
                  : "bg-muted text-muted-foreground/40"
              }`}
              title={
                canContinueChat
                  ? "Send follow-up message"
                  : mode === "plan"
                  ? "Run Plan mode"
                  : "Run Build mode"
              }
            >
              {sending ? (
                <Loader2 size={16} className="animate-spin" />
              ) : (
                <ArrowUp size={16} />
              )}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function getMentionContext(text: string, cursor: number): { start: number; query: string } | null {
  if (cursor < 0 || cursor > text.length) return null;
  let start = cursor - 1;
  while (start >= 0 && !/\s/.test(text[start])) {
    start -= 1;
  }
  start += 1;

  const token = text.slice(start, cursor);
  if (!token.startsWith("@")) return null;
  if (token.includes("\n")) return null;

  return { start, query: token.slice(1) };
}
