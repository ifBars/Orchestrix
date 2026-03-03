import { Button } from "@/components/ui/button";
import { Download, Github, BookOpen, Sparkles } from "lucide-react";

const CTASection = () => {
  return (
    <section className="py-32 border-t border-border/20 relative overflow-hidden">
      {/* Dramatic background */}
      <div className="absolute inset-0 bg-gradient-to-b from-transparent via-primary/[0.03] to-transparent" />
      <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[600px] rounded-full bg-primary/[0.05] blur-[120px]" />
      
      {/* Grid overlay */}
      <div className="absolute inset-0 grid-bg opacity-[0.015]" />

      {/* Corner decorations */}
      <div className="absolute top-0 left-0 w-32 h-32 border-l border-t border-primary/10 rounded-tl-3xl" />
      <div className="absolute top-0 right-0 w-32 h-32 border-r border-t border-primary/10 rounded-tr-3xl" />
      <div className="absolute bottom-0 left-0 w-32 h-32 border-l border-b border-primary/10 rounded-bl-3xl" />
      <div className="absolute bottom-0 right-0 w-32 h-32 border-r border-b border-primary/10 rounded-br-3xl" />

      <div className="container relative mx-auto px-6">
        <div className="max-w-2xl mx-auto text-center">
          {/* Badge */}
          <div className="inline-flex items-center gap-2 px-4 py-1.5 rounded-full bg-card/60 backdrop-blur-sm border border-border/50 mb-8">
            <Sparkles className="w-3.5 h-3.5 text-primary" />
            <span className="text-xs font-mono text-muted-foreground">
              Built in Rust for performance
            </span>
          </div>

          <h2 className="text-4xl sm:text-5xl font-bold tracking-tight leading-tight mb-6">
            Stop Letting Agents
            <br />
            <span className="text-gradient-accent">Run Blind.</span>
          </h2>
          
          <p className="text-lg text-muted-foreground mx-auto mb-10 leading-relaxed">
            Take control of AI agent execution. Plan-first, human-approved, fully observable. Built in Rust for developers who need structure.
          </p>

          {/* CTA Buttons */}
          <div className="flex flex-col sm:flex-row items-center justify-center gap-4 mb-10">
            <Button
              size="lg"
              className="group relative overflow-hidden font-mono text-sm gap-2 px-10 h-14"
            >
              <span className="relative z-10 flex items-center gap-2">
                <Download className="w-4 h-4 group-hover:animate-bounce" />
                Download Orchestrix
              </span>
              <div className="absolute inset-0 bg-gradient-to-r from-primary via-primary/90 to-primary/70 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
            </Button>
            <Button
              variant="outline"
              size="lg"
              className="group font-mono text-sm gap-2 px-8 h-14 bg-card/50 backdrop-blur-sm hover:bg-card/80 transition-all"
            >
              <BookOpen className="w-4 h-4" />
              Read Documentation
            </Button>
          </div>

          {/* Platform badges */}
          <div className="flex items-center justify-center gap-6 text-xs text-muted-foreground/60 font-mono">
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-green-500/60" />
              macOS available
            </div>
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-yellow-500/60 animate-pulse" />
              Windows coming soon
            </div>
            <div className="flex items-center gap-2">
              <Github className="w-3.5 h-3.5" />
              Open source
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default CTASection;
