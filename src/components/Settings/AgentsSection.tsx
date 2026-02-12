import { FormEvent, useEffect, useMemo, useState } from "react";
import { Bot, Plus, Save, Trash2 } from "lucide-react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import type { AgentMode, AgentPreset, CreateAgentPresetInput, ToolPermission } from "@/types";

type AgentFormState = {
  id: string;
  name: string;
  description: string;
  mode: AgentMode;
  model: string;
  temperature: string;
  steps: string;
  prompt: string;
  tags: string;
  toolWrite: boolean;
  toolEdit: boolean;
  toolBash: boolean;
};

const EMPTY_FORM: AgentFormState = {
  id: "",
  name: "",
  description: "",
  mode: "subagent",
  model: "",
  temperature: "",
  steps: "",
  prompt: "",
  tags: "",
  toolWrite: false,
  toolEdit: false,
  toolBash: false,
};

export function AgentsSection() {
  const [
    agentPresets,
    selectedAgentPresetId,
    refreshAgentPresets,
    createAgentPreset,
    updateAgentPreset,
    deleteAgentPreset,
    setSelectedAgentPreset,
  ] = useAppStore(
    useShallow((state) => [
      state.agentPresets,
      state.selectedAgentPresetId,
      state.refreshAgentPresets,
      state.createAgentPreset,
      state.updateAgentPreset,
      state.deleteAgentPreset,
      state.setSelectedAgentPreset,
    ])
  );

  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [form, setForm] = useState<AgentFormState>(EMPTY_FORM);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    refreshAgentPresets().catch(console.error);
  }, [refreshAgentPresets]);

  useEffect(() => {
    if (!selectedId) {
      setForm(EMPTY_FORM);
      return;
    }
    const preset = agentPresets.find((item) => item.id === selectedId);
    if (!preset) {
      setForm(EMPTY_FORM);
      return;
    }
    setForm(fromPreset(preset));
  }, [agentPresets, selectedId]);

  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return agentPresets;
    return agentPresets.filter((preset) => {
      return (
        preset.id.toLowerCase().includes(normalized) ||
        preset.name.toLowerCase().includes(normalized) ||
        preset.description.toLowerCase().includes(normalized) ||
        preset.tags.some((tag) => tag.toLowerCase().includes(normalized))
      );
    });
  }, [agentPresets, query]);

  const handleSavePreset = async (event: FormEvent) => {
    event.preventDefault();
    setError(null);

    if (!form.id.trim()) {
      setError("Preset ID is required.");
      return;
    }
    if (!form.name.trim()) {
      setError("Name is required.");
      return;
    }
    if (!form.prompt.trim()) {
      setError("Prompt is required.");
      return;
    }

    const payload: CreateAgentPresetInput = {
      id: form.id.trim(),
      name: form.name.trim(),
      description: form.description.trim(),
      mode: form.mode,
      model: form.model.trim() ? form.model.trim() : undefined,
      temperature: form.temperature.trim() ? Number(form.temperature) : undefined,
      steps: form.steps.trim() ? Number(form.steps) : undefined,
      prompt: form.prompt,
      tags: form.tags
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean),
      tools: {
        write: form.toolWrite,
        edit: form.toolEdit,
        bash: form.toolBash,
      },
    };

    setSaving(true);
    try {
      if (selectedId) {
        await updateAgentPreset(payload);
      } else {
        await createAgentPreset(payload);
        setSelectedId(payload.id);
      }
    } catch (saveError) {
      console.error(saveError);
      setError("Failed to save preset.");
    } finally {
      setSaving(false);
    }
  };

  const handleRemoveSelectedPreset = async () => {
    if (!selectedId) return;
    setError(null);
    setDeleting(true);

    try {
      await deleteAgentPreset(selectedId);
      setSelectedId(null);
      setForm(EMPTY_FORM);
    } catch (deleteError) {
      console.error(deleteError);
      setError("Failed to delete preset.");
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="h-full min-h-0">
      <div className="mb-4">
        <div className="flex items-center gap-2 text-sm font-semibold">
          <Bot size={16} />
          Agent Presets
        </div>
        <p className="mt-1 text-xs text-muted-foreground">
          Human-authored presets discovered from <code className="rounded bg-muted px-1 py-0.5">.agents/agents</code>,
          <code className="ml-1 rounded bg-muted px-1 py-0.5">.agent/agents</code>, and
          <code className="ml-1 rounded bg-muted px-1 py-0.5">.opencode/agents</code>.
        </p>
      </div>

      <div className="grid min-h-0 gap-4 xl:grid-cols-[320px_1fr]">
        <section className="min-h-0 rounded-xl border border-border bg-card/60 p-3 flex flex-col">
          <div className="mb-2 flex items-center gap-2">
            <Input placeholder="Search presets" value={query} onChange={(event) => setQuery(event.target.value)} />
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="gap-1"
              onClick={() => {
                setSelectedId(null);
                setForm(EMPTY_FORM);
              }}
            >
              <Plus size={12} />
              New
            </Button>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto pr-1">
            {filtered.length === 0 ? (
              <div className="rounded-lg border border-dashed border-border px-3 py-8 text-center text-xs text-muted-foreground">
                No presets found.
              </div>
            ) : (
              <div className="space-y-1.5">
                {filtered.map((preset) => {
                  const active = selectedId === preset.id;
                  const composerSelected = selectedAgentPresetId === preset.id;

                  return (
                    <button
                      key={preset.id}
                      type="button"
                      onClick={() => setSelectedId(preset.id)}
                      className={`w-full rounded-lg border px-3 py-2 text-left transition-colors ${
                        active
                          ? "border-primary/40 bg-primary/8"
                          : "border-border bg-background/40 hover:bg-accent/50"
                      }`}
                    >
                      <div className="flex items-center gap-2">
                        <span className="truncate text-sm font-medium">{preset.name}</span>
                        {composerSelected && (
                          <span className="rounded-full bg-success/15 px-1.5 py-0.5 text-[10px] font-medium text-success">
                            active
                          </span>
                        )}
                      </div>
                      <div className="mt-1 flex items-center gap-2 text-[10px] text-muted-foreground">
                        <span className="rounded border border-border px-1.5 py-0.5">{preset.mode}</span>
                        <span>@agent:{preset.id}</span>
                        {preset.validation_issues && preset.validation_issues.length > 0 && (
                          <span className="text-warning">{preset.validation_issues.length} issue(s)</span>
                        )}
                      </div>
                      {preset.description && (
                        <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">{preset.description}</p>
                      )}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </section>

        <section className="min-h-0 rounded-xl border border-border bg-card/60 p-4">
          <form className="flex min-h-[680px] flex-col gap-3" onSubmit={handleSavePreset}>
            <div className="grid gap-3 md:grid-cols-2">
              <div>
                <label className="mb-1 block text-xs font-medium text-muted-foreground">Preset ID</label>
                <Input
                  placeholder="code-reviewer"
                  value={form.id}
                  onChange={(event) => setForm((prev) => ({ ...prev, id: event.target.value }))}
                  disabled={Boolean(selectedId)}
                  required
                />
              </div>
              <div>
                <label className="mb-1 block text-xs font-medium text-muted-foreground">Name</label>
                <Input
                  placeholder="Code Reviewer"
                  value={form.name}
                  onChange={(event) => setForm((prev) => ({ ...prev, name: event.target.value }))}
                  required
                />
              </div>
              <div>
                <label className="mb-1 block text-xs font-medium text-muted-foreground">Mode</label>
                <select
                  className="h-9 w-full rounded-md border border-input bg-background px-2 text-sm"
                  value={form.mode}
                  onChange={(event) =>
                    setForm((prev) => ({ ...prev, mode: event.target.value as AgentMode }))
                  }
                >
                  <option value="subagent">subagent</option>
                  <option value="primary">primary</option>
                </select>
              </div>
              <div>
                <label className="mb-1 block text-xs font-medium text-muted-foreground">Model (optional)</label>
                <Input
                  placeholder="anthropic/claude-sonnet-4-5"
                  value={form.model}
                  onChange={(event) => setForm((prev) => ({ ...prev, model: event.target.value }))}
                />
              </div>
              <div>
                <label className="mb-1 block text-xs font-medium text-muted-foreground">Temperature</label>
                <Input
                  placeholder="0.1"
                  value={form.temperature}
                  onChange={(event) => setForm((prev) => ({ ...prev, temperature: event.target.value }))}
                />
              </div>
              <div>
                <label className="mb-1 block text-xs font-medium text-muted-foreground">Steps</label>
                <Input
                  placeholder="8"
                  value={form.steps}
                  onChange={(event) => setForm((prev) => ({ ...prev, steps: event.target.value }))}
                />
              </div>
            </div>

            <div>
              <label className="mb-1 block text-xs font-medium text-muted-foreground">Description</label>
              <Input
                placeholder="Reviews code for maintainability and safety"
                value={form.description}
                onChange={(event) => setForm((prev) => ({ ...prev, description: event.target.value }))}
              />
            </div>

            <div>
              <label className="mb-1 block text-xs font-medium text-muted-foreground">Tags (comma separated)</label>
              <Input
                placeholder="review, quality, security"
                value={form.tags}
                onChange={(event) => setForm((prev) => ({ ...prev, tags: event.target.value }))}
              />
            </div>

            <div className="rounded-lg border border-border bg-muted/20 px-3 py-2">
              <p className="mb-2 text-xs font-medium text-muted-foreground">Tool permissions</p>
              <div className="flex flex-wrap gap-4 text-xs">
                <label className="inline-flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={form.toolWrite}
                    onChange={(event) =>
                      setForm((prev) => ({ ...prev, toolWrite: event.target.checked }))
                    }
                  />
                  write
                </label>
                <label className="inline-flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={form.toolEdit}
                    onChange={(event) => setForm((prev) => ({ ...prev, toolEdit: event.target.checked }))}
                  />
                  edit
                </label>
                <label className="inline-flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={form.toolBash}
                    onChange={(event) => setForm((prev) => ({ ...prev, toolBash: event.target.checked }))}
                  />
                  bash
                </label>
              </div>
            </div>

            <div className="flex min-h-0 flex-1 flex-col">
              <label className="mb-1 block text-xs font-medium text-muted-foreground">Prompt (markdown body)</label>
              <Textarea
                className="min-h-0 flex-1 resize-none"
                placeholder="You are a specialist..."
                value={form.prompt}
                onChange={(event) => setForm((prev) => ({ ...prev, prompt: event.target.value }))}
                required
              />
            </div>

            {error ? <p className="text-xs text-destructive">{error}</p> : null}

            <div className="flex items-center justify-between gap-2 border-t border-border pt-3">
              <div className="text-xs text-muted-foreground">
                {form.id ? (
                  <span>
                    Mention token: <code className="rounded bg-muted px-1 py-0.5">@agent:{form.id}</code>
                  </span>
                ) : (
                  <span>Create a preset to use it in chat and delegation.</span>
                )}
              </div>

              <div className="flex items-center gap-2">
                {selectedId && (
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    className="gap-1 text-destructive"
                    onClick={() => handleRemoveSelectedPreset().catch(console.error)}
                    disabled={deleting}
                  >
                    <Trash2 size={12} />
                    {deleting ? "Deleting" : "Delete"}
                  </Button>
                )}

                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  disabled={!form.id.trim()}
                  onClick={() => setSelectedAgentPreset(form.id.trim() || null)}
                >
                  Use In Composer
                </Button>

                <Button type="submit" size="sm" className="gap-1" disabled={saving}>
                  <Save size={12} />
                  {saving ? "Saving" : selectedId ? "Update" : "Create"}
                </Button>
              </div>
            </div>
          </form>
        </section>
      </div>
    </div>
  );
}

function fromPreset(preset: AgentPreset): AgentFormState {
  const tools = preset.tools ?? {};
  return {
    id: preset.id,
    name: preset.name,
    description: preset.description,
    mode: preset.mode,
    model: preset.model ?? "",
    temperature: preset.temperature != null ? String(preset.temperature) : "",
    steps: preset.steps != null ? String(preset.steps) : "",
    prompt: preset.prompt,
    tags: (preset.tags ?? []).join(", "),
    toolWrite: toolFlag(tools.write),
    toolEdit: toolFlag(tools.edit),
    toolBash: toolFlag(tools.bash),
  };
}

function toolFlag(value: ToolPermission | undefined): boolean {
  return typeof value === "boolean" ? value : false;
}
