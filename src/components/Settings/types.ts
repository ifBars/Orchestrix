export type SettingsSectionId = "general" | "providers" | "embeddings" | "agents" | "skills" | "mcp" | "compaction";

export type SettingsSectionItem = {
  id: SettingsSectionId;
  label: string;
  description: string;
};

export const SETTINGS_SECTIONS: readonly SettingsSectionItem[] = [
  {
    id: "general",
    label: "General",
    description: "Workspace path and app-level context",
  },
  {
    id: "providers",
    label: "Providers",
    description: "Model provider API configuration",
  },
  {
    id: "embeddings",
    label: "Embeddings",
    description: "Semantic code search embedding provider configuration",
  },
  {
    id: "agents",
    label: "Agents",
    description: "Custom agent presets and defaults",
  },
  {
    id: "skills",
    label: "Skills",
    description: "Workspace skills and skill catalog",
  },
  {
    id: "mcp",
    label: "MCP",
    description: "MCP servers and discovered tools",
  },
  {
    id: "compaction",
    label: "Compaction",
    description: "Conversation context management settings",
  },
];
