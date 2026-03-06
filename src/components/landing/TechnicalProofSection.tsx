import { Cpu, Database, Network, ShieldCheck, Workflow } from "lucide-react";
import { useRevealGroup } from "@/hooks/useReveal";
import { cn } from "@/lib/utils";

const proofCards = [
  {
    icon: Cpu,
    title: "Rust backend",
    body: "Orchestration, state, and execution stay backend-authoritative instead of leaking into frontend control flow.",
  },
  {
    icon: ShieldCheck,
    title: "Explicit approval gates",
    body: "Approval surfaces are first-class. Risky actions can pause for human review without ambiguity.",
  },
  {
    icon: Workflow,
    title: "Append-only visibility",
    body: "Events map cleanly to user-facing state so the timeline can explain what happened and why.",
  },
  {
    icon: Database,
    title: "SQLite recovery",
    body: "Tasks, runs, artifacts, and events remain reconstructable after crashes or restarts.",
  },
  {
    icon: Network,
    title: "Multi-provider ready",
    body: "Planner and worker flows remain model-agnostic across OpenAI-compatible and other supported providers.",
  },
];

const eventSample = [
  "agent.deciding",
  "agent.tool_calls_preparing",
  "tool.call_started",
  "tool.call_finished",
  "artifact.created",
  "agent.plan_ready",
];

export default function TechnicalProofSection() {
  const { ref, revealed } = useRevealGroup(0.08);

  return (
    <section id="proof" ref={ref as React.RefObject<HTMLElement>} className="landing-section">
      <div className="mx-auto w-full max-w-[1400px] px-6">
        <div className={cn("landing-proof-band reveal", revealed && "revealed")}>
          <div>
            <div className="section-label">Technical proof</div>
            <h2 className="section-heading mt-3 max-w-[12ch]">Premium chrome, serious runtime assumptions.</h2>
            <p className="section-subheading mt-4 max-w-xl">
              The page can look like a premium IDE product surface because the product underneath already has a disciplined
              execution model: review checkpoints, event truth, and a backend that owns orchestration.
            </p>
          </div>

          <div className="grid gap-4 lg:grid-cols-[1.2fr_0.8fr]">
            <div className="grid gap-3 md:grid-cols-2">
              {proofCards.map((card) => (
                <article key={card.title} className="landing-proof-card">
                  <div className="landing-proof-card__icon">
                    <card.icon size={16} />
                  </div>
                  <h3 className="mt-5 text-lg font-semibold tracking-tight text-foreground">{card.title}</h3>
                  <p className="mt-2 text-sm leading-relaxed text-muted-foreground">{card.body}</p>
                </article>
              ))}
            </div>

            <div className="landing-event-sample">
              <div>
                <p className="text-[10px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">Event contract</p>
                <h3 className="mt-3 text-xl font-semibold tracking-tight text-foreground">Visibility that scales with runtime complexity.</h3>
              </div>
              <div className="mt-6 space-y-2">
                {eventSample.map((eventName) => (
                  <div key={eventName} className="landing-event-sample__row">
                    <span className="landing-event-sample__dot" />
                    <code>{eventName}</code>
                  </div>
                ))}
              </div>
              <p className="mt-6 text-sm leading-relaxed text-muted-foreground">
                The shell stays calm because verbose detail is progressive, not hidden. That is the same rule guiding the product and the landing page.
              </p>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

