import { CheckCircle2, Eye, Play, FileText } from "lucide-react";
import { useRevealGroup } from "@/hooks/useReveal";
import { cn } from "@/lib/utils";

/* ─── Phase content mockups ─── */

const PlanMockup = () => (
  <div className="terminal-window text-[11px]">
    <div className="terminal-titlebar">
      <div className="flex gap-1.5">
        <div className="w-3 h-3 rounded-full bg-red-500/70" />
        <div className="w-3 h-3 rounded-full bg-yellow-500/70" />
        <div className="w-3 h-3 rounded-full bg-green-500/70" />
      </div>
      <span className="text-[10px] font-mono text-muted-foreground/40 ml-2">plan.md</span>
    </div>
    <div className="terminal-body p-4 space-y-2" style={{ minHeight: "160px" }}>
      <p className="text-primary/80 font-mono">
        <span className="text-muted-foreground/40">#</span> Execution Plan
      </p>
      <p className="text-muted-foreground/50 font-mono text-[10px]">
        Goal: refactor auth module
      </p>
      <div className="mt-3 space-y-1.5">
        {[
          "1. Read src/auth/index.ts",
          "2. Extract session logic → session.ts",
          "3. Update imports (7 files)",
          "4. Run bun test auth",
          "5. Create patch artifact",
        ].map((step, i) => (
          <div key={i} className="flex items-start gap-2 font-mono text-[10px]">
            <span className="text-primary/50 mt-px">›</span>
            <span className="text-foreground/70">{step}</span>
          </div>
        ))}
      </div>
    </div>
  </div>
);

const ReviewMockup = () => (
  <div className="terminal-window text-[11px]">
    <div className="terminal-titlebar">
      <div className="flex gap-1.5">
        <div className="w-3 h-3 rounded-full bg-red-500/70" />
        <div className="w-3 h-3 rounded-full bg-yellow-500/70" />
        <div className="w-3 h-3 rounded-full bg-green-500/70" />
      </div>
      <span className="text-[10px] font-mono text-muted-foreground/40 ml-2">Review — Plan</span>
      <span className="ml-auto phase-badge phase-badge--review text-[9px]">awaiting_review</span>
    </div>
    <div className="terminal-body p-4" style={{ minHeight: "160px" }}>
      <p className="text-foreground/60 font-mono text-[10px] mb-3">
        5 steps · estimated 3–5 tools · no destructive ops
      </p>
      <div className="space-y-2 mb-4">
        {["Read 1 file", "Write 2 files", "Update imports", "Run tests", "Create artifact"].map((step, i) => (
          <div key={i} className="flex items-center gap-2 font-mono text-[10px]">
            <CheckCircle2 className="w-3 h-3 text-success/70 shrink-0" />
            <span className="text-foreground/60">{step}</span>
          </div>
        ))}
      </div>
      <div className="flex gap-2 mt-4">
        <div className="px-3 py-1.5 rounded bg-primary/20 border border-primary/30 text-primary text-[10px] font-mono font-semibold cursor-pointer hover:bg-primary/30 transition-colors">
          Approve →
        </div>
        <div className="px-3 py-1.5 rounded bg-destructive/10 border border-destructive/20 text-destructive/70 text-[10px] font-mono cursor-pointer hover:bg-destructive/20 transition-colors">
          Reject
        </div>
      </div>
    </div>
  </div>
);

const ExecuteMockup = () => (
  <div className="terminal-window text-[11px]">
    <div className="terminal-titlebar">
      <div className="flex gap-1.5">
        <div className="w-3 h-3 rounded-full bg-red-500/70" />
        <div className="w-3 h-3 rounded-full bg-yellow-500/70" />
        <div className="w-3 h-3 rounded-full bg-green-500/70" />
      </div>
      <span className="text-[10px] font-mono text-muted-foreground/40 ml-2">run_4f9a</span>
      <span className="ml-auto flex items-center gap-1">
        <span className="w-1.5 h-1.5 rounded-full bg-info/80 animate-pulse" />
        <span className="text-[9px] font-mono text-info/70">executing</span>
      </span>
    </div>
    <div className="terminal-body p-4" style={{ minHeight: "160px" }}>
      <div className="space-y-1">
        {[
          { ev: "tool.call_started",  payload: 'fs.read "src/auth/index.ts"',    badge: "tool",      done: true },
          { ev: "tool.call_finished", payload: "bytes=3201",                     badge: "tool",      done: true },
          { ev: "tool.call_started",  payload: 'fs.write "src/auth/session.ts"', badge: "tool",      done: true },
          { ev: "tool.call_finished", payload: "bytes=1842",                     badge: "tool",      done: true },
          { ev: "tool.call_started",  payload: 'cmd.exec "bun test auth"',       badge: "tool",      done: false },
        ].map((row, i) => (
          <div key={i} className="event-row" style={{ opacity: row.done ? 1 : 0.55 }}>
            <span className={`event-badge event-badge--${row.badge}`}>{row.badge}</span>
            <span className="event-name text-[10px]">{row.ev}</span>
            <span className="event-payload text-[10px]">{row.payload}</span>
            {!row.done && (
              <span className="text-[9px] font-mono text-muted-foreground/40 ml-auto shrink-0">…</span>
            )}
          </div>
        ))}
      </div>
    </div>
  </div>
);

/* ─── Steps definition ─── */
const steps = [
  {
    phase: "01",
    badge: "phase-badge--planning",
    badgeLabel: "planning",
    icon: FileText,
    title: "Plan",
    description:
      "Before any action, the agent explores the workspace using read-only tools — listing files, reading source, running searches. When ready, it calls agent.create_artifact to submit a structured execution plan.",
    details: ["Multi-turn exploration", "Tool-assisted analysis", "Artifact-backed plan output"],
    mockup: <PlanMockup />,
  },
  {
    phase: "02",
    badge: "phase-badge--review",
    badgeLabel: "awaiting_review",
    icon: Eye,
    title: "Review",
    description:
      "Execution is gated by human approval. Inspect every step, reject unsafe operations, or send feedback to re-plan. Nothing executes until you confirm.",
    details: ["Inspect all steps", "Modify or reject plan", "Feedback loop to re-plan"],
    mockup: <ReviewMockup />,
  },
  {
    phase: "03",
    badge: "phase-badge--executing",
    badgeLabel: "executing",
    icon: Play,
    title: "Execute",
    description:
      "Tool-based execution with real-time streamed events. Every file write, command, and model decision is visible as it happens. Artifacts are reviewable before commit.",
    details: ["Real-time event streaming", "Full tool visibility", "Artifact review gate"],
    mockup: <ExecuteMockup />,
  },
];

const ExecutionModelSection = () => {
  const { ref: sectionRef, revealed } = useRevealGroup(0.08);

  return (
    <section
      id="execution-model"
      ref={sectionRef as React.RefObject<HTMLElement>}
      className="py-28 border-t border-border/20 relative overflow-hidden"
    >
      {/* Background */}
      <div className="absolute inset-0 grid-bg opacity-[0.018]" />

      <div className="container mx-auto px-6">
        {/* Header */}
        <div className={cn("mb-20 max-w-xl", "reveal reveal-delay-1", revealed && "revealed")}>
          <span className="section-label mb-3 block">Execution Model</span>
          <h2 className="section-heading mb-4">
            Plan → Review → Execute
          </h2>
          <p className="section-subheading">
            A deterministic pipeline. Agents propose, humans approve, tools execute. No hidden side effects.
          </p>
        </div>

        {/* Timeline steps — vertical on mobile, alternate layout on desktop */}
        <div className="relative max-w-6xl">
          {/* Vertical connector line for desktop */}
          <div
            className="hidden lg:block absolute left-[calc(50%-0.5px)] top-0 bottom-0 w-px"
            style={{
              background: "linear-gradient(to bottom, transparent, var(--border) 10%, var(--border) 90%, transparent)",
            }}
          />

          <div className="space-y-20 lg:space-y-32">
            {steps.map((step, i) => {
              const isRight = i % 2 === 0; // odd steps: text left, mockup right
              return (
                <div
                  key={step.phase}
                  className={cn(
                    "reveal",
                    i === 0 && "reveal-delay-2",
                    i === 1 && "reveal-delay-3",
                    i === 2 && "reveal-delay-4",
                    revealed && "revealed",
                    "grid lg:grid-cols-2 gap-10 lg:gap-16 items-center",
                    !isRight && "lg:[&>*:first-child]:order-2 lg:[&>*:last-child]:order-1"
                  )}
                >
                  {/* Text block */}
                  <div className="flex flex-col">
                    {/* Phase indicator */}
                    <div className="flex items-center gap-3 mb-6">
                      <div className="flex items-center justify-center w-10 h-10 rounded-xl bg-card border border-border shrink-0">
                        <step.icon className="w-5 h-5 text-primary" />
                      </div>
                      <div>
                        <span className="text-[10px] font-mono text-muted-foreground/50 uppercase tracking-widest block">
                          Step {step.phase}
                        </span>
                        <span className={`phase-badge ${step.badge} mt-0.5`}>
                          {step.badgeLabel}
                        </span>
                      </div>
                    </div>

                    <h3 className="text-2xl font-bold tracking-tight text-foreground mb-4 font-mono">
                      {step.title}
                    </h3>
                    <p className="text-sm text-muted-foreground leading-relaxed mb-6">
                      {step.description}
                    </p>

                    <ul className="space-y-2.5">
                      {step.details.map((detail) => (
                        <li key={detail} className="flex items-center gap-2.5 text-sm text-foreground/70">
                          <span className="w-1.5 h-1.5 rounded-full bg-primary/60 shrink-0" />
                          {detail}
                        </li>
                      ))}
                    </ul>
                  </div>

                  {/* Mockup block */}
                  <div className="relative">
                    {/* Soft glow */}
                    <div
                      className="absolute -inset-6 rounded-2xl pointer-events-none"
                      style={{
                        background:
                          "radial-gradient(ellipse at 50% 50%, oklch(0.68 0.10 235 / 0.06) 0%, transparent 70%)",
                      }}
                    />
                    {step.mockup}
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Bottom note */}
        <div className={cn("mt-20 flex items-center gap-3 text-xs font-mono text-muted-foreground/50", "reveal reveal-delay-5", revealed && "revealed")}>
          <CheckCircle2 className="w-3.5 h-3.5 text-success/50" />
          Every step is append-only, persisted to SQLite, and deterministically replayable.
        </div>
      </div>
    </section>
  );
};

export default ExecutionModelSection;
