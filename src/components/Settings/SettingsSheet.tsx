import { useEffect, useMemo, useState } from "react";
import { Check, Plus, Server, Trash2, X } from "lucide-react";
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
  const [
    providerConfigs,
    mcpServers,
    mcpTools,
    setProviderConfig,
    upsertMcpServer,
    removeMcpServer,
    refreshMcpServers,
    refreshMcpTools,
  ] = useAppStore(
    useShallow((state) => [
      state.providerConfigs,
      state.mcpServers,
      state.mcpTools,
      state.setProviderConfig,
      state.upsertMcpServer,
      state.removeMcpServer,
      state.refreshMcpServers,
      state.refreshMcpTools,
    ])
  );

  const [provider, setProvider] = useState("minimax");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [baseUrl, setBaseUrl] = useState("");

  const [serverName, setServerName] = useState("");
  const [serverCommand, setServerCommand] = useState("");
  const [serverArgs, setServerArgs] = useState("");
  const [serverEnabled, setServerEnabled] = useState(true);

  useEffect(() => {
    const config = providerConfigs.find((item) => item.provider === provider);
    setModel(config?.default_model ?? "");
    setBaseUrl(config?.base_url ?? "");
  }, [provider, providerConfigs]);

  useEffect(() => {
    if (!open) return;
    refreshMcpServers().catch(console.error);
    refreshMcpTools().catch(console.error);
  }, [open, refreshMcpServers, refreshMcpTools]);

  const toolCountByServer = useMemo(() => {
    const map = new Map<string, number>();
    for (const tool of mcpTools) {
      map.set(tool.server_id, (map.get(tool.server_id) ?? 0) + 1);
    }
    return map;
  }, [mcpTools]);

  if (!open) return null;

  const current = providerConfigs.find((item) => item.provider === provider);
  const modelPlaceholder = provider === "minimax" ? "e.g. MiniMax-M2.1 or MiniMax-M1" : "e.g. kimi-k2.5";
  const baseUrlPlaceholder =
    provider === "minimax"
      ? "https://api.minimaxi.chat (global)"
      : "https://api.kimi.com/coding/v1";

  const saveProvider = async () => {
    if (!apiKey.trim()) return;
    await setProviderConfig(provider, apiKey.trim(), model.trim(), baseUrl.trim());
    setApiKey("");
  };

  const addMcpServer = async () => {
    if (!serverName.trim() || !serverCommand.trim()) return;
    await upsertMcpServer({
      name: serverName.trim(),
      command: serverCommand.trim(),
      args: serverArgs
        .split(" ")
        .map((v) => v.trim())
        .filter(Boolean),
      enabled: serverEnabled,
    });
    setServerName("");
    setServerCommand("");
    setServerArgs("");
    setServerEnabled(true);
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="w-[760px] max-h-[88vh] max-w-[96vw] overflow-hidden rounded-2xl border border-border bg-card shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-border px-5 py-4">
          <div className="flex items-center gap-2 text-sm font-semibold">
            <Server size={16} />
            Provider + MCP Settings
          </div>
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onClose}>
            <X size={14} />
          </Button>
        </div>

        <div className="grid max-h-[calc(88vh-64px)] grid-cols-1 gap-0 overflow-hidden lg:grid-cols-2">
          <section className="overflow-y-auto border-r border-border px-5 py-5">
            <h3 className="mb-3 text-sm font-semibold">LLM Provider</h3>

            <div className="space-y-4">
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
                <Input placeholder={modelPlaceholder} value={model} onChange={(e) => setModel(e.target.value)} />
              </div>

              <div className="space-y-1.5">
                <label className="text-xs font-medium text-muted-foreground">Base URL</label>
                <Input placeholder={baseUrlPlaceholder} value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} />
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

              <div className="pt-1">
                <Button size="sm" onClick={saveProvider} disabled={!apiKey.trim()}>
                  Save Provider
                </Button>
              </div>
            </div>
          </section>

          <section className="overflow-y-auto px-5 py-5">
            <div className="mb-3 flex items-center justify-between">
              <h3 className="text-sm font-semibold">MCP Servers</h3>
              <Button size="sm" variant="outline" onClick={() => refreshMcpTools().catch(console.error)}>
                Refresh Tools
              </Button>
            </div>

            <div className="space-y-2">
              {mcpServers.length === 0 ? (
                <p className="text-xs text-muted-foreground">No MCP servers configured.</p>
              ) : (
                mcpServers.map((server) => (
                  <div key={server.id} className="rounded-lg border border-border bg-background/60 p-3">
                    <div className="flex items-start justify-between gap-2">
                      <div>
                        <p className="text-sm font-medium">{server.name}</p>
                        <p className="text-xs text-muted-foreground">
                          <code>{server.command}</code>
                          {server.args.length > 0 ? ` ${server.args.join(" ")}` : ""}
                        </p>
                        <p className="mt-1 text-[11px] text-muted-foreground">
                          {server.enabled ? "Enabled" : "Disabled"} • {toolCountByServer.get(server.id) ?? 0} tools
                        </p>
                      </div>
                      <button
                        type="button"
                        className="rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-destructive"
                        onClick={() => removeMcpServer(server.id).catch(console.error)}
                        title="Remove MCP server"
                      >
                        <Trash2 size={13} />
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>

            <div className="mt-4 rounded-lg border border-border bg-muted/20 p-3">
              <p className="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">Add MCP Server</p>
              <div className="space-y-2">
                <Input placeholder="Name" value={serverName} onChange={(e) => setServerName(e.target.value)} />
                <Input
                  placeholder="Command (e.g. node, bunx, python)"
                  value={serverCommand}
                  onChange={(e) => setServerCommand(e.target.value)}
                />
                <Input
                  placeholder="Args (space separated)"
                  value={serverArgs}
                  onChange={(e) => setServerArgs(e.target.value)}
                />
                <label className="flex items-center gap-2 text-xs text-muted-foreground">
                  <input
                    type="checkbox"
                    className="h-3.5 w-3.5"
                    checked={serverEnabled}
                    onChange={(e) => setServerEnabled(e.target.checked)}
                  />
                  Enabled
                </label>
                <Button
                  size="sm"
                  className="gap-1"
                  onClick={addMcpServer}
                  disabled={!serverName.trim() || !serverCommand.trim()}
                >
                  <Plus size={12} />
                  Add MCP Server
                </Button>
              </div>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
