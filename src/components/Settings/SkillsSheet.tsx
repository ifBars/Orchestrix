import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  ExternalLink,
  FileText,
  FolderOpen,
  RefreshCw,
  Search,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import type { SkillCatalogItem, WorkspaceSkill } from "@/types";

type SkillsSheetProps = {
  open: boolean;
  onClose: () => void;
};

type Tab = "workspace" | "catalog";

export function SkillsSheet({ open, onClose }: SkillsSheetProps) {
  const [
    skills,
    workspaceSkills,
    searchSkills,
    addCustomSkill,
    importContext7Skill,
    importVercelSkill,
    removeSkill,
    refreshWorkspaceSkills,
  ] = useAppStore(
    useShallow((state) => [
      state.skills,
      state.workspaceSkills,
      state.searchSkills,
      state.addCustomSkill,
      state.importContext7Skill,
      state.importVercelSkill,
      state.removeSkill,
      state.refreshWorkspaceSkills,
    ])
  );

  const [tab, setTab] = useState<Tab>("workspace");
  const [expandedSkill, setExpandedSkill] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  // Catalog state
  const [query, setQuery] = useState("");
  const [sourceFilter, setSourceFilter] = useState("all");
  const [searching, setSearching] = useState(false);
  const [searchResults, setSearchResults] = useState<typeof skills>([]);
  const [error, setError] = useState<string | null>(null);

  // Custom skill form
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [installCommand, setInstallCommand] = useState("");
  const [url, setUrl] = useState("");
  const [source, setSource] = useState("custom");
  const [tags, setTags] = useState("");

  // Import forms
  const [context7Id, setContext7Id] = useState("");
  const [vercelSkill, setVercelSkill] = useState("");

  // Refresh workspace skills when opening
  useEffect(() => {
    if (open) {
      refreshWorkspaceSkills().catch(console.error);
    }
  }, [open, refreshWorkspaceSkills]);

  const sourceOptions = useMemo(() => {
    const set = new Set(skills.map((skill) => skill.source).filter(Boolean));
    return ["all", ...Array.from(set).sort((a, b) => a.localeCompare(b))];
  }, [skills]);

  if (!open) return null;

  const list = searchResults.length > 0 || query.trim() ? searchResults : skills;
  const filtered = list.filter((skill) => {
    if (sourceFilter === "all") return true;
    return skill.source === sourceFilter;
  });

  const runSearch = async (e?: FormEvent) => {
    e?.preventDefault();
    setError(null);
    if (!query.trim()) {
      setSearchResults([]);
      return;
    }
    setSearching(true);
    try {
      const results = await searchSkills(query.trim(), sourceFilter === "all" ? undefined : sourceFilter, 50);
      setSearchResults(results);
    } catch (searchError) {
      console.error(searchError);
      setError("Failed to search skills.");
    } finally {
      setSearching(false);
    }
  };

  const onAddCustom = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    try {
      await addCustomSkill({
        title,
        description,
        install_command: installCommand,
        url,
        source,
        tags: tags.split(",").map((item) => item.trim()).filter(Boolean),
      });
      setTitle("");
      setDescription("");
      setInstallCommand("");
      setUrl("");
      setSource("custom");
      setTags("");
    } catch (addError) {
      console.error(addError);
      setError("Failed to add custom skill.");
    }
  };

  const onImportContext7 = async () => {
    if (!context7Id.trim()) return;
    setError(null);
    try {
      await importContext7Skill(context7Id.trim());
      setContext7Id("");
    } catch (importError) {
      console.error(importError);
      setError("Failed to import Context7 skill.");
    }
  };

  const onImportVercel = async () => {
    if (!vercelSkill.trim()) return;
    setError(null);
    try {
      await importVercelSkill(vercelSkill.trim());
      setVercelSkill("");
    } catch (importError) {
      console.error(importError);
      setError("Failed to import Vercel skill.");
    }
  };

  const handleRefreshWorkspace = async () => {
    setRefreshing(true);
    try {
      await refreshWorkspaceSkills();
    } catch (err) {
      console.error(err);
    } finally {
      setRefreshing(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm" onClick={onClose}>
      <div
        className="w-[960px] max-h-[88vh] max-w-[96vw] overflow-hidden rounded-2xl border border-border bg-card shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-5 py-4">
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <Sparkles size={16} />
              Skills
            </div>

            {/* Tabs */}
            <div className="flex items-center rounded-lg border border-border bg-muted/30 p-0.5">
              <button
                type="button"
                onClick={() => setTab("workspace")}
                className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
                  tab === "workspace"
                    ? "bg-background text-foreground shadow-sm"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                Workspace
                {workspaceSkills.length > 0 && (
                  <span className="ml-1.5 rounded-full bg-primary/15 px-1.5 py-0.5 text-[10px] font-semibold text-primary">
                    {workspaceSkills.length}
                  </span>
                )}
              </button>
              <button
                type="button"
                onClick={() => setTab("catalog")}
                className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
                  tab === "catalog"
                    ? "bg-background text-foreground shadow-sm"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                Catalog
              </button>
            </div>
          </div>

          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onClose}>
            <X size={14} />
          </Button>
        </div>

        {/* Content */}
        <div className="max-h-[calc(88vh-64px)] overflow-y-auto">
          {tab === "workspace" ? (
            <WorkspaceTab
              skills={workspaceSkills}
              expandedSkill={expandedSkill}
              onToggleExpand={(id) => setExpandedSkill(expandedSkill === id ? null : id)}
              refreshing={refreshing}
              onRefresh={handleRefreshWorkspace}
            />
          ) : (
            <CatalogTab
              skills={skills}
              filtered={filtered}
              sourceOptions={sourceOptions}
              query={query}
              setQuery={setQuery}
              sourceFilter={sourceFilter}
              setSourceFilter={setSourceFilter}
              searching={searching}
              runSearch={runSearch}
              removeSkill={removeSkill}
              error={error}
              title={title}
              setTitle={setTitle}
              description={description}
              setDescription={setDescription}
              installCommand={installCommand}
              setInstallCommand={setInstallCommand}
              url={url}
              setUrl={setUrl}
              source={source}
              setSource={setSource}
              tags={tags}
              setTags={setTags}
              onAddCustom={onAddCustom}
              context7Id={context7Id}
              setContext7Id={setContext7Id}
              onImportContext7={onImportContext7}
              vercelSkill={vercelSkill}
              setVercelSkill={setVercelSkill}
              onImportVercel={onImportVercel}
            />
          )}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Workspace Skills Tab
// ---------------------------------------------------------------------------

function WorkspaceTab({
  skills,
  expandedSkill,
  onToggleExpand,
  refreshing,
  onRefresh,
}: {
  skills: WorkspaceSkill[];
  expandedSkill: string | null;
  onToggleExpand: (id: string) => void;
  refreshing: boolean;
  onRefresh: () => void;
}) {
  return (
    <div className="p-5">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold">Workspace Skills</h3>
          <p className="mt-0.5 text-xs text-muted-foreground">
            Skills discovered from <code className="rounded bg-muted px-1 py-0.5">.agents/skills/</code> in your workspace.
            These are automatically injected into agent prompts.
          </p>
        </div>
        <Button
          size="sm"
          variant="outline"
          onClick={onRefresh}
          disabled={refreshing}
          className="gap-1.5"
        >
          <RefreshCw size={12} className={refreshing ? "animate-spin" : ""} />
          Refresh
        </Button>
      </div>

      {skills.length === 0 ? (
        <div className="rounded-xl border border-dashed border-border bg-muted/20 px-6 py-10 text-center">
          <FolderOpen size={28} className="mx-auto mb-3 text-muted-foreground/40" />
          <p className="text-sm font-medium text-muted-foreground">No workspace skills found</p>
          <p className="mt-1.5 text-xs text-muted-foreground/70">
            Create a <code className="rounded bg-muted px-1 py-0.5">.agents/skills/&lt;name&gt;/SKILL.md</code> file
            in your workspace to add skills.
          </p>
          <p className="mt-3 text-xs text-muted-foreground/50">
            Skills follow the same format used by Claude Code, Cursor, and other AI tools.
          </p>
        </div>
      ) : (
        <div className="grid gap-2">
          {skills.map((skill) => {
            const expanded = expandedSkill === skill.id;
            return (
              <div
                key={skill.id}
                className="elevation-1 rounded-xl border border-border bg-background/60 transition-colors"
              >
                {/* Skill header */}
                <button
                  type="button"
                  onClick={() => onToggleExpand(skill.id)}
                  className="flex w-full items-start gap-3 px-4 py-3 text-left"
                >
                  <span className="mt-0.5 text-muted-foreground">
                    {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <h4 className="text-sm font-semibold">{skill.name}</h4>
                      <span className="rounded border border-border px-1.5 py-0.5 text-[10px] text-muted-foreground">
                        {skill.source}
                      </span>
                      {skill.enabled && (
                        <span className="rounded-full bg-success/15 px-1.5 py-0.5 text-[10px] font-medium text-success">
                          active
                        </span>
                      )}
                    </div>
                    {skill.description && (
                      <p className="mt-1 text-xs text-muted-foreground">{skill.description}</p>
                    )}
                    <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
                      {skill.tags.slice(0, 5).map((tag) => (
                        <span key={tag} className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
                          {tag}
                        </span>
                      ))}
                      {skill.files.length > 0 && (
                        <span className="flex items-center gap-1 text-[10px] text-muted-foreground/60">
                          <FileText size={10} />
                          {skill.files.length} file{skill.files.length !== 1 ? "s" : ""}
                        </span>
                      )}
                    </div>
                  </div>
                </button>

                {/* Expanded content */}
                {expanded && (
                  <div className="border-t border-border px-4 py-3">
                    {/* File list */}
                    {skill.files.length > 0 && (
                      <div className="mb-3">
                        <p className="mb-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground/60">
                          Files
                        </p>
                        <div className="flex flex-wrap gap-1.5">
                          {skill.files.map((file) => (
                            <span
                              key={file}
                              className="inline-flex items-center gap-1 rounded border border-border bg-muted/30 px-2 py-0.5 text-xs text-muted-foreground"
                            >
                              <FileText size={10} />
                              {file}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}

                    {/* Skill content preview */}
                    <div>
                      <p className="mb-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground/60">
                        SKILL.md Content
                      </p>
                      <div className="max-h-64 overflow-y-auto rounded-lg border border-border bg-muted/20 p-3">
                        <pre className="whitespace-pre-wrap text-xs text-muted-foreground leading-relaxed">
                          {skill.content.length > 2000
                            ? skill.content.slice(0, 2000) + "\n\n... (truncated)"
                            : skill.content}
                        </pre>
                      </div>
                    </div>

                    {/* Skill directory path */}
                    <div className="mt-3 text-[11px] text-muted-foreground/50">
                      <span className="font-medium">Path:</span>{" "}
                      <code className="rounded bg-muted px-1 py-0.5">{skill.skill_dir}</code>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Catalog Tab (existing skills catalog UI)
// ---------------------------------------------------------------------------

function CatalogTab({
  filtered,
  sourceOptions,
  query,
  setQuery,
  sourceFilter,
  setSourceFilter,
  searching,
  runSearch,
  removeSkill,
  error,
  title,
  setTitle,
  description,
  setDescription,
  installCommand,
  setInstallCommand,
  url,
  setUrl,
  source,
  setSource,
  tags,
  setTags,
  onAddCustom,
  context7Id,
  setContext7Id,
  onImportContext7,
  vercelSkill,
  setVercelSkill,
  onImportVercel,
}: {
  skills: SkillCatalogItem[];
  filtered: SkillCatalogItem[];
  sourceOptions: string[];
  query: string;
  setQuery: (value: string) => void;
  sourceFilter: string;
  setSourceFilter: (value: string) => void;
  searching: boolean;
  runSearch: (e?: FormEvent) => void;
  removeSkill: (id: string) => Promise<void>;
  error: string | null;
  title: string;
  setTitle: (value: string) => void;
  description: string;
  setDescription: (value: string) => void;
  installCommand: string;
  setInstallCommand: (value: string) => void;
  url: string;
  setUrl: (value: string) => void;
  source: string;
  setSource: (value: string) => void;
  tags: string;
  setTags: (value: string) => void;
  onAddCustom: (e: FormEvent) => void;
  context7Id: string;
  setContext7Id: (value: string) => void;
  onImportContext7: () => void;
  vercelSkill: string;
  setVercelSkill: (value: string) => void;
  onImportVercel: () => void;
}) {
  return (
    <div className="grid h-[calc(88vh-64px)] min-h-0 gap-0 overflow-hidden lg:grid-cols-[1.2fr_1fr]">
      <section className="flex min-h-0 flex-col border-r border-border p-5">
        <form className="mb-4 flex items-center gap-2" onSubmit={runSearch}>
          <Input placeholder="Search skills..." value={query} onChange={(e) => setQuery(e.target.value)} />
          <select
            className="h-9 rounded-md border border-input bg-background px-2 text-xs"
            value={sourceFilter}
            onChange={(e) => setSourceFilter(e.target.value)}
          >
            {sourceOptions.map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </select>
          <Button size="sm" type="submit" disabled={searching}>
            <Search size={14} />
            {searching ? "Searching" : "Search"}
          </Button>
        </form>

        {filtered.length === 0 ? (
          <div className="py-8 text-center text-sm text-muted-foreground">No skills found.</div>
        ) : (
          <div className="min-h-0 flex-1 overflow-y-auto pr-1">
            <div className="grid gap-3">
              {filtered.map((skill) => (
                <article key={skill.id} className="elevation-1 rounded-xl border border-border bg-background/60 p-4">
                  <div className="mb-2 flex items-center justify-between gap-2">
                    <h3 className="text-sm font-semibold">{skill.title}</h3>
                    <div className="flex items-center gap-1">
                      <button
                        type="button"
                        onClick={() => openUrl(skill.url).catch(console.error)}
                        className="rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                      >
                        <ExternalLink size={13} />
                      </button>
                      {skill.is_custom ? (
                        <button
                          type="button"
                          onClick={() => removeSkill(skill.id).catch(console.error)}
                          className="rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-destructive"
                          title="Remove skill"
                        >
                          <Trash2 size={13} />
                        </button>
                      ) : null}
                    </div>
                  </div>
                  <p className="mb-2 text-xs text-muted-foreground">{skill.description}</p>
                  <div className="mb-2 flex items-center gap-2 text-[11px] text-muted-foreground">
                    <span className="rounded border border-border px-1.5 py-0.5">{skill.source}</span>
                    {skill.tags.slice(0, 3).map((tag) => (
                      <span key={tag} className="rounded bg-muted px-1.5 py-0.5">
                        {tag}
                      </span>
                    ))}
                  </div>
                  <code className="block rounded-lg border border-border bg-muted/30 px-2.5 py-1.5 text-xs text-muted-foreground">
                    {skill.install_command}
                  </code>
                </article>
              ))}
            </div>
          </div>
        )}
      </section>

      <section className="min-h-0 overflow-y-auto p-5">
        <div className="mb-6">
          <h3 className="mb-2 text-sm font-semibold">Import Existing Skill</h3>
          <div className="mb-2 flex gap-2">
            <Input
              placeholder="/org/project (Context7 library id)"
              value={context7Id}
              onChange={(e) => setContext7Id(e.target.value)}
            />
            <Button size="sm" type="button" onClick={onImportContext7} disabled={!context7Id.trim()}>
              Import
            </Button>
          </div>
          <div className="flex gap-2">
            <Input
              placeholder="vercel-react-best-practices"
              value={vercelSkill}
              onChange={(e) => setVercelSkill(e.target.value)}
            />
            <Button size="sm" type="button" onClick={onImportVercel} disabled={!vercelSkill.trim()}>
              Import
            </Button>
          </div>
        </div>

        <form className="space-y-3" onSubmit={onAddCustom}>
          <h3 className="text-sm font-semibold">Add Custom Skill</h3>
          <Input placeholder="Title" value={title} onChange={(e) => setTitle(e.target.value)} required />
          <Textarea
            placeholder="Description"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            className="min-h-20"
          />
          <Input
            placeholder="Install command"
            value={installCommand}
            onChange={(e) => setInstallCommand(e.target.value)}
            required
          />
          <Input placeholder="URL" value={url} onChange={(e) => setUrl(e.target.value)} required />
          <Input placeholder="Source (custom, team, etc.)" value={source} onChange={(e) => setSource(e.target.value)} />
          <Input placeholder="Tags (comma separated)" value={tags} onChange={(e) => setTags(e.target.value)} />
          <Button type="submit" size="sm" disabled={!title.trim() || !installCommand.trim() || !url.trim()}>
            Add Skill
          </Button>
        </form>

        {error ? <p className="mt-3 text-xs text-destructive">{error}</p> : null}
      </section>
    </div>
  );
}
