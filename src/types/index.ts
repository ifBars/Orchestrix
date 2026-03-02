/**
 * Shared TypeScript type definitions.
 *
 * These types mirror the Rust structs exactly and are used for
 * type-safe communication between the frontend and backend via Tauri IPC.
 *
 * Naming conventions:
 * - Row types (database tables): TaskRow, RunRow, EventRow, etc.
 * - View types (API responses): ProviderConfigView, WorkspaceRootView
 * - Enums use string literals: TaskStatus, ToolStatus
 *
 * @see CODING_STANDARDS.md for TypeScript/Rust type mapping rules
 */

/**
 * Status of a task in its lifecycle.
 * Mirrors the Rust TaskStatus enum.
 */
export type TaskStatus =
  | "pending"        // Task created, not started
  | "planning"       // LLM generating plan
  | "awaiting_review" // Plan ready for user approval
  | "executing"      // Plan execution in progress
  | "completed"      // Task completed successfully
  | "failed"         // Task failed during execution
  | "cancelled";     // Task cancelled by user

export interface TaskRow {
  id: string;
  prompt: string;
  parent_task_id: string | null;
  status: TaskStatus;
  created_at: string;
  updated_at: string;
  /** The workspace root path this task was created in. Null for legacy tasks. */
  workspace_root: string | null;
}

export interface TaskLinkRow {
  source_task_id: string;
  target_task_id: string;
  created_at: string;
}

export interface BusEvent {
  id: string;
  run_id: string | null;
  seq: number;
  category: string;
  event_type: string;
  payload: Record<string, unknown>;
  created_at: string;
}

export interface ProviderConfigView {
  provider: string;
  configured: boolean;
  default_model: string | null;
  base_url: string | null;
}

export interface ModelInfo {
  name: string;
  context_window: number;
}

export interface ModelCatalogEntry {
  provider: string;
  models: ModelInfo[];
}

export type BenchmarkWorkload = "llm" | "business_ops" | "llm_and_business_ops";

export interface RunModelBenchmarkRequest {
  run_id?: string | null;
  workload: BenchmarkWorkload;
  providers?: string[] | null;
  provider_models?: Record<string, string> | null;
  warmup_iterations?: number | null;
  measured_iterations?: number | null;
  business_ops_max_turns?: number | null;
  business_ops_prompts_per_day?: number | null;
  business_ops_scenarios?: string[] | null;
}

export type BenchmarkRealtimeEvent =
  | {
      kind: "run_started";
      run_id: string;
      providers: string[];
      scenario_count: number;
      measured_iterations: number;
    }
  | {
      kind: "provider_started";
      run_id: string;
      provider: string;
      model: string | null;
    }
  | {
      kind: "scenario_started";
      run_id: string;
      provider: string;
      scenario_id: string;
      iteration: number;
    }
  | {
      kind: "prompt_completed";
      run_id: string;
      provider: string;
      scenario_id: string;
      day_index: number;
      prompt_index: number;
      action_kind: string;
      tool_calls: number;
    }
  | {
      kind: "warning";
      run_id: string;
      provider: string;
      scenario_id: string;
      day_index: number;
      prompt_index: number;
      message: string;
    }
  | {
      kind: "day_completed";
      run_id: string;
      provider: string;
      scenario_id: string;
      day_index: number;
      ending_cash: number;
      profit_to_date: number;
      service_level: number;
      stockout_rate: number;
    }
  | {
      kind: "scenario_completed";
      run_id: string;
      provider: string;
      scenario_id: string;
      final_score: number;
      raw_profit: number;
    }
  | {
      kind: "run_completed";
      run_id: string;
    };

export interface BusinessOpsScenarioDescriptor {
  scenario_key: string;
  scenario_id: string;
  seed: number;
  description: string;
}

export interface LlmAggregateBenchmarkResult {
  weighted_score: number;
  pass_rate: number;
  success_rate: number;
  avg_p50_latency_ms: number;
}

export interface LlmProviderBenchmarkResult {
  provider: string;
  model: string | null;
  status: string;
  error: string | null;
  aggregate: LlmAggregateBenchmarkResult | null;
}

export interface LlmBenchReport {
  providers: LlmProviderBenchmarkResult[];
}

export interface BusinessOpsAggregateResult {
  avg_score: number;
  avg_profit: number;
  avg_service_level: number;
  avg_solvency: number;
  avg_compliance: number;
  avg_stockout_rate: number;
  success_rate: number;
  bankruptcy_rate: number;
  avg_tool_calls: number;
  avg_latency_ms: number;
}

export interface BusinessOpsProviderResult {
  provider: string;
  model: string | null;
  status: string;
  error: string | null;
  scenarios: BusinessOpsScenarioRunResult[];
  aggregate: BusinessOpsAggregateResult;
}

export interface BusinessOpsDayTrace {
  day_index: number;
  ending_cash: number;
  profit_to_date: number;
  running_service_level: number;
  running_stockout_rate: number;
  prompt_count: number;
  prompts: BusinessOpsPromptTrace[];
}

export interface BusinessOpsPromptTrace {
  prompt_index: number;
  latency_ms: number;
  reasoning: string | null;
  action_kind: string;
  state_snapshot: string;
  tool_calls: BusinessOpsToolCallTrace[];
}

export interface BusinessOpsToolCallTrace {
  tool_name: string;
  args: unknown;
  success: boolean;
  result: string;
}

export interface BusinessOpsScenarioRunResult {
  scenario_id: string;
  seed: number;
  final_score: number;
  raw_profit: number;
  service_level: number;
  solvency_score: number;
  compliance_score: number;
  stockout_rate: number;
  turns_completed: number;
  bankrupt_turn: number | null;
  total_emails_sent: number;
  tool_call_count: number;
  avg_p50_latency_ms: number;
  success_rate: number;
  error: string | null;
  sample_response: string | null;
  parsing_errors: string[];
  timeline: BusinessOpsDayTrace[];
}

export interface BusinessOpsBenchReport {
  providers: BusinessOpsProviderResult[];
}

export interface ModelBenchmarkReport {
  llm: LlmBenchReport | null;
  business_ops: BusinessOpsBenchReport | null;
}

export type EmbeddingProviderKind = "remote" | "local";

export type EmbeddingTaskType =
  | "RETRIEVAL_QUERY"
  | "RETRIEVAL_DOCUMENT"
  | "SEMANTIC_SIMILARITY"
  | "CLASSIFICATION";

export interface GeminiEmbeddingConfigView {
  api_key_configured: boolean;
  model: string;
  timeout_ms: number;
  base_url: string | null;
}

export interface OllamaEmbeddingConfig {
  base_url: string;
  model: string;
  timeout_ms: number;
}

export interface TransformersJsEmbeddingConfig {
  model: string;
  device: string;
  backend: string | null;
  cache_dir: string | null;
  timeout_ms: number;
  bridge_command: string;
  bridge_script: string | null;
}

export interface RustHfEmbeddingConfig {
  model_id: string;
  model_path: string | null;
  cache_dir: string | null;
  runtime: "onnx" | "candle";
  threads: number | null;
  timeout_ms: number;
}

export interface EmbeddingConfigView {
  enabled: boolean;
  provider: "gemini" | "ollama" | "transformersjs" | "rust-hf";
  normalize_l2: boolean;
  gemini: GeminiEmbeddingConfigView;
  ollama: OllamaEmbeddingConfig;
  transformersjs: TransformersJsEmbeddingConfig;
  rust_hf: RustHfEmbeddingConfig;
}

export interface EmbeddingConfig {
  enabled: boolean;
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
}

export interface EmbeddingProviderInfo {
  id: string;
  kind: EmbeddingProviderKind;
}

export interface EmbeddingIndexStatus {
  workspace_root: string;
  provider: string;
  status: "indexing" | "ready" | "failed" | string;
  dims: number | null;
  file_count: number;
  chunk_count: number;
  indexed_at: string | null;
  updated_at: string;
  error: string | null;
}

export interface WorkspaceRootView {
  workspace_root: string;
}

export interface WorkspaceReferenceCandidate {
  kind: "file" | "directory" | "skill" | "agent";
  value: string;
  display: string;
  description: string;
  group: string;
}

export interface SkillCatalogItem {
  id: string;
  title: string;
  description: string;
  install_command: string;
  url: string;
  source: string;
  tags: string[];
  is_custom: boolean;
}

export interface AgentSkillSearchItem {
  skill_name: string;
  title: string;
  description: string;
  source: string;
  installs: number;
  url: string;
  install_command: string;
}

export interface AgentSkillInstallResult {
  skill_name: string;
  command: string;
  stdout: string;
  stderr: string;
  exit_code: number | null;
}

export interface NewCustomSkill {
  id?: string;
  title: string;
  description: string;
  install_command: string;
  url: string;
  source?: string;
  tags?: string[];
}

export interface RunRow {
  id: string;
  task_id: string;
  status: string;
  plan_json: string | null;
  started_at: string;
  finished_at: string | null;
  failure_reason: string | null;
}

export interface SubAgentRow {
  id: string;
  run_id: string;
  step_idx: number;
  name: string;
  status: string;
  worktree_path: string | null;
  context_json: string | null;
  started_at: string | null;
  finished_at: string | null;
  error: string | null;
}

export interface ToolCallRow {
  id: string;
  run_id: string;
  step_idx: number | null;
  tool_name: string;
  input_json: string;
  output_json: string | null;
  status: string;
  started_at: string | null;
  finished_at: string | null;
  error: string | null;
}

export interface ArtifactRow {
  id: string;
  run_id: string;
  kind: string;
  uri_or_content: string;
  metadata_json: string | null;
  created_at: string;
}

export interface ArtifactContentView {
  path: string;
  content: string;
  is_markdown: boolean;
}

export interface WorktreeView {
  path: string;
  branch: string | null;
  strategy: string;
  run_id: string;
  sub_agent_id: string;
  base_ref: string | null;
}

export interface WorktreeLogRow {
  id: string;
  run_id: string;
  sub_agent_id: string;
  strategy: string;
  branch_name: string | null;
  base_ref: string | null;
  worktree_path: string;
  merge_strategy: string | null;
  merge_success: boolean | null;
  merge_message: string | null;
  conflicted_files_json: string | null;
  created_at: string;
  merged_at: string | null;
  cleaned_at: string | null;
}

export interface EventRow {
  id: string;
  run_id: string | null;
  seq: number;
  category: string;
  event_type: string;
  payload_json: string;
  created_at: string;
}

export interface ApprovalRequestView {
  id: string;
  task_id: string;
  run_id: string;
  sub_agent_id: string;
  tool_call_id: string;
  tool_name: string;
  scope: string;
  reason: string;
  created_at: string;
}

export interface UserQuestionOption {
  id: string;
  label: string;
  description?: string | null;
}

export interface UserQuestionRequestView {
  id: string;
  task_id: string;
  run_id: string;
  sub_agent_id: string;
  tool_call_id: string;
  question: string;
  options: UserQuestionOption[];
  multiple: boolean;
  allow_custom: boolean;
  created_at: string;
}

export interface UserQuestionAnswer {
  selected_option_ids: string[];
  custom_text?: string | null;
  final_text: string;
}

export interface UserMessageRow {
  id: string;
  task_id: string;
  run_id: string | null;
  content: string;
  created_at: string;
}

export interface GitWorktreeEntry {
  path: string;
  head: string | null;
  branch: string | null;
  is_bare: boolean;
}

/**
 * A workspace skill discovered from `.agents/skills/<name>/SKILL.md`.
 * Mirrors the Rust WorkspaceSkill struct.
 */
export interface WorkspaceSkill {
  id: string;
  name: string;
  description: string;
  content: string;
  skill_dir: string;
  skill_file: string;
  source: string;
  files: string[];
  tags: string[];
  enabled: boolean;
  is_builtin: boolean;
}

export type McpTransportType = "stdio" | "http" | "sse";

export interface McpAuthConfig {
  oauth_token?: string;
  headers: Record<string, string>;
  api_key?: string;
  api_key_header?: string;
}

export interface ToolFilter {
  mode: "include" | "exclude";
  tools: string[];
  allow_all_read_only: boolean;
  block_all_modifying: boolean;
}

export interface ToolOverride {
  pattern: string;
  requires_approval: boolean;
  is_glob: boolean;
}

export interface ToolApprovalPolicy {
  global_policy: "always" | "never" | "by_tool";
  tool_overrides: ToolOverride[];
  read_only_never_requires_approval: boolean;
  modifying_always_requires_approval: boolean;
}

export interface McpServerConfig {
  id: string;
  name: string;
  transport: McpTransportType;
  enabled: boolean;
  
  // Stdio fields
  command?: string;
  args: string[];
  env: Record<string, string>;
  working_dir?: string;
  
  // HTTP/SSE fields
  url?: string;
  auth: McpAuthConfig;
  timeout_secs: number;
  pool_size: number;
  health_check_interval_secs: number;
  
  // Filtering and approval
  tool_filter: ToolFilter;
  approval_policy: ToolApprovalPolicy;
}

export interface McpServerInput {
  id?: string;
  name: string;
  transport?: McpTransportType;
  enabled?: boolean;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  working_dir?: string;
  url?: string;
  auth?: McpAuthConfig;
  timeout_secs?: number;
  pool_size?: number;
  tool_filter?: ToolFilter;
  approval_policy?: ToolApprovalPolicy;
}

export interface McpServerHealthView {
  status: "healthy" | "connecting" | "unhealthy" | "disabled";
  last_check?: string;
  connected_at?: string;
  error_count: number;
}

export interface McpServerView {
  id: string;
  name: string;
  transport: string;
  enabled: boolean;
  command?: string;
  args: string[];
  url?: string;
  timeout_secs: number;
  pool_size: number;
  tool_count: number;
  health?: McpServerHealthView;
}

export interface McpToolEntry {
  server_id: string;
  server_name: string;
  tool_name: string;
  description: string;
  input_schema: unknown;
  read_only_hint?: boolean;
  requires_approval: boolean;
}

export interface McpToolView extends McpToolEntry {}

export interface McpToolsCacheView {
  total_tools: number;
  server_count: number;
  updated_at: string;
}

export interface McpToolCallResult {
  success: boolean;
  result?: unknown;
  error?: string;
  duration_ms: number;
}

export interface McpConnectionTestResult {
  success: boolean;
  error?: string;
  latency_ms?: number;
  tool_count?: number;
}

export interface McpStatisticsView {
  server_count: number;
  healthy_server_count: number;
  total_tools: number;
}

/**
 * Agent mode - primary agents can be selected for tasks, subagents can be delegated to.
 */
export type AgentMode = "primary" | "subagent";

/**
 * Tool permission configuration.
 */
export type ToolPermission = boolean | "inherit" | Record<string, unknown>;

/**
 * Permission overrides for tools and operations.
 */
export interface PermissionConfig {
  edit?: ToolPermission;
  bash?: ToolPermission;
  write?: ToolPermission;
  webfetch?: ToolPermission;
  skill?: Record<string, string>;
}

/**
 * A discovered agent preset from an agent markdown file.
 * Mirrors the Rust AgentPreset struct with OpenCode-compatible frontmatter.
 */
export interface AgentPreset {
  id: string;
  name: string;
  description: string;
  mode: AgentMode;
  model?: string;
  temperature?: number;
  steps?: number;
  tools?: Record<string, ToolPermission>;
  permission?: PermissionConfig;
  prompt: string;
  tags: string[];
  file_path: string;
  source: "workspace" | "global" | "opencode";
  enabled: boolean;
  validation_issues: string[];
}

/**
 * Input for creating or updating an agent preset.
 */
export interface CreateAgentPresetInput {
  id: string;
  name: string;
  description: string;
  mode: AgentMode;
  model?: string;
  temperature?: number;
  steps?: number;
  prompt: string;
  tags?: string[];
  tools?: Record<string, unknown>;
}

/**
 * Settings for conversation compaction behavior.
 */
export interface CompactionSettings {
  enabled: boolean;
  /** Percentage of context window to trigger compaction (0.0-1.0) */
  threshold_percentage: number;
  preserve_recent: number;
  custom_prompt: string | null;
  compaction_model: string | null;
}

/**
 * Settings for plan mode behavior.
 */
export interface PlanModeSettings {
  /** Maximum tokens for plan mode responses (content + reasoning + tool calls) */
  max_tokens: number;
}

export interface AutoMemorySettingsView {
  enabled: boolean;
  source: string;
}

export interface ContextUsageSegmentView {
  key: string;
  label: string;
  tokens: number;
  percentage: number;
}

export interface TaskContextSnapshotView {
  task_id: string;
  provider: string | null;
  model: string | null;
  mode: string | null;
  context_window: number;
  used_tokens: number;
  free_tokens: number;
  usage_percentage: number;
  segments: ContextUsageSegmentView[];
  updated_at: string;
  estimated: boolean;
}

export interface AutoMemoryPathView {
  path: string;
}

export interface MemoryPreferenceEntry {
  key: string;
  value: string;
  category: string | null;
  updated_at: string;
}

// ──────────────────────────────────────────────
// Architecture Canvas types
// ──────────────────────────────────────────────

/** A node in the architecture canvas. x/y are optional — AI-added nodes
 *  without position will be auto-laid out by dagre on the frontend. */
export interface CanvasNode {
  id: string;
  label: string;
  kind?: string;         // e.g. "component", "service", "concept" — used for styling
  description?: string;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  metadata?: Record<string, unknown>;
}

/** A directed edge between two canvas nodes. */
export interface CanvasEdge {
  id: string;
  source: string;
  target: string;
  label?: string;
}

/** Full canvas state stored in SQLite and transferred over IPC. */
export interface CanvasState {
  nodes: CanvasNode[];
  edges: CanvasEdge[];
}

/** Mirrors `TaskCanvasRow` from Rust. */
export interface TaskCanvasRow {
  task_id: string;
  state_json: string;
  updated_at: string;
}

/** Payload of the `canvas.updated` Tauri event. */
export interface CanvasUpdatedPayload {
  task_id: string;
  state_json: string;
}
