import { Github, Twitter, Mail } from "lucide-react";

const footerLinks = {
  Product: [
    { label: "Download", href: "#" },
    { label: "Features", href: "#execution-model" },
    { label: "Architecture", href: "#architecture" },
    { label: "Tools", href: "#tools" },
  ],
  Resources: [
    { label: "Documentation", href: "#" },
    { label: "API Reference", href: "#" },
    { label: "GitHub", href: "https://github.com/orchestrix" },
    { label: "Skills Guide", href: "#" },
  ],
  Company: [
    { label: "About", href: "#" },
    { label: "Blog", href: "#" },
    { label: "Contact", href: "#" },
    { label: "Privacy", href: "#" },
  ],
};

const socialLinks = [
  { icon: Github, href: "https://github.com/orchestrix", label: "GitHub" },
  { icon: Twitter, href: "#", label: "Twitter" },
  { icon: Mail, href: "mailto:hello@orchestrix.io", label: "Email" },
];

const Footer = () => {
  return (
    <footer className="border-t border-border/20 py-16 relative overflow-hidden">
      {/* Background */}
      <div className="absolute inset-0 grid-bg opacity-[0.015]" />
      <div className="absolute bottom-0 left-1/2 -translate-x-1/2 w-[600px] h-[300px] rounded-full bg-primary/[0.02] blur-[80px]" />

      <div className="container relative mx-auto px-6">
        <div className="grid sm:grid-cols-2 lg:grid-cols-5 gap-10 mb-12">
          {/* Brand */}
          <div className="lg:col-span-2">
            <a href="/" className="flex items-center gap-2.5 mb-4">
              <div className="h-7 w-7 rounded-lg bg-gradient-to-br from-primary to-primary/60 flex items-center justify-center">
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                  <path
                    d="M8 1L14 4.5V11.5L8 15L2 11.5V4.5L8 1Z"
                    stroke="currentColor"
                    strokeWidth="1.5"
                    className="text-primary-foreground"
                  />
                  <circle cx="8" cy="8" r="2" fill="currentColor" className="text-primary-foreground" />
                </svg>
              </div>
              <span className="text-sm font-bold text-foreground font-mono">Orchestrix</span>
            </a>
            <p className="text-xs text-muted-foreground leading-relaxed max-w-xs mb-6">
              Backend-authoritative AI agent runtime. Plan-first execution with human approval gates and full event visibility.
            </p>
            <div className="flex items-center gap-3">
              {socialLinks.map((social) => (
                <a
                  key={social.label}
                  href={social.href}
                  className="w-9 h-9 rounded-lg bg-card/50 border border-border/50 flex items-center justify-center text-muted-foreground hover:text-foreground hover:border-primary/30 hover:bg-card transition-all duration-300"
                  aria-label={social.label}
                >
                  <social.icon className="w-4 h-4" />
                </a>
              ))}
            </div>
          </div>

          {/* Links */}
          {Object.entries(footerLinks).map(([category, links]) => (
            <div key={category}>
              <h3 className="text-xs font-semibold text-foreground font-mono uppercase tracking-wider mb-4">
                {category}
              </h3>
              <ul className="space-y-3">
                {links.map((link) => (
                  <li key={link.label}>
                    <a
                      href={link.href}
                      className="text-xs text-muted-foreground hover:text-foreground transition-colors"
                    >
                      {link.label}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        {/* Bottom */}
        <div className="pt-8 border-t border-border/20 flex flex-col sm:flex-row items-center justify-between gap-4">
          <p className="text-[10px] text-muted-foreground font-mono">
            © 2026 Orchestrix. Built in Rust.
          </p>
          <div className="flex items-center gap-4 text-[10px] text-muted-foreground/60 font-mono">
            <span>MIT License</span>
            <span>•</span>
            <span>v0.1.0-alpha</span>
          </div>
        </div>
      </div>
    </footer>
  );
};

export default Footer;
