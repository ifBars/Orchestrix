import { ArrowLeft } from "lucide-react";
import { Button } from "@/components/ui/button";
import { AgentsSection } from "@/components/Settings/AgentsSection";
import { CompactionSection } from "@/components/Settings/CompactionSection";
import { GeneralSection } from "@/components/Settings/GeneralSection";
import { McpSection } from "@/components/Settings/McpSection";
import { ProvidersSection } from "@/components/Settings/ProvidersSection";
import { SkillsSection } from "@/components/Settings/SkillsSection";
import { SETTINGS_SECTIONS, type SettingsSectionId } from "@/components/Settings/types";

type SettingsPageProps = {
  section: SettingsSectionId;
  onBackToChat: () => void;
};

export function SettingsPage({ section, onBackToChat }: SettingsPageProps) {
  const activeSection = SETTINGS_SECTIONS.find((item) => item.id === section) ?? SETTINGS_SECTIONS[0];

  return (
    <section className="flex h-full min-h-0 w-full flex-col py-4">
      <div className="flex items-start justify-between gap-3 border-b border-border/70 px-1 pb-4">
        <div>
          <div className="flex items-center gap-2">
            <span className="text-[11px] font-semibold uppercase tracking-widest text-muted-foreground/70">
              Settings
            </span>
            <span className="text-muted-foreground/40">/</span>
            <span className="text-[11px] font-medium text-muted-foreground">{activeSection.label}</span>
          </div>
          <h1 className="mt-1 text-lg font-semibold tracking-tight">{activeSection.label}</h1>
          <p className="mt-1 text-xs text-muted-foreground">{activeSection.description}</p>
        </div>

        <Button size="sm" variant="outline" onClick={onBackToChat}>
          <ArrowLeft size={12} />
          Back to Chat
        </Button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto pt-4">{renderSection(section)}</div>
    </section>
  );
}

function renderSection(section: SettingsSectionId) {
  switch (section) {
    case "general":
      return <GeneralSection />;
    case "providers":
      return <ProvidersSection />;
    case "agents":
      return <AgentsSection />;
    case "skills":
      return <SkillsSection />;
    case "mcp":
      return <McpSection />;
    case "compaction":
      return <CompactionSection />;
    default:
      return <GeneralSection />;
  }
}
