import { Search, Terminal, FileCode, Clock, ChevronRight, Sparkles } from "lucide-react";

const capabilities = [
  {
    icon: Search,
    label: "Inspect decisions",
    description: "See why the agent chose each action",
    gradient: "from-blue-500/20 to-cyan-500/20",
  },
  {
    icon: Terminal,
    label: "Inspect tool calls",
    description: "Full input/output for every execution",
    gradient: "from-purple-500/20 to-pink-500/20",
  },
  {
    icon: FileCode,
    label: "Artifact review",
    description: "Review generated files before commit",
    gradient: "from-orange-500/20 to-amber-500/20",
  },
  {
    icon: Clock,
    label: "Condensed timeline",
    description: "Expandable detail at every step",
    gradient: "from-green-500/20 to-emerald-500/20",
  },
];

const VisibilitySection = () => {
  return (
    <section className="py-28 border-t border-border/20 relative overflow-hidden">
      {/* Background */}
      <div className="absolute inset-0 grid-bg opacity-[0.02]" />

      <div className="container mx-auto px-6">
        <div className="text-center mb-20">
          <span className="section-label mb-4 inline-block">Visibility</span>
          <h2 className="section-heading mb-4">Full execution visibility</h2>
          <p className="section-subheading mx-auto">
            Every decision, tool call, and artifact is logged and inspectable. No black-box execution.
          </p>
        </div>

        <div className="grid sm:grid-cols-2 lg:grid-cols-4 gap-4 max-w-5xl mx-auto">
          {capabilities.map((cap) => (
            <div
              key={cap.label}
              className="group relative bg-card/40 backdrop-blur-sm border border-border/50 rounded-xl p-5 transition-all duration-500 hover:bg-card/80 hover:border-primary/30 hover:shadow-lg hover:shadow-primary/5"
            >
              {/* Animated border gradient on hover */}
              <div className="absolute inset-0 rounded-xl bg-gradient-to-br from-primary/0 via-primary/5 to-primary/0 opacity-0 group-hover:opacity-100 transition-opacity duration-500 -z-10" />

              <div className={`inline-flex items-center justify-center w-10 h-10 rounded-lg bg-gradient-to-br ${cap.gradient} border border-border/50 mb-4`}>
                <cap.icon className="w-5 h-5 text-foreground/80" />
              </div>

              <h3 className="text-sm font-bold text-foreground mb-2 font-mono flex items-center gap-2">
                {cap.label}
                <ChevronRight className="w-3 h-3 text-muted-foreground/0 group-hover:text-muted-foreground group-hover:w-4 transition-all" />
              </h3>
              <p className="text-xs text-muted-foreground leading-relaxed">
                {cap.description}
              </p>
            </div>
          ))}
        </div>

        {/* Feature highlight */}
        <div className="mt-16 flex items-center justify-center">
          <div className="flex items-center gap-2 px-4 py-2 rounded-full bg-primary/5 border border-primary/20">
            <Sparkles className="w-3.5 h-3.5 text-primary" />
            <span className="text-xs font-mono text-muted-foreground">
              Every event is persisted to SQLite
            </span>
          </div>
        </div>
      </div>
    </section>
  );
};

export default VisibilitySection;
