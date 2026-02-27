import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Activity, ArrowRight, FlaskConical, LoaderCircle, Play, Save, SkipForward } from "lucide-react";
import { useShallow } from "zustand/shallow";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useAppStore } from "@/stores/appStore";
import {
  type BenchmarkRealtimeEvent,
  type BenchmarkWorkload,
  type BusinessOpsDayTrace,
  type BusinessOpsProviderResult,
  type BusinessOpsScenarioDescriptor,
  type LlmProviderBenchmarkResult,
  type ModelCatalogEntry,
  type ModelBenchmarkReport,
  type ProviderConfigView,
  type RunModelBenchmarkRequest,
} from "@/types";
import { BenchmarksRealtimeTimeline } from "@/components/BenchmarksRealtimeTimeline";

type StepId = "models" | "simulation" | "review";
type ResultTab = "overview" | "replay";

type BenchmarkRecipe = {
  id: string;
  name: string;
  workload: BenchmarkWorkload;
  providers: string[];
  providerModels: Record<string, string>;
  warmupIterations: number;
  measuredIterations: number;
  days: number;
  promptsPerDay: number;
  scenarioKeys: string[];
};

const RECIPE_STORAGE_KEY = "orchestrix-benchmark-recipes";
const STEP_ORDER: StepId[] = ["models", "simulation", "review"];
const SERIES_COLORS = ["#3b82f6", "#14b8a6", "#f59e0b", "#ef4444", "#a855f7"];

function providerLabel(providerId: string): string {
  if (providerId === "minimax") return "MiniMax";
  if (providerId === "kimi") return "Kimi";
  if (providerId === "zhipu") return "GLM (Zhipu)";
  if (providerId === "modal") return "Modal";
  return providerId;
}

function modelsForProvider(provider: string, modelCatalog: ModelCatalogEntry[]) {
  return modelCatalog.find((entry) => entry.provider === provider)?.models ?? [];
}

function preferredProviderModel(
  provider: string,
  providerConfigs: ProviderConfigView[],
  modelCatalog: ModelCatalogEntry[]
): string | null {
  const configured = providerConfigs.find((entry) => entry.provider === provider)?.default_model;
  if (configured) return configured;
  const fallback = modelsForProvider(provider, modelCatalog)[0];
  return fallback ? fallback.name : null;
}

function toPercent(value: number): string {
  return `${(value * 100).toFixed(1)}%`;
}

function toCurrency(value: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(value);
}

function readStoredRecipes(): BenchmarkRecipe[] {
  try {
    const raw = localStorage.getItem(RECIPE_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed as BenchmarkRecipe[];
  } catch {
    return [];
  }
}

function saveStoredRecipes(recipes: BenchmarkRecipe[]) {
  localStorage.setItem(RECIPE_STORAGE_KEY, JSON.stringify(recipes));
}

type ChartSeries = {
  label: string;
  color: string;
  values: number[];
};

function SimpleLineChart({ title, subtitle, series }: { title: string; subtitle: string; series: ChartSeries[] }) {
  const width = 780;
  const height = 260;
  const margin = 24;

  const maxPoints = Math.max(1, ...series.map((item) => item.values.length));
  const allValues = series.flatMap((item) => item.values);
  const minY = Math.min(0, ...(allValues.length > 0 ? allValues : [0]));
  const maxY = Math.max(1, ...(allValues.length > 0 ? allValues : [1]));
  const yRange = Math.max(1e-6, maxY - minY);

  const toX = (idx: number) =>
    margin + ((width - margin * 2) * idx) / Math.max(1, maxPoints - 1);
  const toY = (value: number) =>
    height - margin - ((height - margin * 2) * (value - minY)) / yRange;

  return (
    <div className="rounded-lg border border-border/70 bg-background/70 p-3">
      <div className="mb-2">
        <p className="text-sm font-semibold text-foreground">{title}</p>
        <p className="text-xs text-muted-foreground">{subtitle}</p>
      </div>
      <div className="overflow-x-auto">
        <svg viewBox={`0 0 ${width} ${height}`} className="min-w-[620px]">
          <line x1={margin} y1={height - margin} x2={width - margin} y2={height - margin} stroke="currentColor" opacity={0.25} />
          <line x1={margin} y1={margin} x2={margin} y2={height - margin} stroke="currentColor" opacity={0.25} />
          {series.map((item) => {
            const points = item.values.map((value, idx) => `${toX(idx)},${toY(value)}`).join(" ");
            return <polyline key={item.label} points={points} fill="none" stroke={item.color} strokeWidth={2.5} />;
          })}
        </svg>
      </div>
      <div className="mt-2 flex flex-wrap gap-2">
        {series.map((item) => (
          <span key={item.label} className="inline-flex items-center gap-1 rounded-full border border-border/60 px-2 py-0.5 text-[11px] text-muted-foreground">
            <span className="inline-block h-2 w-2 rounded-full" style={{ backgroundColor: item.color }} />
            {item.label}
          </span>
        ))}
      </div>
    </div>
  );
}

export function BenchmarkPage() {
  const [modelCatalog, providerConfigs] = useAppStore(
    useShallow((state) => [state.modelCatalog, state.providerConfigs])
  );

  const availableProviders = useMemo(() => modelCatalog.map((entry) => entry.provider), [modelCatalog]);

  const [step, setStep] = useState<StepId>("models");
  const [resultTab, setResultTab] = useState<ResultTab>("overview");

  const [workload, setWorkload] = useState<BenchmarkWorkload>("business_ops");
  const [selectedProviders, setSelectedProviders] = useState<string[]>([]);
  const [selectedModels, setSelectedModels] = useState<Record<string, string>>({});

  const [warmupIterations, setWarmupIterations] = useState(1);
  const [measuredIterations, setMeasuredIterations] = useState(1);
  const [days, setDays] = useState(30);
  const [promptsPerDay, setPromptsPerDay] = useState(10);

  const [availableScenarios, setAvailableScenarios] = useState<BusinessOpsScenarioDescriptor[]>([]);
  const [selectedScenarioKeys, setSelectedScenarioKeys] = useState<string[]>([]);

  const [recipes, setRecipes] = useState<BenchmarkRecipe[]>(() => readStoredRecipes());
  const [recipeName, setRecipeName] = useState("Vending Duel - 30x10");

  const [isRunning, setIsRunning] = useState(false);
  const [runError, setRunError] = useState<string | null>(null);
  const [report, setReport] = useState<ModelBenchmarkReport | null>(null);
  const [timelineEvents, setTimelineEvents] = useState<BenchmarkRealtimeEvent[]>([]);

  const [replayProviderKey, setReplayProviderKey] = useState<string>("");
  const [replayScenarioId, setReplayScenarioId] = useState<string>("");
  const [replayDayIndex, setReplayDayIndex] = useState<number>(1);

  useEffect(() => {
    invoke<BusinessOpsScenarioDescriptor[]>("list_business_ops_scenarios_command")
      .then((scenarios) => {
        setAvailableScenarios(scenarios);
        if (selectedScenarioKeys.length === 0) {
          setSelectedScenarioKeys(scenarios.map((item) => item.scenario_key));
        }
      })
      .catch(console.error);
  }, [selectedScenarioKeys.length]);

  useEffect(() => {
    if (availableProviders.length === 0) return;
    setSelectedProviders((prev) => {
      if (prev.length > 0) return prev.filter((value) => availableProviders.includes(value));
      return availableProviders.slice(0, 2);
    });
  }, [availableProviders]);

  useEffect(() => {
    setSelectedModels((prev) => {
      const next = { ...prev };
      for (const provider of availableProviders) {
        if (next[provider]) continue;
        const preferred = preferredProviderModel(provider, providerConfigs, modelCatalog);
        if (preferred) next[provider] = preferred;
      }
      return next;
    });
  }, [availableProviders, providerConfigs, modelCatalog]);

  const toggleProvider = (provider: string) => {
    setSelectedProviders((prev) => {
      if (prev.includes(provider)) return prev.filter((value) => value !== provider);
      return [...prev, provider];
    });
  };

  const toggleScenario = (scenarioKey: string) => {
    setSelectedScenarioKeys((prev) => {
      if (prev.includes(scenarioKey)) return prev.filter((value) => value !== scenarioKey);
      return [...prev, scenarioKey];
    });
  };

  const setModelForProvider = (provider: string, model: string) => {
    setSelectedModels((prev) => ({ ...prev, [provider]: model }));
  };

  const saveRecipe = () => {
    const trimmed = recipeName.trim();
    if (!trimmed) return;

    const recipe: BenchmarkRecipe = {
      id: `${Date.now()}`,
      name: trimmed,
      workload,
      providers: selectedProviders,
      providerModels: selectedModels,
      warmupIterations,
      measuredIterations,
      days,
      promptsPerDay,
      scenarioKeys: selectedScenarioKeys,
    };

    const next = [recipe, ...recipes.filter((item) => item.name !== recipe.name)].slice(0, 20);
    setRecipes(next);
    saveStoredRecipes(next);
  };

  const applyRecipe = (recipe: BenchmarkRecipe) => {
    setWorkload(recipe.workload);
    setSelectedProviders(recipe.providers);
    setSelectedModels(recipe.providerModels);
    setWarmupIterations(recipe.warmupIterations);
    setMeasuredIterations(recipe.measuredIterations);
    setDays(recipe.days);
    setPromptsPerDay(recipe.promptsPerDay);
    setSelectedScenarioKeys(recipe.scenarioKeys);
    setRecipeName(recipe.name);
    setStep("review");
  };

  const runBenchmark = async () => {
    setIsRunning(true);
    setRunError(null);
    const currentRunId = `bench-${Date.now()}-${Math.floor(Math.random() * 10000)}`;
    setTimelineEvents([]);

    const unlisten = await listen<BenchmarkRealtimeEvent>("benchmark:event", (event) => {
      const payload = event.payload;
      if (!payload || payload.run_id !== currentRunId) {
        return;
      }

      setTimelineEvents((prev) => [...prev, payload].slice(-400));
    });

    try {
      const providerModels = selectedProviders.reduce<Record<string, string>>((acc, provider) => {
        const model = selectedModels[provider]?.trim();
        if (model) acc[provider] = model;
        return acc;
      }, {});

      const request: RunModelBenchmarkRequest = {
        run_id: currentRunId,
        workload,
        providers: selectedProviders,
        provider_models: providerModels,
        warmup_iterations: warmupIterations,
        measured_iterations: measuredIterations,
        business_ops_max_turns: days,
        business_ops_prompts_per_day: promptsPerDay,
        business_ops_scenarios: selectedScenarioKeys.length > 0 ? selectedScenarioKeys : null,
      };

      const nextReport = await invoke<ModelBenchmarkReport>("run_model_benchmark", { request });
      setReport(nextReport);
      setResultTab("overview");
    } catch (error) {
      setRunError(error instanceof Error ? error.message : String(error));
    } finally {
      unlisten();
      setIsRunning(false);
    }
  };

  const llmProviders: LlmProviderBenchmarkResult[] = useMemo(
    () => report?.llm?.providers ?? [],
    [report]
  );
  const businessProviders: BusinessOpsProviderResult[] = useMemo(
    () => report?.business_ops?.providers ?? [],
    [report]
  );

  const replayProviderOptions = useMemo(
    () =>
      businessProviders.map((provider) => ({
        key: `${provider.provider}:${provider.model ?? "-"}`,
        label: `${providerLabel(provider.provider)} / ${provider.model ?? "-"}`,
        provider,
      })),
    [businessProviders]
  );

  useEffect(() => {
    if (replayProviderOptions.length === 0) return;
    if (!replayProviderOptions.some((item) => item.key === replayProviderKey)) {
      setReplayProviderKey(replayProviderOptions[0].key);
    }
  }, [replayProviderOptions, replayProviderKey]);

  const replayProvider = replayProviderOptions.find((item) => item.key === replayProviderKey)?.provider;
  const replayScenarios = replayProvider?.scenarios ?? [];

  useEffect(() => {
    if (replayScenarios.length === 0) return;
    if (!replayScenarios.some((scenario) => scenario.scenario_id === replayScenarioId)) {
      setReplayScenarioId(replayScenarios[0].scenario_id);
      setReplayDayIndex(1);
    }
  }, [replayScenarios, replayScenarioId]);

  const replayScenario = replayScenarios.find((item) => item.scenario_id === replayScenarioId) ?? replayScenarios[0];
  const replayDay: BusinessOpsDayTrace | undefined = replayScenario?.timeline.find(
    (item) => item.day_index === replayDayIndex
  );

  const selectedOverviewScenarioId = useMemo(() => {
    if (selectedScenarioKeys.length > 0) {
      const selected = availableScenarios.find((item) => item.scenario_key === selectedScenarioKeys[0]);
      if (selected) return selected.scenario_id;
    }
    return availableScenarios[0]?.scenario_id;
  }, [availableScenarios, selectedScenarioKeys]);

  const profitSeries = useMemo(() => {
    const series: ChartSeries[] = [];
    businessProviders.forEach((provider, idx) => {
      const scenario = provider.scenarios.find((item) => item.scenario_id === selectedOverviewScenarioId) ?? provider.scenarios[0];
      if (!scenario) return;
      series.push({
        label: providerLabel(provider.provider),
        color: SERIES_COLORS[idx % SERIES_COLORS.length],
        values: scenario.timeline.map((day) => day.profit_to_date),
      });
    });
    return series;
  }, [businessProviders, selectedOverviewScenarioId]);

  const serviceSeries = useMemo(() => {
    const series: ChartSeries[] = [];
    businessProviders.forEach((provider, idx) => {
      const scenario = provider.scenarios.find((item) => item.scenario_id === selectedOverviewScenarioId) ?? provider.scenarios[0];
      if (!scenario) return;
      series.push({
        label: providerLabel(provider.provider),
        color: SERIES_COLORS[idx % SERIES_COLORS.length],
        values: scenario.timeline.map((day) => day.running_service_level * 100),
      });
    });
    return series;
  }, [businessProviders, selectedOverviewScenarioId]);

  const activeStepIndex = STEP_ORDER.indexOf(step);

  return (
    <div className="h-full overflow-y-auto p-5">
      <div className="mx-auto w-full max-w-7xl space-y-4">
        <section className="rounded-xl border border-border/80 bg-card/90 p-4 elevation-1 backdrop-blur-sm">
          <div className="mb-3 flex items-center justify-between gap-3">
            <div>
              <h1 className="text-lg font-semibold tracking-tight text-foreground">Benchmark Recipe Builder</h1>
              <p className="text-xs text-muted-foreground">
                Configure model-vs-model benchmark runs, then inspect executive trends and full prompt-by-prompt replay.
              </p>
            </div>
            <div className="rounded-full border border-info/30 bg-info/10 px-3 py-1 text-[11px] font-medium text-info">
              Harness: orchestrix_lib
            </div>
          </div>

          <div className="grid gap-2 md:grid-cols-3">
            {STEP_ORDER.map((item, index) => {
              const isActive = item === step;
              const done = index < activeStepIndex;
              return (
                <button
                  key={item}
                  type="button"
                  onClick={() => setStep(item)}
                  className={[
                    "rounded-md border px-3 py-2 text-left transition-colors",
                    isActive
                      ? "border-primary/50 bg-primary/10"
                      : done
                        ? "border-success/40 bg-success/10"
                        : "border-border/70 bg-background/70 hover:bg-accent/60",
                  ].join(" ")}
                >
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Step {index + 1}</p>
                  <p className="text-sm font-medium text-foreground">
                    {item === "models" ? "Choose Models" : item === "simulation" ? "Simulation Setup" : "Review & Run"}
                  </p>
                </button>
              );
            })}
          </div>

          {step === "models" && (
            <div className="mt-4 space-y-3 rounded-md border border-border/70 bg-background/70 p-3">
              <div className="flex items-center justify-between gap-3">
                <p className="text-sm font-semibold text-foreground">Models</p>
                <p className="text-xs text-muted-foreground">Pick 2+ competitors for a direct benchmark duel</p>
              </div>
              <div className="flex flex-wrap gap-2">
                {availableProviders.map((provider) => {
                  const selected = selectedProviders.includes(provider);
                  return (
                    <button
                      key={provider}
                      type="button"
                      onClick={() => toggleProvider(provider)}
                      className={[
                        "rounded-full border px-2.5 py-1 text-xs transition-colors",
                        selected
                          ? "border-primary/50 bg-primary/10 text-primary"
                          : "border-border/70 bg-muted/40 text-muted-foreground hover:bg-accent/70",
                      ].join(" ")}
                    >
                      {providerLabel(provider)}
                    </button>
                  );
                })}
              </div>

              <div className="grid gap-3 md:grid-cols-2">
                {selectedProviders.map((provider) => {
                  const models = modelsForProvider(provider, modelCatalog);
                  return (
                    <label key={provider} className="space-y-1 text-xs font-medium text-muted-foreground">
                      {providerLabel(provider)} model
                      <Select
                        value={selectedModels[provider] ?? ""}
                        onChange={(event) => setModelForProvider(provider, event.target.value)}
                      >
                        {models.map((model) => (
                          <option key={model.name} value={model.name}>
                            {model.name} ({Math.round(model.context_window / 1000)}k context)
                          </option>
                        ))}
                      </Select>
                    </label>
                  );
                })}
              </div>

              <div className="flex justify-end">
                <Button onClick={() => setStep("simulation")} disabled={selectedProviders.length < 1}>
                  Next
                  <ArrowRight size={14} />
                </Button>
              </div>
            </div>
          )}

          {step === "simulation" && (
            <div className="mt-4 space-y-3 rounded-md border border-border/70 bg-background/70 p-3">
              <div className="grid gap-3 md:grid-cols-2">
                <label className="space-y-1 text-xs font-medium text-muted-foreground">
                  Workload
                  <Select value={workload} onChange={(event) => setWorkload(event.target.value as BenchmarkWorkload)}>
                    <option value="business_ops">Business ops simulation</option>
                    <option value="llm">LLM tasks</option>
                    <option value="llm_and_business_ops">LLM + business ops</option>
                  </Select>
                </label>

                <label className="space-y-1 text-xs font-medium text-muted-foreground">
                  Measured runs
                  <Input
                    type="number"
                    min={1}
                    value={measuredIterations}
                    onChange={(event) => setMeasuredIterations(Math.max(1, Number(event.target.value) || 1))}
                  />
                </label>
              </div>

              <div className="grid gap-3 md:grid-cols-4">
                <label className="space-y-1 text-xs font-medium text-muted-foreground">
                  Warmup runs
                  <Input
                    type="number"
                    min={0}
                    value={warmupIterations}
                    onChange={(event) => setWarmupIterations(Math.max(0, Number(event.target.value) || 0))}
                  />
                </label>

                <label className="space-y-1 text-xs font-medium text-muted-foreground">
                  Days in simulation
                  <Input
                    type="number"
                    min={1}
                    value={days}
                    onChange={(event) => setDays(Math.max(1, Number(event.target.value) || 1))}
                  />
                </label>

                <label className="space-y-1 text-xs font-medium text-muted-foreground">
                  Prompts per day
                  <Input
                    type="number"
                    min={1}
                    value={promptsPerDay}
                    onChange={(event) => setPromptsPerDay(Math.max(1, Number(event.target.value) || 1))}
                  />
                </label>

                <div className="rounded-md border border-info/30 bg-info/10 px-3 py-2 text-[11px] text-info">
                  Max tokens are auto-set from each selected model's context window.
                </div>
              </div>

              <div className="space-y-2 rounded-md border border-border/70 bg-card/50 p-3">
                <div className="flex items-center justify-between gap-2">
                  <p className="text-xs font-medium text-muted-foreground">Business scenarios</p>
                  <p className="text-[11px] text-muted-foreground">
                    {selectedScenarioKeys.length === 0
                      ? "All scenarios"
                      : `${selectedScenarioKeys.length} selected`}
                  </p>
                </div>
                <div className="grid gap-2 md:grid-cols-2">
                  {availableScenarios.map((scenario) => {
                    const selected = selectedScenarioKeys.includes(scenario.scenario_key);
                    return (
                      <button
                        key={scenario.scenario_key}
                        type="button"
                        onClick={() => toggleScenario(scenario.scenario_key)}
                        className={[
                          "rounded-md border px-2.5 py-2 text-left transition-colors",
                          selected
                            ? "border-primary/50 bg-primary/10"
                            : "border-border/70 bg-background/70 hover:bg-accent/60",
                        ].join(" ")}
                      >
                        <p className="text-xs font-semibold text-foreground">{scenario.scenario_key}</p>
                        <p className="text-[11px] text-muted-foreground">{scenario.scenario_id}</p>
                        <p className="mt-1 text-[11px] text-muted-foreground">{scenario.description}</p>
                      </button>
                    );
                  })}
                </div>
              </div>

              <div className="flex justify-between">
                <Button variant="outline" onClick={() => setStep("models")}>
                  Back
                </Button>
                <Button onClick={() => setStep("review")}>
                  Next
                  <ArrowRight size={14} />
                </Button>
              </div>
            </div>
          )}

          {step === "review" && (
            <div className="mt-4 space-y-3 rounded-md border border-border/70 bg-background/70 p-3">
              <p className="text-sm font-semibold text-foreground">Review recipe</p>

              <div className="rounded-md border border-border/70 bg-card/50 p-3 text-sm text-muted-foreground">
                {selectedProviders.length} models | {days} days | {promptsPerDay} prompts/day | {measuredIterations} measured run
                {measuredIterations === 1 ? "" : "s"}
                <br />
                Goal: run owner-agent simulation and compare profit, service level, and action quality.
              </div>

              <div className="flex flex-wrap gap-2">
                {selectedProviders.map((provider) => (
                  <div key={provider} className="rounded-md border border-border/70 bg-card/40 px-2.5 py-1 text-xs">
                    <span className="font-medium text-foreground">{providerLabel(provider)}</span>
                    <span className="text-muted-foreground"> | {selectedModels[provider] ?? "default"}</span>
                  </div>
                ))}
              </div>

              <div className="grid gap-2 md:grid-cols-[1fr_auto_auto]">
                <Input
                  value={recipeName}
                  onChange={(event) => setRecipeName(event.target.value)}
                  placeholder="Recipe name"
                />
                <Button variant="outline" onClick={saveRecipe}>
                  <Save size={14} />
                  Save Recipe
                </Button>
                <Button onClick={() => runBenchmark().catch(console.error)} disabled={isRunning || selectedProviders.length === 0}>
                  {isRunning ? <LoaderCircle size={14} className="animate-spin" /> : <Play size={14} />}
                  {isRunning ? "Running..." : "Run Benchmark"}
                </Button>
              </div>

              {recipes.length > 0 && (
                <div className="space-y-2 rounded-md border border-border/70 bg-card/50 p-3">
                  <p className="text-xs font-medium text-muted-foreground">Saved recipes</p>
                  <div className="flex flex-wrap gap-2">
                    {recipes.map((recipe) => (
                      <button
                        key={recipe.id}
                        type="button"
                        onClick={() => applyRecipe(recipe)}
                        className="rounded-md border border-border/70 bg-background/70 px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent/60"
                      >
                        {recipe.name}
                      </button>
                    ))}
                  </div>
                </div>
              )}

              <div className="flex justify-between">
                <Button variant="outline" onClick={() => setStep("simulation")}>
                  Back
                </Button>
                <Button variant="outline" onClick={() => setStep("models")}>
                  <SkipForward size={14} />
                  Edit Models
                </Button>
              </div>
            </div>
          )}

          {runError && (
            <div className="mt-3 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {runError}
            </div>
          )}
        </section>

        {report && (
          <section className="rounded-xl border border-border/80 bg-card/90 p-4 elevation-1 backdrop-blur-sm">
            <div className="mb-3 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <FlaskConical size={14} className="text-warning" />
                <h2 className="text-sm font-semibold tracking-tight">Results</h2>
              </div>
              <div className="flex gap-2">
                <Button variant={resultTab === "overview" ? "default" : "outline"} size="sm" onClick={() => setResultTab("overview")}>
                  Overview
                </Button>
                <Button variant={resultTab === "replay" ? "default" : "outline"} size="sm" onClick={() => setResultTab("replay")}>
                  Replay
                </Button>
              </div>
            </div>

            {resultTab === "overview" && (
              <div className="space-y-4">
                {businessProviders.length > 0 && (
                  <>
                    <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
                      {businessProviders.map((provider) => (
                        <div key={`${provider.provider}-${provider.model ?? "none"}`} className="rounded-md border border-border/70 bg-background/70 p-3">
                          <p className="text-xs text-muted-foreground">{providerLabel(provider.provider)}</p>
                          <p className="text-sm font-semibold text-foreground">{provider.model ?? "-"}</p>
                          <div className="mt-2 space-y-1 text-xs">
                            <p>Score: <span className="font-medium text-foreground">{provider.aggregate.avg_score.toFixed(3)}</span></p>
                            <p>Profit: <span className="font-medium text-foreground">{toCurrency(provider.aggregate.avg_profit)}</span></p>
                            <p>Service: <span className="font-medium text-foreground">{toPercent(provider.aggregate.avg_service_level)}</span></p>
                          </div>
                        </div>
                      ))}
                    </div>

                    {profitSeries.length > 0 && (
                      <SimpleLineChart
                        title="Profit trend by day"
                        subtitle="Cumulative profit-to-date across the selected scenario"
                        series={profitSeries}
                      />
                    )}

                    {serviceSeries.length > 0 && (
                      <SimpleLineChart
                        title="Service level trend"
                        subtitle="Running service level percentage through the simulation"
                        series={serviceSeries}
                      />
                    )}
                  </>
                )}

                {llmProviders.length > 0 && (
                  <div className="rounded-md border border-border/70 bg-background/70 p-3">
                    <div className="mb-2 flex items-center gap-2">
                      <Activity size={14} className="text-info" />
                      <p className="text-sm font-semibold text-foreground">LLM Task Scoreboard</p>
                    </div>
                    <div className="overflow-x-auto">
                      <table className="w-full min-w-[620px] text-left text-xs">
                        <thead className="text-muted-foreground">
                          <tr>
                            <th className="px-2 py-1 font-medium">Provider</th>
                            <th className="px-2 py-1 font-medium">Model</th>
                            <th className="px-2 py-1 font-medium">Score</th>
                            <th className="px-2 py-1 font-medium">Pass Rate</th>
                            <th className="px-2 py-1 font-medium">Latency</th>
                          </tr>
                        </thead>
                        <tbody>
                          {llmProviders.map((provider) => (
                            <tr key={`${provider.provider}-${provider.model ?? "none"}`} className="border-t border-border/60">
                              <td className="px-2 py-1 text-foreground">{providerLabel(provider.provider)}</td>
                              <td className="px-2 py-1 text-muted-foreground">{provider.model ?? "-"}</td>
                              <td className="px-2 py-1 text-foreground">{provider.aggregate?.weighted_score.toFixed(3) ?? "-"}</td>
                              <td className="px-2 py-1 text-foreground">{provider.aggregate ? toPercent(provider.aggregate.pass_rate) : "-"}</td>
                              <td className="px-2 py-1 text-foreground">
                                {provider.aggregate ? `${provider.aggregate.avg_p50_latency_ms.toFixed(1)} ms` : "-"}
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                )}
              </div>
            )}

            {resultTab === "replay" && (
              <div className="space-y-3">
                {businessProviders.length === 0 ? (
                  <div className="rounded-md border border-border/70 bg-background/70 p-3 text-sm text-muted-foreground">
                    Replay is available for business-ops runs.
                  </div>
                ) : (
                  <>
                    <div className="grid gap-3 md:grid-cols-3">
                      <label className="space-y-1 text-xs font-medium text-muted-foreground">
                        Provider run
                        <Select value={replayProviderKey} onChange={(event) => setReplayProviderKey(event.target.value)}>
                          {replayProviderOptions.map((item) => (
                            <option key={item.key} value={item.key}>
                              {item.label}
                            </option>
                          ))}
                        </Select>
                      </label>

                      <label className="space-y-1 text-xs font-medium text-muted-foreground">
                        Scenario
                        <Select value={replayScenarioId} onChange={(event) => setReplayScenarioId(event.target.value)}>
                          {replayScenarios.map((scenario) => (
                            <option key={scenario.scenario_id} value={scenario.scenario_id}>
                              {scenario.scenario_id}
                            </option>
                          ))}
                        </Select>
                      </label>

                      <label className="space-y-1 text-xs font-medium text-muted-foreground">
                        Day
                        <Select value={`${replayDayIndex}`} onChange={(event) => setReplayDayIndex(Number(event.target.value))}>
                          {(replayScenario?.timeline ?? []).map((day) => (
                            <option key={day.day_index} value={day.day_index}>
                              Day {day.day_index}
                            </option>
                          ))}
                        </Select>
                      </label>
                    </div>

                    {replayScenario && replayDay && (
                      <div className="space-y-3">
                        <div className="rounded-md border border-border/70 bg-background/70 p-3">
                          <p className="text-sm font-semibold text-foreground">
                            Day {replayDay.day_index} | {providerLabel(replayProvider?.provider ?? "-")}
                          </p>
                          <div className="mt-2 grid gap-2 text-xs text-muted-foreground md:grid-cols-4">
                            <p>Ending cash: <span className="font-medium text-foreground">{toCurrency(replayDay.ending_cash)}</span></p>
                            <p>Profit to date: <span className="font-medium text-foreground">{toCurrency(replayDay.profit_to_date)}</span></p>
                            <p>Service level: <span className="font-medium text-foreground">{toPercent(replayDay.running_service_level)}</span></p>
                            <p>Stockout rate: <span className="font-medium text-foreground">{toPercent(replayDay.running_stockout_rate)}</span></p>
                          </div>
                        </div>

                        <div className="space-y-2">
                          {replayDay.prompts.map((prompt) => (
                            <div key={prompt.prompt_index} className="rounded-md border border-border/70 bg-background/70 p-3">
                              <div className="flex items-center justify-between gap-2">
                                <p className="text-sm font-medium text-foreground">
                                  Prompt {prompt.prompt_index}
                                </p>
                                <p className="text-xs text-muted-foreground">
                                  {prompt.action_kind} | {prompt.latency_ms.toFixed(1)} ms
                                </p>
                              </div>

                              {prompt.reasoning && (
                                <div className="mt-2 rounded-md border border-border/60 bg-card/40 p-2 text-xs text-muted-foreground">
                                  {prompt.reasoning}
                                </div>
                              )}

                              <div className="mt-2 space-y-1">
                                {prompt.tool_calls.length === 0 ? (
                                  <p className="text-xs text-muted-foreground">No tool calls issued.</p>
                                ) : (
                                  prompt.tool_calls.map((call, idx) => (
                                    <div key={`${prompt.prompt_index}-${idx}`} className="rounded border border-border/60 px-2 py-1 text-xs">
                                      <p className="font-medium text-foreground">
                                        {call.tool_name} {call.success ? "(ok)" : "(failed)"}
                                      </p>
                                      <p className="text-muted-foreground">Args: {JSON.stringify(call.args)}</p>
                                      <p className="text-muted-foreground">Result: {call.result}</p>
                                    </div>
                                  ))
                                )}
                              </div>

                              <details className="mt-2">
                                <summary className="cursor-pointer text-xs text-muted-foreground">State snapshot</summary>
                                <pre className="mt-1 max-h-52 overflow-auto rounded-md border border-border/60 bg-card/40 p-2 text-[11px] text-muted-foreground">
                                  {prompt.state_snapshot}
                                </pre>
                              </details>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                  </>
                )}
              </div>
            )}
          </section>
        )}

        {(isRunning || timelineEvents.length > 0) && (
          <BenchmarksRealtimeTimeline events={timelineEvents} />
        )}
      </div>
    </div>
  );
}
