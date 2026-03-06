import { Github } from "lucide-react";
import { ORCHESTRIX_README_URL, ORCHESTRIX_REPO_URL } from "@/components/landing/constants";
import { OrchestrixMark } from "@/components/landing/OrchestrixMark";

export default function Footer() {
  return (
    <footer className="border-t border-border/60 pb-10 pt-8">
      <div className="mx-auto flex w-full max-w-[1400px] flex-col gap-8 px-6 lg:flex-row lg:items-end lg:justify-between">
        <div>
          <div className="flex items-center gap-3">
            <OrchestrixMark className="h-10 w-10 shrink-0" />
            <div>
              <p className="text-sm font-semibold tracking-tight text-foreground">Orchestrix</p>
              <p className="text-xs text-muted-foreground">Human-in-the-loop AI workspace</p>
            </div>
          </div>
          <p className="mt-4 max-w-md text-sm leading-relaxed text-muted-foreground">
            Backend-authoritative orchestration, condensed event visibility, and review-first execution for developers who want receipts.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-3 text-sm text-muted-foreground">
          <a href={ORCHESTRIX_README_URL} target="_blank" rel="noreferrer" className="rounded-full border border-border/70 px-4 py-2 transition-colors hover:bg-accent/50 hover:text-foreground">
            Documentation
          </a>
          <a href={ORCHESTRIX_REPO_URL} target="_blank" rel="noreferrer" className="inline-flex items-center gap-2 rounded-full border border-border/70 px-4 py-2 transition-colors hover:bg-accent/50 hover:text-foreground">
            <Github size={14} />
            GitHub
          </a>
          <span className="text-xs uppercase tracking-[0.18em] text-muted-foreground/70">MIT</span>
        </div>
      </div>
    </footer>
  );
}

