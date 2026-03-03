import type { ModelCatalogEntry, ModelInfo } from "@/types";

export type ProviderOption = {
  id: string;
  label: string;
};

export function providerLabel(providerId: string): string {
  if (providerId === "minimax") return "MiniMax";
  if (providerId === "kimi") return "Kimi";
  if (providerId === "zhipu") return "GLM (Zhipu)";
  if (providerId === "modal") return "Modal";
  if (providerId === "openai-chatgpt") return "OpenAI ChatGPT";
  if (providerId === "gemini") return "Gemini";
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

export function getModelInfo(catalog: ModelCatalogEntry[], provider: string, model: string): ModelInfo | undefined {
  return catalog.find((entry) => entry.provider === provider)?.models.find((m) => m.name === model);
}

export function isModelDeprecated(catalog: ModelCatalogEntry[], provider: string, model: string): boolean {
  return getModelInfo(catalog, provider, model)?.deprecated ?? false;
}

export function getModelDeprecationReason(catalog: ModelCatalogEntry[], provider: string, model: string): string | null {
  return getModelInfo(catalog, provider, model)?.deprecation_reason ?? null;
}

export function getModelSuggestedAlternative(catalog: ModelCatalogEntry[], provider: string, model: string): string | null {
  return getModelInfo(catalog, provider, model)?.suggested_alternative ?? null;
}

export function formatModelLabel(model: ModelInfo): string {
  if (model.deprecated) {
    return `${model.name} (Deprecated)`;
  }
  return model.name;
}
