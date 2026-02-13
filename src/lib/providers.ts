import type { ModelCatalogEntry } from "@/types";

export type ProviderOption = {
  id: string;
  label: string;
};

export function providerLabel(providerId: string): string {
  if (providerId === "minimax") return "MiniMax";
  if (providerId === "kimi") return "Kimi";
  if (providerId === "zhipu") return "GLM (Zhipu)";
  return providerId
    .split(/[-_\s]+/)
    .filter((token) => token.length > 0)
    .map((token) => token[0].toUpperCase() + token.slice(1))
    .join(" ");
}

export function providerOptionsFromCatalog(catalog: ModelCatalogEntry[]): ProviderOption[] {
  return catalog.map((entry) => ({
    id: entry.provider,
    label: providerLabel(entry.provider),
  }));
}

export function firstModelForProvider(catalog: ModelCatalogEntry[], provider: string): string {
  return catalog.find((entry) => entry.provider === provider)?.models[0]?.name ?? "";
}
