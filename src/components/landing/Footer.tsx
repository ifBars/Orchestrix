import { Github, Twitter, Mail } from "lucide-react";

const footerLinks = {
  Product: [
    { label: "Download", href: "#" },
    { label: "How it works", href: "#execution-model" },
    { label: "Architecture", href: "#architecture" },
    { label: "Agents & Tools", href: "#agents" },
    { label: "Providers", href: "#providers" },
  ],
  Resources: [
    { label: "Documentation", href: "#" },
    { label: "API Reference", href: "#" },
    { label: "GitHub", href: "https://github.com/orchestrix" },
    { label: "Skills Guide", href: "#" },
    { label: "AGENTS.md", href: "#" },
  ],
  Company: [
    { label: "About", href: "#" },
    { label: "Blog", href: "#" },
    { label: "Contact", href: "#" },
    { label: "Privacy", href: "#" },
  ],
};

const socialLinks = [
  { icon: Github,  href: "https://github.com/orchestrix", label: "GitHub" },
  { icon: Twitter, href: "#",                              label: "Twitter" },
  { icon: Mail,    href: "mailto:hello@orchestrix.io",    label: "Email" },
];

const OrchestrxLogo = () => (
  <svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true">
    <polygon
      points="10,2 17,6 17,14 10,18 3,14 3,6"
      stroke="currentColor"
      strokeWidth="1.5"
      fill="none"
    />
    <circle cx="10" cy="10" r="2.5" fill="currentColor" />
  </svg>
);

const Footer = () => {
  return (
    <footer className="relative overflow-hidden border-t border-border/20">
      {/* Top scanning accent */}
      <div
        className="absolute top-0 left-0 right-0 h-px"
        style={{
          background:
            "linear-gradient(90deg, transparent 0%, oklch(0.68 0.12 235 / 0.15) 30%, oklch(0.68 0.12 235 / 0.15) 70%, transparent 100%)",
        }}
      />

      <div className="absolute bottom-0 left-1/2 -translate-x-1/2 w-[500px] h-[200px] pointer-events-none"
        style={{
          background: "radial-gradient(ellipse at bottom, oklch(0.68 0.10 235 / 0.04) 0%, transparent 70%)",
        }}
      />

      <div className="container relative mx-auto px-6 pt-16 pb-10">
        <div className="grid sm:grid-cols-2 lg:grid-cols-5 gap-10 mb-12">

          {/* Brand block */}
          <div className="lg:col-span-2">
            <a href="/" className="inline-flex items-center gap-2.5 mb-5 group">
              <div className="flex items-center justify-center w-8 h-8 rounded-lg border border-border/60 bg-card/50 text-primary group-hover:border-primary/40 transition-colors">
                <OrchestrxLogo />
              </div>
              <span className="text-sm font-semibold text-foreground font-mono tracking-tight">
                Orchestrix
              </span>
            </a>
            <p className="text-xs text-muted-foreground leading-relaxed max-w-xs mb-6">
              Backend-authoritative AI agent runtime. Plan-first execution with human approval gates and full event visibility. Built in Rust.
            </p>

            {/* Social links */}
            <div className="flex items-center gap-2">
              {socialLinks.map((social) => (
                <a
                  key={social.label}
                  href={social.href}
                  className="flex items-center justify-center w-8 h-8 rounded-lg border border-border/50 bg-card/30 text-muted-foreground hover:text-foreground hover:border-primary/30 hover:bg-card/70 transition-all duration-200"
                  aria-label={social.label}
                  target={social.href.startsWith("http") ? "_blank" : undefined}
                  rel={social.href.startsWith("http") ? "noopener noreferrer" : undefined}
                >
                  <social.icon className="w-3.5 h-3.5" />
                </a>
              ))}
            </div>
          </div>

          {/* Link columns */}
          {Object.entries(footerLinks).map(([category, links]) => (
            <div key={category}>
              <h3 className="text-[10px] font-semibold text-foreground/80 font-mono uppercase tracking-[0.15em] mb-4">
                {category}
              </h3>
              <ul className="space-y-2.5">
                {links.map((link) => (
                  <li key={link.label}>
                    <a
                      href={link.href}
                      className="text-xs text-muted-foreground hover:text-foreground transition-colors duration-150 font-mono"
                      target={link.href.startsWith("http") ? "_blank" : undefined}
                      rel={link.href.startsWith("http") ? "noopener noreferrer" : undefined}
                    >
                      {link.label}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        {/* Bottom bar */}
        <div className="pt-8 border-t border-border/20 flex flex-col sm:flex-row items-center justify-between gap-4">
          <p className="text-[10px] text-muted-foreground/50 font-mono">
            © 2026 Orchestrix. Built in Rust.
          </p>
          <div className="flex items-center gap-4 text-[10px] text-muted-foreground/35 font-mono">
            <span>MIT License</span>
            <span className="text-muted-foreground/20">·</span>
            <code className="px-1.5 py-0.5 rounded bg-muted/40 text-primary/50">v0.1.0-alpha</code>
            <span className="text-muted-foreground/20">·</span>
            <a href="#" className="hover:text-muted-foreground/60 transition-colors">Terms</a>
            <span className="text-muted-foreground/20">·</span>
            <a href="#" className="hover:text-muted-foreground/60 transition-colors">Privacy</a>
          </div>
        </div>
      </div>
    </footer>
  );
};

export default Footer;
