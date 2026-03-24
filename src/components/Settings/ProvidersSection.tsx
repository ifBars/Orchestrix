import { Check, ExternalLink, Loader2, RefreshCw, Server, Trash2, Wallet } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/shallow";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "@/stores/appStore";
import { providerLabel, providerOptionsFromCatalog } from "@/lib/providers";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useProviderUsage } from "@/hooks/useProviderUsage";
import type { ProviderUsageSnapshotView } from "@/types";

interface ChatGPTAuthStatus {
  authenticated: boolean;
  is_expired: boolean;
  account_id: string | null;
}

interface ChatGPTAuthUrl {
  url: string;
  state: string;
  pkce_verifier: string;
}

function ProviderUsageRow({
  snapshot,
  isLoading,
  onRefresh,
}: {
  snapshot: ProviderUsageSnapshotView | null;
  isLoading: boolean;
  onRefresh: () => void;
}) {
  const formattedTime = useMemo(() => {
    if (!snapshot?.last_updated_at) return null;
    const date = new Date(snapshot.last_updated_at);
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }, [snapshot?.last_updated_at]);

  if (!snapshot) {
    return (
      <div className="mt-2 flex items-center gap-2 text-[11px] text-muted-foreground">
        <Wallet size={11} />
        <span>Loading usage info...</span>
      </div>
    );
  }

  return (
    <div className="mt-2 flex items-center justify-between">
      <div className="flex items-center gap-2 text-[11px]">
        <Wallet size={11} className="shrink-0" />
        {snapshot.available ? (
          <span className="text-success">
            {snapshot.balance
              ? `${snapshot.balance}${snapshot.currency ? ` ${snapshot.currency}` : ""}`
              : snapshot.remaining_quota ?? "Usage available"}
          </span>
        ) : (
          <span className="text-muted-foreground">
            {snapshot.note || "Usage info unavailable"}
          </span>
        )}
      </div>
      <div className="flex items-center gap-2">
        {formattedTime && (
          <span className="text-[10px] text-muted-foreground">
            Updated {formattedTime}
          </span>
        )}
        <button
          type="button"
          onClick={onRefresh}
          disabled={isLoading}
          className="flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent/60 hover:text-foreground disabled:opacity-50"
          title="Refresh usage"
        >
          <RefreshCw size={11} className={isLoading ? "animate-spin" : ""} />
        </button>
      </div>
    </div>
  );
}

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
  const { usage, isLoading: usageLoading, refresh: refreshUsage } = useProviderUsage();

  // ChatGPT OAuth state
  const [chatgptAuth, setChatgptAuth] = useState<ChatGPTAuthStatus | null>(null);
  const [isLoadingChatgptAuth, setIsLoadingChatgptAuth] = useState(false);
  // Waiting for the backend callback server to receive the redirect
  const [isAwaitingCallback, setIsAwaitingCallback] = useState(false);
  // Manual fallback: if the user wants to paste a code instead
  const [pkceVerifier, setPkceVerifier] = useState<string | null>(null);
  const [showCodeInput, setShowCodeInput] = useState(false);
  const [oauthCode, setOauthCode] = useState("");
  const unlistenRef = useRef<(() => void) | null>(null);

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

  // Load ChatGPT auth status when that provider is selected
  useEffect(() => {
    if (provider === "openai-chatgpt") {
      loadChatGPTAuthStatus();
    }
  }, [provider]);

  // Cleanup the oauth event listener on unmount
  useEffect(() => {
    return () => {
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
    };
  }, []);

  const loadChatGPTAuthStatus = async () => {
    try {
      const status = await invoke<ChatGPTAuthStatus>("get_chatgpt_auth_status");
      setChatgptAuth(status);
    } catch (e) {
      console.error("Failed to load ChatGPT auth status:", e);
    }
  };

  const completeOAuthWithCode = async (code: string, verifier: string) => {
    setIsLoadingChatgptAuth(true);
    try {
      await invoke("complete_chatgpt_oauth", { code, pkceVerifier: verifier });
      setPkceVerifier(null);
      setShowCodeInput(false);
      setOauthCode("");
      await loadChatGPTAuthStatus();
    } catch (e) {
      console.error("Failed to complete OAuth:", e);
      setError("Failed to complete ChatGPT authentication.");
    } finally {
      setIsLoadingChatgptAuth(false);
    }
  };

  const handleStartChatGPTOAuth = async () => {
    setIsLoadingChatgptAuth(true);
    setError(null);
    // Clean up any previous listener
    if (unlistenRef.current) {
      unlistenRef.current();
      unlistenRef.current = null;
    }

    try {
      // Subscribe to the backend completion event BEFORE opening the browser,
      // so we don't miss it if the browser is very fast.
      interface OAuthCompletePayload {
        success: boolean;
        error?: string;
        account_id?: string;
      }
      const unlisten = await listen<OAuthCompletePayload>("chatgpt://oauth-complete", (event) => {
        // Clean up listener immediately — it fires exactly once.
        if (unlistenRef.current) {
          unlistenRef.current();
          unlistenRef.current = null;
        }
        setIsAwaitingCallback(false);
        if (event.payload.success) {
          loadChatGPTAuthStatus();
        } else {
          setError(event.payload.error ?? "ChatGPT authentication failed.");
        }
      });
      unlistenRef.current = unlisten;

      // Ask the backend to start listening on port 1455 AND return the auth URL.
      const authUrl = await invoke<ChatGPTAuthUrl>("start_chatgpt_oauth_and_listen");
      // Keep pkce_verifier in case the user falls back to manual paste.
      setPkceVerifier(authUrl.pkce_verifier);

      // Open auth URL in the system browser.
      await openUrl(authUrl.url);

      // Show the waiting indicator. The backend will fire the event when done.
      setIsAwaitingCallback(true);
    } catch (e) {
      console.error("Failed to start OAuth:", e);
      setError("Failed to start ChatGPT OAuth flow.");
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
    } finally {
      setIsLoadingChatgptAuth(false);
    }
  };

  const handleManualCodeSubmit = () => {
    if (!oauthCode.trim() || !pkceVerifier) return;
    completeOAuthWithCode(oauthCode.trim(), pkceVerifier);
  };

  const handleRemoveChatGPTAuth = async () => {
    setIsLoadingChatgptAuth(true);
    try {
      await invoke("remove_chatgpt_auth");
      setChatgptAuth(null);
    } catch (e) {
      console.error("Failed to remove ChatGPT auth:", e);
      setError("Failed to remove ChatGPT authentication.");
    } finally {
      setIsLoadingChatgptAuth(false);
    }
  };

  const modelPlaceholder =
    provider === "minimax"
      ? "e.g. MiniMax-M2.1"
      : provider === "zhipu"
        ? "e.g. glm-4.7"
        : provider === "modal"
          ? "e.g. zai-org/GLM-5-FP8"
          : provider === "openai-chatgpt"
            ? "e.g. gpt-5.3-codex"
            : provider === "gemini"
              ? "e.g. gemini-3-flash-preview"
              : "e.g. kimi-k2.5";

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
          : provider === "gemini"
            ? "Get your key from aistudio.google.com/apikey."
            : provider === "openai-chatgpt"
              ? "Connect with OAuth (recommended) or enter an OpenAI API key directly."
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
      // If removing ChatGPT, also clear its OAuth tokens
      if (providerId === "openai-chatgpt" && chatgptAuth?.authenticated) {
        await invoke("remove_chatgpt_auth").catch(() => {});
        setChatgptAuth(null);
      }
    } catch (removeError) {
      console.error(removeError);
      setError(`Failed to remove ${providerId} configuration.`);
    } finally {
      setRemovingProvider(null);
    }
  };

  const isChatGPTProvider = provider === "openai-chatgpt";

  return (
    <div className="grid min-h-0 gap-4 lg:grid-cols-[1.2fr_1fr]">
      <section className="rounded-xl border border-border bg-card/60 p-4">
        <h3 className="mb-1 text-sm font-semibold">Provider Configuration</h3>
        <p className="mb-4 text-xs text-muted-foreground">
          Configure API keys and defaults used by planning and execution agents.
        </p>

        <div className="space-y-3">
          {/* Provider selector */}
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">Provider</label>
            <Select value={provider} onChange={(e) => setProvider(e.target.value)}>
              {providers.map((opt) => (
                <option key={opt.id} value={opt.id}>
                  {opt.label}
                </option>
              ))}
            </Select>
          </div>

          {/* ChatGPT-specific OAuth panel */}
          {isChatGPTProvider && (
            <div className="space-y-3 rounded-lg border border-border bg-background/50 p-3">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium text-muted-foreground">
                  ChatGPT Authentication
                </span>
                {chatgptAuth?.authenticated ? (
                  <span className="inline-flex items-center gap-1 rounded-full bg-success/15 px-2 py-0.5 text-[11px] font-medium text-success">
                    <Check size={11} />
                    Connected
                  </span>
                ) : (
                  <span className="rounded-full bg-warning/15 px-2 py-0.5 text-[11px] font-medium text-warning">
                    Not connected
                  </span>
                )}
              </div>

              {chatgptAuth?.authenticated ? (
                <div className="space-y-2">
                  <p className="text-xs text-muted-foreground">
                    {chatgptAuth.account_id
                      ? `Account: ${chatgptAuth.account_id}`
                      : "Connected via ChatGPT Plus/Pro subscription"}
                    {chatgptAuth.is_expired && (
                      <span className="ml-1 text-warning">(Token expired — will auto-refresh)</span>
                    )}
                  </p>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => handleRemoveChatGPTAuth().catch(console.error)}
                    disabled={isLoadingChatgptAuth}
                  >
                    Disconnect
                  </Button>
                </div>
              ) : (
                <div className="space-y-2">
                  <p className="text-xs text-muted-foreground">
                    Connect your ChatGPT Plus or Pro subscription to use GPT‑5 Codex models.
                  </p>

                  {isAwaitingCallback ? (
                    <div className="flex items-center gap-2 rounded-md border border-border/60 bg-background/50 px-3 py-2 text-xs text-muted-foreground">
                      <Loader2 size={12} className="animate-spin shrink-0" />
                      <span>Waiting for authorization in browser…</span>
                    </div>
                  ) : (
                    <Button
                      size="sm"
                      onClick={() => handleStartChatGPTOAuth().catch(console.error)}
                      disabled={isLoadingChatgptAuth}
                      className="gap-1"
                    >
                      <ExternalLink size={12} />
                      {isLoadingChatgptAuth ? "Opening browser…" : "Connect with ChatGPT"}
                    </Button>
                  )}

                  {/* Manual fallback — only shown if user explicitly wants it */}
                  {showCodeInput && (
                    <div className="mt-2 space-y-2">
                      <p className="text-[10px] text-muted-foreground">
                        If automatic capture failed, paste the <code>code=</code> value from the
                        browser's address bar here:
                      </p>
                      <div className="flex gap-2">
                        <Input
                          placeholder="Paste authorization code"
                          value={oauthCode}
                          onChange={(e) => setOauthCode(e.target.value)}
                          onKeyDown={(e) => e.key === "Enter" && handleManualCodeSubmit()}
                          className="h-8 flex-1 text-xs"
                          autoFocus
                        />
                        <Button
                          size="sm"
                          onClick={handleManualCodeSubmit}
                          disabled={!oauthCode.trim() || !pkceVerifier || isLoadingChatgptAuth}
                        >
                          Submit
                        </Button>
                      </div>
                    </div>
                  )}

                  {/* Offer manual fallback link when waiting */}
                  {isAwaitingCallback && !showCodeInput && (
                    <button
                      type="button"
                      className="text-[10px] text-muted-foreground/60 underline underline-offset-2 hover:text-muted-foreground"
                      onClick={() => setShowCodeInput(true)}
                    >
                      Enter code manually instead
                    </button>
                  )}
                </div>
              )}
            </div>
          )}

          {/* API key — always visible; for ChatGPT this is the manual/direct key path */}
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">
              {isChatGPTProvider ? "API Key (optional — overrides OAuth)" : "API Key"}
            </label>
            <Input
              type="password"
              placeholder={`Enter ${provider} API key`}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
            />
            <p className="text-[11px] text-muted-foreground/70">{apiKeyHint}</p>
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">Default Model</label>
            <Input
              placeholder={modelPlaceholder}
              value={model}
              onChange={(e) => setModel(e.target.value)}
            />
          </div>

          {/* Hide base URL for ChatGPT — endpoint is fixed */}
          {!isChatGPTProvider && (
            <div className="space-y-1.5">
              <label className="text-xs font-medium text-muted-foreground">Base URL</label>
              <Input
                placeholder={baseUrlPlaceholder}
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
              />
            </div>
          )}

          <div className="pt-1">
            <Button
              size="sm"
              onClick={() => handleSaveProvider().catch(console.error)}
              disabled={!apiKey.trim() || saving}
            >
              {saving ? "Saving…" : "Save Provider"}
            </Button>
          </div>

          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>
      </section>

      {/* Provider status panel */}
      <section className="rounded-xl border border-border bg-card/60 p-4">
        <h3 className="mb-3 text-sm font-semibold">Provider Status</h3>
        <div className="space-y-2">
          {providers.map((opt) => {
            const item = providerConfigs.find((c) => c.provider === opt.id);
            const isThisChatGPT = opt.id === "openai-chatgpt";
            // ChatGPT is configured when OAuth tokens exist OR an API key is saved
            const configured =
              Boolean(item?.configured) || (isThisChatGPT && Boolean(chatgptAuth?.authenticated));
            return (
              <article
                key={opt.id}
                className="rounded-lg border border-border bg-background/60 p-3"
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2 text-sm font-medium">
                    <Server size={13} />
                    <span>{providerLabel(opt.id)}</span>
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
                          onClick={() => handleRemoveProvider(opt.id)}
                          disabled={removingProvider === opt.id}
                          className="flex h-6 w-6 cursor-pointer items-center justify-center rounded text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:opacity-50"
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
                  Model:{" "}
                  <span className="text-foreground">{item?.default_model ?? "(default)"}</span>
                </p>
                {!isThisChatGPT && (
                  <p className="mt-1 text-xs text-muted-foreground">
                    Base URL:{" "}
                    <span className="text-foreground">{item?.base_url ?? "(default)"}</span>
                  </p>
                )}
                {isThisChatGPT && chatgptAuth?.authenticated && (
                  <p className="mt-1 text-xs text-muted-foreground">
                    Auth:{" "}
                    <span className="text-success">
                      OAuth{chatgptAuth.is_expired ? " (expired)" : ""}
                    </span>
                  </p>
                )}
                {configured && (
                  <ProviderUsageRow
                    snapshot={usage?.find((u) => u.provider === opt.id) ?? null}
                    isLoading={usageLoading}
                    onRefresh={refreshUsage}
                  />
                )}
              </article>
            );
          })}
        </div>
      </section>
    </div>
  );
}
