import { useRevealGroup } from "@/hooks/useReveal";
import { cn } from "@/lib/utils";
import { PreviewWorkbench } from "@/components/landing/preview/PreviewWorkbench";

export default function PreviewSection() {
  const { ref, revealed } = useRevealGroup(0.08);

  return (
    <section id="preview" ref={ref as React.RefObject<HTMLElement>} className="landing-section">
      <div className="mx-auto w-full max-w-[1400px] px-6">
        <div className={cn("mb-10 max-w-3xl reveal", revealed && "revealed")}>
          <div className="section-label">Preview</div>
          <h2 className="section-heading mt-3">A product window, not a marketing screenshot.</h2>
          <p className="section-subheading mt-4 max-w-2xl">
            The preview uses real Orchestrix shell patterns. Switch between planning, review, and execution states,
            inspect the artifact rail, and move through the review workspace without sending a single live AI message.
          </p>
        </div>

        <div className={cn("reveal reveal-delay-1", revealed && "revealed")}>
          <PreviewWorkbench initialScenario="awaiting_review" />
        </div>
      </div>
    </section>
  );
}

