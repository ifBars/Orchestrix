import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { Download, Github } from "lucide-react";

const navLinks = [
  { href: "#execution-model", label: "How it works" },
  { href: "#architecture", label: "Architecture" },
  { href: "#agents", label: "Agents" },
  { href: "#providers", label: "Providers" },
];

const OrchestrxLogo = () => (
  <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
    <polygon
      points="10,2 17,6 17,14 10,18 3,14 3,6"
      stroke="currentColor"
      strokeWidth="1.5"
      fill="none"
      className="text-primary"
    />
    <circle cx="10" cy="10" r="2.5" fill="currentColor" className="text-primary" />
    <line x1="10" y1="7.5" x2="10" y2="4" stroke="currentColor" strokeWidth="1.2" className="text-primary/50" />
    <line x1="10" y1="12.5" x2="10" y2="16" stroke="currentColor" strokeWidth="1.2" className="text-primary/50" />
  </svg>
);

const Navbar = () => {
  const [scrolled, setScrolled] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);

  useEffect(() => {
    const handleScroll = () => setScrolled(window.scrollY > 30);
    window.addEventListener("scroll", handleScroll, { passive: true });
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  return (
    <nav
      className={cn(
        "fixed top-0 left-0 right-0 z-50 transition-all duration-400",
        scrolled
          ? "bg-background/80 backdrop-blur-2xl border-b border-border/40"
          : "bg-transparent"
      )}
    >
      {/* Thin accent progress line — visible once scrolled */}
      <div
        className={cn(
          "absolute bottom-0 left-0 right-0 h-px transition-opacity duration-500",
          scrolled ? "opacity-100" : "opacity-0"
        )}
      >
        <div className="nav-border-active h-full w-full" />
      </div>

      <div className="container mx-auto flex h-16 items-center justify-between px-6">
        {/* Brand */}
        <a href="/" className="group flex items-center gap-2.5 shrink-0">
          <div className="relative flex items-center justify-center w-8 h-8 rounded-lg bg-card border border-border group-hover:border-primary/40 transition-all duration-300">
            <OrchestrxLogo />
            <div className="absolute inset-0 rounded-lg bg-primary/0 group-hover:bg-primary/5 transition-colors duration-300" />
          </div>
          <span className="text-sm font-semibold tracking-tight text-foreground font-mono">
            Orchestrix
          </span>
        </a>

        {/* Desktop links */}
        <div className="hidden md:flex items-center gap-0.5">
          {navLinks.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className="relative px-3.5 py-2 text-xs text-muted-foreground hover:text-foreground transition-colors duration-200 font-mono rounded-md group"
            >
              {link.label}
              <span className="absolute inset-x-3.5 bottom-1 h-px scale-x-0 group-hover:scale-x-100 transition-transform duration-250 origin-left bg-primary/60 rounded-full" />
            </a>
          ))}
        </div>

        {/* Right actions */}
        <div className="flex items-center gap-2">
          <a
            href="https://github.com/orchestrix"
            target="_blank"
            rel="noopener noreferrer"
            className="hidden sm:flex items-center justify-center w-9 h-9 rounded-lg border border-border/60 bg-card/40 text-muted-foreground hover:text-foreground hover:border-border transition-all duration-200"
            aria-label="GitHub"
          >
            <Github className="w-4 h-4" />
          </a>

          {/* Version pill + download */}
          <Button
            size="sm"
            className="group font-mono text-xs h-9 gap-2 px-4 relative overflow-hidden"
          >
            <span className="relative z-10 flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-success/90 animate-pulse" />
              <span>v0.1.0</span>
              <span className="hidden sm:inline text-primary-foreground/60">—</span>
              <Download className="hidden sm:block w-3 h-3" />
              <span className="hidden sm:inline">Download</span>
            </span>
          </Button>

          {/* Mobile menu toggle */}
          <button
            className="md:hidden flex flex-col gap-1 p-2 rounded-md border border-border/50 bg-card/40"
            onClick={() => setMenuOpen(!menuOpen)}
            aria-label="Toggle menu"
          >
            <span className={cn("w-4 h-px bg-foreground/70 transition-all duration-200", menuOpen && "rotate-45 translate-y-[5px]")} />
            <span className={cn("w-4 h-px bg-foreground/70 transition-all duration-200", menuOpen && "opacity-0")} />
            <span className={cn("w-4 h-px bg-foreground/70 transition-all duration-200", menuOpen && "-rotate-45 -translate-y-[5px]")} />
          </button>
        </div>
      </div>

      {/* Mobile menu */}
      <div
        className={cn(
          "md:hidden overflow-hidden transition-all duration-300 bg-background/95 backdrop-blur-xl border-b border-border/40",
          menuOpen ? "max-h-64 opacity-100" : "max-h-0 opacity-0"
        )}
      >
        <div className="container mx-auto px-6 py-4 flex flex-col gap-1">
          {navLinks.map((link) => (
            <a
              key={link.href}
              href={link.href}
              onClick={() => setMenuOpen(false)}
              className="px-3 py-2.5 text-sm text-muted-foreground hover:text-foreground font-mono rounded-md hover:bg-muted/50 transition-colors"
            >
              {link.label}
            </a>
          ))}
          <div className="mt-2 pt-3 border-t border-border/30 flex items-center gap-3">
            <a
              href="https://github.com/orchestrix"
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-2 text-sm text-muted-foreground font-mono"
            >
              <Github className="w-4 h-4" /> GitHub
            </a>
          </div>
        </div>
      </div>
    </nav>
  );
};

export default Navbar;
