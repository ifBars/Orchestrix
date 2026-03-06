import { Database, Eye, ShieldCheck, Sparkles } from "lucide-react";
import { useRevealGroup } from "@/hooks/useReveal";
import { cn } from "@/lib/utils";

const proofItems = [
  {
    icon: ShieldCheck,
    label: "Approval-gated",
    body: "Plans stop for review before execution. Risky actions stay visible and interruptible.",
  },
  {
    icon: Eye,
    label: "Event-visible",
    body: "Model turns, tool preparation, calls, artifacts, and recoveries stay in one explorable timeline.",
  },
  {
    icon: Database,
    label: "Crash-recoverable",
    body: "SQLite-backed tasks and events make state reconstructable after restart.",
  },
  {
    icon: Sparkles,
    label: "App-adjacent",
    body: "Marketing surfaces reuse real shell components instead of drifting into generic mockups.",
  },
];

export default function ProofStripSection() {
  const { ref, revealed } = useRevealGroup(0.08);

  return (
    <section ref={ref as React.RefObject<HTMLElement>} className="landing-section pt-8">
      <div className={cn("mx-auto w-full max-w-[1400px] px-6 reveal", revealed && "revealed")}>
        <div className="landing-proof-strip">
          {proofItems.map((item) => (
            <div key={item.label} className="landing-proof-strip__item">
              <div className="landing-proof-strip__icon">
                <item.icon size={16} />
              </div>
              <div>
                <p className="text-sm font-semibold text-foreground">{item.label}</p>
                <p className="mt-1 text-sm leading-relaxed text-muted-foreground">{item.body}</p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

