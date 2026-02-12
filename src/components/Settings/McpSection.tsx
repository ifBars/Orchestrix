import { invoke } from "@tauri-apps/api/core";
import { CircleCheck, CircleX, Plus, RefreshCw, TestTube2, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import type {
  McpConnectionTestResult,
  McpServerHealthView,
  McpServerInput,
  McpTransportType,
  ToolOverride,
} from "@/types";

const TRANSPORT_OPTIONS: McpTransportType[] = ["stdio", "http", "sse"];

export function McpSection() {
  const [mcpServers, mcpTools, upsertMcpServer, removeMcpServer, refreshMcpServers, refreshMcpTools] = useAppStore(
    useShallow((state) => [
      state.mcpServers,
      state.mcpTools,
      state.upsertMcpServer,
      state.removeMcpServer,
      state.refreshMcpServers,
      state.refreshMcpTools,
    ])
  );

  const [serverName, setServerName] = useState("");
  const [transport, setTransport] = useState<McpTransportType>("stdio");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [workingDir, setWorkingDir] = useState("");
  const [envRaw, setEnvRaw] = useState("");
  const [url, setUrl] = useState("");
  const [oauthToken, setOauthToken] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [apiKeyHeader, setApiKeyHeader] = useState("X-API-Key");
  const [headersRaw, setHeadersRaw] = useState("");
  const [timeoutSecs, setTimeoutSecs] = useState("30");
  const [poolSize, setPoolSize] = useState("5");
  const [enabled, setEnabled] = useState(true);

  const [filterMode, setFilterMode] = useState<"include" | "exclude">("include");
  const [toolList, setToolList] = useState("");
  const [allowAllReadOnly, setAllowAllReadOnly] = useState(false);
  const [blockAllModifying, setBlockAllModifying] = useState(false);

  const [globalPolicy, setGlobalPolicy] = useState<"always" | "never" | "by_tool">("by_tool");
  const [readOnlyNeverApproval, setReadOnlyNeverApproval] = useState(true);
  const [modifyingAlwaysApproval, setModifyingAlwaysApproval] = useState(false);
  const [toolOverridesRaw, setToolOverridesRaw] = useState("");

  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [testingServerId, setTestingServerId] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, McpConnectionTestResult>>({});

  useEffect(() => {
    refreshMcpServers().catch(console.error);
    refreshMcpTools().catch(console.error);
  }, [refreshMcpServers, refreshMcpTools]);

  const toolCountByServer = useMemo(() => {
    const map = new Map<string, number>();
    for (const tool of mcpTools) {
      map.set(tool.server_id, (map.get(tool.server_id) ?? 0) + 1);
    }
    return map;
  }, [mcpTools]);

  const requiresCommand = transport === "stdio";
  const requiresUrl = transport === "http" || transport === "sse";
  const canSubmit =
    !!serverName.trim() &&
    ((requiresCommand && !!command.trim()) || (requiresUrl && !!url.trim())) &&
    !saving;

  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      await Promise.all([refreshMcpServers(), refreshMcpTools()]);
    } catch (refreshError) {
      console.error(refreshError);
    } finally {
      setRefreshing(false);
    }
  };

  const handleAddServer = async () => {
    if (!canSubmit) return;

    setError(null);
    setSaving(true);

    try {
      const parsedOverrides = parseOverrides(toolOverridesRaw);
      if (!parsedOverrides.ok) {
        setError(parsedOverrides.error);
        return;
      }

      const input: McpServerInput = {
        name: serverName.trim(),
        transport,
        enabled,
        command: requiresCommand ? command.trim() : undefined,
        args: requiresCommand ? parseCommandArgs(args) : [],
        env: requiresCommand ? parseKeyValueLines(envRaw, "=") : {},
        working_dir: requiresCommand && workingDir.trim() ? workingDir.trim() : undefined,
        url: requiresUrl ? url.trim() : undefined,
        auth: {
          oauth_token: oauthToken.trim() || undefined,
          api_key: apiKey.trim() || undefined,
          api_key_header: apiKeyHeader.trim() || undefined,
          headers: parseKeyValueLines(headersRaw, ":"),
        },
        timeout_secs: Math.max(1, Number.parseInt(timeoutSecs, 10) || 30),
        pool_size: Math.max(1, Number.parseInt(poolSize, 10) || 5),
        tool_filter: {
          mode: filterMode,
          tools: parseList(toolList),
          allow_all_read_only: allowAllReadOnly,
          block_all_modifying: blockAllModifying,
        },
        approval_policy: {
          global_policy: globalPolicy,
          tool_overrides: parsedOverrides.value,
          read_only_never_requires_approval: readOnlyNeverApproval,
          modifying_always_requires_approval: modifyingAlwaysApproval,
        },
      };

      await upsertMcpServer(input);
      resetForm();
    } catch (saveError) {
      console.error(saveError);
      setError("Failed to save MCP server configuration.");
    } finally {
      setSaving(false);
    }
  };

  const handleRemoveServer = async (serverId: string) => {
    setError(null);
    try {
      await removeMcpServer(serverId);
      setTestResults((prev) => {
        const next = { ...prev };
        delete next[serverId];
        return next;
      });
    } catch (removeError) {
      console.error(removeError);
      setError("Failed to remove MCP server.");
    }
  };

  const handleTestConnection = async (serverId: string) => {
    setTestingServerId(serverId);
    try {
      const result = await invoke<McpConnectionTestResult>("test_mcp_server_connection", { serverId });
      setTestResults((prev) => ({ ...prev, [serverId]: result }));
      await refreshMcpTools();
    } catch (testError) {
      console.error(testError);
      setTestResults((prev) => ({
        ...prev,
        [serverId]: {
          success: false,
          error: "Connection test failed.",
        },
      }));
    } finally {
      setTestingServerId(null);
    }
  };

  const resetForm = () => {
    setServerName("");
    setTransport("stdio");
    setCommand("");
    setArgs("");
    setWorkingDir("");
    setEnvRaw("");
    setUrl("");
    setOauthToken("");
    setApiKey("");
    setApiKeyHeader("X-API-Key");
    setHeadersRaw("");
    setTimeoutSecs("30");
    setPoolSize("5");
    setEnabled(true);
    setFilterMode("include");
    setToolList("");
    setAllowAllReadOnly(false);
    setBlockAllModifying(false);
    setGlobalPolicy("by_tool");
    setReadOnlyNeverApproval(true);
    setModifyingAlwaysApproval(false);
    setToolOverridesRaw("");
    setError(null);
  };

  return (
    <div className="grid min-h-0 gap-4 lg:grid-cols-[1.2fr_1fr]">
      <section className="rounded-xl border border-border bg-card/60 p-4">
        <div className="mb-3 flex items-center justify-between gap-2">
          <div>
            <h3 className="text-sm font-semibold">Configured MCP Servers</h3>
            <p className="text-xs text-muted-foreground">Local and remote MCP servers available to agents.</p>
          </div>
          <Button
            size="sm"
            variant="outline"
            className="gap-1"
            onClick={() => handleRefresh().catch(console.error)}
            disabled={refreshing}
          >
            <RefreshCw size={12} className={refreshing ? "animate-spin" : ""} />
            Refresh
          </Button>
        </div>

        {mcpServers.length === 0 ? (
          <div className="rounded-lg border border-dashed border-border px-3 py-8 text-center text-xs text-muted-foreground">
            No MCP servers configured.
          </div>
        ) : (
          <div className="space-y-2">
            {mcpServers.map((server) => {
              const test = testResults[server.id];
              const summary = test?.success
                ? `OK${typeof test.tool_count === "number" ? ` - ${test.tool_count} tools` : ""}${
                    typeof test.latency_ms === "number" ? ` - ${test.latency_ms} ms` : ""
                  }`
                : test?.error;

              return (
                <article key={server.id} className="rounded-lg border border-border bg-background/60 p-3">
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <p className="truncate text-sm font-medium">{server.name}</p>
                        <span className="rounded border border-border px-1.5 py-0.5 text-[10px] text-muted-foreground">
                          {server.transport}
                        </span>
                        <HealthPill health={server.health} />
                      </div>

                      <p className="mt-1 truncate text-xs text-muted-foreground">
                        {server.transport === "stdio" ? (
                          <>
                            <code>{server.command ?? "(missing command)"}</code>
                            {server.args.length > 0 ? ` ${server.args.join(" ")}` : ""}
                          </>
                        ) : (
                          <code>{server.url ?? "(missing url)"}</code>
                        )}
                      </p>

                      <p className="mt-1 text-[11px] text-muted-foreground">
                        {server.enabled ? "Enabled" : "Disabled"} - {toolCountByServer.get(server.id) ?? 0} tools - timeout {server.timeout_secs}s
                      </p>

                      {summary ? <p className="mt-1 text-[11px] text-muted-foreground">Last test: {summary}</p> : null}
                    </div>

                    <div className="flex items-center gap-1">
                      <button
                        type="button"
                        className="rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-info"
                        onClick={() => handleTestConnection(server.id).catch(console.error)}
                        title="Test MCP server connection"
                        disabled={testingServerId === server.id}
                      >
                        <TestTube2 size={13} className={testingServerId === server.id ? "animate-pulse" : ""} />
                      </button>

                      <button
                        type="button"
                        className="rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-destructive"
                        onClick={() => handleRemoveServer(server.id).catch(console.error)}
                        title="Remove MCP server"
                      >
                        <Trash2 size={13} />
                      </button>
                    </div>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </section>

      <section className="rounded-xl border border-border bg-card/60 p-4">
        <h3 className="mb-1 text-sm font-semibold">Add MCP Server</h3>
        <p className="mb-3 text-xs text-muted-foreground">
          Register local stdio or remote HTTP/SSE MCP servers for planning and execution tools.
        </p>

        <div className="space-y-2">
          <Input placeholder="Name" value={serverName} onChange={(event) => setServerName(event.target.value)} />

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">Transport</label>
            <Select value={transport} onChange={(event) => setTransport(event.target.value as McpTransportType)}>
              {TRANSPORT_OPTIONS.map((option) => (
                <option key={option} value={option}>
                  {option.toUpperCase()}
                </option>
              ))}
            </Select>
          </div>

          {requiresCommand ? (
            <>
              <Input
                placeholder="Command (bunx, node, python, etc.)"
                value={command}
                onChange={(event) => setCommand(event.target.value)}
              />
              <Input placeholder="Args (space separated)" value={args} onChange={(event) => setArgs(event.target.value)} />
              <Input
                placeholder="Working directory (optional)"
                value={workingDir}
                onChange={(event) => setWorkingDir(event.target.value)}
              />
              <Textarea
                className="min-h-20"
                placeholder="Env vars (KEY=VALUE, one per line)"
                value={envRaw}
                onChange={(event) => setEnvRaw(event.target.value)}
              />
            </>
          ) : (
            <>
              <Input placeholder="Server URL (https://...)" value={url} onChange={(event) => setUrl(event.target.value)} />
              <Input
                type="password"
                placeholder="OAuth token (optional)"
                value={oauthToken}
                onChange={(event) => setOauthToken(event.target.value)}
              />
              <Input
                type="password"
                placeholder="API key (optional)"
                value={apiKey}
                onChange={(event) => setApiKey(event.target.value)}
              />
              <Input
                placeholder="API key header"
                value={apiKeyHeader}
                onChange={(event) => setApiKeyHeader(event.target.value)}
              />
              <Textarea
                className="min-h-20"
                placeholder="HTTP headers (Header: Value, one per line)"
                value={headersRaw}
                onChange={(event) => setHeadersRaw(event.target.value)}
              />
            </>
          )}

          <div className="grid grid-cols-2 gap-2">
            <Input
              placeholder="Timeout (secs)"
              value={timeoutSecs}
              onChange={(event) => setTimeoutSecs(event.target.value)}
            />
            <Input placeholder="Pool size" value={poolSize} onChange={(event) => setPoolSize(event.target.value)} />
          </div>

          <label className="flex items-center gap-2 text-xs text-muted-foreground">
            <input
              type="checkbox"
              className="h-3.5 w-3.5 rounded border-input text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
              checked={enabled}
              onChange={(event) => setEnabled(event.target.checked)}
            />
            Enabled
          </label>

          <details className="rounded-md border border-border bg-background/50 px-3 py-2">
            <summary className="cursor-pointer text-xs font-medium text-muted-foreground">Tool Filtering</summary>
            <div className="mt-2 space-y-2">
              <Select value={filterMode} onChange={(event) => setFilterMode(event.target.value as "include" | "exclude") }>
                <option value="include">Allow list</option>
                <option value="exclude">Block list</option>
              </Select>
              <Textarea
                className="min-h-16"
                placeholder="Tool names (comma-separated or one per line)"
                value={toolList}
                onChange={(event) => setToolList(event.target.value)}
              />
              <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <input
                  type="checkbox"
                  className="h-3.5 w-3.5 rounded border-input text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
                  checked={allowAllReadOnly}
                  onChange={(event) => setAllowAllReadOnly(event.target.checked)}
                />
                Allow all read-only tools
              </label>
              <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <input
                  type="checkbox"
                  className="h-3.5 w-3.5 rounded border-input text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
                  checked={blockAllModifying}
                  onChange={(event) => setBlockAllModifying(event.target.checked)}
                />
                Block all modifying tools
              </label>
            </div>
          </details>

          <details className="rounded-md border border-border bg-background/50 px-3 py-2">
            <summary className="cursor-pointer text-xs font-medium text-muted-foreground">Approval Policy</summary>
            <div className="mt-2 space-y-2">
              <Select
                value={globalPolicy}
                onChange={(event) => setGlobalPolicy(event.target.value as "always" | "never" | "by_tool")}
              >
                <option value="by_tool">By tool</option>
                <option value="always">Always require approval</option>
                <option value="never">Never require approval</option>
              </Select>

              <Textarea
                className="min-h-16"
                placeholder="Overrides (one per line): pattern,true|false"
                value={toolOverridesRaw}
                onChange={(event) => setToolOverridesRaw(event.target.value)}
              />

              <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <input
                  type="checkbox"
                  className="h-3.5 w-3.5 rounded border-input text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
                  checked={readOnlyNeverApproval}
                  onChange={(event) => setReadOnlyNeverApproval(event.target.checked)}
                />
                Read-only tools never need approval
              </label>

              <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <input
                  type="checkbox"
                  className="h-3.5 w-3.5 rounded border-input text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
                  checked={modifyingAlwaysApproval}
                  onChange={(event) => setModifyingAlwaysApproval(event.target.checked)}
                />
                Modifying tools always need approval
              </label>
            </div>
          </details>

          <Button
            size="sm"
            className="gap-1"
            onClick={() => handleAddServer().catch(console.error)}
            disabled={!canSubmit}
          >
            <Plus size={12} />
            {saving ? "Saving" : "Add MCP Server"}
          </Button>

          {error ? <p className="text-xs text-destructive">{error}</p> : null}
        </div>
      </section>
    </div>
  );
}

function HealthPill({ health }: { health?: McpServerHealthView }) {
  if (!health) {
    return <span className="rounded-full bg-muted/70 px-2 py-0.5 text-[10px] font-medium text-muted-foreground">unknown</span>;
  }

  if (health.status === "healthy") {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-success/15 px-2 py-0.5 text-[10px] font-medium text-success">
        <CircleCheck size={10} />
        healthy
      </span>
    );
  }

  if (health.status === "unhealthy") {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-destructive/15 px-2 py-0.5 text-[10px] font-medium text-destructive">
        <CircleX size={10} />
        unhealthy
      </span>
    );
  }

  if (health.status === "connecting") {
    return <span className="rounded-full bg-info/15 px-2 py-0.5 text-[10px] font-medium text-info">connecting</span>;
  }

  return <span className="rounded-full bg-warning/15 px-2 py-0.5 text-[10px] font-medium text-warning">disabled</span>;
}

function parseList(raw: string): string[] {
  return raw
    .split(/[,\n\r]+/)
    .map((value) => value.trim())
    .filter(Boolean);
}

function parseCommandArgs(raw: string): string[] {
  return raw
    .split(/\s+/)
    .map((value) => value.trim())
    .filter(Boolean);
}

function parseKeyValueLines(raw: string, separator: ":" | "="): Record<string, string> {
  const out: Record<string, string> = {};

  for (const line of raw.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;

    const idx = trimmed.indexOf(separator);
    if (idx <= 0) continue;

    const key = trimmed.slice(0, idx).trim();
    const value = trimmed.slice(idx + 1).trim();
    if (!key) continue;
    out[key] = value;
  }

  return out;
}

function parseOverrides(raw: string): { ok: true; value: ToolOverride[] } | { ok: false; error: string } {
  if (!raw.trim()) {
    return { ok: true, value: [] };
  }

  const overrides: ToolOverride[] = [];
  const lines = raw.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);

  for (const line of lines) {
    const [patternRaw, requiresRaw] = line.split(",").map((part) => part.trim());

    if (!patternRaw || !requiresRaw) {
      return { ok: false, error: "Invalid override format. Use: pattern,true|false" };
    }

    const lowered = requiresRaw.toLowerCase();
    if (lowered !== "true" && lowered !== "false") {
      return { ok: false, error: `Invalid override value for '${patternRaw}'. Use true or false.` };
    }

    overrides.push({
      pattern: patternRaw,
      requires_approval: lowered === "true",
      is_glob: patternRaw.includes("*") || patternRaw.includes("?"),
    });
  }

  return { ok: true, value: overrides };
}
