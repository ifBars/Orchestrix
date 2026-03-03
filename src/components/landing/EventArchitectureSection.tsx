import { Layers, Cpu, Terminal, Database, ArrowRight, Activity } from "lucide-react";

const blocks = [
  {
    label: "Frontend",
    sublabel: "React",
    description: "Renders state. Never controls execution.",
    icon: Layers,
    gradient: "from-blue-500/20 to-cyan-500/20",
  },
  {
    label: "Backend",
    sublabel: "Rust Orchestrator",
    description: "Owns all state, decisions, and event flow.",
    icon: Cpu,
    gradient: "from-purple-500/20 to-pink-500/20",
  },
  {
    label: "Tools",
    sublabel: "FS / Git / Shell",
    description: "Sandboxed execution in isolated worktrees.",
    icon: Terminal,
    gradient: "from-orange-500/20 to-red-500/20",
  },
  {
    label: "Storage",
    sublabel: "SQLite",
    description: "Full event log. Crash recovery. Audit trail.",
    icon: Database,
    gradient: "from-green-500/20 to-emerald-500/20",
  },
];

const EventArchitectureSection = () => {
  return (
    <section id="architecture" className="py-28 border-t border-border/20 relative overflow-hidden">
      {/* Background */}
      <div className="absolute inset-0 grid-bg opacity-[0.02]" />
      <div className="absolute top-0 right-0 w-[500px] h-[500px] rounded-full bg-primary/[0.03] blur-[120px]" />
      <div className="absolute bottom-0 left-0 w-[400px] h-[400px] rounded-full bg-primary/[0.02] blur-[100px]" />

      <div className="container relative mx-auto px-6">
        <div className="text-center mb-20">
          <span className="section-label mb-4 inline-block">Architecture</span>
          <h2 className="section-heading mb-4">Event-driven. Backend-authoritative.</h2>
          <p className="section-subheading mx-auto">
            The Rust backend owns all orchestration state. The frontend is a pure renderer. Events stream one-way. No hidden mutations.
          </p>
        </div>

        <div className="max-w-4xl mx-auto">
          {/* Architecture diagram */}
          <div className="relative">
            {/* Central event bus */}
            <div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 z-20">
              <div className="flex items-center justify-center w-28 h-28 rounded-2xl bg-card/80 backdrop-blur-xl border border-primary/30 shadow-lg shadow-primary/10">
                <div className="text-center">
                  <Activity className="w-7 h-7 text-primary mx-auto mb-2 animate-pulse" />
                  <span className="text-[11px] font-mono text-muted-foreground">Event</span>
                  <br />
                  <span className="text-[11px] font-mono text-muted-foreground">Bus</span>
                </div>
              </div>
            </div>

            {/* Connection lines */}
            <svg className="absolute inset-0 w-full h-full pointer-events-none z-10" style={{ minHeight: '400px' }}>
              <defs>
                <linearGradient id="lineGradient" x1="0%" y1="0%" x2="100%" y2="0%">
                  <stop offset="0%" stopColor="oklch(0.68 0.14 235 / 0.3)" />
                  <stop offset="50%" stopColor="oklch(0.68 0.14 235 / 0.1)" />
                  <stop offset="100%" stopColor="oklch(0.68 0.14 235 / 0.3)" />
                </linearGradient>
              </defs>
              {/* Lines from each block to center */}
              <line x1="25%" y1="28%" x2="42%" y2="42%" stroke="url(#lineGradient)" strokeWidth="1" />
              <line x1="75%" y1="28%" x2="58%" y2="42%" stroke="url(#lineGradient)" strokeWidth="1" />
              <line x1="25%" y1="72%" x2="42%" y2="58%" stroke="url(#lineGradient)" strokeWidth="1" />
              <line x1="75%" y1="72%" x2="58%" y2="58%" stroke="url(#lineGradient)" strokeWidth="1" />
            </svg>

            {/* Blocks grid */}
            <div className="grid grid-cols-2 gap-6 lg:gap-8">
              {blocks.map((block, i) => (
                <div
                  key={block.label}
                  className={`relative group ${
                    i === 0 || i === 1 ? "mb-8" : ""
                  }`}
                >
                  <div className="h-full bg-card/50 backdrop-blur-sm border border-border/50 rounded-xl p-6 transition-all duration-500 group-hover:border-primary/30 group-hover:bg-card/80">
                    <div className="flex items-start gap-4">
                      <div className={`flex-shrink-0 w-12 h-12 rounded-lg bg-gradient-to-br ${block.gradient} border border-border/50 flex items-center justify-center`}>
                        <block.icon className="w-5 h-5 text-foreground/80" />
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="text-[10px] font-mono uppercase tracking-wider text-primary mb-1">
                          {block.sublabel}
                        </div>
                        <h3 className="text-sm font-bold text-foreground mb-1.5 font-mono">
                          {block.label}
                        </h3>
                        <p className="text-xs text-muted-foreground leading-relaxed">
                          {block.description}
                        </p>
                      </div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Code flow example */}
          <div className="mt-12 text-center">
            <div className="inline-flex items-center gap-3 px-5 py-3 rounded-lg border border-border/50 bg-card/50 backdrop-blur-sm">
              <div className="flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-primary animate-pulse" />
                <span className="text-xs font-mono text-muted-foreground">event_bus::stream →</span>
              </div>
              <ArrowRight className="w-3 h-3 text-muted-foreground/50" />
              <span className="text-xs font-mono text-foreground">frontend::render(state)</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default EventArchitectureSection;
