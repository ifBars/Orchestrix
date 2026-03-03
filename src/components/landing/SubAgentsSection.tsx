import { GitBranch, Users, Layers, AlertTriangle, CheckCircle, ArrowRight } from "lucide-react";

const features = [
  {
    title: "Task delegation",
    description: "Primary agents decompose work and delegate to specialized sub-agents with scoped context.",
    icon: Users,
    gradient: "from-blue-500/20 to-cyan-500/20",
  },
  {
    title: "Parallel execution",
    description: "Multiple sub-agents operate concurrently on independent subtasks.",
    icon: Layers,
    gradient: "from-purple-500/20 to-pink-500/20",
  },
  {
    title: "Git worktree isolation",
    description: "Each agent works in its own worktree. No merge conflicts during execution.",
    icon: GitBranch,
    gradient: "from-green-500/20 to-emerald-500/20",
  },
  {
    title: "Conflict detection",
    description: "Overlapping file modifications are detected and surfaced before merge.",
    icon: AlertTriangle,
    gradient: "from-orange-500/20 to-amber-500/20",
  },
];

const SubAgentsSection = () => {
  return (
    <section className="py-28 border-t border-border/20 relative overflow-hidden">
      {/* Background */}
      <div className="absolute inset-0 grid-bg opacity-[0.02]" />
      <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[500px] rounded-full bg-primary/[0.02] blur-[100px]" />

      <div className="container relative mx-auto px-6">
        <div className="max-w-3xl mx-auto">
          <span className="section-label mb-4 inline-block">Sub-Agents</span>
          <h2 className="section-heading mb-4">Delegation & worktree isolation</h2>
          <p className="section-subheading mb-12">
            Orchestrix delegates subtasks to specialized sub-agents that execute in isolated git worktrees. Parallel work without conflicts.
          </p>

          <div className="grid sm:grid-cols-2 gap-4">
            {features.map((feature) => (
              <div
                key={feature.title}
                className="group relative bg-card/40 backdrop-blur-sm border border-border/50 rounded-xl p-5 transition-all duration-500 hover:bg-card/80 hover:border-primary/30 hover:shadow-lg hover:shadow-primary/5"
              >
                <div className="flex items-start gap-4">
                  <div className={`flex-shrink-0 w-10 h-10 rounded-lg bg-gradient-to-br ${feature.gradient} border border-border/50 flex items-center justify-center group-hover:scale-110 transition-transform duration-300`}>
                    <feature.icon className="w-5 h-5 text-foreground/80" />
                  </div>
                  <div className="flex-1">
                    <h3 className="text-sm font-bold text-foreground mb-1.5 font-mono flex items-center gap-2">
                      {feature.title}
                      <ArrowRight className="w-3 h-3 text-muted-foreground/0 group-hover:text-muted-foreground group-hover:w-4 transition-all" />
                    </h3>
                    <p className="text-xs text-muted-foreground leading-relaxed">
                      {feature.description}
                    </p>
                  </div>
                </div>
              </div>
            ))}
          </div>

          {/* Lifecycle indicator */}
          <div className="mt-12 p-4 rounded-lg border border-border/50 bg-card/30 backdrop-blur-sm">
            <div className="flex items-center gap-2 mb-3">
              <CheckCircle className="w-3.5 h-3.5 text-primary" />
              <span className="text-xs font-mono text-foreground font-semibold">Sub-agent lifecycle</span>
            </div>
            <div className="flex items-center gap-2 text-xs font-mono text-muted-foreground overflow-x-auto pb-1">
              <span className="px-2 py-1 rounded bg-primary/10 text-primary">created</span>
              <ArrowRight className="w-3 h-3 flex-shrink-0" />
              <span className="px-2 py-1 rounded bg-muted">running</span>
              <ArrowRight className="w-3 h-3 flex-shrink-0" />
              <span className="px-2 py-1 rounded bg-muted">waiting_for_merge</span>
              <ArrowRight className="w-3 h-3 flex-shrink-0" />
              <span className="px-2 py-1 rounded bg-muted">completed</span>
              <ArrowRight className="w-3 h-3 flex-shrink-0" />
              <span className="px-2 py-1 rounded bg-muted">closed</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default SubAgentsSection;
