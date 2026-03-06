import { Download, Github } from "lucide-react";
import { ORCHESTRIX_REPO_URL, ORCHESTRIX_RELEASES_URL } from "@/components/landing/constants";

export default function CTASection() {
  return (
    <section className="landing-section pt-10">
      <div className="mx-auto w-full max-w-[1400px] px-6">
        <div className="landing-cta-band">
          <div>
            <div className="section-label">Start here</div>
            <h2 className="mt-3 max-w-[12ch] text-4xl font-semibold tracking-[-0.05em] text-foreground sm:text-5xl">
              Ship agent workflows with the discipline of a real IDE.
            </h2>
            <p className="mt-4 max-w-2xl text-base leading-relaxed text-muted-foreground">
              Download the app and inspect the repository before you ever hand it a real codebase.
            </p>
          </div>

          <div className="landing-cta-band__actions">
            <a href={ORCHESTRIX_RELEASES_URL} target="_blank" rel="noreferrer" className="landing-cta-button landing-cta-button--primary">
              <Download size={16} />
              Download alpha
            </a>
            <a href={ORCHESTRIX_REPO_URL} target="_blank" rel="noreferrer" className="landing-cta-button landing-cta-button--secondary">
              <Github size={16} />
              GitHub
            </a>
          </div>
        </div>
      </div>
    </section>
  );
}

