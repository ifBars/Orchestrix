import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink, Search, Trash2, Download, Box } from "lucide-react";
import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type {
  AgentSkillInstallResult,
  AgentSkillSearchItem,
  WorkspaceSkill,
} from "@/types";

type Tab = "installed" | "search";

export function SkillsSection() {
  const state = useAppStore();
  const [activeTab, setActiveTab] = useState<Tab>("installed");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    state.refreshWorkspaceSkills().catch(console.error);
  }, []);

  return (
    <div className="flex h-full flex-col gap-4">
      <div className="flex items-center justify-between">
        <div className="flex gap-2 rounded-lg bg-muted/50 p-1">
          <TabButton active={activeTab === "installed"} onClick={() => setActiveTab("installed")}>
            Installed ({state.workspaceSkills.length})
          </TabButton>
          <TabButton active={activeTab === "search"} onClick={() => setActiveTab("search")}>
            Skills Search
          </TabButton>
        </div>
      </div>

      {error ? (
        <div className="rounded-md border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      ) : null}

      <div className="flex-1 overflow-hidden">
        {activeTab === "installed" && (
          <InstalledPanel
            skills={state.workspaceSkills}
            onRemove={async (id) => {
              try {
                await state.removeSkill(id);
              } catch (err: any) {
                setError(err.message || String(err));
              }
            }}
          />
        )}

        {activeTab === "search" && (
          <SkillsSearchPanel
            search={state.searchAgentSkills}
            install={state.installAgentSkill}
            setError={setError}
            importContext7={state.importContext7Skill}
            importVercel={state.importVercelSkill}
          />
        )}
      </div>
    </div>
  );
}

function TabButton({
  active,
  children,
  onClick,
}: {
  active: boolean;
  children: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={`rounded-md px-3 py-1.5 text-sm font-medium transition-colors ${
        active ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:bg-background/50 hover:text-foreground"
      }`}
    >
      {children}
    </button>
  );
}

function InstalledPanel({
  skills,
  onRemove,
}: {
  skills: WorkspaceSkill[];
  onRemove: (id: string) => void;
}) {
  if (skills.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center text-muted-foreground">
        <Box className="mb-4 h-12 w-12 opacity-20" />
        <p className="text-sm">No skills installed in this workspace.</p>
        <p className="text-xs opacity-70">Use the Skills Search tab to discover and install skills.</p>
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto pr-2">
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {skills.map((skill) => (
          <div key={skill.id} className="group relative flex flex-col justify-between rounded-lg border border-border bg-card p-4 transition-colors hover:border-primary/50">
            <div>
              <div className="mb-2 flex items-start justify-between">
                <div className="flex items-center gap-2">
                  <h3 className="font-semibold text-card-foreground">{skill.name}</h3>
                  {skill.is_builtin && (
                    <span className="rounded-full bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium text-primary">
                      Built-in
                    </span>
                  )}
                </div>
                {!skill.is_builtin && (
                  <button
                    onClick={() => onRemove(skill.id)}
                    className="rounded p-1 text-muted-foreground opacity-0 transition-all hover:bg-destructive/10 hover:text-destructive group-hover:opacity-100"
                    title="Remove skill"
                  >
                    <Trash2 size={14} />
                  </button>
                )}
              </div>
              <p className="mb-3 text-xs text-muted-foreground line-clamp-2">
                {skill.description || "No description provided."}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <span className="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
                {skill.source === "builtin" ? "Built-in" : skill.skill_dir || skill.source}
              </span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function SkillsSearchPanel({
  search,
  install,
  setError,
  importContext7,
  importVercel,
}: {
  search: (query: string, limit?: number) => Promise<AgentSkillSearchItem[]>;
  install: (name: string, repo?: string) => Promise<AgentSkillInstallResult>;
  setError: (msg: string | null) => void;
  importContext7: (id: string, title?: string) => Promise<void>;
  importVercel: (name: string) => Promise<void>;
}) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<AgentSkillSearchItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [output, setOutput] = useState<string | null>(null);

  // Manual import state
  const [c7Id, setC7Id] = useState("");
  const [vercelName, setVercelName] = useState("");

  const doSearch = async (q: string) => {
    setLoading(true);
    setError(null);
    try {
      // Empty query returns all skills from backend now
      const items = await search(q, 50); 
      setResults(items);
    } catch (err: any) {
      setError(`Search failed: ${err.message || String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  // Initial load
  useEffect(() => {
    doSearch("");
  }, []);

  const handleInstall = async (item: AgentSkillSearchItem) => {
    setInstalling(item.skill_name);
    setOutput(null);
    setError(null);
    try {
      // Pass the source from search result as repo URL if it's not the default one
      // The backend default is vercel-labs/agent-skills, but explicit is safer
      const res = await install(item.skill_name, item.source);
      // Strip ANSI codes for cleaner display
      // eslint-disable-next-line no-control-regex
      const stripAnsi = (str: string) => str.replace(/[\u001b\u009b][[()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-ORZcf-nqry=><]/g, "");
      
      const cleanStdout = stripAnsi(res.stdout || "");
      const cleanStderr = stripAnsi(res.stderr || "");
      
      const log = `$ ${res.command}\n${cleanStdout}\n${cleanStderr ? `[stderr]\n${cleanStderr}` : ""}`;
      setOutput(log);
    } catch (err: any) {
        setError(`Install failed: ${err.message || String(err)}`);
    } finally {
      setInstalling(null);
    }
  };

  const handleImportC7 = async () => {
      if(!c7Id.trim()) return;
      try { await importContext7(c7Id); setC7Id(""); } catch(e:any) { setError(e.message); }
  }

  const handleImportVercel = async () => {
      if(!vercelName.trim()) return;
      try { await importVercel(vercelName); setVercelName(""); } catch(e:any) { setError(e.message); }
  }

  return (
    <div className="flex h-full flex-col gap-4">
      <div className="flex flex-col gap-2">
        <div className="rounded-lg border border-border bg-card p-4">
          <form 
            onSubmit={(e) => { e.preventDefault(); doSearch(query); }}
            className="flex gap-2"
          >
            <div className="relative flex-1">
              <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
              <Input
                placeholder="Search skills (e.g., react, rust, python)..."
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                className="pl-9"
              />
            </div>
            <Button type="submit" disabled={loading}>
              {loading ? "Searching..." : "Search"}
            </Button>
          </form>
        </div>
        
        {output && (
            <div className="max-h-32 overflow-y-auto rounded-lg border border-border bg-muted/30 p-3 font-mono text-xs">
                <div className="mb-1 font-semibold text-muted-foreground">Last Install Output:</div>
                <pre className="whitespace-pre-wrap">{output}</pre>
            </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto pr-2">
        {results.length === 0 && !loading ? (
            <div className="flex flex-col items-center justify-center py-8 text-center text-muted-foreground opacity-60">
                <Box className="mb-2 h-10 w-10" />
                <p>No skills found.</p>
            </div>
        ) : (
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {results.map((item) => (
                <div key={item.skill_name} className="flex flex-col justify-between rounded-lg border border-border bg-card p-4 transition-all hover:border-primary/40">
                <div>
                    <div className="mb-1 flex items-start justify-between">
                    <h4 className="font-semibold">{item.title}</h4>
                    <button
                        onClick={() => openUrl(item.url).catch(console.error)}
                        className="text-muted-foreground hover:text-primary"
                    >
                        <ExternalLink size={14} />
                    </button>
                    </div>
                    <p className="mb-3 text-xs text-muted-foreground line-clamp-3">{item.description}</p>
                </div>
                
                <div className="space-y-3">
                    <div className="flex items-center justify-between text-[10px] text-muted-foreground">
                        <span className="font-mono bg-muted px-1.5 py-0.5 rounded">{item.skill_name}</span>
                        <span>{item.installs > 0 ? `${item.installs.toLocaleString()} installs` : 'New'}</span>
                    </div>
                    <Button 
                        size="sm" 
                        className="w-full" 
                        onClick={() => handleInstall(item)}
                        disabled={installing === item.skill_name}
                    >
                        {installing === item.skill_name ? (
                            "Installing..."
                        ) : (
                            <>
                                <Download size={14} className="mr-2" />
                                Install Skill
                            </>
                        )}
                    </Button>
                </div>
                </div>
            ))}
            </div>
        )}
      </div>

      <div className="mt-auto border-t border-border pt-4">
        <details className="group">
            <summary className="cursor-pointer text-xs font-medium text-muted-foreground hover:text-foreground">
                Advanced: Manual Import
            </summary>
            <div className="mt-3 grid gap-4 p-2 bg-muted/20 rounded sm:grid-cols-2">
                <div className="space-y-2">
                    <label className="text-[10px] font-medium uppercase text-muted-foreground">Context7 Library ID</label>
                    <div className="flex gap-2">
                        <Input placeholder="/org/project" value={c7Id} onChange={(e) => setC7Id(e.target.value)} className="h-8 text-xs" />
                        <Button size="sm" variant="secondary" onClick={handleImportC7} disabled={!c7Id.trim()} className="h-8">Import</Button>
                    </div>
                </div>
                <div className="space-y-2">
                    <label className="text-[10px] font-medium uppercase text-muted-foreground">Vercel Skill Name</label>
                    <div className="flex gap-2">
                        <Input placeholder="skill-name" value={vercelName} onChange={(e) => setVercelName(e.target.value)} className="h-8 text-xs" />
                        <Button size="sm" variant="secondary" onClick={handleImportVercel} disabled={!vercelName.trim()} className="h-8">Import</Button>
                    </div>
                </div>
            </div>
        </details>
      </div>
    </div>
  );
}
