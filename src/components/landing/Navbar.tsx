import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const navLinks = [
  { href: "#execution-model", label: "Execution" },
  { href: "#architecture", label: "Architecture" },
  { href: "#tools", label: "Tools" },
  { href: "https://github.com/orchestrix", label: "GitHub" },
];

const Navbar = () => {
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const handleScroll = () => {
      setScrolled(window.scrollY > 20);
    };
    window.addEventListener("scroll", handleScroll);
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  return (
    <nav
      className={cn(
        "fixed top-0 left-0 right-0 z-50 transition-all duration-500",
        scrolled
          ? "bg-background/70 backdrop-blur-2xl border-b border-border/30 shadow-sm"
          : "bg-transparent"
      )}
    >
      <div className="container mx-auto flex h-16 items-center justify-between px-6">
        <a href="/" className="group flex items-center gap-2.5">
          <div className="relative">
            <div className="h-8 w-8 rounded-lg bg-gradient-to-br from-primary to-primary/60 flex items-center justify-center shadow-lg shadow-primary/25 group-hover:shadow-primary/40 transition-shadow duration-300">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path
                  d="M8 1L14 4.5V11.5L8 15L2 11.5V4.5L8 1Z"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  className="text-primary-foreground"
                />
                <circle cx="8" cy="8" r="2.5" fill="currentColor" className="text-primary-foreground" />
              </svg>
            </div>
            <div className="absolute -inset-1 rounded-lg bg-primary/20 blur-md opacity-0 group-hover:opacity-100 transition-opacity duration-300 -z-10" />
          </div>
          <span className="text-sm font-bold tracking-tight text-foreground font-mono">
            Orchestrix
          </span>
        </a>

        <div className="hidden md:flex items-center gap-1">
          {navLinks.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className="relative px-4 py-2 text-xs text-muted-foreground hover:text-foreground transition-colors font-mono rounded-md hover:bg-muted/50 group"
            >
              {link.label}
              <span className="absolute inset-x-4 -bottom-0.5 h-px scale-x-0 group-hover:scale-x-100 transition-transform duration-300 bg-primary" />
            </a>
          ))}
        </div>

        <Button
          size="sm"
          className="relative overflow-hidden group font-mono text-xs h-9"
        >
          <span className="relative z-10">Download</span>
          <div className="absolute inset-0 bg-gradient-to-r from-primary to-primary/70 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
        </Button>
      </div>
    </nav>
  );
};

export default Navbar;
