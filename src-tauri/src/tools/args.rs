//! Typed argument structs for tool inputs.
//!
//! Each struct derives both `serde::Deserialize` and `schemars::JsonSchema` to ensure
//! the JSON schema matches the actual parsing logic. This eliminates hand-written
//! schema drift and provides compile-time type safety.

use schemars::{schema_for, JsonSchema};
use serde::Deserialize;

/// Generate a JSON schema value from a type implementing JsonSchema.
pub fn schema_for_type<T: JsonSchema>() -> serde_json::Value {
    let schema = schema_for!(T);
    serde_json::to_value(schema).unwrap_or_else(|_| serde_json::json!({"type": "object"}))
}

// ============================================================================
// Filesystem tools (fs.rs)
// ============================================================================

/// Arguments for `fs.read` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct FsReadArgs {
    /// Path to the file
    pub path: String,
    /// Start reading from this line number (1-indexed). Default: 1.
    #[serde(default)]
    pub offset: Option<u64>,
    /// Maximum number of lines to read. Default: 2000.
    #[serde(default)]
    pub limit: Option<u64>,
    /// If true, prefix each line with its number (e.g. '1: content'). Default: true.
    #[serde(default = "default_true")]
    pub line_numbers: Option<bool>,
}

fn default_true() -> Option<bool> {
    Some(true)
}

/// Arguments for `fs.write` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct FsWriteArgs {
    /// Path to write to
    pub path: String,
    /// Content to write to the file
    pub content: String,
}

/// Arguments for `fs.list` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct FsListArgs {
    /// Directory path relative to workspace root (default: .)
    #[serde(default)]
    pub path: Option<String>,
    /// If true, walk subdirectories recursively
    #[serde(default)]
    pub recursive: Option<bool>,
    /// Max depth when recursive=true (0 means only the target directory)
    #[serde(default)]
    pub max_depth: Option<u64>,
    /// Max number of entries to return (default: 200)
    #[serde(default)]
    pub limit: Option<u64>,
    /// If true, only include files
    #[serde(default)]
    pub files_only: Option<bool>,
    /// If true, only include directories
    #[serde(default)]
    pub dirs_only: Option<bool>,
}

// ============================================================================
// Git tools (git.rs)
// ============================================================================

/// Arguments for `git.diff` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct GitDiffArgs {
    /// If true, show staged (cached) changes instead of unstaged
    #[serde(default)]
    pub staged: Option<bool>,
}

/// Arguments for `git.apply_patch` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct GitApplyPatchArgs {
    /// The patch content to apply
    pub patch: String,
}

/// Arguments for `git.commit` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct GitCommitArgs {
    /// Commit message
    pub message: String,
}

/// Arguments for `git.log` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct GitLogArgs {
    /// Number of log entries to show (default: 10)
    #[serde(default)]
    pub count: Option<u64>,
}

// ============================================================================
// Command execution tools (cmd.rs)
// ============================================================================

/// Arguments for `cmd.exec` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct CmdExecArgs {
    /// Binary name (e.g. 'mkdir', 'bun', 'node')
    #[serde(default)]
    pub cmd: Option<String>,
    /// Arguments array
    #[serde(default)]
    pub args: Option<Vec<String>>,
    /// Alternative: full shell command string
    #[serde(default)]
    pub command: Option<String>,
    /// Optional relative working directory (e.g. 'frontend'). Avoid using shell 'cd'.
    #[serde(default)]
    pub workdir: Option<String>,
}

// ============================================================================
// Search tools (search.rs)
// ============================================================================

/// Arguments for `search.rg` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SearchRgArgs {
    /// Search pattern (regex by default)
    pub pattern: String,
    /// Directory or file to search (default: workspace root)
    #[serde(default)]
    pub path: Option<String>,
    /// Return structured JSON with file, line, text for each match (default: false)
    #[serde(default)]
    pub json_output: Option<bool>,
    /// Force case-sensitive search (default: smart case)
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    /// Treat pattern as literal string, not regex (default: false)
    #[serde(default)]
    pub fixed_strings: Option<bool>,
    /// Filter by file type (e.g. 'rust', 'ts', 'py', 'js', 'css', 'html', 'json', 'md')
    #[serde(default)]
    pub file_type: Option<String>,
    /// Number of context lines before and after each match (default: 0)
    #[serde(default)]
    pub context_lines: Option<u64>,
    /// Maximum number of matching lines to return (default: unlimited)
    #[serde(default)]
    pub max_results: Option<u64>,
    /// Only return file names that contain a match, not the matching lines (default: false)
    #[serde(default)]
    pub files_with_matches: Option<bool>,
}

// ============================================================================
// File search tools (file_search.rs)
// ============================================================================

/// Arguments for `search.files` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SearchFilesArgs {
    /// Fuzzy search pattern (partial file name, e.g. 'mod.rs', 'component', 'config')
    pub pattern: String,
    /// Directory to search in (relative to workspace root, default: '.')
    #[serde(default)]
    pub path: Option<String>,
    /// Maximum number of results (default: 20, max: 100)
    #[serde(default)]
    pub limit: Option<u64>,
}

// ============================================================================
// Memory tools (memory.rs)
// ============================================================================

/// Arguments for `memory.upsert` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MemoryUpsertArgs {
    /// The preference key
    pub key: String,
    /// The preference value
    pub value: String,
    /// Optional category for grouping preferences
    #[serde(default)]
    pub category: Option<String>,
}

/// Arguments for `memory.delete` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MemoryDeleteArgs {
    /// The preference key to delete
    pub key: String,
}

// ============================================================================
// Patch tools (patch/mod.rs)
// ============================================================================

/// Arguments for `fs.patch` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct FsPatchArgs {
    /// The patch text in apply-patch format. Envelope: *** Begin Patch\n[operations]\n*** End Patch.
    /// Operations: *** Add File: <path> (lines prefixed with +), *** Delete File: <path>,
    /// *** Update File: <path> with @@ context markers and +/- lines.
    /// CRITICAL: Text after @@ must MATCH actual file content (used to find the change location).
    /// Use @@ alone (no context) if uncertain, or use fs.read to verify file content first.
    /// Context lines (prefixed with space) provide additional matching context.
    pub patch: String,
}

// ============================================================================
// Web snapshot tools (web_snapshot.rs)
// ============================================================================

/// Arguments for `web.snapshot` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct WebSnapshotArgs {
    /// URL to capture (e.g., 'http://localhost:3000')
    pub url: String,
    /// CSS selector to wait for before capturing (optional)
    #[serde(default)]
    pub wait_for_selector: Option<String>,
    /// Additional time to wait after page load in milliseconds (default: 1000)
    #[serde(default)]
    pub wait_for_timeout_ms: Option<u64>,
    /// Viewport width in pixels (default: 1280)
    #[serde(default)]
    pub viewport_width: Option<u32>,
    /// Viewport height in pixels (default: 720)
    #[serde(default)]
    pub viewport_height: Option<u32>,
    /// Capture full page scroll height (default: false)
    #[serde(default)]
    pub full_page: Option<bool>,
    /// Maximum time to wait for page load in seconds (default: 30, max: 120)
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

// ============================================================================
// Skills tools (skills.rs)
// ============================================================================

/// Arguments for `skills.list_installed` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SkillsListInstalledArgs {
    /// Filter by skill source (default: all)
    #[serde(default)]
    pub source: Option<String>,
}

/// Arguments for `skills.search` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SkillsSearchArgs {
    /// Search query (e.g., 'react', 'rust', 'documentation')
    pub query: String,
    /// Max results to return (default: 10)
    #[serde(default)]
    pub limit: Option<u64>,
}

/// Arguments for `skills.load` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SkillsLoadArgs {
    /// The skill ID to load (from skills.list_installed or skills.search)
    #[serde(default)]
    pub skill_id: Option<String>,
    /// Fuzzy name match (alternative to skill_id)
    #[serde(default)]
    pub name: Option<String>,
    /// Search query to auto-discover skill (alternative to skill_id)
    #[serde(default)]
    pub query: Option<String>,
}

/// Arguments for `skills.install` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub struct SkillsInstallArgs {
    /// Name of the skill to install (e.g., 'react-best-practices')
    pub skill_name: String,
    /// Optional repo URL (defaults to vercel-labs/agent-skills)
    #[serde(default)]
    pub repo: Option<String>,
}

/// Arguments for `skills.remove` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub struct SkillsRemoveArgs {
    /// ID of the skill to remove
    pub skill_id: String,
}

// ============================================================================
// Semantic search tools (semantic_search.rs)
// ============================================================================

/// Arguments for `search.embeddings` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SearchEmbeddingsArgs {
    /// Semantic query text
    pub query: String,
    /// Maximum number of result chunks to return (default: 8, max: 50)
    #[serde(default)]
    pub limit: Option<u64>,
}

// ============================================================================
// Agent tools (agent.rs)
// ============================================================================

/// Arguments for `agent.ask_user` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AgentAskUserArgs {
    /// The question to ask the user
    pub question: String,
    /// Available options for the user to choose from
    pub options: Vec<UserQuestionOptionInput>,
    /// Allow selecting multiple choices
    #[serde(default)]
    pub multiple: Option<bool>,
    /// Allow custom text input (default: true)
    #[serde(default = "default_true")]
    pub allow_custom: Option<bool>,
    /// Timeout in seconds for the question (default: 300)
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// ID of the option to pre-select as default
    #[serde(default)]
    pub default_option_id: Option<String>,
}

/// Option for user questions.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct UserQuestionOptionInput {
    /// Unique identifier for this option
    pub id: String,
    /// Display label for this option
    pub label: String,
    /// Optional description of this option
    #[serde(default)]
    pub description: Option<String>,
}

/// Arguments for `agent.task` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AgentTaskArgs {
    /// Action to perform: list, set, add, update, clear
    #[serde(default)]
    pub action: Option<String>,
    /// For 'set' or 'update' actions. For update, array position determines which task to update.
    #[serde(default)]
    pub tasks: Option<Vec<serde_json::Value>>,
    /// For 'add' action or 'update' with index
    #[serde(default)]
    pub item: Option<serde_json::Value>,
    /// Optional: specific index for update (legacy)
    #[serde(default)]
    pub index: Option<u64>,
    /// Optional: scope this task list to a specific ID (e.g., agent/run identifier)
    #[serde(default)]
    pub list_id: Option<String>,
}

/// Arguments for `agent.create_artifact` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct CreateArtifactArgs {
    /// Name of the artifact file
    pub filename: String,
    /// Content of the artifact
    pub content: String,
    /// Type of artifact (e.g., 'plan', 'summary')
    #[serde(default)]
    pub kind: Option<String>,
}

/// Arguments for `subagent.spawn` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub struct SubAgentSpawnArgs {
    /// Focused delegated objective
    pub objective: String,
    /// Optional agent preset reference for delegated execution constraints/prompt
    #[serde(default)]
    pub agent_preset_id: Option<String>,
    /// Optional retries for delegated objective
    #[serde(default)]
    pub max_retries: Option<u32>,
}

/// Arguments for `agent.complete` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AgentCompleteArgs {
    /// Required concise completion summary
    pub summary: String,
    /// Optional output paths or artifacts produced
    #[serde(default)]
    pub outputs: Option<Vec<String>>,
    /// Optional completion confidence
    #[serde(default)]
    pub confidence: Option<String>,
}

/// Arguments for `agent.create_preset` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AgentCreatePresetArgs {
    /// Unique kebab-case identifier (e.g., 'code-reviewer'). Must be alphanumeric with hyphens/underscores only.
    pub id: String,
    /// Human-readable display name for the preset
    pub name: String,
    /// Short description of what this agent does (default: empty)
    #[serde(default)]
    pub description: Option<String>,
    /// Agent mode: 'primary' for main task agents, 'subagent' for delegated work
    #[serde(default)]
    pub mode: Option<String>,
    /// System prompt that defines the agent's behavior (markdown body after frontmatter)
    pub prompt: String,
    /// Optional model override (e.g., 'anthropic/claude-sonnet-4-5', 'MiniMax-M2.1')
    #[serde(default)]
    pub model: Option<String>,
    /// Optional temperature setting (0.0 - 1.0). Lower = more deterministic.
    #[serde(default)]
    pub temperature: Option<f64>,
    /// Optional maximum number of steps/iterations (default: unlimited)
    #[serde(default)]
    pub steps: Option<u32>,
    /// Optional tags for categorizing the preset
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Tool permissions (default: all false for subagent, all true for primary)
    #[serde(default)]
    pub tools: Option<AgentToolPermissions>,
}

/// Tool permissions for agent presets.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AgentToolPermissions {
    /// Allow file write operations
    #[serde(default)]
    pub write: Option<bool>,
    /// Allow file edit operations
    #[serde(default)]
    pub edit: Option<bool>,
    /// Allow command execution
    #[serde(default)]
    pub bash: Option<bool>,
}

// ============================================================================
// Dev server tools (dev_server.rs)
// ============================================================================

/// Arguments for `dev_server.start` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct DevServerStartArgs {
    /// Command to start the dev server (e.g., 'bun dev', 'npm run dev', 'vite')
    pub command: String,
    /// Expected port number for health checking (optional, auto-detected if not provided)
    #[serde(default)]
    pub port: Option<u16>,
    /// Working directory relative to workspace root (optional)
    #[serde(default)]
    pub workdir: Option<String>,
    /// URL to health check after starting (defaults to http://localhost:{port})
    #[serde(default)]
    pub health_check_url: Option<String>,
    /// Max seconds to wait for server to be ready (default: 30)
    #[serde(default)]
    pub max_wait_secs: Option<u64>,
}

/// Arguments for `dev_server.stop` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct DevServerStopArgs {
    /// Server ID returned from dev_server.start
    pub server_id: String,
}

/// Arguments for `dev_server.status` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct DevServerStatusArgs {
    /// Server ID returned from dev_server.start
    pub server_id: String,
}

/// Arguments for `dev_server.logs` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct DevServerLogsArgs {
    /// Server ID returned from dev_server.start
    pub server_id: String,
    /// Maximum number of log lines to return (default: 100, max: 500)
    #[serde(default)]
    pub limit: Option<usize>,
    /// Stream to read: 'stdout', 'stderr', or 'both' (default: 'both')
    #[serde(default)]
    pub stream: Option<String>,
}
