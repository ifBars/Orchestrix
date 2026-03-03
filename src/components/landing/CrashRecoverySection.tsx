import { Database, RotateCcw, FileSearch, Shield, CheckCircle } from "lucide-react";

const features = [
  {
    icon: Database,
    title: "SQLite persistence",
    description: "All events written to local SQLite. No external dependencies.",
    gradient: "from-blue-500/20 to-cyan-500/20",
  },
  {
    icon: RotateCcw,
    title: "Recoverable runs",
    description: "Resume interrupted executions from the last checkpoint.",
    gradient: "from-purple-500/20 to-pink-500/20",
  },
  {
    icon: FileSearch,
    title: "Event replay",
    description: "Reconstruct any run deterministically from its event log.",
    gradient: "from-green-500/20 to-emerald-500/20",
  },
];

const CrashRecoverySection = () => {
  return (
    <section className="py-24 border-t border-border/20 relative overflow-hidden">
      {/* Background */}
      <div className="absolute inset-0 grid-bg opacity-[0.02]" />
      <div className="absolute top-0 right-1/4 w-[400px] h-[400px] rounded-full bg-primary/[0.02] blur-[100px]" />

      <div className="container mx-auto px-6">
        <div className="max-w-3xl mx-auto">
          <div className="text-center mb-12">
            <span className="section-label mb-4 inline-block">Persistence</span>
            <h2 className="text-2xl sm:text-3xl font-bold tracking-tight text-foreground mb-4">
              Crash recovery & audit trail
            </h2>
            <p className="text-sm text-muted-foreground max-w-xl mx-auto leading-relaxed">
              Every event is persisted to SQLite. Runs are recoverable. State can be deterministically reconstructed from event history.
            </p>
          </div>

          <div className="grid sm:grid-cols-3 gap-4">
            {features.map((feature) => (
              <div
                key={feature.title}
                className="group relative bg-card/40 backdrop-blur-sm border border-border/50 rounded-xl p-5 transition-all duration-500 hover:bg-card/80 hover:border-primary/30 hover:shadow-lg hover:shadow-primary/5"
              >
                <div className={`inline-flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br ${feature.gradient} border border-border/50 mb-4 group-hover:scale-110 transition-transform duration-300`}>
                  <feature.icon className="w-6 h-6 text-foreground/80" />
                </div>
                <h3 className="text-sm font-bold text-foreground mb-2 font-mono">
                  {feature.title}
                </h3>
                <p className="text-xs text-muted-foreground leading-relaxed">
                  {feature.description}
                </p>
              </div>
            ))}
          </div>

          {/* Guarantees */}
          <div className="mt-12 p-5 rounded-xl border border-border/50 bg-card/30 backdrop-blur-sm">
            <div className="flex items-center gap-2 mb-4">
              <Shield className="w-4 h-4 text-primary" />
              <span className="text-sm font-semibold text-foreground font-mono">Recovery guarantees</span>
            </div>
            <div className="grid sm:grid-cols-3 gap-4">
              <div className="flex items-start gap-2">
                <CheckCircle className="w-4 h-4 text-primary flex-shrink-0 mt-0.5" />
                <span className="text-xs text-muted-foreground">No event loss on crash</span>
              </div>
              <div className="flex items-start gap-2">
                <CheckCircle className="w-4 h-4 text-primary flex-shrink-0 mt-0.5" />
                <span className="text-xs text-muted-foreground">Deterministic replay</span>
              </div>
              <div className="flex items-start gap-2">
                <CheckCircle className="w-4 h-4 text-primary flex-shrink-0 mt-0.5" />
                <span className="text-xs text-muted-foreground">Full audit capability</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default CrashRecoverySection;
