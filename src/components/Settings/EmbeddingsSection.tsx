import { invoke } from "@tauri-apps/api/core";
import { Check, Cpu, KeyRound, Server, Sparkles } from "lucide-react";
import { type ReactNode, useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/shallow";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useAppStore } from "@/stores/appStore";
import type { EmbeddingConfig, RecommendedEmbeddingConfig } from "@/types";

const PROVIDER_LABELS: Record<EmbeddingConfig["provider"], string> = {
  gemini: "Google Gemini",
  ollama: "Ollama",
  transformersjs: "Transformers.js",
  "rust-hf": "Rust HF",
};

const PROVIDERS: EmbeddingConfig["provider"][] = [
  "gemini",
  "ollama",
  "transformersjs",
  "rust-hf",
];

type FormState = {
  enabled: boolean;
  provider: EmbeddingConfig["provider"];
  normalize_l2: boolean;
  gemini: {
    api_key: string;
    model: string;
    timeout_ms: string;
    base_url: string;
  };
  ollama: {
    base_url: string;
    model: string;
    timeout_ms: string;
  };
  transformersjs: {
    model: string;
    device: string;
    backend: string;
    cache_dir: string;
    timeout_ms: string;
    bridge_command: string;
    bridge_script: string;
  };
  rust_hf: {
    model_id: string;
    model_path: string;
    cache_dir: string;
    runtime: "onnx" | "candle";
    threads: string;
    timeout_ms: string;
  };
};

export function EmbeddingsSection() {
  const [embeddingConfig, refreshEmbeddingConfig, setEmbeddingConfig] = useAppStore(
    useShallow((state) => [state.embeddingConfig, state.refreshEmbeddingConfig, state.setEmbeddingConfig]),
  );

  const [form, setForm] = useState<FormState | null>(null);
  const [saving, setSaving] = useState(false);
  const [autoConfiguring, setAutoConfiguring] = useState(false);
  const [autoConfigPreference, setAutoConfigPreference] = useState<"local" | "quality">("local");
  const [autoConfigNotes, setAutoConfigNotes] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!embeddingConfig) {
      refreshEmbeddingConfig().catch(console.error);
    }
  }, [embeddingConfig, refreshEmbeddingConfig]);

  useEffect(() => {
    if (!embeddingConfig) return;
    setForm({
      enabled: embeddingConfig.enabled,
      provider: embeddingConfig.provider,
      normalize_l2: embeddingConfig.normalize_l2,
      gemini: {
        api_key: "",
        model: embeddingConfig.gemini.model,
        timeout_ms: String(embeddingConfig.gemini.timeout_ms),
        base_url: embeddingConfig.gemini.base_url ?? "",
      },
      ollama: {
        base_url: embeddingConfig.ollama.base_url,
        model: embeddingConfig.ollama.model,
        timeout_ms: String(embeddingConfig.ollama.timeout_ms),
      },
      transformersjs: {
        model: embeddingConfig.transformersjs.model,
        device: embeddingConfig.transformersjs.device,
        backend: embeddingConfig.transformersjs.backend ?? "",
        cache_dir: embeddingConfig.transformersjs.cache_dir ?? "",
        timeout_ms: String(embeddingConfig.transformersjs.timeout_ms),
        bridge_command: embeddingConfig.transformersjs.bridge_command,
        bridge_script: embeddingConfig.transformersjs.bridge_script ?? "",
      },
      rust_hf: {
        model_id: embeddingConfig.rust_hf.model_id,
        model_path: embeddingConfig.rust_hf.model_path ?? "",
        cache_dir: embeddingConfig.rust_hf.cache_dir ?? "",
        runtime: embeddingConfig.rust_hf.runtime,
        threads: embeddingConfig.rust_hf.threads ? String(embeddingConfig.rust_hf.threads) : "",
        timeout_ms: String(embeddingConfig.rust_hf.timeout_ms),
      },
    });
  }, [embeddingConfig]);

  const providerStatus = useMemo(() => {
    if (!embeddingConfig) return null;
    return {
      geminiConfigured: embeddingConfig.gemini.api_key_configured,
      provider: embeddingConfig.provider,
    };
  }, [embeddingConfig]);

  const handleSave = async () => {
    if (!form) return;

    setSaving(true);
    setError(null);
    try {
      const payload: EmbeddingConfig = {
        enabled: form.enabled,
        provider: form.provider,
        normalize_l2: form.normalize_l2,
        gemini: {
          api_key: form.gemini.api_key.trim() || null,
          model: form.gemini.model.trim(),
          timeout_ms: toPositiveNumber(form.gemini.timeout_ms),
          base_url: form.gemini.base_url.trim() || null,
        },
        ollama: {
          base_url: form.ollama.base_url.trim(),
          model: form.ollama.model.trim(),
          timeout_ms: toPositiveNumber(form.ollama.timeout_ms),
        },
        transformersjs: {
          model: form.transformersjs.model.trim(),
          device: form.transformersjs.device.trim(),
          backend: form.transformersjs.backend.trim() || null,
          cache_dir: form.transformersjs.cache_dir.trim() || null,
          timeout_ms: toPositiveNumber(form.transformersjs.timeout_ms),
          bridge_command: form.transformersjs.bridge_command.trim(),
          bridge_script: form.transformersjs.bridge_script.trim() || null,
        },
        rust_hf: {
          model_id: form.rust_hf.model_id.trim(),
          model_path: form.rust_hf.model_path.trim() || null,
          cache_dir: form.rust_hf.cache_dir.trim() || null,
          runtime: form.rust_hf.runtime,
          threads: form.rust_hf.threads.trim() ? toPositiveNumber(form.rust_hf.threads) : null,
          timeout_ms: toPositiveNumber(form.rust_hf.timeout_ms),
        },
      };

      await setEmbeddingConfig(payload);
      setForm((current) =>
        current
          ? {
              ...current,
              gemini: {
                ...current.gemini,
                api_key: "",
              },
            }
          : current,
      );
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : "Failed to save embedding config");
    } finally {
      setSaving(false);
    }
  };

  const handleAutoConfigure = async () => {
    if (!form) return;

    setAutoConfiguring(true);
    setError(null);
    try {
      const recommended = await invoke<RecommendedEmbeddingConfig>("get_recommended_embedding_config", {
        preference: autoConfigPreference,
      });

      const next = recommended.config;
      setForm((current) =>
        current
          ? {
              ...current,
              enabled: next.enabled,
              provider: next.provider,
              normalize_l2: next.normalize_l2,
              gemini: {
                ...current.gemini,
                model: next.gemini.model,
                timeout_ms: String(next.gemini.timeout_ms),
                base_url: next.gemini.base_url ?? "",
              },
              ollama: {
                base_url: next.ollama.base_url,
                model: next.ollama.model,
                timeout_ms: String(next.ollama.timeout_ms),
              },
              transformersjs: {
                model: next.transformersjs.model,
                device: next.transformersjs.device,
                backend: next.transformersjs.backend ?? "",
                cache_dir: next.transformersjs.cache_dir ?? "",
                timeout_ms: String(next.transformersjs.timeout_ms),
                bridge_command: next.transformersjs.bridge_command,
                bridge_script: next.transformersjs.bridge_script ?? "",
              },
              rust_hf: {
                model_id: next.rust_hf.model_id,
                model_path: next.rust_hf.model_path ?? "",
                cache_dir: next.rust_hf.cache_dir ?? "",
                runtime: next.rust_hf.runtime,
                threads: next.rust_hf.threads ? String(next.rust_hf.threads) : "",
                timeout_ms: String(next.rust_hf.timeout_ms),
              },
            }
          : current,
      );
      setAutoConfigNotes(recommended.notes);
    } catch (autoConfigError) {
      setError(autoConfigError instanceof Error ? autoConfigError.message : "Failed to auto-configure embeddings");
    } finally {
      setAutoConfiguring(false);
    }
  };

  if (!form || !embeddingConfig) {
    return <p className="text-sm text-muted-foreground">Loading embedding configuration...</p>;
  }

  return (
    <div className="grid min-h-0 gap-4 lg:grid-cols-[1.35fr_1fr]">
      <section className="rounded-xl border border-border bg-card/60 p-4">
        <h3 className="mb-1 text-sm font-semibold">Embeddings Configuration</h3>
        <p className="mb-4 text-xs text-muted-foreground">
          Select the embedding backend used for semantic code search and retrieval.
        </p>

        <div className="space-y-3">
          <label className="flex items-center gap-2 rounded-md border border-border/70 bg-background/60 px-3 py-2 text-xs">
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={(event) =>
                setForm((current) => (current ? { ...current, enabled: event.target.checked } : current))
              }
            />
            Enable semantic search (search.embeddings tool)
          </label>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground">Active Provider</label>
            <Select
              value={form.provider}
              onChange={(event) =>
                setForm((current) => (current ? { ...current, provider: event.target.value as FormState["provider"] } : current))
              }
            >
              {PROVIDERS.map((provider) => (
                <option key={provider} value={provider}>
                  {PROVIDER_LABELS[provider]}
                </option>
              ))}
            </Select>
          </div>

          <label className="flex items-center gap-2 rounded-md border border-border/70 bg-background/60 px-3 py-2 text-xs">
            <input
              type="checkbox"
              checked={form.normalize_l2}
              onChange={(event) =>
                setForm((current) => (current ? { ...current, normalize_l2: event.target.checked } : current))
              }
            />
            L2-normalize vectors after provider output
          </label>

          {form.provider === "gemini" ? (
            <>
              <Field label="API Key (optional unless missing)">
                <Input
                  type="password"
                  placeholder="Leave blank to keep existing key"
                  value={form.gemini.api_key}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? { ...current, gemini: { ...current.gemini, api_key: event.target.value } }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Model">
                <Input
                  value={form.gemini.model}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? { ...current, gemini: { ...current.gemini, model: event.target.value } }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Base URL">
                <Input
                  placeholder="https://generativelanguage.googleapis.com/v1beta"
                  value={form.gemini.base_url}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? { ...current, gemini: { ...current.gemini, base_url: event.target.value } }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Timeout (ms)">
                <Input
                  value={form.gemini.timeout_ms}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? { ...current, gemini: { ...current.gemini, timeout_ms: event.target.value } }
                        : current,
                    )
                  }
                />
              </Field>
            </>
          ) : null}

          {form.provider === "ollama" ? (
            <>
              <Field label="Base URL">
                <Input
                  value={form.ollama.base_url}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? { ...current, ollama: { ...current.ollama, base_url: event.target.value } }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Model">
                <Input
                  value={form.ollama.model}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? { ...current, ollama: { ...current.ollama, model: event.target.value } }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Timeout (ms)">
                <Input
                  value={form.ollama.timeout_ms}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? { ...current, ollama: { ...current.ollama, timeout_ms: event.target.value } }
                        : current,
                    )
                  }
                />
              </Field>
            </>
          ) : null}

          {form.provider === "transformersjs" ? (
            <>
              <Field label="Model">
                <Input
                  value={form.transformersjs.model}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? {
                            ...current,
                            transformersjs: { ...current.transformersjs, model: event.target.value },
                          }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Device">
                <Input
                  value={form.transformersjs.device}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? {
                            ...current,
                            transformersjs: { ...current.transformersjs, device: event.target.value },
                          }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Backend">
                <Input
                  placeholder="Optional"
                  value={form.transformersjs.backend}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? {
                            ...current,
                            transformersjs: { ...current.transformersjs, backend: event.target.value },
                          }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Cache Directory">
                <Input
                  placeholder="Optional"
                  value={form.transformersjs.cache_dir}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? {
                            ...current,
                            transformersjs: { ...current.transformersjs, cache_dir: event.target.value },
                          }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Bridge Command">
                <Input
                  value={form.transformersjs.bridge_command}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? {
                            ...current,
                            transformersjs: {
                              ...current.transformersjs,
                              bridge_command: event.target.value,
                            },
                          }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Bridge Script">
                <Input
                  placeholder="Optional custom script path"
                  value={form.transformersjs.bridge_script}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? {
                            ...current,
                            transformersjs: {
                              ...current.transformersjs,
                              bridge_script: event.target.value,
                            },
                          }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Timeout (ms)">
                <Input
                  value={form.transformersjs.timeout_ms}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? {
                            ...current,
                            transformersjs: { ...current.transformersjs, timeout_ms: event.target.value },
                          }
                        : current,
                    )
                  }
                />
              </Field>
            </>
          ) : null}

          {form.provider === "rust-hf" ? (
            <>
              <Field label="Model ID">
                <Input
                  placeholder="Qdrant/all-MiniLM-L6-v2-onnx or onnx-community/embeddinggemma-300m-ONNX"
                  value={form.rust_hf.model_id}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? { ...current, rust_hf: { ...current.rust_hf, model_id: event.target.value } }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Model Path">
                <Input
                  placeholder="Optional local model directory"
                  value={form.rust_hf.model_path}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? { ...current, rust_hf: { ...current.rust_hf, model_path: event.target.value } }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Runtime">
                <Select
                  value={form.rust_hf.runtime}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? {
                            ...current,
                            rust_hf: {
                              ...current.rust_hf,
                              runtime: event.target.value as "onnx" | "candle",
                            },
                          }
                        : current,
                    )
                  }
                >
                  <option value="onnx">ONNX</option>
                  <option value="candle">Candle</option>
                </Select>
              </Field>
              <Field label="Threads">
                <Input
                  placeholder="Optional"
                  value={form.rust_hf.threads}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? { ...current, rust_hf: { ...current.rust_hf, threads: event.target.value } }
                        : current,
                    )
                  }
                />
              </Field>
              <Field label="Timeout (ms)">
                <Input
                  value={form.rust_hf.timeout_ms}
                  onChange={(event) =>
                    setForm((current) =>
                      current
                        ? { ...current, rust_hf: { ...current.rust_hf, timeout_ms: event.target.value } }
                        : current,
                    )
                  }
                />
              </Field>
            </>
          ) : null}

          <div className="space-y-2 pt-1">
            <label className="text-xs font-medium text-muted-foreground">Auto-Configure Preference</label>
            <Select
              value={autoConfigPreference}
              onChange={(event) => setAutoConfigPreference(event.target.value as "local" | "quality")}
            >
              <option value="local">Prefer local (fast, no API costs)</option>
              <option value="quality">Prefer best quality (Gemini if configured)</option>
            </Select>
            <p className="text-[11px] text-muted-foreground">
              Gemini embeddings usually provide stronger quality, but free-tier rate limits can trigger quickly. Use paid keys for sustained indexing.
            </p>
            <div className="flex flex-wrap gap-2">
              <Button size="sm" variant="outline" onClick={() => handleAutoConfigure().catch(console.error)} disabled={autoConfiguring}>
                <Sparkles size={13} className="mr-1" />
                {autoConfiguring ? "Auto-configuring" : "Auto-configure"}
              </Button>
              <Button size="sm" onClick={() => handleSave().catch(console.error)} disabled={saving}>
                {saving ? "Saving" : "Save Embeddings Config"}
              </Button>
            </div>
          </div>

          {autoConfigNotes.length ? (
            <div className="rounded-md border border-border/70 bg-background/60 px-3 py-2 text-xs text-muted-foreground">
              {autoConfigNotes.map((note) => (
                <p key={note}>{note}</p>
              ))}
            </div>
          ) : null}

          {error ? <p className="text-xs text-destructive">{error}</p> : null}
        </div>
      </section>

      <section className="rounded-xl border border-border bg-card/60 p-4">
        <h3 className="mb-3 text-sm font-semibold">Embedding Provider Status</h3>
        <div className="space-y-2">
          {PROVIDERS.map((provider) => {
            const isActive = providerStatus?.provider === provider;
            const isConfigured = provider === "gemini" ? providerStatus?.geminiConfigured : true;
            const icon = provider === "gemini" ? <KeyRound size={13} /> : provider === "rust-hf" ? <Cpu size={13} /> : <Server size={13} />;

            return (
              <article key={provider} className="rounded-lg border border-border bg-background/60 p-3">
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2 text-sm font-medium">
                    {icon}
                    <span>{PROVIDER_LABELS[provider]}</span>
                  </div>
                  {isActive ? (
                    <span className="inline-flex items-center gap-1 rounded-full bg-info/15 px-2 py-0.5 text-[11px] font-medium text-info">
                      <Check size={11} />
                      Active
                    </span>
                  ) : null}
                </div>
                <p className="mt-2 text-xs text-muted-foreground">
                  {provider === "gemini"
                    ? isConfigured
                      ? "API key configured"
                      : "API key missing"
                    : "No API key required"}
                </p>
              </article>
            );
          })}
        </div>
      </section>
    </div>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="space-y-1.5">
      <label className="text-xs font-medium text-muted-foreground">{label}</label>
      {children}
    </div>
  );
}

function toPositiveNumber(value: string): number {
  const parsed = Number.parseInt(value, 10);
  if (Number.isNaN(parsed) || parsed <= 0) return 1;
  return parsed;
}
