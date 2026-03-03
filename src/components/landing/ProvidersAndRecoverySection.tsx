import { Settings, Globe, Key, Database, RotateCcw, FileSearch, CheckCircle2, Zap } from "lucide-react";
import { useRevealGroup } from "@/hooks/useReveal";
import { cn } from "@/lib/utils";

/* ─── Provider data ─── */
const PROVIDERS = [
  {
    name: "OpenAI",
    monogram: "O",
    models: ["gpt-4o", "o1"],
    status: "stable",
    ctx: "128k",
  },
  {
    name: "Anthropic",
    monogram: "A",
    models: ["claude-opus-4", "claude-sonnet-4"],
    status: "stable",
    ctx: "200k",
  },
  {
    name: "Gemini",
    monogram: "G",
    models: ["gemini-2.5-pro", "gemini-2.0-flash"],
    status: "stable",
    ctx: "1M",
  },
  {
    name: "GLM / Zhipu",
    monogram: "Z",
    models: ["glm-5", "glm-4"],
    status: "stable",
    ctx: "128k",
  },
  {
    name: "MiniMax",
    monogram: "M",
    models: ["MiniMax-M2.5"],
    status: "stable",
    ctx: "1M",
  },
  {
    name: "Kimi",
    monogram: "K",
    models: ["kimi-k2.5"],
    status: "stable",
    ctx: "200k",
  },
];

/* ─── Recovery event log ─── */
const RECOVERY_LOG = [
  { ts: "14:32:01.008", event: "task.build_started",      status: "ok"   },
  { ts: "14:32:01.441", event: "tool.call_started",       status: "ok"   },
  { ts: "14:32:02.230", event: "tool.call_finished",      status: "ok"   },
  { ts: "14:32:04.881", event: "tool.call_started",       status: "ok"   },
  { ts: "14:32:08.112", event: "process.crash",           status: "fail" },
  { ts: "14:32:08.112", event: "─ ─ ─ restart ─ ─ ─",    status: "gap"  },
  { ts: "14:32:09.004", event: "recovery.checkpoint_found", status: "ok"  },
  { ts: "14:32:09.011", event: "recovery.state_restored", status: "ok"   },
  { ts: "14:32:09.044", event: "task.build_resumed",      status: "ok"   },
];

function RecoveryLog() {
  return (
    <div className="space-y-0 font-mono text-[10px]">
      {RECOVERY_LOG.map((row, i) => (
        <div
          key={i}
          className={cn(
            "flex items-center gap-3 py-1.5 px-2 rounded",
            row.status === "fail" && "bg-destructive/8",
            row.status === "gap"  && "my-1"
          )}
        >
          {row.status === "gap" ? (
            <span className="text-muted-foreground/25 w-full text-center tracking-widest">
              {row.event}
            </span>
          ) : (
            <>
              <span className="text-muted-foreground/35 shrink-0 tabular-nums">{row.ts}</span>
              <span
                className={cn(
                  "flex-1",
                  row.status === "fail" ? "text-destructive/80" : "text-foreground/70",
                  row.status === "ok" && i >= 6 ? "text-success/80" : ""
                )}
              >
                {row.event}
              </span>
              <span
                className={cn(
                  "w-1.5 h-1.5 rounded-full shrink-0",
                  row.status === "fail" ? "bg-destructive/70" : "bg-success/60"
                )}
              />
            </>
          )}
        </div>
      ))}
    </div>
  );
}

const ProvidersAndRecoverySection = () => {
  const { ref, revealed } = useRevealGroup(0.08);

  return (
    <section
      id="providers"
      ref={ref as React.RefObject<HTMLElement>}
      className="py-28 border-t border-border/20 relative overflow-hidden"
    >
      <div className="absolute inset-0 grid-bg opacity-[0.018]" />

      <div className="container mx-auto px-6">

        {/* ─ Providers ─ */}
        <div className="mb-24">
          <div className={cn("mb-12 max-w-2xl", "reveal", revealed && "revealed")}>
            <span className="section-label mb-3 block">Providers</span>
            <h2 className="section-heading mb-4">Model-agnostic by design</h2>
            <p className="section-subheading">
              Connect any supported LLM provider through the same planner/worker interfaces. Configure from the UI, via API endpoints, or with environment variables.
            </p>
          </div>

          <div
            className={cn(
              "grid grid-cols-1 lg:grid-cols-2 gap-px bg-border rounded-2xl overflow-hidden border border-border",
              "reveal reveal-delay-2",
              revealed && "revealed"
            )}
          >
            {/* Provider table */}
            <div className="bento-cell overflow-x-auto">
              <p className="text-[10px] font-mono text-muted-foreground/50 uppercase tracking-wider mb-4">
                Supported providers
              </p>
              <table className="w-full text-[11px] font-mono">
                <thead>
                  <tr className="border-b border-border/40">
                    <th className="text-left text-muted-foreground/40 font-normal pb-2 pr-4">Provider</th>
                    <th className="text-left text-muted-foreground/40 font-normal pb-2 pr-4">Models</th>
                    <th className="text-left text-muted-foreground/40 font-normal pb-2 pr-4">Context</th>
                    <th className="text-left text-muted-foreground/40 font-normal pb-2">Status</th>
                  </tr>
                </thead>
                <tbody>
                  {PROVIDERS.map((p) => (
                    <tr key={p.name} className="border-b border-border/20 hover:bg-primary/3 transition-colors">
                      <td className="py-2.5 pr-4">
                        <div className="flex items-center gap-2">
                          <span className="inline-flex items-center justify-center w-5 h-5 rounded bg-primary/10 text-primary text-[9px] font-bold">
                            {p.monogram}
                          </span>
                          <span className="text-foreground/80">{p.name}</span>
                        </div>
                      </td>
                      <td className="py-2.5 pr-4">
                        <span className="text-muted-foreground/60">{p.models[0]}</span>
                        {p.models.length > 1 && (
                          <span className="text-muted-foreground/30 ml-1">+{p.models.length - 1}</span>
                        )}
                      </td>
                      <td className="py-2.5 pr-4 text-muted-foreground/60">{p.ctx}</td>
                      <td className="py-2.5">
                        <span className="flex items-center gap-1 text-success/70">
                          <span className="w-1.5 h-1.5 rounded-full bg-success/70" />
                          {p.status}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {/* Config methods */}
            <div className="bento-cell flex flex-col gap-4">
              <p className="text-[10px] font-mono text-muted-foreground/50 uppercase tracking-wider">
                Configuration methods
              </p>
              {[
                {
                  icon: Settings,
                  title: "Settings UI",
                  desc: "Configure models and credentials from the built-in settings panel. No config files required.",
                  code: null,
                },
                {
                  icon: Globe,
                  title: "Custom API Endpoints",
                  desc: "Point to any OpenAI-compatible endpoint for local models or proxies.",
                  code: "base_url = \"http://localhost:11434\"",
                },
                {
                  icon: Key,
                  title: "Environment Variables",
                  desc: "Auto-detected on startup. Set API keys and default models without touching the UI.",
                  code: "OPENAI_API_KEY=sk-...",
                },
              ].map((method) => (
                <div key={method.title} className="flex items-start gap-3 p-3 rounded-xl border border-border/40 bg-muted/20 hover:bg-muted/40 transition-colors">
                  <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-primary/10 border border-primary/15 shrink-0 mt-0.5">
                    <method.icon className="w-4 h-4 text-primary" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-xs font-semibold text-foreground font-mono mb-1">{method.title}</p>
                    <p className="text-xs text-muted-foreground leading-relaxed">{method.desc}</p>
                    {method.code && (
                      <code className="block mt-2 text-[10px] font-mono text-primary/60 bg-muted/40 px-2 py-1 rounded truncate">
                        {method.code}
                      </code>
                    )}
                  </div>
                </div>
              ))}

              <div className="mt-auto flex items-center gap-2 text-[10px] font-mono text-muted-foreground/40 pt-2 border-t border-border/30">
                <Zap className="w-3 h-3" />
                Same planner/worker interface for every provider
              </div>
            </div>
          </div>
        </div>

        {/* ─ Crash recovery ─ */}
        <div className="grid lg:grid-cols-2 gap-16 items-start">
          {/* Left: content */}
          <div className={cn("reveal reveal-delay-1", revealed && "revealed")}>
            <span className="section-label mb-3 block">Persistence</span>
            <h2 className="text-2xl sm:text-3xl font-bold tracking-tight text-foreground mb-4 font-mono">
              Crash recovery &amp; audit trail
            </h2>
            <p className="text-base text-muted-foreground leading-relaxed mb-8">
              Every event is appended to local SQLite before it's acted on. Runs are resumable. State is deterministically reconstructable from the event log.
            </p>

            <div className="space-y-3">
              {[
                { icon: Database,    title: "SQLite persistence",  desc: "No external dependencies. Local-first. Events written before execution." },
                { icon: RotateCcw,   title: "Recoverable runs",    desc: "Resume interrupted executions from the last checkpoint after any crash." },
                { icon: FileSearch,  title: "Event replay",        desc: "Deterministically replay any run from its append-only event log." },
              ].map((feat) => (
                <div key={feat.title} className="flex items-start gap-4 p-4 rounded-xl border border-border/40 bg-card/40 hover:bg-card/70 transition-colors">
                  <div className="flex items-center justify-center w-9 h-9 rounded-xl bg-primary/8 border border-primary/12 shrink-0">
                    <feat.icon className="w-4 h-4 text-primary" />
                  </div>
                  <div>
                    <p className="text-sm font-semibold text-foreground font-mono mb-1">{feat.title}</p>
                    <p className="text-xs text-muted-foreground leading-relaxed">{feat.desc}</p>
                  </div>
                </div>
              ))}
            </div>

            {/* Guarantees */}
            <div className="mt-8 p-5 rounded-xl border border-success/15 bg-success/5">
              <div className="flex items-center gap-2 mb-4">
                <CheckCircle2 className="w-4 h-4 text-success/70" />
                <span className="text-sm font-semibold font-mono text-foreground">Recovery guarantees</span>
              </div>
              <div className="space-y-2">
                {[
                  "No event loss on crash — writes are atomic",
                  "Deterministic replay — same inputs, same output",
                  "Full audit capability — every tool call logged",
                ].map((g) => (
                  <div key={g} className="flex items-start gap-2 text-xs text-muted-foreground">
                    <span className="w-1.5 h-1.5 rounded-full bg-success/60 mt-1 shrink-0" />
                    {g}
                  </div>
                ))}
              </div>
            </div>
          </div>

          {/* Right: recovery log */}
          <div className={cn("reveal reveal-delay-3", revealed && "revealed")}>
            <div className="terminal-window">
              <div className="terminal-titlebar">
                <div className="flex gap-1.5">
                  <div className="w-3 h-3 rounded-full bg-red-500/70" />
                  <div className="w-3 h-3 rounded-full bg-yellow-500/70" />
                  <div className="w-3 h-3 rounded-full bg-green-500/70" />
                </div>
                <span className="text-[10px] font-mono text-muted-foreground/40 ml-2">
                  Event log — crash + recovery
                </span>
              </div>
              <div className="terminal-body p-4">
                <RecoveryLog />
              </div>
              <div className="flex items-center gap-2 px-4 py-2 border-t border-border/20 bg-muted/20">
                <CheckCircle2 className="w-3.5 h-3.5 text-success/70" />
                <span className="text-[10px] font-mono text-success/60">
                  State recovered — run resumed from checkpoint
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default ProvidersAndRecoverySection;
