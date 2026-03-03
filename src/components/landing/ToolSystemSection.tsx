import { FolderOpen, TerminalSquare, GitBranch, Puzzle, ArrowRight, Shield, Clock, Search } from "lucide-react";

const tools = [
  {
    icon: FolderOpen,
    title: "File system",
    description: "Read, write, patch, and search files with structured diffs and rollback support.",
    gradient: "from-blue-500/20 to-cyan-500/20",
    features: ["Structured diffs", "Rollback support"],
  },
  {
    icon: TerminalSquare,
    title: "Command execution",
    description: "Sandboxed shell execution with timeout controls and output streaming.",
    gradient: "from-orange-500/20 to-red-500/20",
    features: ["Sandboxed execution", "Output streaming"],
  },
  {
    icon: GitBranch,
    title: "Git integration",
    description: "Branch, commit, diff, and worktree management built into the execution pipeline.",
    gradient: "from-green-500/20 to-emerald-500/20",
    features: ["Worktree isolation", "Commit signing"],
  },
  {
    icon: Puzzle,
    title: "Skills (MCP)",
    description: "Extensible skill modules compatible with the Model Context Protocol standard.",
    gradient: "from-purple-500/20 to-pink-500/20",
    features: ["MCP compatible", "Extensible modules"],
  },
];

const ToolSystemSection = () => {
  return (
    <section id="tools" className="py-28 border-t border-border/20 relative overflow-hidden">
      {/* Background */}
      <div className="absolute inset-0 grid-bg opacity-[0.02]" />

      <div className="container mx-auto px-6">
        <div className="text-center mb-20">
          <span className="section-label mb-4 inline-block">Tool System</span>
          <h2 className="section-heading mb-4">Structured tool execution</h2>
          <p className="section-subheading mx-auto">
            Agents interact with the environment through a typed tool interface. Every invocation is logged, reversible, and inspectable.
          </p>
        </div>

        <div className="grid sm:grid-cols-2 lg:grid-cols-4 gap-4 max-w-5xl mx-auto">
          {tools.map((tool) => (
            <div
              key={tool.title}
              className="group relative bg-card/40 backdrop-blur-sm border border-border/50 rounded-xl p-5 transition-all duration-500 hover:bg-card/80 hover:border-primary/30 hover:shadow-lg hover:shadow-primary/5"
            >
              {/* Top accent line */}
              <div className="absolute top-0 left-4 right-4 h-px bg-gradient-to-r from-transparent via-primary/30 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-500" />

              <div className={`inline-flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br ${tool.gradient} border border-border/50 mb-4 group-hover:scale-110 transition-transform duration-300`}>
                <tool.icon className="w-6 h-6 text-foreground/80" />
              </div>

              <h3 className="text-sm font-bold text-foreground mb-2 font-mono flex items-center gap-2">
                {tool.title}
                <ArrowRight className="w-3 h-3 text-muted-foreground/0 group-hover:text-muted-foreground group-hover:w-4 transition-all" />
              </h3>

              <p className="text-xs text-muted-foreground leading-relaxed mb-4">
                {tool.description}
              </p>

              {/* Feature tags */}
              <div className="flex flex-wrap gap-1.5">
                {tool.features.map((feature) => (
                  <span
                    key={feature}
                    className="px-2 py-0.5 rounded text-[10px] font-mono bg-muted/50 text-muted-foreground/80"
                  >
                    {feature}
                  </span>
                ))}
              </div>
            </div>
          ))}
        </div>

        {/* Permission system note */}
        <div className="mt-16 flex items-center justify-center gap-8 flex-wrap">
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Shield className="w-4 h-4 text-primary" />
            <span className="font-mono">Permission-gated</span>
          </div>
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Clock className="w-4 h-4 text-primary" />
            <span className="font-mono">Timeout controls</span>
          </div>
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Search className="w-4 h-4 text-primary" />
            <span className="font-mono">Full audit trail</span>
          </div>
        </div>
      </div>
    </section>
  );
};

export default ToolSystemSection;
