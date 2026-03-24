import {
  ArrowUp,
  Bot,
  Loader2,
  Paperclip,
  X,
  XCircle,
  AlertTriangle,
  Wallet,
  RefreshCw,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/shallow";
import { ContextUsageChip } from "@/components/Chat/ContextUsage";
import { useTaskContextSnapshot } from "@/components/Chat/ChatInterface/useTaskContextSnapshot";
import { useProviderUsage } from "@/hooks/useProviderUsage";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import type { ProviderUsageSnapshotView } from "@/types";

// Provider Usage Chip - shows balance/usage for current provider
function ProviderUsageChip({
  snapshot,
  isLoading,
  onRefresh,
}: {
  snapshot: ProviderUsageSnapshotView | null;
  isLoading: boolean;
  onRefresh: () => void;
}) {
  if (!snapshot) return null;

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="inline-flex h-7 items-center gap-1.5 rounded-lg border border-border/70 bg-background/70 px-2 text-xs transition-colors hover:bg-accent/60 hover:text-foreground"
          title="View provider usage"
        >
          <Wallet size={13} />
          {snapshot.available ? (
            <span className="font-mono text-[11px] text-foreground">
              {snapshot.balance ?? snapshot.remaining_quota ?? "Usage"}
            </span>
          ) : (
            <span className="text-[11px] text-muted-foreground">
              {snapshot.provider}
            </span>
          )}
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[280px] p-0">
        <div className="p-3">
          <div className="mb-3 flex items-center justify-between">
            <span className="text-sm font-semibold text-foreground">
              {snapshot.provider} Usage
            </span>
            <button
              type="button"
              onClick={onRefresh}
              disabled={isLoading}
              className="flex h-6 w-6 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent/60 hover:text-foreground disabled:opacity-50"
            >
              <RefreshCw size={12} className={isLoading ? "animate-spin" : ""} />
            </button>
          </div>
          {snapshot.available ? (
            <div className="space-y-2">
              {snapshot.balance && (
                <div className="flex items-center justify-between text-xs">
                  <span className="text-muted-foreground">Balance</span>
                  <span className="font-medium text-foreground">
                    {snapshot.balance} {snapshot.currency}
                  </span>
                </div>
              )}
              {snapshot.remaining_quota && (
                <div className="flex items-center justify-between text-xs">
                  <span className="text-muted-foreground">Remaining Quota</span>
                  <span className="font-medium text-foreground">
                    {snapshot.remaining_quota}
                  </span>
                </div>
              )}
              {snapshot.period && (
                <div className="flex items-center justify-between text-xs">
                  <span className="text-muted-foreground">Period</span>
                  <span className="font-medium text-foreground">
                    {snapshot.period}
                  </span>
                </div>
              )}
            </div>
          ) : (
            <div className="space-y-2">
              <p className="text-xs text-muted-foreground">
                {snapshot.note || "Provider usage information is not available."}
              </p>
              {snapshot.error && (
                <p className="text-xs text-destructive">{snapshot.error}</p>
              )}
            </div>
          )}
          {snapshot.last_updated_at && (
            <p className="mt-3 text-[10px] text-muted-foreground">
              Last updated: {new Date(snapshot.last_updated_at).toLocaleTimeString()}
            </p>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}

import { ComposerSuggestionPopup } from "./ComposerSuggestionPopup";
import {
  getSlashContext,
  matchSlashCommands,
  type SlashMatch,
} from "./slashCommands";
import { useAppStore } from "@/stores/appStore";
import { useCanvasStore } from "@/stores/canvasStore";
import {
  firstModelForProvider,
  providerOptionsFromCatalog,
  getModelInfo,
} from "@/lib/providers";
import type {
  CanvasNode,
  WorkspaceReferenceCandidate,
  ModelCatalogEntry,
} from "@/types";

export function Composer() {
  const [prompt, setPrompt] = useState("");
  const [attachments, setAttachments] = useState<string[]>([]);
  const [stopping, setStopping] = useState(false);
  const [sending, setSending] = useState(false);
  const [suggestion, setSuggestion] = useState<string>("");
  const [isFetchingSuggestion, setIsFetchingSuggestion] = useState(false);

  // Mention state
  const [mentionOpen, setMentionOpen] = useState(false);
  const [mentionQuery, setMentionQuery] = useState("");
  const [mentionStart, setMentionStart] = useState<number | null>(null);
  const [mentionItems, setMentionItems] = useState<WorkspaceReferenceCandidate[]>(
    []
  );
  const [mentionIndex, setMentionIndex] = useState(0);

  // Slash command state
  const [slashOpen, setSlashOpen] = useState(false);
  const [slashQuery, setSlashQuery] = useState("");
  const [slashStart, setSlashStart] = useState<number | null>(null);
  const [slashMatches, setSlashMatches] = useState<SlashMatch[]>([]);
  const [slashIndex, setSlashIndex] = useState(0);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const mentionRequestSeq = useRef(0);
  const lastFocusedTaskId = useRef<string | null>(null);

  const [
    createTask,
    sendMessageToTask,
    cancelTask,
    branchTask,
    selectTask,
    selectedTask,
    selectedAgentPresetId,
    setSelectedAgentPreset,
    modelCatalog,
    selectedProvider,
    selectedModel,
    selectProviderModel,
    workflowMode,
    setWorkflowMode,
    promptSuggestionSettings,
  ] = useAppStore(
    useShallow((state) => [
      state.createTask,
      state.sendMessageToTask,
      state.cancelTask,
      state.branchTask,
      state.selectTask,
      state.tasks.find((t) => t.id === state.selectedTaskId) ?? null,
      state.selectedAgentPresetId,
      state.setSelectedAgentPreset,
      state.modelCatalog,
      state.selectedProvider,
      state.selectedModel,
      state.selectProviderModel,
      state.workflowMode,
      state.setWorkflowMode,
      state.promptSuggestionSettings,
    ])
  );

  // Canvas node context
  const {
    activeTaskId: canvasActiveTaskId,
    selectedNodes: canvasSelectedNodes,
    clearSelection: clearCanvasSelection,
  } = useCanvasStore();
  const activeCanvasNodes: CanvasNode[] =
    canvasActiveTaskId && selectedTask && canvasActiveTaskId === selectedTask.id
      ? canvasSelectedNodes
      : [];

  const models =
    modelCatalog.find((item) => item.provider === selectedProvider)?.models ?? [];
  const providerOptions = providerOptionsFromCatalog(modelCatalog);

  const canContinueChat =
    selectedTask &&
    (selectedTask.status === "completed" ||
      selectedTask.status === "failed" ||
      selectedTask.status === "cancelled");
  const isWorking =
    selectedTask &&
    (selectedTask.status === "planning" || selectedTask.status === "executing");
  const contextSnapshot = useTaskContextSnapshot(selectedTask?.id ?? null);
  const {
    usage: providerUsage,
    isLoading: providerUsageLoading,
    refresh: refreshProviderUsage,
  } = useProviderUsage(selectedProvider);

  const pickFiles = async () => {
    const result = await openDialog({ multiple: true, title: "Attach files" });
    if (Array.isArray(result)) setAttachments(result);
    if (typeof result === "string") setAttachments([result]);
  };

  const submit = async () => {
    const value = prompt.trim();
    if (!value) return;

    setMentionOpen(false);
    setSlashOpen(false);
    setPrompt("");
    setAttachments([]);

    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = "auto";
      textarea.style.height = "42px";
    }

    // Check if this is a slash command
    const slashMatch = value.match(/^\/(\w+)(\s+)?(.*)?$/);
    if (slashMatch) {
      const commandId = slashMatch[1];
      const remainingText = slashMatch[3] || "";

      if (commandId === "new" || commandId === "newchat" || commandId === "fresh") {
        selectTask(null);
        if (remainingText.trim()) {
          await createTask(remainingText, {
            mode: workflowMode,
            agentPresetId: selectedAgentPresetId ?? undefined,
          });
        }
        return;
      }

      if (commandId === "fork" || commandId === "branch") {
        if (selectedTask) {
          await branchTask(selectedTask.id);
        }
        if (remainingText.trim()) {
          const forkedTask = useAppStore
            .getState()
            .tasks.find((t) => t.parent_task_id === selectedTask?.id);
          if (forkedTask) {
            await sendMessageToTask(forkedTask.id, remainingText);
          }
        }
        return;
      }

      if (commandId === "compact" || commandId === "summarize" || commandId === "summary" || commandId === "summ") {
        if (selectedTask) {
          try {
            await invoke("compact_task_conversation", {
              taskId: selectedTask.id,
            });
          } catch (error) {
            console.error("Failed to compact conversation:", error);
          }
        }
        return;
      }
    }

    const presetIdFromPrompt = extractAgentPresetId(value);
    const effectivePresetId = presetIdFromPrompt ?? selectedAgentPresetId;
    if (presetIdFromPrompt && presetIdFromPrompt !== selectedAgentPresetId) {
      setSelectedAgentPreset(presetIdFromPrompt);
    }

    const messageWithContext =
      activeCanvasNodes.length > 0
        ? `${buildNodeContextBlock(activeCanvasNodes)}\n\n${value}`
        : value;

    if (canContinueChat && selectedTask) {
      setSending(true);
      try {
        await sendMessageToTask(selectedTask.id, messageWithContext);
      } finally {
        setSending(false);
      }
    } else {
      await createTask(messageWithContext, {
        mode: workflowMode,
        agentPresetId: effectivePresetId ?? undefined,
      });
    }
  };

  const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setPrompt(e.target.value);
    const el = e.target;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;

    if (suggestion) {
      setSuggestion("");
    }

    const cursor = e.target.selectionStart ?? e.target.value.length;

    // Check for mention context first
    const mentionCtx = getMentionContext(e.target.value, cursor);
    if (mentionCtx) {
      setMentionOpen(true);
      setMentionQuery(mentionCtx.query);
      setMentionStart(mentionCtx.start);
      setMentionIndex(0);
      // Close slash if mention is active
      setSlashOpen(false);
      setSlashQuery("");
      setSlashStart(null);
      return;
    }

    // Check for slash context
    const slashCtx = getSlashContext(e.target.value, cursor);
    if (slashCtx) {
      setSlashOpen(true);
      setSlashQuery(slashCtx.query);
      setSlashStart(slashCtx.start);
      setSlashIndex(0);
      // Close mention if slash is active
      setMentionOpen(false);
      setMentionQuery("");
      setMentionStart(null);
      return;
    }

    // Close both if neither is active
    setMentionOpen(false);
    setMentionQuery("");
    setMentionStart(null);
    setSlashOpen(false);
    setSlashQuery("");
    setSlashStart(null);
  };

  const handleFocus = async () => {
    if (
      !selectedTask ||
      !canContinueChat ||
      !promptSuggestionSettings.enabled ||
      prompt.trim().length > 0
    ) {
      return;
    }

    if (lastFocusedTaskId.current === selectedTask.id && suggestion) {
      return;
    }

    lastFocusedTaskId.current = selectedTask.id;

    setIsFetchingSuggestion(true);
    try {
      const result = await invoke<string>("generate_prompt_suggestion", {
        taskId: selectedTask.id,
        provider: selectedProvider,
        model: selectedModel,
      });
      if (result && result.trim().length > 0) {
        setSuggestion(result.trim());
      }
    } catch (err) {
      console.debug("Failed to fetch prompt suggestion:", err);
    } finally {
      setIsFetchingSuggestion(false);
    }
  };

  const acceptSuggestion = () => {
    if (suggestion && !prompt.trim()) {
      setPrompt(suggestion);
      setSuggestion("");
      const textarea = textareaRef.current;
      if (textarea) {
        textarea.style.height = "auto";
        textarea.style.height = `${Math.min(textarea.scrollHeight, 200)}px`;
      }
      return true;
    }
    return false;
  };

  // Update mention items when query changes
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

  // Update slash matches when query changes
  useEffect(() => {
    if (!slashOpen) return;
    const matches = matchSlashCommands(slashQuery);
    setSlashMatches(matches);
    setSlashIndex(0);
    if (matches.length === 0) setSlashOpen(false);
  }, [slashOpen, slashQuery]);

  const insertMention = (item: WorkspaceReferenceCandidate) => {
    if (mentionStart == null) return;
    const el = textareaRef.current;
    const cursor = el?.selectionStart ?? prompt.length;
    const before = prompt.slice(0, mentionStart);
    const after = prompt.slice(cursor);
    const next = `${before}@${item.value} ${after}`;
    setPrompt(next);
    if (item.kind === "agent") {
      const presetId = item.value.startsWith("agent:")
        ? item.value.slice("agent:".length)
        : item.value;
      if (presetId.length > 0) {
        setSelectedAgentPreset(presetId);
      }
    }
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

  const insertSlashCommand = (match: SlashMatch) => {
    if (slashStart == null) return;
    const el = textareaRef.current;
    const cursor = el?.selectionStart ?? prompt.length;
    const before = prompt.slice(0, slashStart);
    const after = prompt.slice(cursor);
    const next = `${before}${match.command.command} ${after}`;
    setPrompt(next);
    setSlashOpen(false);
    setSlashMatches([]);
    setSlashQuery("");
    setSlashStart(null);

    requestAnimationFrame(() => {
      if (!el) return;
      const nextCursor = slashStart + match.command.command.length + 1;
      el.focus();
      el.setSelectionRange(nextCursor, nextCursor);
      el.style.height = "auto";
      el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
    });
  };

  const handleStop = async () => {
    if (!selectedTask) return;
    setStopping(true);
    try {
      await cancelTask(selectedTask.id);
    } finally {
      setStopping(false);
    }
  };

  const mentionGroups = useMemo(() => {
    const grouped = new Map<string, WorkspaceReferenceCandidate[]>();
    for (const item of mentionItems) {
      const key =
        item.kind === "agent"
          ? "Agent Presets"
          : item.kind === "skill"
          ? "Skills"
          : item.group || "(root)";
      const bucket = grouped.get(key);
      if (bucket) {
        bucket.push(item);
      } else {
        grouped.set(key, [item]);
      }
    }
    return [...grouped.entries()].map(([group, items]) => ({ group, items }));
  }, [mentionItems]);

  const canSubmit = prompt.trim().length > 0 && !sending;

  return (
    <div className="w-full">
      {/* Attached files */}
      {attachments.length > 0 && (
        <div className="mb-2.5 flex flex-wrap gap-1.5">
          {attachments.slice(0, 5).map((file) => (
            <span
              key={file}
              className="inline-flex items-center rounded-full border border-border/80 bg-background/80 px-2.5 py-0.5 text-xs text-muted-foreground"
            >
              {file.split(/[/\\]/).pop() ?? file}
            </span>
          ))}
          {attachments.length > 5 && (
            <span className="text-xs text-muted-foreground/80">
              +{attachments.length - 5} more
            </span>
          )}
        </div>
      )}

      {/* Selected agent preset */}
      {!canContinueChat && selectedAgentPresetId && (
        <div className="mb-2 flex items-center gap-2 rounded-md border border-border/70 bg-background/65 px-2.5 py-1 text-xs text-muted-foreground">
          <Bot size={12} />
          <span>Using preset @agent:{selectedAgentPresetId}</span>
          <button
            type="button"
            onClick={() => setSelectedAgentPreset(null)}
            className="rounded px-1.5 py-0.5 text-[10px] text-muted-foreground/80 hover:bg-accent/70 hover:text-foreground"
          >
            Clear
          </button>
        </div>
      )}

      {/* Canvas node context */}
      {activeCanvasNodes.length > 0 && (
        <div className="mb-2 flex flex-wrap items-center gap-1.5">
          <span className="text-[10px] font-medium uppercase tracking-wide text-muted-foreground/60">
            Canvas context
          </span>
          {activeCanvasNodes.map((node) => (
            <span
              key={node.id}
              className="inline-flex items-center gap-1 rounded-full border border-primary/30 bg-primary/8 px-2 py-0.5 text-[11px] font-medium text-primary"
            >
              {node.label}
              {node.kind && (
                <span className="text-[9px] text-primary/70">{node.kind}</span>
              )}
            </span>
          ))}
          <button
            type="button"
            onClick={clearCanvasSelection}
            className="ml-auto rounded p-0.5 text-muted-foreground/60 transition-colors hover:text-muted-foreground"
            title="Clear canvas context"
          >
            <X size={11} />
          </button>
        </div>
      )}

      {/* Input container */}
      <div className="elevation-2 rounded-2xl border border-border/80 bg-card/92 transition-colors focus-within:border-ring/40">
        <div className="relative">
          <textarea
            ref={textareaRef}
            value={prompt}
            onChange={handleInput}
            onFocus={handleFocus}
            onKeyDown={(e) => {
              // Handle Tab to accept suggestion
              if (e.key === "Tab" && !mentionOpen && !slashOpen) {
                if (acceptSuggestion()) {
                  e.preventDefault();
                  return;
                }
              }

              // Handle mention popup navigation
              if (mentionOpen && mentionItems.length > 0) {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setMentionIndex((idx) => (idx + 1) % mentionItems.length);
                  return;
                }
                if (e.key === "ArrowUp") {
                  e.preventDefault();
                  setMentionIndex(
                    (idx) =>
                      (idx - 1 + mentionItems.length) % mentionItems.length
                  );
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

              // Handle slash popup navigation
              if (slashOpen && slashMatches.length > 0) {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setSlashIndex((idx) => (idx + 1) % slashMatches.length);
                  return;
                }
                if (e.key === "ArrowUp") {
                  e.preventDefault();
                  setSlashIndex(
                    (idx) =>
                      (idx - 1 + slashMatches.length) % slashMatches.length
                  );
                  return;
                }
                if (e.key === "Enter" || e.key === "Tab") {
                  e.preventDefault();
                  insertSlashCommand(
                    slashMatches[slashIndex] ?? slashMatches[0]
                  );
                  return;
                }
                if (e.key === "Escape") {
                  e.preventDefault();
                  setSlashOpen(false);
                  return;
                }
              }

              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                submit().catch(console.error);
              }
            }}
            className="block w-full resize-none bg-transparent px-4 pt-3 pb-2 text-sm leading-relaxed text-foreground outline-none placeholder:text-muted-foreground/70"
            placeholder={
              isFetchingSuggestion
                ? "Getting suggestion..."
                : canContinueChat && suggestion
                ? suggestion
                : canContinueChat
                ? "Send a follow-up message..."
                : "Describe what you want to build..."
            }
            rows={1}
            style={{ minHeight: "42px", maxHeight: "200px" }}
          />
        </div>

        {/* Mention popup */}
        <ComposerSuggestionPopup
          type="mention"
          isOpen={mentionOpen && mentionItems.length > 0}
          items={mentionGroups}
          activeIndex={mentionIndex}
          onSelect={(index) => {
            let flatIndex = 0;
            for (const group of mentionGroups) {
              for (const item of group.items) {
                if (flatIndex === index) {
                  insertMention(item);
                  return;
                }
                flatIndex++;
              }
            }
          }}
        />

        {/* Slash popup */}
        <ComposerSuggestionPopup
          type="slash"
          isOpen={slashOpen && slashMatches.length > 0}
          items={slashMatches}
          activeIndex={slashIndex}
          onSelect={(index) => {
            const match = slashMatches[index];
            if (match) {
              insertSlashCommand(match);
            }
          }}
        />

        {/* Bottom bar */}
        <div className="flex items-center justify-between gap-2 border-t border-border/70 px-3 pb-2.5 pt-2">
          <div className="flex items-center gap-1">
            {/* Plan/Build mode toggle */}
            {!canContinueChat && !isWorking && (
              <div className="mr-1 inline-flex items-center rounded-md border border-border/80 bg-background/65 p-0.5">
                <button
                  type="button"
                  onClick={() => setWorkflowMode("plan")}
                  className={`rounded px-2 py-1 text-[11px] transition-colors ${
                    workflowMode === "plan"
                      ? "bg-primary/15 text-foreground"
                      : "text-muted-foreground hover:bg-accent/50"
                  }`}
                >
                  Plan
                </button>
                <button
                  type="button"
                  onClick={() => setWorkflowMode("build")}
                  className={`rounded px-2 py-1 text-[11px] transition-colors ${
                    workflowMode === "build"
                      ? "bg-primary/15 text-foreground"
                      : "text-muted-foreground hover:bg-accent/50"
                  }`}
                >
                  Build
                </button>
              </div>
            )}

            {/* Attach button */}
            {!canContinueChat && (
              <button
                type="button"
                onClick={pickFiles}
                className="rounded-lg p-1.5 text-muted-foreground/70 transition-colors hover:bg-accent/70 hover:text-foreground"
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
                  const fallback = firstModelForProvider(modelCatalog, provider);
                  selectProviderModel(provider, fallback);
                }}
                className="h-7 rounded-md border border-border/70 bg-background/70 px-2 text-[11px] text-muted-foreground outline-none transition-colors hover:bg-accent/60 focus-visible:border-ring/70"
              >
                {providerOptions.map((option) => (
                  <option
                    key={option.id}
                    value={option.id}
                    className="bg-card text-foreground"
                  >
                    {option.label}
                  </option>
                ))}
              </select>
              <select
                value={selectedModel}
                onChange={(e) => selectProviderModel(selectedProvider, e.target.value)}
                className="h-7 rounded-md border border-border/70 bg-background/70 px-2 text-[11px] text-muted-foreground outline-none transition-colors hover:bg-accent/60 focus-visible:border-ring/70"
              >
                {models.map((model) => (
                  <option
                    key={model.name}
                    value={model.name}
                    className="bg-card text-foreground"
                  >
                    {model.name}
                  </option>
                ))}
              </select>
              <ModelDeprecationWarning
                catalog={modelCatalog}
                provider={selectedProvider}
                model={selectedModel}
                onSwitchAlternative={(alt) => selectProviderModel(selectedProvider, alt)}
              />
            </div>
          </div>

          <div className="flex items-center gap-2">
            {contextSnapshot && <ContextUsageChip snapshot={contextSnapshot} />}
            
            {providerUsage && (
              <ProviderUsageChip
                snapshot={providerUsage.find(u => u.provider === selectedProvider) ?? null}
                isLoading={providerUsageLoading}
                onRefresh={refreshProviderUsage}
              />
            )}

            <span className="hidden pr-1 text-[10px] text-muted-foreground/75 lg:inline">
              Use @ to reference, / for commands
            </span>

            {isWorking ? (
              <button
                type="button"
                onClick={() => handleStop().catch(console.error)}
                disabled={stopping}
                className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-destructive/40 bg-destructive/10 px-3 text-xs font-medium text-destructive transition-colors hover:bg-destructive/20 disabled:cursor-not-allowed disabled:opacity-60"
                title="Stop current task"
              >
                {stopping ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <XCircle size={14} />
                )}
                <span>Stop</span>
              </button>
            ) : (
              <button
                type="button"
                onClick={() => submit().catch(console.error)}
                disabled={!canSubmit}
                className={`flex h-8 w-8 items-center justify-center rounded-lg transition-all ${
                  canSubmit
                    ? "bg-primary text-primary-foreground hover:bg-primary/90"
                    : "bg-muted text-muted-foreground/40"
                }`}
                title={
                  canContinueChat
                    ? "Send follow-up message"
                    : workflowMode === "plan"
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
    </div>
  );
}

function getMentionContext(
  text: string,
  cursor: number
): { start: number; query: string } | null {
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

function extractAgentPresetId(content: string): string | null {
  const match = content.match(/(?:^|\s)@agent:([A-Za-z0-9._-]+)/);
  return match?.[1] ?? null;
}

interface ModelDeprecationWarningProps {
  catalog: ModelCatalogEntry[];
  provider: string;
  model: string;
  onSwitchAlternative: (alternative: string) => void;
}

function ModelDeprecationWarning({
  catalog,
  provider,
  model,
  onSwitchAlternative,
}: ModelDeprecationWarningProps) {
  const modelInfo = getModelInfo(catalog, provider, model);

  if (!modelInfo?.deprecated) {
    return null;
  }

  const reason = modelInfo.deprecation_reason;
  const alternative = modelInfo.suggested_alternative;

  return (
    <div className="group relative flex items-center">
      <AlertTriangle size={14} className="text-warning cursor-help" />
      <div className="absolute bottom-full left-1/2 mb-2 hidden w-64 -translate-x-1/2 rounded-lg border border-border/80 bg-background/95 p-2.5 text-[11px] shadow-lg elevation-2 group-hover:block z-50">
        <div className="mb-1.5 flex items-center gap-1.5 text-warning">
          <AlertTriangle size={12} />
          <span className="font-medium">Deprecated Model</span>
        </div>
        {reason && (
          <p className="mb-1.5 text-muted-foreground">{reason}</p>
        )}
        {alternative && (
          <button
            type="button"
            onClick={() => onSwitchAlternative(alternative)}
            className="mt-1 inline-flex items-center gap-1 rounded bg-primary/15 px-2 py-0.5 text-primary hover:bg-primary/20 transition-colors"
          >
            Switch to {alternative}
          </button>
        )}
      </div>
    </div>
  );
}

function buildNodeContextBlock(nodes: CanvasNode[]): string {
  const lines = nodes.map((n) => {
    const kind = n.kind ? ` (${n.kind})` : "";
    const desc = n.description ? `: ${n.description}` : "";
    return `- **${n.label}**${kind}${desc}`;
  });
  return `---\n## Selected Architecture Nodes\n${lines.join("\n")}\n---`;
}
