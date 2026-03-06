import { ClipboardCheck, FileSearch, Play } from "lucide-react";
import { useRevealGroup } from "@/hooks/useReveal";
import { cn } from "@/lib/utils";

const workflowSteps = [
  {
    icon: FileSearch,
    step: "01",
    title: "Plan with context",
    body: "The agent inspects the workspace first. Read-only exploration, file discovery, and plan output stay visible from the first turn.",
  },
  {
    icon: ClipboardCheck,
    step: "02",
    title: "Review before action",
    body: "Approve or redirect the plan before work starts. Artifacts and comments stay first-class instead of bolted on later.",
  },
  {
    icon: Play,
    step: "03",
    title: "Execute with receipts",
    body: "Tool calls, progress, and final artifacts stay inspectable as the run moves forward. No invisible background automation.",
  },
];

export default function WorkflowSection() {
  const { ref, revealed } = useRevealGroup(0.08);

  return (
    <section id="workflow" ref={ref as React.RefObject<HTMLElement>} className="landing-section">
      <div className="mx-auto w-full max-w-[1400px] px-6">
        <div className={cn("mb-10 max-w-2xl reveal", revealed && "revealed")}>
          <div className="section-label">Workflow</div>
          <h2 className="section-heading mt-3">The shortest path still goes through review.</h2>
          <p className="section-subheading mt-4">
            Orchestrix is opinionated about involving humans at the right time: early enough to prevent surprises,
            but without forcing you to wade through noise.
          </p>
        </div>

        <div className={cn("grid gap-4 lg:grid-cols-3 reveal reveal-delay-1", revealed && "revealed")}>
          {workflowSteps.map((step) => (
            <article key={step.step} className="landing-workflow-card">
              <div className="landing-workflow-card__top">
                <div className="landing-workflow-card__icon">
                  <step.icon size={18} />
                </div>
                <span className="landing-workflow-card__step">Step {step.step}</span>
              </div>
              <h3 className="mt-6 text-2xl font-semibold tracking-tight text-foreground">{step.title}</h3>
              <p className="mt-3 text-sm leading-relaxed text-muted-foreground">{step.body}</p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

