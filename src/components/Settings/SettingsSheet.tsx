import { useEffect, useState } from "react";
import { Check, Server, X } from "lucide-react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";

type SettingsSheetProps = {
  open: boolean;
  onClose: () => void;
};

export function SettingsSheet({ open, onClose }: SettingsSheetProps) {
  const [providerConfigs, setProviderConfig] = useAppStore(
    useShallow((state) => [state.providerConfigs, state.setProviderConfig])
  );

  const [provider, setProvider] = useState("minimax");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [baseUrl, setBaseUrl] = useState("");

  useEffect(() => {
    const config = providerConfigs.find((item) => item.provider === provider);
    setModel(config?.default_model ?? "");
    setBaseUrl(config?.base_url ?? "");
  }, [provider, providerConfigs]);

  if (!open) return null;

  const current = providerConfigs.find((item) => item.provider === provider);
  const modelPlaceholder = provider === "minimax" ? "e.g. MiniMax-M2.1 or MiniMax-M1" : "e.g. kimi-k2.5";
  const baseUrlPlaceholder =
    provider === "minimax"
      ? "https://api.minimaxi.chat (global)"
      : "https://api.kimi.com/coding/v1";

  const save = async () => {
    if (!apiKey.trim()) return;
    await setProviderConfig(provider, apiKey.trim(), model.trim(), baseUrl.trim());
    setApiKey("");
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="w-[420px] max-w-[92vw] rounded-2xl border border-border bg-card shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-5 py-4">
          <div className="flex items-center gap-2 text-sm font-semibold">
            <Server size={16} />
            Provider Settings
          </div>
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onClose}>
            <X size={14} />
          </Button>
        </div>

        {/* Body */}
        <div className="space-y-4 px-5 py-5">
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">Provider</label>
            <Select value={provider} onChange={(e) => setProvider(e.target.value)}>
              <option value="minimax">MiniMax</option>
              <option value="kimi">Kimi</option>
            </Select>
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">API key</label>
            <Input
              type="password"
              placeholder={`Enter ${provider} API key`}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
            />
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">Default model</label>
            <Input
              placeholder={modelPlaceholder}
              value={model}
              onChange={(e) => setModel(e.target.value)}
            />
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">Base URL</label>
            <Input
              placeholder={baseUrlPlaceholder}
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
            />
          </div>

          <div className="flex items-center gap-2 text-xs">
            {current?.configured ? (
              <span className="inline-flex items-center gap-1 text-success">
                <Check size={12} /> Configured
              </span>
            ) : (
              <span className="text-warning">Not configured</span>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 border-t border-border px-5 py-4">
          <Button variant="outline" size="sm" onClick={onClose}>
            Cancel
          </Button>
          <Button size="sm" onClick={save} disabled={!apiKey.trim()}>
            Save
          </Button>
        </div>
      </div>
    </div>
  );
}
