import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Slider } from "@/components/ui/slider";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import type { ModelCatalogEntry } from "@/types";

interface PromptSuggestionSettings {
  enabled: boolean;
  context_turns: number;
  suggestion_model: string | null;
  system_prompt: string | null;
}

export function SuggestionsSection() {
  const [settings, setSettings] = useState<PromptSuggestionSettings>({
    enabled: true,
    context_turns: 2,
    suggestion_model: null,
    system_prompt: null,
  });
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
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
      const [suggestionSettings, catalog] = await Promise.all([
        invoke<PromptSuggestionSettings>("get_prompt_suggestion_settings"),
        invoke<ModelCatalogEntry[]>("get_model_catalog"),
      ]);
      setSettings(suggestionSettings);
      setModelCatalog(catalog);
    } catch (err) {
      console.error("Failed to load suggestion settings:", err);
      setError("Failed to load suggestion settings");
    } finally {
      setLoading(false);
    }
  };

  const saveSettings = async () => {
    try {
      setSaving(true);
      setError(null);
      setSuccess(null);
      await invoke("set_prompt_suggestion_settings", { settings });
      setSuccess("Suggestion settings saved successfully");
    } catch (err) {
      console.error("Failed to save suggestion settings:", err);
      setError("Failed to save suggestion settings");
    } finally {
      setSaving(false);
    }
  };

  const resetDefaults = () => {
    setSettings({
      enabled: true,
      context_turns: 2,
      suggestion_model: null,
      system_prompt: null,
    });
  };

  const allModels: Array<{ provider: string; name: string }> = useMemo(
    () =>
      modelCatalog.flatMap((entry) =>
        entry.models.map((model) => ({
          provider: entry.provider,
          name: model.name,
        }))
      ),
    [modelCatalog]
  );

  if (loading) {
    return (
      <div className="rounded-xl border border-border bg-card/60 p-4">
        <h3 className="text-lg font-semibold tracking-tight">Prompt Suggestions</h3>
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
        <h3 className="text-sm font-semibold">Prompt Suggestions</h3>

        <div className="flex items-center justify-between rounded-lg border border-border bg-background/60 p-4">
          <div className="space-y-1">
            <Label htmlFor="enable-suggestions-switch">Enable Prompt Suggestions</Label>
            <p className="text-xs text-muted-foreground">
              Show contextual follow-up suggestions when focusing the composer in existing conversations.
            </p>
          </div>
          <Switch
            id="enable-suggestions-switch"
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
            <div className="space-y-3 rounded-lg border border-border bg-background/60 p-3">
              <Label>Context Turns</Label>
              <p className="text-xs text-muted-foreground">
                Number of recent message pairs (user + assistant) to include in context for generating suggestions.
              </p>
              <div className="flex items-center gap-4">
                <Slider
                  min={1}
                  max={6}
                  step={1}
                  value={[settings.context_turns]}
                  onValueChange={([value]) =>
                    setSettings((prev) => ({
                      ...prev,
                      context_turns: value,
                    }))
                  }
                  className="flex-1"
                />
                <span className="w-16 text-sm font-mono text-foreground">{settings.context_turns}</span>
              </div>
            </div>

            <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
              <Label htmlFor="suggestion-model">Suggestion Model (Optional)</Label>
              <p className="text-xs text-muted-foreground">
                Specific model to use for generating suggestions. Leave empty to use the current chat model.
              </p>
              <Select
                id="suggestion-model"
                value={settings.suggestion_model || ""}
                onChange={(e: React.ChangeEvent<HTMLSelectElement>) =>
                  setSettings((prev) => ({
                    ...prev,
                    suggestion_model: e.target.value === "" ? null : e.target.value,
                  }))
                }
              >
                <option value="">Use current chat model (recommended)</option>
                {allModels.map((m) => (
                  <option key={m.name} value={m.name}>
                    {m.provider}: {m.name}
                  </option>
                ))}
              </Select>
            </div>

            <div className="space-y-2 rounded-lg border border-border bg-background/60 p-3">
              <Label htmlFor="system-prompt">Custom System Prompt (Optional)</Label>
              <p className="text-xs text-muted-foreground">
                Custom prompt for generating suggestions. Leave empty to use the default prompt.
              </p>
              <Textarea
                id="system-prompt"
                placeholder="Based on the recent conversation, suggest a brief (1-2 sentence) follow-up prompt..."
                value={settings.system_prompt || ""}
                onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) =>
                  setSettings((prev) => ({
                    ...prev,
                    system_prompt: e.target.value || null,
                  }))
                }
                rows={6}
                className="font-mono text-sm"
              />
            </div>
          </>
        )}

        <Separator />

        <div className="flex gap-2 pt-2">
          <Button onClick={saveSettings} disabled={saving}>
            {saving ? "Saving..." : "Save Settings"}
          </Button>
          <Button variant="outline" onClick={resetDefaults} disabled={saving}>
            Reset Defaults
          </Button>
        </div>
      </section>
    </div>
  );
}
