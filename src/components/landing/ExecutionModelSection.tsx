import { FileText, Eye, Play, CheckCircle, ArrowRight, Clock } from "lucide-react";

const steps = [
  {
    phase: "01",
    title: "Plan",
    description: "Before any action, the agent generates a structured execution plan. Tasks are decomposed, dependencies mapped, and tool calls identified.",
    icon: FileText,
    details: ["Task decomposition", "Dependency mapping", "Tool identification"],
  },
  {
    phase: "02",
    title: "Review",
    description: "Human approval gate. Inspect the plan, modify steps, reject unsafe operations. Nothing executes without explicit confirmation.",
    icon: Eye,
    details: ["Inspect all steps", "Modify or reject", "Explicit confirmation"],
  },
  {
    phase: "03",
    title: "Execute",
    description: "Tool-based execution with real-time streamed events. Every file write, command, and decision is visible as it happens.",
    icon: Play,
    details: ["Real-time streaming", "Full visibility", "Artifact review"],
  },
];

const ExecutionModelSection = () => {
  return (
    <section id="execution-model" className="py-28 border-t border-border/20 relative overflow-hidden">
      {/* Background decoration */}
      <div className="absolute inset-0 grid-bg opacity-[0.02]" />
      <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[1000px] h-[600px] rounded-full bg-primary/[0.02] blur-[100px]" />

      <div className="container relative mx-auto px-6">
        <div className="text-center mb-20">
          <span className="section-label mb-4 inline-block">Execution Model</span>
          <h2 className="section-heading mb-4">Plan → Review → Execute</h2>
          <p className="section-subheading mx-auto">
            A deterministic execution pipeline. Agents propose, humans approve, tools execute. No hidden side effects.
          </p>
        </div>

        <div className="relative max-w-5xl mx-auto">
          {/* Connecting line */}
          <div className="hidden lg:block absolute top-1/2 left-0 right-0 h-px bg-gradient-to-r from-transparent via-border to-transparent -translate-y-1/2 z-0" />

          <div className="grid lg:grid-cols-3 gap-8 lg:gap-12 relative">
            {steps.map((step, i) => (
              <div key={step.title} className="relative group">
                {/* Card */}
                <div className="relative h-full bg-card/50 backdrop-blur-sm border border-border/50 rounded-xl p-8 transition-all duration-500 group-hover:border-primary/40 group-hover:bg-card/80 group-hover:shadow-lg group-hover:shadow-primary/5">
                  {/* Step number */}
                  <div className="absolute -top-3 left-8 px-3 py-0.5 rounded-full bg-background border border-border/50 text-xs font-mono text-primary">
                    {step.phase}
                  </div>

                  {/* Icon */}
                  <div className="relative mb-8 mt-4">
                    <div className="inline-flex items-center justify-center w-14 h-14 rounded-xl bg-primary/10 border border-primary/20 group-hover:border-primary/40 group-hover:bg-primary/15 transition-all duration-300">
                      <step.icon className="w-6 h-6 text-primary" />
                    </div>
                    {/* Glow on hover */}
                    <div className="absolute inset-0 rounded-xl bg-primary/20 blur-xl opacity-0 group-hover:opacity-100 transition-opacity duration-500 -z-10" />
                  </div>

                  {/* Title */}
                  <h3 className="text-lg font-bold text-foreground mb-3 font-mono tracking-tight">
                    {step.title}
                  </h3>

                  {/* Description */}
                  <p className="text-sm text-muted-foreground leading-relaxed mb-4">
                    {step.description}
                  </p>

                  {/* Details list */}
                  <ul className="space-y-3 mt-6">
                    {step.details.map((detail) => (
                      <li key={detail} className="flex items-center gap-2.5 text-xs text-muted-foreground/80">
                        <CheckCircle className="w-3.5 h-3.5 text-primary/60 flex-shrink-0" />
                        {detail}
                      </li>
                    ))}
                  </ul>

                  {/* Connector arrow for desktop */}
                  {i < 2 && (
                    <div className="hidden lg:flex absolute top-1/2 -right-6 lg:-right-10 xl:-right-12 -translate-y-1/2 z-10">
                      <div className="w-10 h-10 rounded-full bg-card border border-border flex items-center justify-center group-hover:border-primary/30 group-hover:bg-primary/5 transition-colors">
                        <ArrowRight className="w-4 h-4 text-muted-foreground group-hover:text-primary transition-colors" />
                      </div>
                    </div>
                  )}
                </div>

                {/* Mobile connector */}
                {i < 2 && (
                  <div className="lg:hidden flex items-center justify-center my-2">
                    <ArrowRight className="w-4 h-4 text-muted-foreground/40" />
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>

        {/* Bottom note */}
        <div className="mt-16 flex items-center justify-center gap-2 text-xs text-muted-foreground/60 font-mono">
          <Clock className="w-3.5 h-3.5" />
          <span>Each step is logged and recoverable</span>
        </div>
      </div>
    </section>
  );
};

export default ExecutionModelSection;
