import { Settings, Globe, Key } from "lucide-react";

const providers = [
  { name: "OpenAI", logo: "O" },
  { name: "Anthropic", logo: "A" },
  { name: "Gemini", logo: "G" },
  { name: "GLM", logo: "Z" },
  { name: "MiniMax", logo: "M" },
  { name: "Kimi", logo: "K" },
];

const ProvidersSection = () => {
  return (
    <section className="py-24 border-t border-border/20 relative overflow-hidden">
      {/* Background */}
      <div className="absolute inset-0 grid-bg opacity-[0.02]" />

      <div className="container mx-auto px-6">
        <div className="max-w-3xl mx-auto text-center">
          <span className="section-label mb-4 inline-block">Providers</span>
          <h2 className="text-2xl sm:text-3xl font-bold tracking-tight text-foreground mb-4">
            Multi-provider, configurable via UI
          </h2>
          <p className="text-sm text-muted-foreground max-w-xl mx-auto mb-10 leading-relaxed">
            Connect any supported LLM provider. Configure models, credentials, and defaults from the settings panel or bootstrap with environment variables.
          </p>

          {/* Provider badges */}
          <div className="flex flex-wrap items-center justify-center gap-3 mb-10">
            {providers.map((provider) => (
              <div
                key={provider.name}
                className="group relative px-5 py-2.5 rounded-lg border border-border/50 bg-card/40 backdrop-blur-sm text-sm font-mono text-secondary-foreground transition-all duration-300 hover:border-primary/30 hover:bg-card/80 hover:shadow-sm"
              >
                <div className="flex items-center gap-2">
                  <span className="w-6 h-6 rounded bg-gradient-to-br from-primary/20 to-primary/5 flex items-center justify-center text-xs font-bold text-primary">
                    {provider.logo}
                  </span>
                  {provider.name}
                </div>
              </div>
            ))}
          </div>

          {/* Configuration options */}
          <div className="grid sm:grid-cols-3 gap-4 max-w-lg mx-auto">
            <div className="flex items-center gap-3 p-3 rounded-lg bg-card/30 border border-border/30">
              <div className="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center">
                <Settings className="w-4 h-4 text-primary" />
              </div>
              <div className="text-left">
                <span className="block text-xs font-semibold text-foreground font-mono">UI Config</span>
                <span className="block text-[10px] text-muted-foreground">Settings panel</span>
              </div>
            </div>
            <div className="flex items-center gap-3 p-3 rounded-lg bg-card/30 border border-border/30">
              <div className="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center">
                <Globe className="w-4 h-4 text-primary" />
              </div>
              <div className="text-left">
                <span className="block text-xs font-semibold text-foreground font-mono">API Endpoints</span>
                <span className="block text-[10px] text-muted-foreground">Custom URLs</span>
              </div>
            </div>
            <div className="flex items-center gap-3 p-3 rounded-lg bg-card/30 border border-border/30">
              <div className="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center">
                <Key className="w-4 h-4 text-primary" />
              </div>
              <div className="text-left">
                <span className="block text-xs font-semibold text-foreground font-mono">Env Vars</span>
                <span className="block text-[10px] text-muted-foreground">Auto-detect</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default ProvidersSection;
