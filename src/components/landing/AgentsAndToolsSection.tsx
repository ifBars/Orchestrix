import { GitBranch, Layers, Users, AlertTriangle, FolderOpen, TerminalSquare, Puzzle, Shield, Clock, Search } from "lucide-react";
import { useRevealGroup } from "@/hooks/useReveal";
import { cn } from "@/lib/utils";

/* ─── Sub-agent delegation diagram ─── */
function DelegationDiagram() {
  return (
    <div className="relative flex flex-col items-center gap-0 select-none">
      {/* Parent agent */}
      <div className="flex items-center gap-3 px-4 py-3 rounded-xl border border-primary/30 bg-primary/8 w-full max-w-xs relative z-10">
        <div className="w-8 h-8 rounded-lg bg-primary/15 flex items-center justify-center">
          <Users className="w-4 h-4 text-primary" />
        </div>
        <div>
          <p className="text-xs font-semibold font-mono text-foreground">Plan-mode Agent</p>
          <p className="text-[10px] font-mono text-muted-foreground/60">orchestrates</p>
        </div>
        <span className="ml-auto phase-badge phase-badge--planning text-[9px]">planning</span>
      </div>

      {/* Connector lines */}
      <div className="flex items-start justify-center w-full max-w-xs relative" style={{ height: "40px" }}>
        {/* Center vertical */}
        <div className="absolute top-0 left-1/2 -translate-x-1/2 w-px h-full bg-border/50" />
        {/* Horizontal spread */}
        <div className="absolute bottom-0 left-[20%] right-[20%] h-px bg-border/50" />
        {/* Left drop */}
        <div className="absolute bottom-0 left-[20%] w-px h-3 bg-border/50" />
        {/* Right drop */}
        <div className="absolute bottom-0 right-[20%] w-px h-3 bg-border/50" />
      </div>

      {/* Sub-agent row */}
      <div className="flex items-stretch gap-2 w-full">
        {[
          { label: "Worker A", sub: "auth refactor", phase: "phase-badge--executing", badge: "executing" },
          { label: "Worker B", sub: "test runner",   phase: "phase-badge--complete",  badge: "complete" },
        ].map((agent) => (
          <div
            key={agent.label}
            className="flex-1 flex flex-col gap-2 p-3 rounded-xl border border-border/50 bg-card/50"
          >
            <div className="flex items-center gap-2">
              <div className="w-6 h-6 rounded-md bg-muted/60 flex items-center justify-center">
                <Layers className="w-3 h-3 text-muted-foreground/60" />
              </div>
              <div>
                <p className="text-[10px] font-semibold font-mono text-foreground">{agent.label}</p>
                <p className="text-[9px] font-mono text-muted-foreground/50">{agent.sub}</p>
              </div>
            </div>
            <span className={`phase-badge ${agent.phase} text-[8px] self-start`}>{agent.badge}</span>
            {/* Worktree indicator */}
            <div className="flex items-center gap-1.5 mt-1 text-[9px] font-mono text-muted-foreground/40">
              <GitBranch className="w-2.5 h-2.5" />
              <span>worktree/{agent.label.toLowerCase().replace(" ", "-")}</span>
            </div>
          </div>
        ))}
      </div>

      {/* Merge node */}
      <div className="flex items-start justify-center w-full relative" style={{ height: "32px" }}>
        <div className="absolute top-0 left-1/4 right-1/4 h-px bg-border/50" />
        <div className="absolute top-0 left-1/4 w-px h-full bg-border/50" />
        <div className="absolute top-0 right-1/4 w-px h-full bg-border/50" />
        <div className="absolute bottom-0 left-1/2 -translate-x-1/2 w-px h-3 bg-border/50" />
      </div>

      <div className="flex items-center gap-2 px-3 py-2 rounded-lg border border-success/30 bg-success/8 text-[10px] font-mono">
        <span className="w-1.5 h-1.5 rounded-full bg-success" />
        <span className="text-success/80 font-semibold">waiting_for_merge</span>
        <span className="text-muted-foreground/40 ml-1">→ parent integrates</span>
      </div>
    </div>
  );
}

/* ─── Tool interface cards ─── */
const TOOLS = [
  {
    icon: FolderOpen,
    name: "fs",
    ops: ["read", "write", "list", "patch"],
    tags: ["Structured diffs", "Rollback"],
    desc: "Workspace-scoped filesystem access with structured diff output.",
  },
  {
    icon: TerminalSquare,
    name: "cmd",
    ops: ["exec", "stream"],
    tags: ["Sandboxed", "Timeout"],
    desc: "Sandboxed shell commands with configurable timeout and output streaming.",
  },
  {
    icon: GitBranch,
    name: "git",
    ops: ["status", "diff", "commit", "worktree"],
    tags: ["Worktree isolation", "Audit"],
    desc: "Branch, diff, and worktree management built into the execution pipeline.",
  },
  {
    icon: Puzzle,
    name: "skill",
    ops: ["invoke", "list"],
    tags: ["MCP-compatible", "Extensible"],
    desc: "MCP-compatible skill modules. External providers, no hardcoded integrations.",
  },
];

const AgentsAndToolsSection = () => {
  const { ref, revealed } = useRevealGroup(0.08);

  return (
    <section
      id="agents"
      ref={ref as React.RefObject<HTMLElement>}
      className="py-28 border-t border-border/20 relative overflow-hidden"
    >
      <div className="absolute inset-0 grid-bg opacity-[0.018]" />

      <div className="container mx-auto px-6">
        {/* ── Sub-agents block ── */}
        <div className="grid lg:grid-cols-2 gap-16 xl:gap-24 items-start mb-24">
          {/* Left: diagram */}
          <div className={cn("reveal", revealed && "revealed")}>
            <span className="section-label mb-3 block">Sub-Agents</span>
            <h2 className="section-heading mb-6">
              Delegation & worktree isolation
            </h2>
            <p className="section-subheading mb-8">
              Primary agents decompose work and delegate to specialized sub-agents running in isolated git worktrees. Conflicts are detected before merge — not after.
            </p>

            {/* Feature grid */}
            <div className="grid sm:grid-cols-2 gap-3">
              {[
                { icon: Users,         title: "Task delegation",      desc: "Scoped context, bounded tools" },
                { icon: Layers,        title: "Parallel execution",   desc: "Independent subtasks run concurrently" },
                { icon: GitBranch,     title: "Worktree isolation",   desc: "Each agent owns a separate branch" },
                { icon: AlertTriangle, title: "Conflict detection",   desc: "Overlapping edits surfaced pre-merge" },
              ].map((feat) => (
                <div
                  key={feat.title}
                  className="flex items-start gap-3 p-4 rounded-xl border border-border/50 bg-card/40 backdrop-blur-sm hover:bg-card/70 hover:border-primary/20 transition-all duration-200"
                >
                  <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-muted/50 shrink-0">
                    <feat.icon className="w-4 h-4 text-foreground/60" />
                  </div>
                  <div>
                    <p className="text-xs font-semibold text-foreground font-mono mb-0.5">{feat.title}</p>
                    <p className="text-xs text-muted-foreground">{feat.desc}</p>
                  </div>
                </div>
              ))}
            </div>

            {/* Lifecycle states */}
            <div className="mt-8 p-4 rounded-xl border border-border/40 bg-card/30">
              <p className="text-[10px] font-mono text-muted-foreground/50 uppercase tracking-wider mb-3">
                Sub-agent lifecycle
              </p>
              <div className="lifecycle-states gap-1">
                {[
                  { label: "created",           cls: "lifecycle-state--active" },
                  { label: "→",                 cls: "lifecycle-arrow" },
                  { label: "running",           cls: "lifecycle-state--active" },
                  { label: "→",                 cls: "lifecycle-arrow" },
                  { label: "waiting_for_merge", cls: "lifecycle-state--muted" },
                  { label: "→",                 cls: "lifecycle-arrow" },
                  { label: "completed",         cls: "lifecycle-state--complete" },
                  { label: "→",                 cls: "lifecycle-arrow" },
                  { label: "closed",            cls: "lifecycle-state--muted" },
                ].map((item, i) =>
                  item.cls === "lifecycle-arrow" ? (
                    <span key={i} className={item.cls}>{item.label}</span>
                  ) : (
                    <span key={i} className={`lifecycle-state ${item.cls}`}>{item.label}</span>
                  )
                )}
              </div>
            </div>
          </div>

          {/* Right: delegation diagram */}
          <div className={cn("reveal reveal-delay-2", revealed && "revealed")}>
            <div className="relative">
              <div
                className="absolute -inset-8 rounded-3xl pointer-events-none"
                style={{
                  background: "radial-gradient(ellipse 80% 60% at 50% 50%, oklch(0.68 0.10 235 / 0.05) 0%, transparent 70%)",
                }}
              />
              <div className="p-6 rounded-2xl border border-border/40 bg-card/30 backdrop-blur-sm">
                <p className="text-[10px] font-mono text-muted-foreground/50 uppercase tracking-wider mb-5">
                  Agent delegation model
                </p>
                <DelegationDiagram />
              </div>
            </div>
          </div>
        </div>

        {/* ── Tool system block ── */}
        <div className={cn("reveal reveal-delay-1", revealed && "revealed")}>
          <div className="mb-10 flex items-end justify-between flex-wrap gap-4">
            <div>
              <span className="section-label mb-3 block" id="tools">Tool System</span>
              <h2 className="text-2xl sm:text-3xl font-bold tracking-tight text-foreground font-mono">
                Typed. Audited. Reversible.
              </h2>
            </div>
            <div className="flex items-center gap-6 text-xs text-muted-foreground/60 font-mono">
              <div className="flex items-center gap-1.5">
                <Shield className="w-3.5 h-3.5 text-primary/60" />
                Permission-gated
              </div>
              <div className="flex items-center gap-1.5">
                <Clock className="w-3.5 h-3.5 text-primary/60" />
                Timeout-controlled
              </div>
              <div className="flex items-center gap-1.5">
                <Search className="w-3.5 h-3.5 text-primary/60" />
                Full audit trail
              </div>
            </div>
          </div>

          <div className="grid sm:grid-cols-2 xl:grid-cols-4 gap-3">
            {TOOLS.map((tool, i) => (
              <div
                key={tool.name}
                className={cn(
                  "group relative p-5 rounded-xl border border-border/50 bg-card/40 backdrop-blur-sm",
                  "hover:border-primary/25 hover:bg-card/70 transition-all duration-250",
                  "flex flex-col gap-4",
                  "reveal",
                  i === 0 && "reveal-delay-1",
                  i === 1 && "reveal-delay-2",
                  i === 2 && "reveal-delay-3",
                  i === 3 && "reveal-delay-4",
                  revealed && "revealed"
                )}
              >
                {/* Top accent line on hover */}
                <div className="absolute top-0 left-4 right-4 h-px bg-gradient-to-r from-transparent via-primary/40 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-300 rounded-full" />

                <div className="flex items-start justify-between">
                  <div className="flex items-center justify-center w-10 h-10 rounded-xl bg-muted/50 border border-border/50 group-hover:border-primary/20 transition-colors">
                    <tool.icon className="w-5 h-5 text-foreground/60" />
                  </div>
                  <code className="text-[10px] font-mono text-muted-foreground/40 bg-muted/40 px-2 py-0.5 rounded">
                    {tool.name}.*
                  </code>
                </div>

                <p className="text-xs text-muted-foreground leading-relaxed flex-1">
                  {tool.desc}
                </p>

                {/* Ops list */}
                <div className="flex flex-wrap gap-1">
                  {tool.ops.map((op) => (
                    <code
                      key={op}
                      className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-primary/8 text-primary/70 border border-primary/12"
                    >
                      .{op}
                    </code>
                  ))}
                </div>

                {/* Tags */}
                <div className="flex flex-wrap gap-1.5 pt-1 border-t border-border/30">
                  {tool.tags.map((tag) => (
                    <span
                      key={tag}
                      className="text-[9px] font-mono px-2 py-0.5 rounded-full bg-muted/40 text-muted-foreground/60"
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
};

export default AgentsAndToolsSection;
