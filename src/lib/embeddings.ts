import { invoke } from "@tauri-apps/api/core";

export type EmbeddingProviderKind = "remote" | "local";
export type EmbeddingTaskType =
  | "RETRIEVAL_QUERY"
  | "RETRIEVAL_DOCUMENT"
  | "SEMANTIC_SIMILARITY"
  | "CLASSIFICATION";

export type EmbeddingOptions = {
  task?: EmbeddingTaskType;
};

export type EmbeddingProviderInfo = {
  id: string;
  kind: EmbeddingProviderKind;
};

export type GeminiEmbeddingConfigView = {
  api_key_configured: boolean;
  model: string;
  timeout_ms: number;
  base_url: string | null;
};

export type OllamaEmbeddingConfig = {
  base_url: string;
  model: string;
  timeout_ms: number;
};

export type TransformersJsEmbeddingConfig = {
  model: string;
  device: string;
  backend: string | null;
  cache_dir: string | null;
  timeout_ms: number;
  bridge_command: string;
  bridge_script: string | null;
};

export type RustHfEmbeddingConfig = {
  model_id: string;
  model_path: string | null;
  cache_dir: string | null;
  runtime: "onnx" | "candle";
  threads: number | null;
  timeout_ms: number;
};

export type EmbeddingConfig = {
  provider: "gemini" | "ollama" | "transformersjs" | "rust-hf";
  normalize_l2: boolean;
  gemini: {
    api_key?: string | null;
    model: string;
    timeout_ms: number;
    base_url?: string | null;
  };
  ollama: OllamaEmbeddingConfig;
  transformersjs: TransformersJsEmbeddingConfig;
  rust_hf: RustHfEmbeddingConfig;
};

export type EmbeddingConfigView = {
  provider: "gemini" | "ollama" | "transformersjs" | "rust-hf";
  normalize_l2: boolean;
  gemini: GeminiEmbeddingConfigView;
  ollama: OllamaEmbeddingConfig;
  transformersjs: TransformersJsEmbeddingConfig;
  rust_hf: RustHfEmbeddingConfig;
};

export type SemanticEmbeddingProvider = {
  id: string;
  kind: EmbeddingProviderKind;
  dims: () => Promise<number | null>;
  embed: (texts: string[], opts?: EmbeddingOptions) => Promise<number[][]>;
};

export async function getEmbeddingConfig(): Promise<EmbeddingConfigView> {
  return invoke<EmbeddingConfigView>("get_embedding_config");
}

export async function setEmbeddingConfig(config: EmbeddingConfig): Promise<EmbeddingConfigView> {
  return invoke<EmbeddingConfigView>("set_embedding_config", { config });
}

export async function getSemanticEmbeddingProvider(): Promise<SemanticEmbeddingProvider> {
  const info = await invoke<EmbeddingProviderInfo>("get_embedding_provider_info");

  return {
    id: info.id,
    kind: info.kind,
    dims: () => invoke<number | null>("embedding_dims"),
    embed: (texts: string[], opts?: EmbeddingOptions) =>
      invoke<number[][]>("embed_texts", {
        texts,
        opts: opts ?? null,
      }),
  };
}
