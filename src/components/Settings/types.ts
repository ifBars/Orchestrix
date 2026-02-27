import type { LucideIcon } from "lucide-react";
import { Bot, Box, Brain, Cpu, Server, Settings, TestTube2 } from "lucide-react";

export type SettingsSectionId = "general" | "providers" | "embeddings" | "agents" | "skills" | "mcp" | "context";

export type SettingsSectionItem = {
  id: SettingsSectionId;
  label: string;
  description: string;
  icon: LucideIcon;
};

export const SETTINGS_SECTIONS: readonly SettingsSectionItem[] = [
  {
    id: "general",
    label: "General",
    description: "Workspace path and app-level context",
    icon: Settings,
  },
  {
    id: "providers",
    label: "Providers",
    description: "Model provider API configuration",
    icon: Server,
  },
  {
    id: "embeddings",
    label: "Embeddings",
    description: "Semantic code search embedding provider configuration",
    icon: Cpu,
  },
  {
    id: "agents",
    label: "Agents",
    description: "Human-authored presets discovered from .agents/agents, .agent/agents, and .opencode/agents.",
    icon: Bot,
  },
  {
    id: "skills",
    label: "Skills",
    description: "Workspace skills and skill catalog",
    icon: Box,
  },
  {
    id: "mcp",
    label: "MCP",
    description: "MCP servers and discovered tools",
    icon: TestTube2,
  },
  {
    id: "context",
    label: "Context",
    description: "Memory, compaction, and context budget controls",
    icon: Brain,
  },
];
