import { Bot, Download, Folder, LoaderCircle, PlugZap, Server, Sparkles } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { ReactNode } from "react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { useUpdaterStore } from "@/stores/updaterStore";
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
  const [
    currentVersion,
    pendingUpdate,
    updaterStatus,
    lastCheckedAt,
    downloadedBytes,
    contentLength,
    updaterError,
    checkForUpdates,
    installUpdate,
  ] = useUpdaterStore(
    useShallow((state) => [
      state.currentVersion,
      state.pendingUpdate,
      state.status,
      state.lastCheckedAt,
      state.downloadedBytes,
      state.contentLength,
      state.error,
      state.checkForUpdates,
      state.installUpdate,
    ])
  );

  const configuredProviders = providerConfigs.filter((item) => item.configured).length;
  const updaterBusy =
    updaterStatus === "checking" ||
    updaterStatus === "downloading" ||
    updaterStatus === "installing" ||
    updaterStatus === "restarting";

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
        <div className="mb-3 flex flex-wrap items-start justify-between gap-3">
          <div className="max-w-2xl">
            <h3 className="text-sm font-semibold">Application Updates</h3>
            <p className="mt-1 text-xs text-muted-foreground">
              Stable desktop releases ship on GitHub Releases. Installed builds check for updates on startup
              and can install them without requiring a fresh manual download.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Button
              size="sm"
              variant="outline"
              disabled={updaterBusy}
              onClick={() => checkForUpdates({ interactive: true, promptOnAvailable: true }).catch(console.error)}
            >
              {updaterStatus === "checking" ? (
                <LoaderCircle size={12} className="animate-spin" />
              ) : (
                <Server size={12} />
              )}
              Check Now
            </Button>
            {pendingUpdate ? (
              <Button
                size="sm"
                disabled={updaterBusy}
                onClick={() => installUpdate().catch(console.error)}
              >
                {updaterBusy ? (
                  <LoaderCircle size={12} className="animate-spin" />
                ) : (
                  <Download size={12} />
                )}
                Install v{pendingUpdate.version}
              </Button>
            ) : null}
          </div>
        </div>

        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(240px,320px)]">
          <div className="rounded-lg border border-border bg-background/70 px-3 py-2">
            <p className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground/70">Status</p>
            <p className="mt-1 text-sm text-foreground">{resolveUpdaterStatus(updaterStatus, pendingUpdate?.version)}</p>
            <div className="mt-2 space-y-1 text-xs text-muted-foreground">
              <p>Current version: {currentVersion ? `v${currentVersion}` : "Loading..."}</p>
              <p>Release channel: stable</p>
              <p>Last checked: {formatTimestamp(lastCheckedAt)}</p>
              {pendingUpdate?.publishedAt ? <p>Published: {formatTimestamp(pendingUpdate.publishedAt)}</p> : null}
              {contentLength ? (
                <p>
                  Download progress: {formatBytes(downloadedBytes)} / {formatBytes(contentLength)}
                </p>
              ) : null}
            </div>
            {pendingUpdate?.notes ? (
              <p className="mt-3 rounded-md border border-border/80 bg-card/80 px-3 py-2 text-xs text-muted-foreground">
                {summarizeNotes(pendingUpdate.notes)}
              </p>
            ) : null}
            {updaterError ? (
              <p className="mt-3 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {updaterError}
              </p>
            ) : null}
          </div>

          <div className="rounded-lg border border-border bg-background/70 px-3 py-2">
            <p className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground/70">Policy</p>
            <div className="mt-1 space-y-1.5 text-xs text-muted-foreground">
              <p>Stable releases auto-update from the latest non-prerelease GitHub release.</p>
              <p>Prereleases stay opt-in so beta builds cannot surprise stable users.</p>
              <p>Updater bundles and metadata are signed during the release workflow.</p>
            </div>
          </div>
        </div>
      </section>

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
          icon={<Download size={13} />}
          label="App Version"
          value={currentVersion ? `v${currentVersion}` : "..."}
          helper="Stable releases install from GitHub Releases"
        />
        <SummaryCard
          icon={<Server size={13} />}
          label="Configured Providers"
          value={String(configuredProviders)}
          helper="Configured model providers"
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

function resolveUpdaterStatus(status: string, pendingVersion?: string) {
  switch (status) {
    case "disabled":
      return "Updater checks are disabled in development builds.";
    case "checking":
      return "Checking GitHub Releases for the next stable build...";
    case "available":
      return pendingVersion ? `Update v${pendingVersion} is ready to install.` : "A new update is available.";
    case "up-to-date":
      return "This build is already on the latest stable release.";
    case "downloading":
      return "Downloading the update package...";
    case "installing":
      return "Installing the downloaded update...";
    case "restarting":
      return "Restarting Orchestrix to finish the update...";
    case "error":
      return "Update check failed. Review the error details below.";
    default:
      return "Packaged builds check for updates automatically on startup.";
  }
}

function formatTimestamp(value: string | null) {
  if (!value) return "Not checked yet";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function summarizeNotes(value: string) {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (normalized.length <= 180) return normalized;
  return `${normalized.slice(0, 177)}...`;
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
