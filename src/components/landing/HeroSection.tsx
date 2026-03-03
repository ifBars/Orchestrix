import { Button } from "@/components/ui/button";
import { ArrowRight, Download, Sparkles, Terminal, Shield, Zap } from "lucide-react";
import heroMockup from "@/assets/hero-mockup.png";

const features = [
  { icon: Shield, label: "Human-in-the-loop", description: "Every action requires approval" },
  { icon: Terminal, label: "Full visibility", description: "Every tool call is logged" },
  { icon: Zap, label: "Crash recovery", description: "State reconstructed from events" },
];

const HeroSection = () => {
  return (
    <section className="relative pt-32 pb-20 overflow-hidden min-h-[90vh] flex items-center">
      {/* Animated background */}
      <div className="absolute inset-0 -z-10">
        <div className="absolute inset-0 grid-bg opacity-[0.03]" />
        <div className="absolute top-0 left-1/4 w-[600px] h-[600px] rounded-full bg-primary/5 blur-[150px] animate-pulse-slow" />
        <div className="absolute bottom-0 right-1/4 w-[400px] h-[400px] rounded-full bg-primary/3 blur-[120px] animate-pulse-slow-delayed" />
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[800px] rounded-full bg-gradient-radial from-primary/5 to-transparent opacity-50" />
      </div>

      {/* Floating particles */}
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        {[...Array(6)].map((_, i) => (
          <div
            key={i}
            className="absolute w-1 h-1 bg-primary/30 rounded-full animate-float"
            style={{
              left: `${15 + i * 15}%`,
              top: `${20 + (i % 3) * 25}%`,
              animationDelay: `${i * 0.8}s`,
              animationDuration: `${4 + i * 0.5}s`,
            }}
          />
        ))}
      </div>

      <div className="container relative mx-auto px-6">
        <div className="max-w-4xl mx-auto text-center">
          {/* Badge */}
          <div className="animate-fade-up mb-8">
            <div className="inline-flex items-center gap-2 px-4 py-1.5 rounded-full bg-card/60 backdrop-blur-sm border border-border/50 shadow-sm hover:border-primary/30 transition-colors group cursor-pointer">
              <Sparkles className="w-3.5 h-3.5 text-primary" />
              <span className="text-xs font-mono text-muted-foreground group-hover:text-foreground transition-colors">
                Backend-authoritative AI runtime
              </span>
            </div>
          </div>

          {/* Main heading with gradient */}
          <h1 className="animate-fade-up-delay-1 text-5xl sm:text-6xl lg:text-7xl font-bold tracking-tight leading-[1.05] mb-8">
            <span className="text-foreground">Run AI Agents</span>
            <br />
            <span className="text-gradient-accent relative">
              With Structure.
              <svg
                className="absolute -bottom-2 left-0 w-full h-1 text-primary"
                viewBox="0 0 200 4"
                preserveAspectRatio="none"
              >
                <path
                  d="M0 2C30 2 60 0 100 0C140 0 170 2 200 2"
                  stroke="currentColor"
                  strokeWidth="2"
                  fill="none"
                  className="animate-draw-line"
                />
              </svg>
            </span>
          </h1>

          {/* Subheading */}
          <p className="animate-fade-up-delay-2 text-lg sm:text-xl text-muted-foreground max-w-2xl mx-auto mb-10 leading-relaxed">
            Orchestrix is a backend-authoritative desktop runtime for AI agents.{" "}
            <span className="text-foreground font-medium">Plan-first execution</span>, explicit approval gates, full event visibility, and crash-recoverable orchestration — built in Rust.
          </p>

          {/* CTA Buttons */}
          <div className="animate-fade-up-delay-3 flex flex-col sm:flex-row items-center justify-center gap-4 mb-16">
            <Button
              size="lg"
              className="group relative overflow-hidden font-mono text-sm gap-2 px-8 h-12"
            >
              <span className="relative z-10 flex items-center gap-2">
                <Download className="w-4 h-4 group-hover:animate-bounce" />
                Download
              </span>
              <div className="absolute inset-0 bg-gradient-to-r from-primary via-primary/90 to-primary/70 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
            </Button>
            <Button
              variant="outline"
              size="lg"
              className="font-mono text-sm gap-2 px-8 h-12 bg-card/50 backdrop-blur-sm hover:bg-card/80 transition-all"
            >
              View Architecture
              <ArrowRight className="w-4 h-4 group-hover:translate-x-1 transition-transform" />
            </Button>
          </div>

          {/* Feature pills */}
          <div className="animate-fade-up-delay-4 flex flex-wrap items-center justify-center gap-3 mb-16">
            {features.map((feature) => (
              <div
                key={feature.label}
                className="flex items-center gap-2 px-4 py-2 rounded-lg bg-card/40 backdrop-blur-sm border border-border/30 hover:border-primary/20 transition-all duration-300 hover:shadow-sm"
              >
                <feature.icon className="w-4 h-4 text-primary" />
                <div className="text-left">
                  <span className="block text-xs font-semibold text-foreground font-mono">
                    {feature.label}
                  </span>
                  <span className="block text-[10px] text-muted-foreground">
                    {feature.description}
                  </span>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Mockup with enhanced visuals */}
        <div className="animate-fade-up-delay-4 mt-8">
          <div className="relative mx-auto max-w-5xl">
            {/* Glow effect behind mockup */}
            <div className="absolute -inset-4 bg-gradient-to-r from-primary/10 via-primary/5 to-primary/10 rounded-2xl blur-2xl opacity-50" />

            <div className="relative rounded-xl overflow-hidden border border-border/50 bg-card shadow-2xl shadow-black/20">
              {/* Window controls */}
              <div className="flex items-center gap-2 px-4 py-3 border-b border-border/50 bg-muted/30">
                <div className="flex gap-1.5">
                  <div className="w-3 h-3 rounded-full bg-red-500/80" />
                  <div className="w-3 h-3 rounded-full bg-yellow-500/80" />
                  <div className="w-3 h-3 rounded-full bg-green-500/80" />
                </div>
                <div className="flex-1 text-center">
                  <span className="text-[11px] text-muted-foreground/60 font-mono">
                    Orchestrix — Agent Runtime
                  </span>
                </div>
                <div className="w-10" />
              </div>

              {/* Mockup image */}
              <div className="relative">
                <img
                  src={heroMockup}
                  alt="Orchestrix desktop runtime showing plan-first AI agent execution with review gates"
                  className="w-full"
                  loading="eager"
                />
                {/* Gradient overlay at bottom */}
                <div className="absolute bottom-0 left-0 right-0 h-20 bg-gradient-to-t from-background to-transparent pointer-events-none" />
              </div>
            </div>

            {/* Decorative corner accents */}
            <div className="absolute -top-px -left-px w-8 h-8 border-l-2 border-t-2 border-primary/30 rounded-tl-lg" />
            <div className="absolute -top-px -right-px w-8 h-8 border-r-2 border-t-2 border-primary/30 rounded-tr-lg" />
            <div className="absolute -bottom-px -left-px w-8 h-8 border-l-2 border-b-2 border-primary/30 rounded-bl-lg" />
            <div className="absolute -bottom-px -right-px w-8 h-8 border-r-2 border-b-2 border-primary/30 rounded-br-lg" />
          </div>
        </div>
      </div>
    </section>
  );
};

export default HeroSection;
