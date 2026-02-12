import { Folder, Sparkles, Bot, PlugZap, Server } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { ReactNode } from "react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";

export function GeneralSection() {
  const [
    workspaceRoot,
    setWorkspaceRoot,
    providerConfigs,
    workspaceSkills,
    agentPresets,
    mcpServers,
    mcpTools,
  ] = useAppStore(
    useShallow((state) => [
      state.workspaceRoot,
      state.setWorkspaceRoot,
      state.providerConfigs,
      state.workspaceSkills,
      state.agentPresets,
      state.mcpServers,
      state.mcpTools,
    ])
  );

  const configuredProviders = providerConfigs.filter((item) => item.configured).length;

  const handlePickWorkspace = async () => {
    const selected = await openDialog({
      directory: true,
      title: "Select workspace folder",
      defaultPath: workspaceRoot || undefined,
    });

    if (typeof selected === "string" && selected.length > 0) {
      await setWorkspaceRoot(selected);
    }
  };

  return (
    <div className="space-y-4">
      <section className="rounded-xl border border-border bg-card/60 p-4">
        <div className="mb-3 flex items-start justify-between gap-3">
          <div>
            <h3 className="text-sm font-semibold">Workspace</h3>
            <p className="mt-1 text-xs text-muted-foreground">
              Orchestrix scopes file and tool access to the active workspace root.
            </p>
          </div>
          <Button size="sm" variant="outline" onClick={() => handlePickWorkspace().catch(console.error)}>
            <Folder size={12} />
            Change Workspace
          </Button>
        </div>

        <div className="rounded-lg border border-border bg-background/70 px-3 py-2">
          <p className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground/70">Current Path</p>
          <code className="mt-1 block break-all text-xs text-foreground">{workspaceRoot || "(not set)"}</code>
        </div>
      </section>

      <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
        <SummaryCard
          icon={<Server size={13} />}
          label="Configured Providers"
          value={String(configuredProviders)}
          helper="MiniMax and Kimi"
        />
        <SummaryCard
          icon={<Bot size={13} />}
          label="Agent Presets"
          value={String(agentPresets.length)}
          helper="Primary + subagent presets"
        />
        <SummaryCard
          icon={<Sparkles size={13} />}
          label="Workspace Skills"
          value={String(workspaceSkills.length)}
          helper="Discovered from .agents/skills"
        />
        <SummaryCard
          icon={<PlugZap size={13} />}
          label="MCP Servers"
          value={String(mcpServers.length)}
          helper="Server definitions"
        />
        <SummaryCard
          icon={<PlugZap size={13} />}
          label="MCP Tools"
          value={String(mcpTools.length)}
          helper="Cached discovered tools"
        />
      </section>
    </div>
  );
}

function SummaryCard({
  icon,
  label,
  value,
  helper,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  helper: string;
}) {
  return (
    <article className="rounded-xl border border-border bg-card/60 p-3">
      <div className="mb-2 inline-flex items-center gap-1.5 rounded-md border border-border bg-background/70 px-2 py-1 text-[11px] text-muted-foreground">
        {icon}
        {label}
      </div>
      <p className="text-xl font-semibold tracking-tight">{value}</p>
      <p className="mt-1 text-xs text-muted-foreground">{helper}</p>
    </article>
  );
}
