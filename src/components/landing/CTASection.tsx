import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Download, BookOpen, Github, ArrowRight } from "lucide-react";
import { useRevealGroup } from "@/hooks/useReveal";
import { cn } from "@/lib/utils";

/* ─── Animated terminal prompt ─── */
const COMMANDS = [
  "orchestrix run --plan-first task.md",
  "orchestrix review run_4f9a",
  "orchestrix replay --from-checkpoint",
  "orchestrix agent status --watch",
];

function TerminalPrompt() {
  const [cmdIndex, setCmdIndex] = useState(0);
  const [displayed, setDisplayed] = useState("");
  const [typing, setTyping] = useState(true);

  useEffect(() => {
    const full = COMMANDS[cmdIndex];
    if (typing) {
      if (displayed.length < full.length) {
        const t = setTimeout(() => {
          setDisplayed(full.slice(0, displayed.length + 1));
        }, 45 + Math.random() * 30);
        return () => clearTimeout(t);
      } else {
        // Pause before erasing
        const t = setTimeout(() => setTyping(false), 2200);
        return () => clearTimeout(t);
      }
    } else {
      if (displayed.length > 0) {
        const t = setTimeout(() => {
          setDisplayed((d) => d.slice(0, -1));
        }, 22);
        return () => clearTimeout(t);
      } else {
        setCmdIndex((i) => (i + 1) % COMMANDS.length);
        setTyping(true);
      }
    }
  }, [displayed, typing, cmdIndex]);

  return (
    <div
      className="flex items-center gap-3 px-5 py-3 rounded-xl border border-border/30 bg-card/20 backdrop-blur-sm font-mono text-sm"
      aria-live="polite"
      aria-label={`Terminal command: ${COMMANDS[cmdIndex]}`}
    >
      <span className="text-primary/60 shrink-0">$</span>
      <span className="text-foreground/90">{displayed}</span>
      <span className="inline-block w-1.5 h-4 bg-primary/70 animate-cursor-blink ml-0.5 align-middle" />
    </div>
  );
}

const CTASection = () => {
  const { ref, revealed } = useRevealGroup(0.1);

  return (
    <section
      className="py-32 relative overflow-hidden"
      ref={ref as React.RefObject<HTMLElement>}
      style={{
        background: "linear-gradient(180deg, transparent 0%, oklch(0.08 0.012 260) 15%, oklch(0.08 0.012 260) 85%, transparent 100%)",
      }}
    >
      {/* Background grid — slightly stronger in this dark section */}
      <div className="absolute inset-0 grid-bg opacity-[0.04]" />

      {/* Ambient glow */}
      <div
        className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[500px] pointer-events-none"
        style={{
          background: "radial-gradient(ellipse at center, oklch(0.68 0.12 235 / 0.08) 0%, transparent 65%)",
        }}
      />

      {/* Subtle horizontal scan line */}
      <div
        className="absolute left-0 right-0 h-px pointer-events-none"
        style={{
          top: "50%",
          background: "linear-gradient(90deg, transparent 0%, oklch(0.68 0.12 235 / 0.12) 30%, oklch(0.68 0.12 235 / 0.12) 70%, transparent 100%)",
        }}
      />

      <div className="container mx-auto px-6 relative">
        <div className="max-w-2xl mx-auto text-center">

          {/* Eyebrow */}
          <div className={cn("mb-8", "reveal", revealed && "revealed")}>
            <div className="inline-flex items-center gap-2 px-3 py-1.5 rounded-full border border-border/40 bg-card/20">
              <span className="w-1.5 h-1.5 rounded-full bg-success/80 animate-pulse" />
              <span className="text-[11px] font-mono text-muted-foreground/70">
                Open source — MIT License
              </span>
            </div>
          </div>

          {/* Headline */}
          <h2
            className={cn(
              "text-4xl sm:text-5xl lg:text-6xl font-bold tracking-tight leading-[1.05] mb-6",
              "reveal reveal-delay-1",
              revealed && "revealed"
            )}
          >
            Stop letting agents
            <br />
            <span className="text-gradient-accent">run blind.</span>
          </h2>

          <p
            className={cn(
              "text-base sm:text-lg text-muted-foreground leading-relaxed mb-8 max-w-xl mx-auto",
              "reveal reveal-delay-2",
              revealed && "revealed"
            )}
          >
            Plan-first execution. Explicit approval gates. Full event visibility. A Rust backend that never hides what it's doing.
          </p>

          {/* Terminal command */}
          <div
            className={cn(
              "mb-10 max-w-lg mx-auto",
              "reveal reveal-delay-3",
              revealed && "revealed"
            )}
          >
            <TerminalPrompt />
          </div>

          {/* CTAs */}
          <div
            className={cn(
              "flex flex-col sm:flex-row items-center justify-center gap-3 mb-12",
              "reveal reveal-delay-3",
              revealed && "revealed"
            )}
          >
            <Button
              size="lg"
              className="group font-mono text-sm gap-2.5 px-8 h-13 w-full sm:w-auto"
            >
              <Download className="w-4 h-4 shrink-0" />
              Download Orchestrix
              <ArrowRight className="w-3.5 h-3.5 group-hover:translate-x-0.5 transition-transform duration-200" />
            </Button>
            <Button
              variant="outline"
              size="lg"
              className="group font-mono text-sm gap-2 px-7 h-13 w-full sm:w-auto border-border/50 bg-card/20 hover:bg-card/40 hover:border-border transition-all backdrop-blur-sm"
            >
              <BookOpen className="w-4 h-4 shrink-0" />
              Read the docs
            </Button>
            <a
              href="https://github.com/orchestrix"
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-2 px-5 h-13 rounded-lg border border-border/40 bg-card/10 hover:bg-card/30 transition-all text-sm font-mono text-muted-foreground hover:text-foreground w-full sm:w-auto justify-center"
            >
              <Github className="w-4 h-4" />
              GitHub
            </a>
          </div>

          {/* Platform badges */}
          <div
            className={cn(
              "flex flex-wrap items-center justify-center gap-6 text-xs font-mono text-muted-foreground/40",
              "reveal reveal-delay-4",
              revealed && "revealed"
            )}
          >
            <div className="flex items-center gap-2">
              <span className="w-1.5 h-1.5 rounded-full bg-success/60" />
              macOS available
            </div>
            <span className="text-muted-foreground/20">·</span>
            <div className="flex items-center gap-2">
              <span className="w-1.5 h-1.5 rounded-full bg-warning/60 animate-pulse" />
              Windows coming soon
            </div>
            <span className="text-muted-foreground/20">·</span>
            <div className="flex items-center gap-2">
              <span className="w-1.5 h-1.5 rounded-full bg-primary/60" />
              Built in Rust
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default CTASection;
