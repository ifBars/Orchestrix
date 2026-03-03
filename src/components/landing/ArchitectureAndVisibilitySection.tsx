import { useEffect, useState } from "react";
import { Activity, Cpu, Database, Layers, Terminal, Search, FileCode, Clock } from "lucide-react";
import { useRevealGroup } from "@/hooks/useReveal";
import { cn } from "@/lib/utils";

/* ─── Animated event counter ─── */
function EventCounter() {
  const [count, setCount] = useState(1042);
  useEffect(() => {
    const interval = setInterval(() => {
      setCount((c) => c + Math.floor(Math.random() * 3 + 1));
    }, 600);
    return () => clearInterval(interval);
  }, []);
  return (
    <span className="text-4xl font-bold font-mono tracking-tight text-foreground tabular-nums">
      {count.toLocaleString()}
    </span>
  );
}

/* ─── Scrolling event ticker ─── */
const TICKER_EVENTS = [
  "task.created",
  "agent.deciding",
  "tool.call_started",
  "tool.call_finished",
  "agent.tool_calls_preparing",
  "artifact.created",
  "agent.plan_ready",
  "task.awaiting_review",
  "task.build_started",
  "agent.message_delta",
  "tool.call_started",
  "task.completed",
];

function EventTicker() {
  const [, setIndex] = useState(0);
  const [visible, setVisible] = useState(TICKER_EVENTS.slice(0, 5));

  useEffect(() => {
    const t = setInterval(() => {
      setIndex((i) => {
        const next = (i + 1) % TICKER_EVENTS.length;
        setVisible((prev) => [...prev.slice(-4), TICKER_EVENTS[next]]);
        return next;
      });
    }, 900);
    return () => clearInterval(t);
  }, []);

  return (
    <div className="space-y-1 overflow-hidden">
      {visible.map((ev, i) => (
        <div
          key={`${ev}-${i}`}
          className={cn(
            "flex items-center gap-2 py-1 px-2 rounded font-mono text-[10px] transition-all duration-300",
            i === visible.length - 1
              ? "bg-primary/10 text-primary"
              : "text-muted-foreground/50"
          )}
        >
          <span
            className={cn(
              "w-1.5 h-1.5 rounded-full shrink-0",
              i === visible.length - 1 ? "bg-primary animate-pulse" : "bg-muted-foreground/20"
            )}
          />
          {ev}
        </div>
      ))}
    </div>
  );
}

/* ─── Mini timeline mockup ─── */
const TIMELINE_ROWS = [
  { phase: "planning", label: "agent.deciding",            detail: "turn=1",                    expand: false },
  { phase: "tool",     label: "tool.call_started",         detail: 'fs.list "src/auth"',        expand: true },
  { phase: "tool",     label: "tool.call_finished",        detail: "files=12",                  expand: false },
  { phase: "planning", label: "agent.deciding",            detail: "turn=2",                    expand: false },
  { phase: "artifact", label: "artifact.created",          detail: 'plan.md  kind="plan"',      expand: true },
  { phase: "complete", label: "agent.plan_ready",          detail: "steps=5",                   expand: false },
];

const PHASE_COLORS: Record<string, string> = {
  planning: "bg-info/80",
  tool:     "bg-success/80",
  artifact: "bg-warning/80",
  complete: "bg-success/80",
};

function TimelineMockup() {
  return (
    <div className="space-y-0 font-mono text-[10px]">
      {TIMELINE_ROWS.map((row, i) => (
        <div
          key={i}
          className={cn(
            "flex items-start gap-3 py-2 px-3 rounded-md transition-colors duration-150",
            row.expand ? "bg-primary/5 border border-primary/10" : "hover:bg-muted/30"
          )}
        >
          {/* Phase dot */}
          <div className="flex flex-col items-center gap-0 pt-0.5">
            <span className={`w-2 h-2 rounded-full shrink-0 ${PHASE_COLORS[row.phase]}`} />
            {i < TIMELINE_ROWS.length - 1 && (
              <div className="w-px h-3 bg-border/40 mt-0.5" />
            )}
          </div>
          <div className="flex-1 min-w-0">
            <span className="text-foreground/80">{row.label}</span>
            {row.expand && (
              <div className="mt-1 text-muted-foreground/50">
                ↳ {row.detail}
              </div>
            )}
          </div>
          {!row.expand && (
            <span className="text-muted-foreground/30 text-[9px] shrink-0">{row.detail}</span>
          )}
        </div>
      ))}
    </div>
  );
}

/* ─── Architecture node ─── */
function ArchNode({
  icon: Icon,
  label,
  sub,
  accent = false,
}: {
  icon: React.ElementType;
  label: string;
  sub: string;
  accent?: boolean;
}) {
  return (
    <div
      className={cn(
        "flex items-center gap-3 p-3 rounded-xl border transition-all duration-200",
        accent
          ? "border-primary/30 bg-primary/8"
          : "border-border/50 bg-muted/20 hover:bg-muted/40"
      )}
    >
      <div
        className={cn(
          "flex items-center justify-center w-9 h-9 rounded-lg shrink-0",
          accent ? "bg-primary/15" : "bg-muted/50"
        )}
      >
        <Icon className={cn("w-4 h-4", accent ? "text-primary" : "text-foreground/60")} />
      </div>
      <div>
        <p className="text-xs font-semibold text-foreground font-mono">{label}</p>
        <p className="text-[10px] text-muted-foreground font-mono">{sub}</p>
      </div>
    </div>
  );
}

/* ─── Main section ─── */
const ArchitectureAndVisibilitySection = () => {
  const { ref, revealed } = useRevealGroup(0.08);

  return (
    <section
      id="architecture"
      ref={ref as React.RefObject<HTMLElement>}
      className="py-28 border-t border-border/20 relative overflow-hidden"
    >
      <div className="absolute inset-0 grid-bg opacity-[0.018]" />

      <div className="container mx-auto px-6">
        {/* Header */}
        <div className={cn("mb-16 max-w-2xl", "reveal", revealed && "revealed")}>
          <span className="section-label mb-3 block">Architecture & Visibility</span>
          <h2 className="section-heading mb-4">
            Event-driven. Transparent by design.
          </h2>
          <p className="section-subheading">
            The Rust backend owns all state. The frontend is a pure renderer. Every transition produces an auditable event — visible in real time.
          </p>
        </div>

        {/* Bento grid */}
        <div
          className={cn(
            "grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-px bg-border rounded-2xl overflow-hidden border border-border",
            "reveal reveal-delay-2",
            revealed && "revealed"
          )}
        >

          {/* ─ Cell 1: Event bus pulse (tall, spans 1 row) ─ */}
          <div className="bento-cell flex flex-col gap-4 xl:row-span-1">
            <div className="flex items-center justify-between">
              <div>
                <span className="section-label text-[10px]">Event Bus</span>
                <p className="text-sm font-semibold text-foreground mt-1 font-mono">
                  Append-only stream
                </p>
              </div>
              <div className="flex items-center justify-center w-10 h-10 rounded-xl border border-primary/30 bg-primary/10">
                <Activity className="w-5 h-5 text-primary" />
              </div>
            </div>
            <div className="mt-auto">
              <EventCounter />
              <p className="text-[10px] font-mono text-muted-foreground/60 mt-1">
                events persisted this session
              </p>
            </div>
            <div className="mt-2">
              <EventTicker />
            </div>
          </div>

          {/* ─ Cell 2: Architecture layers ─ */}
          <div className="bento-cell flex flex-col gap-3">
            <div className="mb-2">
              <span className="section-label text-[10px]">Layers</span>
              <p className="text-sm font-semibold text-foreground mt-1 font-mono">
                Backend-authoritative
              </p>
            </div>
            <ArchNode icon={Layers}   label="Frontend"   sub="React — renders state only" />
            <ArchNode icon={Cpu}      label="Backend"     sub="Rust — owns all decisions"  accent />
            <ArchNode icon={Terminal} label="Tool Layer"  sub="FS · Git · Shell · MCP" />
            <ArchNode icon={Database} label="Storage"     sub="SQLite — full event log" />
          </div>

          {/* ─ Cell 3: Live timeline mockup ─ */}
          <div className="bento-cell flex flex-col gap-3 xl:row-span-1">
            <div className="mb-2">
              <span className="section-label text-[10px]">Timeline</span>
              <p className="text-sm font-semibold text-foreground mt-1 font-mono">
                Condensed. Expandable.
              </p>
            </div>
            <div className="flex-1 rounded-lg border border-border/50 bg-muted/20 p-3 overflow-hidden">
              <TimelineMockup />
            </div>
            <p className="text-[10px] font-mono text-muted-foreground/50">
              Summary-first · full detail on demand
            </p>
          </div>

          {/* ─ Cell 4: Visibility callouts (wide, bottom row) ─ */}
          <div className="bento-cell md:col-span-2 xl:col-span-3">
            <div className="flex flex-wrap gap-6 lg:gap-10">
              {[
                { icon: Search,   label: "Inspect decisions",   desc: "See why the agent chose each action — model reasoning visible per turn" },
                { icon: Terminal, label: "Inspect tool calls",  desc: "Full input/output for every FS, shell, and git operation" },
                { icon: FileCode, label: "Artifact review",     desc: "Review generated files, patches, and plans before they commit" },
                { icon: Clock,    label: "Condensed timeline",  desc: "Expandable event rows — one-line summary, full detail on demand" },
              ].map((item) => (
                <div key={item.label} className="flex items-start gap-3 min-w-[200px] flex-1">
                  <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-primary/10 border border-primary/20 shrink-0 mt-0.5">
                    <item.icon className="w-4 h-4 text-primary" />
                  </div>
                  <div>
                    <p className="text-xs font-semibold text-foreground font-mono mb-1">{item.label}</p>
                    <p className="text-xs text-muted-foreground leading-relaxed">{item.desc}</p>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* Data flow note */}
        <div
          className={cn(
            "mt-8 inline-flex items-center gap-3 px-4 py-2 rounded-full border border-border/50 bg-card/40 backdrop-blur-sm",
            "reveal reveal-delay-4",
            revealed && "revealed"
          )}
        >
          <span className="w-1.5 h-1.5 rounded-full bg-primary animate-pulse" />
          <code className="text-xs font-mono text-muted-foreground">
            event_bus::emit() → SQLite → frontend::render(state)
          </code>
          <span className="text-muted-foreground/40 text-xs">one-way</span>
        </div>
      </div>
    </section>
  );
};

export default ArchitectureAndVisibilitySection;
