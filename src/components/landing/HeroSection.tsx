import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { ArrowRight, Download, Github } from "lucide-react";

/* ─────────────────────────────────────────────
   Simulated event stream data
───────────────────────────────────────────── */
type EventEntry = {
  time: string;
  name: string;
  payload: string;
  badge: "deciding" | "tool" | "artifact" | "complete" | "plan";
};

const EVENT_POOL: EventEntry[] = [
  { time: "00:00.012", name: "task.created",              payload: 'id="run_4f9a"  goal="refactor auth module"', badge: "plan" },
  { time: "00:00.041", name: "agent.planning_started",    payload: 'task_id="run_4f9a"', badge: "deciding" },
  { time: "00:00.098", name: "agent.deciding",            payload: 'turn=1  step_idx=0', badge: "deciding" },
  { time: "00:00.834", name: "tool.call_started",         payload: 'name="fs.list"  path="src/auth"', badge: "tool" },
  { time: "00:00.862", name: "tool.call_finished",        payload: 'name="fs.list"  files=12', badge: "tool" },
  { time: "00:01.102", name: "tool.call_started",         payload: 'name="fs.read"  path="src/auth/index.ts"', badge: "tool" },
  { time: "00:01.155", name: "tool.call_finished",        payload: 'name="fs.read"  bytes=3201', badge: "tool" },
  { time: "00:01.890", name: "agent.deciding",            payload: 'turn=2  step_idx=0', badge: "deciding" },
  { time: "00:02.441", name: "tool.call_started",         payload: 'name="search.rg"  pattern="useAuth"', badge: "tool" },
  { time: "00:02.503", name: "tool.call_finished",        payload: 'name="search.rg"  matches=7', badge: "tool" },
  { time: "00:03.012", name: "artifact.created",          payload: 'kind="plan"  file="plan.md"', badge: "artifact" },
  { time: "00:03.041", name: "agent.plan_ready",          payload: 'steps=5  goal="refactor auth module"', badge: "plan" },
  { time: "00:03.100", name: "task.awaiting_review",      payload: 'task_id="run_4f9a"', badge: "plan" },
  { time: "00:15.320", name: "task.build_started",        payload: 'approved_by="user"', badge: "deciding" },
  { time: "00:15.341", name: "agent.deciding",            payload: 'turn=1  mode="build"', badge: "deciding" },
  { time: "00:15.880", name: "tool.call_started",         payload: 'name="fs.write"  path="src/auth/session.ts"', badge: "tool" },
  { time: "00:15.912", name: "tool.call_finished",        payload: 'name="fs.write"  bytes=1842', badge: "tool" },
  { time: "00:16.441", name: "tool.call_started",         payload: 'name="cmd.exec"  cmd="bun test auth"', badge: "tool" },
  { time: "00:18.023", name: "tool.call_finished",        payload: 'name="cmd.exec"  exit=0  tests=14', badge: "tool" },
  { time: "00:18.090", name: "artifact.created",          payload: 'kind="patch"  file="auth.patch"', badge: "artifact" },
  { time: "00:18.140", name: "task.completed",            payload: 'task_id="run_4f9a"  tools=8', badge: "complete" },
];

const BADGE_COLORS: Record<string, string> = {
  deciding: "event-badge--deciding",
  tool:     "event-badge--tool",
  artifact: "event-badge--artifact",
  complete: "event-badge--complete",
  plan:     "event-badge--plan",
};

/* ─────────────────────────────────────────────
   Live event stream component
───────────────────────────────────────────── */
function EventStream({ containerRef }: { containerRef: React.RefObject<HTMLDivElement | null> }) {
  const [visible, setVisible] = useState<EventEntry[]>([]);
  const [cursor, setCursor] = useState(0);

  useEffect(() => {
    setVisible(EVENT_POOL.slice(0, 4));
    setCursor(4);
  }, []);

  useEffect(() => {
    if (cursor >= EVENT_POOL.length) {
      const resetTimer = setTimeout(() => {
        setVisible([]);
        setCursor(0);
      }, 3500);
      return () => clearTimeout(resetTimer);
    }

    const delay = cursor < 4 ? 0 : 420 + Math.random() * 380;
    const timer = setTimeout(() => {
      setVisible((prev) => [...prev.slice(-14), EVENT_POOL[cursor]]);
      setCursor((c) => c + 1);
    }, delay);
    return () => clearTimeout(timer);
  }, [cursor]);

  // Scroll only within the terminal container — never touch the page scroll
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [visible, containerRef]);

  return (
    <div className="flex flex-col gap-0 overflow-hidden">
      {visible.map((ev, i) => (
        <div
          key={`${ev.time}-${i}`}
          className="event-row animate-event-slide-in"
        >
          <span className="event-time">{ev.time}</span>
          <span className={`event-badge ${BADGE_COLORS[ev.badge]}`}>{ev.badge}</span>
          <span className="event-name">{ev.name}</span>
          <span className="event-payload">{ev.payload}</span>
        </div>
      ))}
      {/* Blinking cursor */}
      <div className="event-row">
        <span className="event-time">
          {visible.length > 0 && cursor < EVENT_POOL.length ? "..." : "—"}
        </span>
        <span
          className="inline-block w-1.5 h-3.5 bg-primary/80 animate-cursor-blink ml-0.5"
          style={{ verticalAlign: "middle" }}
        />
      </div>
    </div>
  );
}

/* ─────────────────────────────────────────────
   Hero section
───────────────────────────────────────────── */
const HeroSection = () => {
  const streamBodyRef = useRef<HTMLDivElement>(null);

  return (
    <section className="relative pt-28 pb-20 overflow-hidden min-h-screen flex items-center">
      {/* Single ambient background — no competing blob stacks */}
      <div className="absolute inset-0 -z-10 pointer-events-none">
        <div className="absolute inset-0 grid-bg opacity-[0.025]" />
        <div
          className="absolute top-[-10%] left-[30%] w-[700px] h-[700px] rounded-full animate-pulse-slow"
          style={{
            background:
              "radial-gradient(circle, oklch(0.68 0.10 235 / 0.07) 0%, transparent 70%)",
          }}
        />
      </div>

      <div className="container mx-auto px-6">
        <div className="grid lg:grid-cols-[1fr_1fr] gap-16 xl:gap-24 items-center">

          {/* ── Left column: editorial headline ── */}
          <div className="max-w-xl">
            {/* Status line */}
            <div className="animate-fade-up mb-8 flex items-center gap-3">
              <div className="flex items-center gap-2 px-3 py-1.5 rounded-full border border-border/60 bg-card/50 backdrop-blur-sm">
                <span className="w-1.5 h-1.5 rounded-full bg-success animate-pulse" />
                <span className="text-[11px] font-mono text-muted-foreground">
                  Rust — Backend-authoritative runtime
                </span>
              </div>
            </div>

            {/* Main headline — staggered word reveal */}
            <h1 className="mb-8 leading-[1.04] font-bold tracking-tight">
              <span className="animate-fade-up-delay-1 block text-5xl sm:text-6xl lg:text-[3.75rem] xl:text-7xl text-foreground">
                AI agents that
              </span>
              <span className="animate-fade-up-delay-2 block text-5xl sm:text-6xl lg:text-[3.75rem] xl:text-7xl text-foreground mt-1">
                actually{" "}
                <span className="relative inline-block text-gradient-accent">
                  report back.
                  {/* Underline draw */}
                  <svg
                    className="absolute -bottom-1 left-0 w-full"
                    height="3"
                    viewBox="0 0 300 3"
                    preserveAspectRatio="none"
                    aria-hidden="true"
                  >
                    <path
                      d="M0 1.5 Q75 0 150 1.5 Q225 3 300 1.5"
                      stroke="url(#heroUnderline)"
                      strokeWidth="2"
                      fill="none"
                      className="animate-draw-line"
                    />
                    <defs>
                      <linearGradient id="heroUnderline" x1="0%" y1="0%" x2="100%" y2="0%">
                        <stop offset="0%" stopColor="oklch(0.68 0.14 235 / 0.8)" />
                        <stop offset="100%" stopColor="oklch(0.78 0.12 240 / 0.3)" />
                      </linearGradient>
                    </defs>
                  </svg>
                </span>
              </span>
            </h1>

            {/* Subheading */}
            <p className="animate-fade-up-delay-3 text-base sm:text-lg text-muted-foreground leading-relaxed mb-10 max-w-lg">
              Plan-first execution, explicit approval gates, and a full event
              audit trail — all running in a{" "}
              <span className="text-foreground font-medium font-mono">
                Rust backend
              </span>{" "}
              that owns every state transition.
            </p>

            {/* CTAs */}
            <div className="animate-fade-up-delay-4 flex flex-col sm:flex-row items-start gap-3 mb-12">
              <Button
                size="lg"
                className="group font-mono text-sm gap-2.5 px-7 h-12 relative overflow-hidden"
              >
                <Download className="w-4 h-4 shrink-0" />
                Download for macOS
                <div className="absolute inset-0 bg-gradient-to-r from-primary to-primary/80 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
              </Button>
              <Button
                variant="outline"
                size="lg"
                className="group font-mono text-sm gap-2 px-6 h-12 bg-card/50 backdrop-blur-sm border-border hover:border-primary/40 hover:bg-card/80 transition-all duration-200"
              >
                <Github className="w-4 h-4 shrink-0" />
                View on GitHub
                <ArrowRight className="w-3.5 h-3.5 ml-1 group-hover:translate-x-0.5 transition-transform duration-200" />
              </Button>
            </div>

            {/* Three stat callouts */}
            <div className="animate-fade-up-delay-5 flex items-center gap-8 pt-8 border-t border-border/30">
              <div className="stat-callout">
                <span className="stat-value">100%</span>
                <span className="stat-label">event-sourced state</span>
              </div>
              <div className="w-px h-8 bg-border/40" />
              <div className="stat-callout">
                <span className="stat-value">0</span>
                <span className="stat-label">silent side effects</span>
              </div>
              <div className="w-px h-8 bg-border/40" />
              <div className="stat-callout">
                <span className="stat-value">SQLite</span>
                <span className="stat-label">crash recovery</span>
              </div>
            </div>
          </div>

          {/* ── Right column: live event stream terminal ── */}
          <div className="animate-fade-in relative lg:block hidden">
            {/* Ambient glow behind terminal */}
            <div
              className="absolute -inset-8 rounded-3xl pointer-events-none"
              style={{
                background:
                  "radial-gradient(ellipse 80% 60% at 50% 50%, oklch(0.68 0.14 235 / 0.08) 0%, transparent 70%)",
              }}
            />

            <div className="terminal-window relative">
              {/* Titlebar */}
              <div className="terminal-titlebar">
                <div className="flex gap-1.5">
                  <div className="terminal-dot bg-red-500/75 w-3 h-3" />
                  <div className="terminal-dot bg-yellow-500/75 w-3 h-3" />
                  <div className="terminal-dot bg-green-500/75 w-3 h-3" />
                </div>
                <div className="flex-1 text-center">
                  <span className="text-[11px] text-muted-foreground/50 font-mono">
                    Orchestrix — Event Stream
                  </span>
                </div>
                <div className="flex items-center gap-1.5">
                  <span className="w-1.5 h-1.5 rounded-full bg-success/80 animate-pulse" />
                  <span className="text-[10px] font-mono text-success/70">live</span>
                </div>
              </div>

              {/* Event stream body — ref used for internal scroll only */}
              <div
                ref={streamBodyRef}
                className="terminal-body overflow-y-auto"
                style={{ minHeight: "380px", maxHeight: "420px" }}
              >
                {/* Header row */}
                <div className="flex items-center gap-2 mb-3 pb-2 border-b border-border/20">
                  <span className="text-[10px] font-mono text-muted-foreground/40 uppercase tracking-wider">
                    timestamp
                  </span>
                  <span className="text-[10px] font-mono text-muted-foreground/40 ml-8 uppercase tracking-wider">
                    event
                  </span>
                </div>
                <EventStream containerRef={streamBodyRef} />
              </div>

              {/* Bottom status bar */}
              <div className="flex items-center justify-between px-4 py-2 border-t border-border/20 bg-muted/20">
                <div className="flex items-center gap-3">
                  <span className="text-[10px] font-mono text-muted-foreground/50">run_4f9a</span>
                  <span className="text-muted-foreground/20 text-[10px]">·</span>
                  <span className="text-[10px] font-mono text-primary/60">task: refactor auth module</span>
                </div>
                <span className="text-[10px] font-mono text-muted-foreground/40">SQLite ✓</span>
              </div>
            </div>

            {/* Corner accent */}
            <div className="absolute -top-px -right-px w-6 h-6 border-t-2 border-r-2 border-primary/20 rounded-tr-xl" />
            <div className="absolute -bottom-px -left-px w-6 h-6 border-b-2 border-l-2 border-primary/20 rounded-bl-xl" />
          </div>
        </div>
      </div>
    </section>
  );
};

export default HeroSection;
