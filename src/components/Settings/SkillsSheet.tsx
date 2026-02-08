import { FormEvent, useMemo, useState } from "react";
import { ExternalLink, Search, Sparkles, Trash2, X } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";

type SkillsSheetProps = {
  open: boolean;
  onClose: () => void;
};

export function SkillsSheet({ open, onClose }: SkillsSheetProps) {
  const [skills, searchSkills, addCustomSkill, importContext7Skill, importVercelSkill, removeSkill] = useAppStore(
    useShallow((state) => [
      state.skills,
      state.searchSkills,
      state.addCustomSkill,
      state.importContext7Skill,
      state.importVercelSkill,
      state.removeSkill,
    ])
  );

  const [query, setQuery] = useState("");
  const [sourceFilter, setSourceFilter] = useState("all");
  const [searching, setSearching] = useState(false);
  const [searchResults, setSearchResults] = useState<typeof skills>([]);
  const [error, setError] = useState<string | null>(null);

  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [installCommand, setInstallCommand] = useState("");
  const [url, setUrl] = useState("");
  const [source, setSource] = useState("custom");
  const [tags, setTags] = useState("");

  const [context7Id, setContext7Id] = useState("");
  const [vercelSkill, setVercelSkill] = useState("");

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
        tags: tags
          .split(",")
          .map((item) => item.trim())
          .filter(Boolean),
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

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm" onClick={onClose}>
      <div
        className="w-[920px] max-h-[88vh] max-w-[96vw] overflow-hidden rounded-2xl border border-border bg-card shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-border px-5 py-4">
          <div className="flex items-center gap-2 text-sm font-semibold">
            <Sparkles size={16} />
            Skills
          </div>
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onClose}>
            <X size={14} />
          </Button>
        </div>

        <div className="grid max-h-[calc(88vh-64px)] gap-0 overflow-hidden lg:grid-cols-[1.2fr_1fr]">
          <section className="overflow-y-auto border-r border-border p-5">
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
              <div className="grid gap-3">
                {filtered.map((skill) => (
                  <article key={skill.id} className="rounded-xl border border-border bg-background/60 p-4">
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
            )}
          </section>

          <section className="overflow-y-auto p-5">
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
      </div>
    </div>
  );
}
