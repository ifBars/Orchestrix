import { useEffect, useState } from "react";
import { Download, Github, Menu, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ORCHESTRIX_REPO_URL, ORCHESTRIX_RELEASES_URL } from "@/components/landing/constants";
import { OrchestrixMark } from "@/components/landing/OrchestrixMark";
import { cn } from "@/lib/utils";

const navLinks = [
  { href: "#preview", label: "Preview" },
  { href: "#workflow", label: "Workflow" },
  { href: "#proof", label: "Proof" },
];

export default function Navbar() {
  const [scrolled, setScrolled] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);

  useEffect(() => {
    const handleScroll = () => setScrolled(window.scrollY > 20);
    window.addEventListener("scroll", handleScroll, { passive: true });
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  return (
    <nav
      className={cn(
        "fixed inset-x-0 top-0 z-50 border-b transition-all duration-200",
        scrolled ? "border-border/70 bg-background/84 backdrop-blur-xl" : "border-transparent bg-transparent"
      )}
    >
      <div className="mx-auto flex h-14 w-full max-w-[1400px] items-center justify-between gap-4 px-6">
        <a href="#top" className="flex items-center gap-3.5">
          <OrchestrixMark className="h-10 w-10 shrink-0" />
          <div>
            <div className="text-sm font-semibold tracking-tight text-foreground">Orchestrix</div>
            <div className="text-[10px] font-medium uppercase tracking-[0.18em] text-muted-foreground">AI workspace</div>
          </div>
        </a>

        <div className="hidden items-center gap-1 md:flex">
          {navLinks.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className="rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground"
            >
              {link.label}
            </a>
          ))}
        </div>

        <div className="hidden items-center gap-2 md:flex">
          <a
            href={ORCHESTRIX_REPO_URL}
            target="_blank"
            rel="noreferrer"
            className="inline-flex h-9 items-center gap-2 rounded-full border border-border/70 px-4 text-sm text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground"
          >
            <Github size={14} />
            GitHub
          </a>
          <Button
            size="sm"
            className="h-9 rounded-full px-4"
            onClick={() => window.open(ORCHESTRIX_RELEASES_URL, "_blank", "noopener,noreferrer")}
          >
            <Download size={14} />
            Download
          </Button>
        </div>

        <button
          type="button"
          className="inline-flex h-9 w-9 items-center justify-center rounded-xl border border-border/70 bg-card/60 text-muted-foreground md:hidden"
          onClick={() => setMenuOpen((prev) => !prev)}
          aria-label="Toggle navigation"
        >
          {menuOpen ? <X size={16} /> : <Menu size={16} />}
        </button>
      </div>

      {menuOpen ? (
        <div className="border-t border-border/70 bg-background/94 px-6 py-4 backdrop-blur-xl md:hidden">
          <div className="flex flex-col gap-2">
            {navLinks.map((link) => (
              <a
                key={link.href}
                href={link.href}
                onClick={() => setMenuOpen(false)}
                className="rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground"
              >
                {link.label}
              </a>
            ))}
            <a
              href={ORCHESTRIX_REPO_URL}
              target="_blank"
              rel="noreferrer"
              className="rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground"
            >
              GitHub
            </a>
          </div>
        </div>
      ) : null}
    </nav>
  );
}

