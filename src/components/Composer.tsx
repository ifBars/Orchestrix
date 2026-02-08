import { ArrowUp, Paperclip, Loader2, XCircle, MessageCircle } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useRef, useState } from "react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";

export function Composer() {
  const [prompt, setPrompt] = useState("");
  const [attachments, setAttachments] = useState<string[]>([]);
  const [mode, setMode] = useState<"plan" | "build">("plan");
  const [stopping, setStopping] = useState(false);
  const [sending, setSending] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

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
  };

  const canSubmit = prompt.trim().length > 0 && !sending;

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
