import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Slider } from "@/components/ui/slider";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import type {
  AutoMemoryPathView,
  AutoMemorySettingsView,
  CompactionSettings,
  MemoryPreferenceEntry,
  ModelCatalogEntry,
  ModelInfo,
  PlanModeSettings,
} from "@/types";

export function ContextSection() {
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
  const [memorySettings, setMemorySettings] = useState<AutoMemorySettingsView>({
    enabled: true,
    source: "default",
  });
  const [memoryPath, setMemoryPath] = useState<string>("");
  const [memoryPreferences, setMemoryPreferences] = useState<MemoryPreferenceEntry[]>([]);
  const [startupMemory, setStartupMemory] = useState<string>("");

  const [newMemoryKey, setNewMemoryKey] = useState("");
  const [newMemoryValue, setNewMemoryValue] = useState("");
  const [newMemoryCategory, setNewMemoryCategory] = useState("");

  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [memorySaving, setMemorySaving] = useState(false);
  const [modelCatalog, setModelCatalog] = useState<ModelCatalogEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  useEffect(() => {
    loadAll();
  }, []);

  const loadAll = async () => {
    try {
      setLoading(true);
      setError(null);
      const [
        compaction,
        plan,
        catalog,
        autoMemory,
        memoryPathView,
        preferences,
        startup,
      ] = await Promise.all([
        invoke<CompactionSettings>("get_compaction_settings"),
        invoke<PlanModeSettings>("get_plan_mode_settings"),
        invoke<ModelCatalogEntry[]>("get_model_catalog"),
        invoke<AutoMemorySettingsView>("get_auto_memory_settings"),
        invoke<AutoMemoryPathView>("get_auto_memory_entrypoint"),
        invoke<MemoryPreferenceEntry[]>("list_auto_memory_preferences"),
        invoke<string>("read_auto_memory_context"),
      ]);
      setSettings(compaction);
      setPlanModeSettings(plan);
      setModelCatalog(catalog);
      setMemorySettings(autoMemory);
      setMemoryPath(memoryPathView.path);
      setMemoryPreferences(preferences);
      setStartupMemory(startup);
    } catch (err) {
      console.error("Failed to load context settings:", err);
      setError("Failed to load context settings");
    } finally {
      setLoading(false);
    }
  };

  const saveCompactionSettings = async () => {
    try {
      setSaving(true);
      setError(null);
      setSuccess(null);
      await invoke("set_compaction_settings", { settings });
      await invoke("set_plan_mode_settings", { settings: planModeSettings });
      setSuccess("Context settings saved successfully");
    } catch (err) {
      console.error("Failed to save context settings:", err);
      setError("Failed to save context settings");
    } finally {
      setSaving(false);
    }
  };

  const setAutoMemoryEnabled = async (enabled: boolean) => {
    try {
      setMemorySaving(true);
      setError(null);
      await invoke("set_auto_memory_settings", { enabled });
      const updated = await invoke<AutoMemorySettingsView>("get_auto_memory_settings");
      setMemorySettings(updated);
      await refreshMemoryData();
    } catch (err) {
      console.error("Failed to update auto memory setting:", err);
      setError("Failed to update auto memory setting");
    } finally {
      setMemorySaving(false);
    }
  };

  const refreshMemoryData = async () => {
    const [preferences, startup] = await Promise.all([
      invoke<MemoryPreferenceEntry[]>("list_auto_memory_preferences"),
      invoke<string>("read_auto_memory_context"),
    ]);
    setMemoryPreferences(preferences);
    setStartupMemory(startup);
  };

  const addMemoryPreference = async () => {
    const key = newMemoryKey.trim();
    const value = newMemoryValue.trim();
    if (!key || !value) return;

    try {
      setMemorySaving(true);
      setError(null);
      await invoke("upsert_auto_memory_preference", {
        key,
        value,
        category: newMemoryCategory.trim() || null,
      });
      setNewMemoryKey("");
      setNewMemoryValue("");
      setNewMemoryCategory("");
      await refreshMemoryData();
      setSuccess("Memory preference saved");
    } catch (err) {
      console.error("Failed to save memory preference:", err);
      setError("Failed to save memory preference");
    } finally {
      setMemorySaving(false);
    }
  };

  const deleteMemoryPreference = async (key: string) => {
    try {
      setMemorySaving(true);
      setError(null);
      await invoke("delete_auto_memory_preference", { key });
      await refreshMemoryData();
    } catch (err) {
      console.error("Failed to delete memory preference:", err);
      setError("Failed to delete memory preference");
    } finally {
      setMemorySaving(false);
    }
  };

  const compactMemory = async () => {
    try {
      setMemorySaving(true);
      setError(null);
      const removed = await invoke<number>("compact_auto_memory");
      await refreshMemoryData();
      setSuccess(`Memory compacted (${removed} old entries removed)`);
    } catch (err) {
      console.error("Failed to compact memory:", err);
      setError("Failed to compact memory");
    } finally {
      setMemorySaving(false);
    }
  };

  const resetCompactionDefaults = () => {
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

  const allModels: Array<{ provider: string } & ModelInfo> = useMemo(
    () =>
      modelCatalog.flatMap((entry) =>
        entry.models.map((model) => ({
          provider: entry.provider,
          ...model,
        }))
      ),
    [modelCatalog]
  );

  const formatContextWindow = (tokens: number): string => {
    if (tokens >= 1000) {
      return `${(tokens / 1000).toFixed(0)}k`;
    }
    return tokens.toString();
  };

  if (loading) {
    return (
      <div className="rounded-xl border border-border bg-card/60 p-4">
        <h3 className="text-lg font-semibold tracking-tight">Context Settings</h3>
        <p className="mt-1 text-sm text-muted-foreground">Loading settings...</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
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

      <section className="space-y-4 rounded-xl border border-border bg-card/60 p-4">
        <h3 className="text-sm font-semibold">Memory</h3>

        <div className="flex items-center justify-between rounded-lg border border-border bg-background/60 p-4">
          <div className="space-y-1">
            <Label htmlFor="auto-memory-switch">Enable Auto Memory</Label>
            <p className="text-xs text-muted-foreground">
              Persist per-project preferences to MEMORY.md and inject concise startup context.
            </p>
            <p className="text-[11px] text-muted-foreground">
              Source: <span className="font-mono">{memorySettings.source}</span>
            </p>
          </div>
          <Switch
            id="auto-memory-switch"
            checked={memorySettings.enabled}
            onCheckedChange={(checked) => setAutoMemoryEnabled(checked).catch(console.error)}
            disabled={memorySaving}
          />
        </div>

        <div className="rounded-lg border border-border bg-background/60 p-3">
          <p className="text-xs text-muted-foreground">Memory file</p>
          <p className="mt-1 break-all font-mono text-xs text-foreground">{memoryPath}</p>
          <div className="mt-3 flex gap-2">
            <Button variant="outline" size="sm" onClick={() => refreshMemoryData().catch(console.error)} disabled={memorySaving}>
              Refresh Memory
            </Button>
            <Button variant="outline" size="sm" onClick={() => compactMemory().catch(console.error)} disabled={memorySaving}>
              Compact Memory
            </Button>
          </div>
        </div>

        <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
          <Label>Add or Update Preference</Label>
          <div className="grid gap-2 sm:grid-cols-3">
            <Input placeholder="Key" value={newMemoryKey} onChange={(e) => setNewMemoryKey(e.target.value)} />
            <Input placeholder="Category (optional)" value={newMemoryCategory} onChange={(e) => setNewMemoryCategory(e.target.value)} />
            <Input placeholder="Value" value={newMemoryValue} onChange={(e) => setNewMemoryValue(e.target.value)} />
          </div>
          <div className="flex justify-end">
            <Button size="sm" onClick={() => addMemoryPreference().catch(console.error)} disabled={memorySaving || !newMemoryKey.trim() || !newMemoryValue.trim()}>
              Save Preference
            </Button>
          </div>
        </div>

        <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
          <Label>Stored Preferences</Label>
          {memoryPreferences.length === 0 ? (
            <p className="text-xs text-muted-foreground">No stored preferences.</p>
          ) : (
            <div className="space-y-2">
              {memoryPreferences.slice(0, 20).map((entry) => (
                <div key={entry.key} className="rounded border border-border/70 px-2 py-1.5">
                  <div className="flex items-start justify-between gap-3">
                    <div>
                      <p className="text-xs font-medium text-foreground">{entry.key}</p>
                      <p className="text-xs text-muted-foreground">{entry.value}</p>
                      <p className="text-[11px] text-muted-foreground/80">
                        {entry.category ? `${entry.category} | ` : ""}{entry.updated_at}
                      </p>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => deleteMemoryPreference(entry.key).catch(console.error)}
                      disabled={memorySaving}
                    >
                      Delete
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
          <Label>Startup Memory Context</Label>
          <Textarea value={startupMemory} readOnly rows={8} className="font-mono text-xs" />
        </div>
      </section>

      <section className="space-y-4 rounded-xl border border-border bg-card/60 p-4">
        <h3 className="text-sm font-semibold">Compaction</h3>

        <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
          <Label htmlFor="plan-max-tokens">Plan Mode Max Tokens</Label>
          <p className="text-xs text-muted-foreground">
            Maximum tokens for plan mode responses (content + reasoning + tool calls). Default: 25,000.
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
                max_tokens: parseInt(e.target.value, 10) || 25000,
              }))
            }
          />
        </div>

        <div className="flex items-center justify-between rounded-lg border border-border bg-background/60 p-4">
          <div className="space-y-0.5">
            <Label htmlFor="enable-compaction-switch">Enable Compaction</Label>
            <p className="text-xs text-muted-foreground">
              Automatically summarize conversation history when approaching token limits.
            </p>
          </div>
          <Switch
            id="enable-compaction-switch"
            checked={settings.enabled}
            onCheckedChange={(checked) =>
              setSettings((prev) => ({
                ...prev,
                enabled: checked,
              }))
            }
          />
        </div>

        {settings.enabled && (
          <>
            <div className="rounded-lg border border-border bg-background/60 p-4">
              <h4 className="mb-2 text-sm font-medium">Model Context Windows</h4>
              <div className="grid grid-cols-2 gap-2 text-sm">
                {allModels.slice(0, 6).map((m) => (
                  <div key={m.name} className="flex justify-between text-muted-foreground">
                    <span>{m.name}</span>
                    <span className="font-mono">{formatContextWindow(m.context_window)} tokens</span>
                  </div>
                ))}
              </div>
            </div>

            <div className="space-y-3 rounded-lg border border-border bg-background/60 p-3">
              <Label htmlFor="threshold">Compaction Threshold (%)</Label>
              <div className="flex items-center gap-4">
              <Slider
                id="threshold"
                min={50}
                max={95}
                step={5}
                value={[Math.round(settings.threshold_percentage * 100)]}
                onValueChange={([value]) =>
                  setSettings((prev) => ({
                    ...prev,
                    threshold_percentage: value / 100,
                  }))
                }
                className="flex-1"
              />
                <span className="w-16 text-sm font-mono text-foreground">{Math.round(settings.threshold_percentage * 100)}%</span>
              </div>
            </div>

            <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
              <Label htmlFor="preserve">Preserve Recent Messages</Label>
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
                    preserve_recent: parseInt(e.target.value, 10) || 4,
                  }))
                }
              />
            </div>

            <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
              <Label htmlFor="model">Compaction Model (Optional)</Label>
              <Select
                id="model"
                value={settings.compaction_model || ""}
                onChange={(e: React.ChangeEvent<HTMLSelectElement>) =>
                  setSettings((prev) => ({
                    ...prev,
                    compaction_model: e.target.value === "" ? null : e.target.value,
                  }))
                }
              >
                <option value="">Use current chat model (recommended)</option>
                {allModels.map((m) => (
                  <option key={m.name} value={m.name}>
                    {m.provider}: {m.name} ({formatContextWindow(m.context_window)})
                  </option>
                ))}
              </Select>
            </div>

            <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
              <Label htmlFor="prompt">Custom Summarization Prompt (Optional)</Label>
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
            </div>
          </>
        )}

        <Separator />

        <div className="flex gap-2 pt-2">
          <Button onClick={saveCompactionSettings} disabled={saving}>
            {saving ? "Saving..." : "Save Context Settings"}
          </Button>
          <Button variant="outline" onClick={resetCompactionDefaults} disabled={saving}>
            Reset Compaction Defaults
          </Button>
        </div>
      </section>
    </div>
  );
}