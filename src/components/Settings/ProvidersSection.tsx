import { Check, Server, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { providerLabel, providerOptionsFromCatalog } from "@/lib/providers";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";

export function ProvidersSection() {
  const [providerConfigs, modelCatalog, setProviderConfig, removeProviderConfig] = useAppStore(
    useShallow((state) => [state.providerConfigs, state.modelCatalog, state.setProviderConfig, state.removeProviderConfig])
  );

  const providers = providerOptionsFromCatalog(modelCatalog);

  const [provider, setProvider] = useState<string>(providers[0]?.id ?? "");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [removingProvider, setRemovingProvider] = useState<string | null>(null);

  const current = useMemo(
    () => providerConfigs.find((item) => item.provider === provider),
    [provider, providerConfigs]
  );

  useEffect(() => {
    if (!provider && providers[0]?.id) {
      setProvider(providers[0].id);
    }
  }, [provider, providers]);

  useEffect(() => {
    setModel(current?.default_model ?? "");
    setBaseUrl(current?.base_url ?? "");
  }, [current?.base_url, current?.default_model, provider]);

  const modelPlaceholder = provider === "minimax" ? "e.g. MiniMax-M2.1" : provider === "zhipu" ? "e.g. glm-4.7" : provider === "modal" ? "e.g. zai-org/GLM-5-FP8" : "e.g. kimi-k2.5";
  const baseUrlPlaceholder =
    provider === "minimax"
      ? "https://api.minimaxi.chat"
      : provider === "zhipu"
        ? "https://api.z.ai/api/coding/paas/v4"
        : provider === "modal"
          ? "https://api.us-west-2.modal.direct/v1"
          : "https://api.moonshot.cn";

  const apiKeyHint =
    provider === "zhipu"
      ? "Get your key from z.ai/manage-apikey. Requires an active GLM Coding Plan."
      : provider === "minimax"
        ? "Get your key from platform.minimaxi.com."
        : provider === "modal"
          ? "Get your key from modal.com/settings."
          : "Get your key from platform.moonshot.cn.";

  const handleSaveProvider = async () => {
    if (!apiKey.trim()) return;

    setError(null);
    setSaving(true);
    try {
      await setProviderConfig(provider, apiKey.trim(), model.trim(), baseUrl.trim());
      setApiKey("");
    } catch (saveError) {
      console.error(saveError);
      setError("Failed to save provider configuration.");
    } finally {
      setSaving(false);
    }
  };

  const handleRemoveProvider = async (providerId: string) => {
    setRemovingProvider(providerId);
    try {
      await removeProviderConfig(providerId);
    } catch (removeError) {
      console.error(removeError);
      setError(`Failed to remove ${providerId} configuration.`);
    } finally {
      setRemovingProvider(null);
    }
  };

  return (
    <div className="grid min-h-0 gap-4 lg:grid-cols-[1.2fr_1fr]">
      <section className="rounded-xl border border-border bg-card/60 p-4">
        <h3 className="mb-1 text-sm font-semibold">Provider Configuration</h3>
        <p className="mb-4 text-xs text-muted-foreground">
          Configure API keys and defaults used by planning and execution agents.
        </p>

        <div className="space-y-3">
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">Provider</label>
            <Select
              value={provider}
              onChange={(event) => setProvider(event.target.value)}
            >
              {providers.map((providerOption) => (
                <option key={providerOption.id} value={providerOption.id}>
                  {providerOption.label}
                </option>
              ))}
            </Select>
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">API Key</label>
            <Input
              type="password"
              placeholder={`Enter ${provider} API key`}
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
            />
            <p className="text-[11px] text-muted-foreground/70">{apiKeyHint}</p>
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">Default Model</label>
            <Input
              placeholder={modelPlaceholder}
              value={model}
              onChange={(event) => setModel(event.target.value)}
            />
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">Base URL</label>
            <Input
              placeholder={baseUrlPlaceholder}
              value={baseUrl}
              onChange={(event) => setBaseUrl(event.target.value)}
            />
          </div>

          <div className="pt-1">
            <Button
              size="sm"
              onClick={() => handleSaveProvider().catch(console.error)}
              disabled={!apiKey.trim() || saving}
            >
              {saving ? "Saving" : "Save Provider"}
            </Button>
          </div>

          {error ? <p className="text-xs text-destructive">{error}</p> : null}
        </div>
      </section>

      <section className="rounded-xl border border-border bg-card/60 p-4">
        <h3 className="mb-3 text-sm font-semibold">Provider Status</h3>
        <div className="space-y-2">
          {providers.map((providerOption) => {
            const item = providerConfigs.find((config) => config.provider === providerOption.id);
            const configured = Boolean(item?.configured);
            return (
              <article key={providerOption.id} className="rounded-lg border border-border bg-background/60 p-3">
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2 text-sm font-medium">
                    <Server size={13} />
                    <span>{providerLabel(providerOption.id)}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    {configured ? (
                      <>
                        <span className="inline-flex items-center gap-1 rounded-full bg-success/15 px-2 py-0.5 text-[11px] font-medium text-success">
                          <Check size={11} />
                          Configured
                        </span>
                        <button
                          type="button"
                          onClick={() => handleRemoveProvider(providerOption.id)}
                          disabled={removingProvider === providerOption.id}
                          className="flex h-6 w-6 cursor-pointer items-center justify-center rounded text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                          title="Remove provider"
                        >
                          <Trash2 size={14} />
                        </button>
                      </>
                    ) : (
                      <span className="rounded-full bg-warning/15 px-2 py-0.5 text-[11px] font-medium text-warning">
                        Not configured
                      </span>
                    )}
                  </div>
                </div>
                <p className="mt-2 text-xs text-muted-foreground">
                  Model: <span className="text-foreground">{item?.default_model ?? "(default)"}</span>
                </p>
                <p className="mt-1 text-xs text-muted-foreground">
                  Base URL: <span className="text-foreground">{item?.base_url ?? "(default)"}</span>
                </p>
              </article>
            );
          })}
        </div>
      </section>
    </div>
  );
}
