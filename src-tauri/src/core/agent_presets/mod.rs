//! Agent preset scanner and loader.
//!
//! Discovers agent markdown files in workspace and global directories,
//! parses YAML frontmatter per OpenCode format, and exposes structured
//! agent preset data for use in the app.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::provider::parse_model_override as parse_provider_model_override;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Agent mode - primary agents can be selected for tasks, subagents can be delegated to.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    #[default]
    Subagent,
    Primary,
}

/// Tool permission configuration.
/// Supports simple boolean or per-tool granular permissions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum ToolPermission {
    #[default]
    Inherit,
    Bool(bool),
    Granular(HashMap<String, serde_json::Value>),
}

/// Permission overrides for tools and operations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit: Option<ToolPermission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bash: Option<ToolPermission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write: Option<ToolPermission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webfetch: Option<ToolPermission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<HashMap<String, String>>,
}

/// A discovered agent preset from an agent markdown file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPreset {
    /// Unique identifier derived from the filename (without .md extension).
    pub id: String,
    /// Human-readable name (from frontmatter `name`, or filename).
    pub name: String,
    /// Short description (from frontmatter `description`).
    pub description: String,
    /// Agent mode: primary or subagent.
    pub mode: AgentMode,
    /// The model to use (e.g., "anthropic/claude-sonnet-4-5", "minimax/MiniMax-M2.1").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Temperature setting (0.0 - 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Maximum steps/iterations allowed for this agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
    /// Tool permissions map (tool_name -> allow/deny/granular).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<HashMap<String, ToolPermission>>,
    /// Permission overrides (edit, bash, write, webfetch, skill).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<PermissionConfig>,
    /// The system prompt (markdown body after frontmatter).
    pub prompt: String,
    /// Optional tags from frontmatter.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
    /// Absolute path to the agent file.
    pub file_path: String,
    /// Where the agent was found: "workspace", "global", or "opencode".
    pub source: String,
    /// Whether the agent is currently enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Validation issues detected during parsing.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub validation_issues: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl AgentPreset {
    /// Get the effective tool permission for a given tool name.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn tool_allowed(&self, tool_name: &str) -> bool {
        // If there's an explicit tools map, check it first
        if let Some(tools) = &self.tools {
            match tools.get(tool_name) {
                Some(ToolPermission::Bool(false)) => return false,
                Some(ToolPermission::Bool(true)) => return true,
                _ => {}
            }
        }

        // Check permission overrides for specific tool categories
        if let Some(perm) = &self.permission {
            let allowed = match tool_name {
                "fs.write" | "edit" => perm
                    .edit
                    .as_ref()
                    .or(perm.write.as_ref())
                    .map(|p| matches!(p, ToolPermission::Bool(true))),
                "cmd.exec" | "bash" => perm
                    .bash
                    .as_ref()
                    .map(|p| matches!(p, ToolPermission::Bool(true))),
                "webfetch" => perm
                    .webfetch
                    .as_ref()
                    .map(|p| matches!(p, ToolPermission::Bool(true))),
                _ => None,
            };

            if let Some(allowed) = allowed {
                return allowed;
            }
        }

        // Default: allow for primary, deny for subagent
        self.mode == AgentMode::Primary
    }

    /// Get a summary of constraints for display/debugging.
    pub fn constraints_summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(steps) = self.steps {
            parts.push(format!("max {} steps", steps));
        }

        if self.mode == AgentMode::Subagent {
            parts.push("subagent mode".to_string());
        }

        if let Some(tools) = &self.tools {
            let denied: Vec<_> = tools
                .iter()
                .filter_map(|(name, perm)| match perm {
                    ToolPermission::Bool(false) => Some(name.clone()),
                    _ => None,
                })
                .collect();
            if !denied.is_empty() {
                parts.push(format!("no {}", denied.join(", ")));
            }
        }

        if parts.is_empty() {
            "default constraints".to_string()
        } else {
            parts.join(" | ")
        }
    }
}

/// Parsed YAML frontmatter from an agent markdown file.
#[derive(Debug, Clone, Default)]
struct AgentFrontmatter {
    name: Option<String>,
    description: Option<String>,
    mode: Option<AgentMode>,
    model: Option<String>,
    temperature: Option<f32>,
    steps: Option<u32>,
    tools: Option<HashMap<String, ToolPermission>>,
    permission: Option<PermissionConfig>,
    tags: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Scanner
// ---------------------------------------------------------------------------

/// Directory precedence for agent discovery (highest to lowest).
/// Workspace-local agents take priority over global ones.
pub const AGENT_DIR_PRECEDENCE: &[(&str, &str)] = &[
    (".agents/agents", "workspace"),  // Custom Orchestrix location
    (".agent/agents", "workspace"),   // Custom alias
    (".opencode/agents", "opencode"), // OpenCode standard
];

/// Global config directory precedence.
pub const GLOBAL_AGENT_DIR_PRECEDENCE: &[(&str, &str)] = &[
    ("orchestrix/agents", "global"), // Orchestrix global
    ("opencode/agents", "opencode"), // OpenCode standard global
];

/// Scan for agent presets in the given workspace root and global directories.
///
/// Follows OpenCode-compatible precedence:
/// 1. Workspace `.agents/agents/*.md`
/// 2. Workspace `.agent/agents/*.md`
/// 3. Workspace `.opencode/agents/*.md`
/// 4. Global `~/.config/orchestrix/agents/*.md`
/// 5. Global `~/.config/opencode/agents/*.md`
///
/// First agent by ID wins (workspace takes priority over global).
pub fn scan_agent_presets(workspace_root: &Path) -> Vec<AgentPreset> {
    let mut presets = Vec::new();
    let mut seen = HashSet::new();

    // 1. Scan workspace directories in precedence order
    for (dir_name, source) in AGENT_DIR_PRECEDENCE {
        let dir = workspace_root.join(dir_name);
        if dir.is_dir() {
            collect_agents_from_dir(&dir, source, &mut presets, &mut seen);
        }
    }

    // 2. Scan global directories
    for global_config_dir in global_config_dirs() {
        for (subdir, source) in GLOBAL_AGENT_DIR_PRECEDENCE {
            let dir = global_config_dir.join(subdir);
            if dir.is_dir() {
                collect_agents_from_dir(&dir, source, &mut presets, &mut seen);
            }
        }
    }

    // 3. Sort by name for consistent display
    presets.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });

    presets
}

/// Get a single agent preset by ID.
pub fn get_agent_preset(workspace_root: &Path, id: &str) -> Option<AgentPreset> {
    let presets = scan_agent_presets(workspace_root);
    presets.into_iter().find(|p| p.id == id)
}

/// Extract the first `@agent:<id>` mention from prompt text.
pub fn extract_agent_preset_id_from_prompt(prompt: &str) -> Option<String> {
    let marker = "@agent:";
    let idx = prompt.find(marker)?;
    let start = idx + marker.len();
    let mut out = String::new();

    for ch in prompt[start..].chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
            continue;
        }
        break;
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Resolve an agent preset directly from a prompt containing `@agent:<id>`.
pub fn resolve_agent_preset_from_prompt(
    prompt: &str,
    workspace_root: &Path,
) -> Option<AgentPreset> {
    let id = extract_agent_preset_id_from_prompt(prompt)?;
    get_agent_preset(workspace_root, &id)
}

/// Parse model override strings in `<provider>/<model>` format.
/// Returns optional provider override and normalized model value.
pub fn parse_model_override(value: &str) -> (Option<String>, String) {
    let (provider, model) = parse_provider_model_override(value);
    let provider = provider.map(|p| p.as_str().to_string());
    (provider, model)
}

/// Read the content of an agent file.
pub fn read_agent_file(file_path: &str) -> Result<String, String> {
    std::fs::read_to_string(file_path).map_err(|e| format!("failed to read agent file: {e}"))
}

/// Write an agent preset to a file.
pub fn write_agent_preset(
    workspace_root: &Path,
    id: &str,
    preset: &AgentPreset,
) -> Result<String, String> {
    // Use .agents/agents as the default write location
    let agents_dir = workspace_root.join(".agents").join("agents");
    std::fs::create_dir_all(&agents_dir)
        .map_err(|e| format!("failed to create agents directory: {e}"))?;

    let file_path = agents_dir.join(format!("{}.md", id));

    // Build frontmatter
    let mut frontmatter = String::from("---\n");
    frontmatter.push_str(&format!("name: {}\n", preset.name));
    frontmatter.push_str(&format!("description: {}\n", preset.description));
    frontmatter.push_str(&format!(
        "mode: {}\n",
        match preset.mode {
            AgentMode::Primary => "primary",
            AgentMode::Subagent => "subagent",
        }
    ));

    if let Some(model) = &preset.model {
        frontmatter.push_str(&format!("model: {}\n", model));
    }
    if let Some(temp) = preset.temperature {
        frontmatter.push_str(&format!("temperature: {}\n", temp));
    }
    if let Some(steps) = preset.steps {
        frontmatter.push_str(&format!("steps: {}\n", steps));
    }

    if !preset.tags.is_empty() {
        frontmatter.push_str("tags:\n");
        for tag in &preset.tags {
            frontmatter.push_str(&format!("  - {}\n", tag));
        }
    }

    // Tools permissions
    if let Some(tools) = &preset.tools {
        frontmatter.push_str("tools:\n");
        for (name, perm) in tools {
            let value = match perm {
                ToolPermission::Bool(true) => "true",
                ToolPermission::Bool(false) => "false",
                _ => "inherit",
            };
            frontmatter.push_str(&format!("  {}: {}\n", name, value));
        }
    }

    // Permission overrides
    if let Some(perm) = &preset.permission {
        frontmatter.push_str("permission:\n");
        if let Some(edit) = &perm.edit {
            if let ToolPermission::Bool(val) = edit {
                frontmatter.push_str(&format!("  edit: {}\n", val));
            }
        }
        if let Some(bash) = &perm.bash {
            if let ToolPermission::Bool(val) = bash {
                frontmatter.push_str(&format!("  bash: {}\n", val));
            }
        }
        if let Some(write) = &perm.write {
            if let ToolPermission::Bool(val) = write {
                frontmatter.push_str(&format!("  write: {}\n", val));
            }
        }
    }

    frontmatter.push_str("---\n\n");

    // Write full content
    let content = format!("{}{}", frontmatter, preset.prompt);
    std::fs::write(&file_path, content).map_err(|e| format!("failed to write agent file: {e}"))?;

    Ok(file_path.to_string_lossy().to_string())
}

/// Delete an agent preset file.
pub fn delete_agent_preset(workspace_root: &Path, id: &str) -> Result<(), String> {
    let agents_dir = workspace_root.join(".agents").join("agents");
    let file_path = agents_dir.join(format!("{}.md", id));

    if !file_path.exists() {
        return Err(format!("agent preset not found: {}", id));
    }

    std::fs::remove_file(&file_path).map_err(|e| format!("failed to delete agent file: {e}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn collect_agents_from_dir(
    dir: &Path,
    source: &str,
    out: &mut Vec<AgentPreset>,
    seen: &mut HashSet<String>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Only process .md files directly in this directory
        if !path.is_file() {
            continue;
        }

        let is_markdown = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
            .unwrap_or(false);
        if !is_markdown {
            continue;
        }

        let file_name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Skip if we've already seen this ID (higher precedence wins)
        if seen.contains(&file_name) {
            continue;
        }

        let raw = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        let (mut preset, issues) = parse_agent_file(&file_name, &raw, &path, source);
        preset.validation_issues = issues;

        seen.insert(file_name.clone());
        out.push(preset);
    }
}

fn parse_agent_file(id: &str, raw: &str, path: &Path, source: &str) -> (AgentPreset, Vec<String>) {
    let mut issues = Vec::new();
    let (frontmatter, body) = parse_frontmatter(raw);

    let fm = parse_agent_frontmatter(&frontmatter, &mut issues);

    // Validate and normalize
    let mode = fm.mode.unwrap_or_else(|| {
        // Default: subagent for safety
        issues.push("mode not specified, defaulting to 'subagent'".to_string());
        AgentMode::Subagent
    });

    // Validate temperature range
    let temperature = fm.temperature.filter(|t| {
        if *t < 0.0 || *t > 2.0 {
            issues.push(format!(
                "temperature {} out of range [0.0, 2.0], ignoring",
                t
            ));
            false
        } else {
            true
        }
    });

    // Validate steps is reasonable
    let steps = fm.steps.filter(|s| {
        if *s > 1000 {
            issues.push(format!("steps {} seems excessive, capping at 1000", s));
            false
        } else {
            true
        }
    });

    let preset = AgentPreset {
        id: id.to_string(),
        name: fm.name.unwrap_or_else(|| title_case(id)),
        description: fm.description.unwrap_or_default(),
        mode,
        model: fm.model,
        temperature,
        steps,
        tools: fm.tools,
        permission: fm.permission,
        prompt: body,
        file_path: path.to_string_lossy().to_string(),
        source: source.to_string(),
        tags: fm.tags.unwrap_or_default(),
        enabled: true,
        validation_issues: Vec::new(),
    };

    (preset, issues)
}

/// Parse YAML-style frontmatter delimited by `---` at the top of a markdown file.
fn parse_frontmatter(raw: &str) -> (String, String) {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return (String::new(), raw.to_string());
    }

    // Find the closing ---
    let after_first = &trimmed[3..];
    let close_idx = match after_first.find("\n---") {
        Some(idx) => idx,
        None => return (String::new(), raw.to_string()),
    };

    let frontmatter_block = after_first[..close_idx].trim();
    let body_start = 3 + close_idx + 4; // skip past "\n---"
    let body = if body_start < trimmed.len() {
        trimmed[body_start..].trim_start().to_string()
    } else {
        String::new()
    };

    (frontmatter_block.to_string(), body)
}

fn parse_agent_frontmatter(block: &str, issues: &mut Vec<String>) -> AgentFrontmatter {
    let mut fm = AgentFrontmatter::default();
    let mut current_section: Option<&str> = None;
    let mut tools: HashMap<String, ToolPermission> = HashMap::new();
    let mut permission = PermissionConfig::default();
    let mut tags: Vec<String> = Vec::new();

    for raw_line in block.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }

        let is_indented = raw_line.starts_with("  ") || raw_line.starts_with('\t');
        let trimmed = line.trim();

        if is_indented {
            match current_section {
                Some("tools") => {
                    if let Some((k, v)) = parse_key_value(trimmed) {
                        tools.insert(k.to_string(), parse_tool_permission(v));
                    }
                }
                Some("permission") => {
                    if let Some((k, v)) = parse_key_value(trimmed) {
                        let parsed = Some(parse_tool_permission(v));
                        match k {
                            "edit" => permission.edit = parsed,
                            "bash" => permission.bash = parsed,
                            "write" => permission.write = parsed,
                            "webfetch" => permission.webfetch = parsed,
                            _ => {}
                        }
                    }
                }
                Some("tags") => {
                    if let Some(item) = trimmed.strip_prefix("- ") {
                        let value = item.trim();
                        if !value.is_empty() {
                            tags.push(value.to_string());
                        }
                    }
                }
                _ => {}
            }
            continue;
        }

        if let Some((key, value)) = parse_key_value(trimmed) {
            current_section = None;
            match key {
                "name" => fm.name = Some(value.to_string()),
                "description" => fm.description = Some(value.to_string()),
                "mode" => {
                    fm.mode = match value.to_ascii_lowercase().as_str() {
                        "primary" => Some(AgentMode::Primary),
                        "subagent" => Some(AgentMode::Subagent),
                        _ => {
                            issues.push(format!(
                                "unknown mode '{}', expected primary|subagent",
                                value
                            ));
                            None
                        }
                    };
                }
                "model" => fm.model = Some(value.to_string()),
                "temperature" => match value.parse::<f32>() {
                    Ok(v) => fm.temperature = Some(v),
                    Err(_) => {
                        issues.push(format!("invalid temperature '{}', expected float", value))
                    }
                },
                "steps" => match value.parse::<u32>() {
                    Ok(v) => fm.steps = Some(v),
                    Err(_) => issues.push(format!("invalid steps '{}', expected integer", value)),
                },
                "tags" => {
                    if value.is_empty() {
                        current_section = Some("tags");
                    } else {
                        tags.extend(
                            value
                                .split(',')
                                .map(|t| t.trim())
                                .filter(|t| !t.is_empty())
                                .map(|t| t.to_string()),
                        );
                    }
                }
                "tools" => {
                    current_section = Some("tools");
                }
                "permission" => {
                    current_section = Some("permission");
                }
                _ => {}
            }
        }
    }

    if !tools.is_empty() {
        fm.tools = Some(tools);
    }
    if permission.edit.is_some()
        || permission.bash.is_some()
        || permission.write.is_some()
        || permission.webfetch.is_some()
        || permission.skill.is_some()
    {
        fm.permission = Some(permission);
    }
    if !tags.is_empty() {
        fm.tags = Some(tags);
    }

    fm
}

fn parse_key_value(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    Some((key.trim(), value.trim()))
}

fn parse_tool_permission(value: &str) -> ToolPermission {
    match value.to_ascii_lowercase().as_str() {
        "true" | "allow" => ToolPermission::Bool(true),
        "false" | "deny" => ToolPermission::Bool(false),
        _ => ToolPermission::Inherit,
    }
}

fn title_case(s: &str) -> String {
    s.split(|c: char| c == '-' || c == '_' || c == ' ')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn global_config_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            let trimmed = app_data.trim();
            if !trimmed.is_empty() {
                dirs.push(PathBuf::from(trimmed));
            }
        }

        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let trimmed = user_profile.trim();
            if !trimmed.is_empty() {
                dirs.push(PathBuf::from(trimmed).join(".config"));
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let trimmed = home.trim();
            if !trimmed.is_empty() {
                dirs.push(PathBuf::from(trimmed).join(".config"));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let trimmed = home.trim();
            if !trimmed.is_empty() {
                dirs.push(PathBuf::from(trimmed).join(".config"));
            }
        }
    }

    let mut unique = HashSet::new();
    dirs.retain(|path| unique.insert(path.clone()));
    dirs
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_basic() {
        let input = r#"---
name: Code Reviewer
description: Reviews code for quality
mode: subagent
model: anthropic/claude-sonnet-4-5
temperature: 0.1
tools:
  write: false
  edit: false
---

You are a code reviewer. Focus on quality."#;

        let (fm, body) = parse_frontmatter(input);
        assert!(fm.contains("name: Code Reviewer"));
        assert!(body.contains("You are a code reviewer"));
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let input = "# Just markdown\n\nNo frontmatter.";
        let (fm, body) = parse_frontmatter(input);
        assert!(fm.is_empty());
        assert_eq!(body, input);
    }

    #[test]
    fn test_title_case() {
        assert_eq!(title_case("code-reviewer"), "Code Reviewer");
        assert_eq!(title_case("security_auditor"), "Security Auditor");
        assert_eq!(title_case("quick thinker"), "Quick Thinker");
    }

    #[test]
    fn test_tool_allowed_defaults() {
        let primary = AgentPreset {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            mode: AgentMode::Primary,
            model: None,
            temperature: None,
            steps: None,
            tools: None,
            permission: None,
            prompt: "Test".to_string(),
            file_path: "/test.md".to_string(),
            source: "test".to_string(),
            tags: vec![],
            enabled: true,
            validation_issues: vec![],
        };

        let subagent = AgentPreset {
            mode: AgentMode::Subagent,
            ..primary.clone()
        };

        // Primary defaults to allowed
        assert!(primary.tool_allowed("fs.write"));
        assert!(primary.tool_allowed("cmd.exec"));

        // Subagent defaults to denied for potentially dangerous tools
        assert!(!subagent.tool_allowed("fs.write"));
        assert!(!subagent.tool_allowed("cmd.exec"));
    }

    #[test]
    fn test_tool_allowed_explicit() {
        let mut tools = HashMap::new();
        tools.insert("write".to_string(), ToolPermission::Bool(true));
        tools.insert("bash".to_string(), ToolPermission::Bool(false));

        let preset = AgentPreset {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            mode: AgentMode::Subagent,
            model: None,
            temperature: None,
            steps: None,
            tools: Some(tools),
            permission: None,
            prompt: "Test".to_string(),
            file_path: "/test.md".to_string(),
            source: "test".to_string(),
            tags: vec![],
            enabled: true,
            validation_issues: vec![],
        };

        assert!(preset.tool_allowed("write"));
        assert!(!preset.tool_allowed("bash"));
    }

    #[test]
    fn test_extract_agent_preset_id_from_prompt() {
        assert_eq!(
            extract_agent_preset_id_from_prompt("Please use @agent:code-reviewer for this"),
            Some("code-reviewer".to_string())
        );
        assert_eq!(
            extract_agent_preset_id_from_prompt("No agent mention"),
            None
        );
    }

    #[test]
    fn test_parse_model_override() {
        let (provider, model) = parse_model_override("kimi/kimi-k2.5");
        assert_eq!(provider.as_deref(), Some("kimi"));
        assert_eq!(model, "kimi-k2.5");

        let (provider, model) = parse_model_override("MiniMax-M2.1");
        assert_eq!(provider, None);
        assert_eq!(model, "MiniMax-M2.1");
    }

    #[test]
    fn test_scan_agent_presets_workspace_agents_dir() {
        let root =
            std::env::temp_dir().join(format!("orchestrix-agent-presets-{}", uuid::Uuid::new_v4()));
        let agents_dir = root.join(".agents").join("agents");
        std::fs::create_dir_all(&agents_dir).expect("create agents dir");

        let agent_file = agents_dir.join("code-reviewer.md");
        std::fs::write(
            &agent_file,
            "---\ndescription: Reviews code\nmode: subagent\n---\n\nYou are a code reviewer.",
        )
        .expect("write agent file");

        let presets = scan_agent_presets(&root);
        assert_eq!(presets.len(), 1);

        let preset = &presets[0];
        assert_eq!(preset.id, "code-reviewer");
        assert_eq!(preset.description, "Reviews code");
        assert_eq!(preset.mode, AgentMode::Subagent);
        assert_eq!(preset.source, "workspace");

        std::fs::remove_dir_all(&root).expect("cleanup temp dir");
    }
}
