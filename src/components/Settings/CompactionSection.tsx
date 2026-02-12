import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Select } from "@/components/ui/select";
import type { CompactionSettings, ModelCatalogEntry, ModelInfo, PlanModeSettings } from "@/types";

export function CompactionSection() {
  const [settings, setSettings] = useState<CompactionSettings>({
    enabled: true,
    threshold_percentage: 0.8,
    preserve_recent: 4,
    custom_prompt: null,
    compaction_model: null,
  });
  const [planModeSettings, setPlanModeSettings] = useState<PlanModeSettings>({
    max_tokens: 25000,
  });
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [modelCatalog, setModelCatalog] = useState<ModelCatalogEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  useEffect(() => {
    loadSettings();
    loadPlanModeSettings();
    loadModelCatalog();
  }, []);

  const loadSettings = async () => {
    try {
      setLoading(true);
      setError(null);
      const data = await invoke<CompactionSettings>("get_compaction_settings");
      setSettings(data);
    } catch (err) {
      console.error("Failed to load compaction settings:", err);
      setError("Failed to load compaction settings");
    } finally {
      setLoading(false);
    }
  };

  const loadPlanModeSettings = async () => {
    try {
      const data = await invoke<PlanModeSettings>("get_plan_mode_settings");
      setPlanModeSettings(data);
    } catch (err) {
      console.error("Failed to load plan mode settings:", err);
    }
  };

  const loadModelCatalog = async () => {
    try {
      const data = await invoke<ModelCatalogEntry[]>("get_model_catalog");
      setModelCatalog(data);
    } catch (err) {
      console.error("Failed to load model catalog:", err);
    }
  };

  const saveSettings = async () => {
    try {
      setSaving(true);
      setError(null);
      setSuccess(null);
      await invoke("set_compaction_settings", { settings });
      await invoke("set_plan_mode_settings", { settings: planModeSettings });
      setSuccess("Settings saved successfully");
    } catch (err) {
      console.error("Failed to save settings:", err);
      setError("Failed to save settings");
    } finally {
      setSaving(false);
    }
  };

  const resetToDefaults = () => {
    setSettings({
      enabled: true,
      threshold_percentage: 0.8,
      preserve_recent: 4,
      custom_prompt: null,
      compaction_model: null,
    });
    setPlanModeSettings({
      max_tokens: 25000,
    });
  };

  // Flatten all models with their context windows
  const allModels: Array<{ provider: string } & ModelInfo> = modelCatalog.flatMap(
    (entry) =>
      entry.models.map((model) => ({
        provider: entry.provider,
        ...model,
      }))
  );

  // Format context window for display
  const formatContextWindow = (tokens: number): string => {
    if (tokens >= 1000) {
      return `${(tokens / 1000).toFixed(0)}k`;
    }
    return tokens.toString();
  };

  if (loading) {
    return (
      <div className="rounded-xl border border-border bg-card/60 p-4">
        <h3 className="text-lg font-semibold tracking-tight">Conversation Compaction</h3>
        <p className="mt-1 text-sm text-muted-foreground">Loading settings...</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="rounded-xl border border-border bg-card/60 p-4">
        <h3 className="text-lg font-semibold tracking-tight">Conversation Compaction</h3>
        <p className="text-sm text-muted-foreground">
          Configure how conversation history is summarized when context limits
          are approached. This helps maintain context continuity in long
          conversations.
        </p>
      </div>

      {error && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
          {error}
        </div>
      )}

      {success && (
        <div className="rounded-lg border border-success/30 bg-success/10 p-4 text-sm text-success">
          {success}
        </div>
      )}

      <div className="space-y-4 rounded-xl border border-border bg-card/60 p-4">
        {/* Plan Mode Max Tokens */}
        <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
          <label htmlFor="plan-max-tokens" className="text-sm font-medium">
            Plan Mode Max Tokens
          </label>
          <p className="text-sm text-muted-foreground">
            Maximum tokens for plan mode responses (content + reasoning + tool calls).
            Default: 25,000. Worker mode uses 180,000.
          </p>
          <Input
            id="plan-max-tokens"
            type="number"
            min={1000}
            max={200000}
            step={1000}
            value={planModeSettings.max_tokens}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
              setPlanModeSettings((prev) => ({
                ...prev,
                max_tokens: parseInt(e.target.value) || 25000,
              }))
            }
          />
        </div>

        {/* Enable/Disable Compaction */}
        <div className="flex items-center justify-between rounded-lg border border-border bg-background/60 p-4">
          <div className="space-y-0.5">
            <label className="text-sm font-medium">Enable Compaction</label>
            <p className="text-sm text-muted-foreground">
              Automatically summarize conversation history when approaching
              token limits
            </p>
          </div>
          <label className="relative inline-flex cursor-pointer items-center">
            <input
              type="checkbox"
              checked={settings.enabled}
              onChange={(e) =>
                setSettings((prev) => ({
                  ...prev,
                  enabled: e.target.checked,
                }))
              }
              className="peer sr-only"
            />
            <div className="peer h-6 w-11 rounded-full border border-border bg-muted transition-colors after:absolute after:left-[2px] after:top-[2px] after:h-5 after:w-5 after:rounded-full after:bg-background after:transition-all after:content-[''] peer-checked:bg-primary/80 peer-checked:after:translate-x-full peer-focus-visible:ring-2 peer-focus-visible:ring-ring/60"></div>
          </label>
        </div>

        {settings.enabled && (
          <>
            {/* Context Window Info */}
            <div className="rounded-lg border border-border bg-background/60 p-4">
              <h4 className="mb-2 text-sm font-medium">Model Context Windows</h4>
              <div className="grid grid-cols-2 gap-2 text-sm">
                {allModels.slice(0, 6).map((m) => (
                  <div
                    key={m.name}
                    className="flex justify-between text-muted-foreground"
                  >
                    <span>{m.name}</span>
                    <span className="font-mono">
                      {formatContextWindow(m.context_window)} tokens
                    </span>
                  </div>
                ))}
              </div>
              <p className="mt-2 text-xs text-muted-foreground">
                Compaction threshold is calculated as a percentage of the
                current model's context window
              </p>
            </div>

            {/* Threshold Percentage */}
            <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
              <label htmlFor="threshold" className="text-sm font-medium">
                Compaction Threshold (%)
              </label>
              <p className="text-sm text-muted-foreground">
                Percentage of model's context window that triggers compaction
                (default: 80%)
              </p>
              <div className="flex items-center gap-4">
                <Input
                  id="threshold"
                  type="range"
                  min={50}
                  max={95}
                  step={5}
                  value={Math.round(settings.threshold_percentage * 100)}
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                    setSettings((prev) => ({
                      ...prev,
                      threshold_percentage: parseInt(e.target.value) / 100,
                    }))
                  }
                  className="flex-1"
                />
                <span className="w-16 text-sm font-mono text-foreground">
                  {Math.round(settings.threshold_percentage * 100)}%
                </span>
              </div>
            </div>

            {/* Preserve Recent Messages */}
            <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
              <label htmlFor="preserve" className="text-sm font-medium">
                Preserve Recent Messages
              </label>
              <p className="text-sm text-muted-foreground">
                Number of most recent messages to keep verbatim (not
                summarized)
              </p>
              <Input
                id="preserve"
                type="number"
                min={1}
                max={20}
                step={1}
                value={settings.preserve_recent}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                  setSettings((prev) => ({
                    ...prev,
                    preserve_recent: parseInt(e.target.value) || 4,
                  }))
                }
              />
            </div>

            {/* Compaction Model */}
            <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
              <label htmlFor="model" className="text-sm font-medium">
                Compaction Model (Optional)
              </label>
              <p className="text-sm text-muted-foreground">
                Specific model to use for summarization. If not set, uses the
                current chat model.
              </p>
              <Select
                id="model"
                value={settings.compaction_model || ""}
                onChange={(e: React.ChangeEvent<HTMLSelectElement>) =>
                  setSettings((prev) => ({
                    ...prev,
                    compaction_model:
                      e.target.value === "" ? null : e.target.value,
                  }))
                }
              >
                <option value="">Use current chat model (recommended)</option>
                {allModels.map((m) => (
                  <option key={m.name} value={m.name}>
                    {m.provider}: {m.name} ({formatContextWindow(m.context_window)}
                    )
                  </option>
                ))}
              </Select>
            </div>

            {/* Custom Prompt */}
            <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
              <label htmlFor="prompt" className="text-sm font-medium">
                Custom Summarization Prompt (Optional)
              </label>
              <p className="text-sm text-muted-foreground">
                Override the default prompt used to summarize conversations.
                Leave empty to use the default.
              </p>
              <Textarea
                id="prompt"
                placeholder="Enter custom summarization prompt..."
                value={settings.custom_prompt || ""}
                onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) =>
                  setSettings((prev) => ({
                    ...prev,
                    custom_prompt: e.target.value || null,
                  }))
                }
                rows={8}
                className="font-mono text-sm"
              />
              {settings.custom_prompt && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    setSettings((prev) => ({
                      ...prev,
                      custom_prompt: null,
                    }))
                  }
                >
                  Reset to default
                </Button>
              )}
            </div>
          </>
        )}
      </div>

      {/* Actions */}
      <div className="flex gap-2 border-t border-border/70 pt-2">
        <Button onClick={saveSettings} disabled={saving}>
          {saving ? "Saving..." : "Save Settings"}
        </Button>
        <Button variant="outline" onClick={resetToDefaults} disabled={saving}>
          Reset to Defaults
        </Button>
      </div>

      {/* Info Card */}
      <div className="rounded-xl border border-border bg-card/60 p-4">
        <h4 className="mb-2 text-sm font-medium">How it works</h4>
        <ul className="list-inside list-disc space-y-1 text-sm text-muted-foreground">
          <li>
            Each model has a different context window size (MiniMax: 204k,
            Kimi: 128k-256k tokens)
          </li>
          <li>
            Compaction triggers when conversation reaches the configured
            percentage of the current model's context window
          </li>
          <li>
            Recent messages are always preserved verbatim for context
            continuity
          </li>
          <li>
            Summaries are generated using AI and stored for future follow-ups
          </li>
        </ul>
      </div>
    </div>
  );
}
