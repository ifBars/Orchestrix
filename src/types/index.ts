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

export interface ModelCatalogEntry {
  provider: string;
  models: string[];
}

export interface WorkspaceRootView {
  workspace_root: string;
}

export interface WorkspaceReferenceCandidate {
  kind: "file" | "directory" | "skill";
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
}

export interface McpServerConfig {
  id: string;
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  enabled: boolean;
}

export interface McpServerInput {
  id?: string;
  name: string;
  command: string;
  args?: string[];
  env?: Record<string, string>;
  enabled?: boolean;
}

export interface McpToolEntry {
  server_id: string;
  server_name: string;
  tool_name: string;
  description: string;
  input_schema: unknown;
}
