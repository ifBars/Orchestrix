//! Workspace skills scanner.
//!
//! Discovers `.agents/skills/` directories in the workspace root (and
//! optionally the global user config directory), parses `SKILL.md`
//! frontmatter, and exposes structured skill data to the rest of the app.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single workspace skill discovered from `.agents/skills/<name>/SKILL.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSkill {
    /// Unique identifier derived from the directory name.
    pub id: String,
    /// Human-readable name (from frontmatter `name`, or the directory name).
    pub name: String,
    /// Short description (from frontmatter `description`).
    pub description: String,
    /// The full markdown body of SKILL.md (after frontmatter).
    pub content: String,
    /// Absolute path to the skill directory.
    pub skill_dir: String,
    /// Absolute path to the SKILL.md file.
    pub skill_file: String,
    /// Where the skill was found: "workspace" or "global".
    pub source: String,
    /// Extra files in the skill directory (relative to skill_dir).
    pub files: Vec<String>,
    /// Optional tags from frontmatter.
    pub tags: Vec<String>,
    /// Whether the skill is currently enabled for agent injection.
    /// Defaults to true when discovered.
    pub enabled: bool,
}

/// Parsed YAML-style frontmatter from a SKILL.md file.
#[derive(Debug, Clone, Default)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// Scanner
// ---------------------------------------------------------------------------

/// Scan for workspace skills in the given workspace root.
///
/// Looks for:
/// 1. `<workspace_root>/.agents/skills/*/SKILL.md`
/// 2. Global config: `~/.config/orchestrix/skills/*/SKILL.md` (or %APPDATA%\Orchestrix\skills)
///
/// Returns all discovered skills sorted alphabetically by name.
pub fn scan_workspace_skills(workspace_root: &Path) -> Vec<WorkspaceSkill> {
    let mut skills = Vec::new();

    // 1. Workspace-local skills
    let workspace_skills_dir = workspace_root.join(".agents").join("skills");
    if workspace_skills_dir.is_dir() {
        collect_skills_from_dir(&workspace_skills_dir, "workspace", &mut skills);
    }

    // 2. Global user skills
    if let Some(global_dir) = global_skills_dir() {
        if global_dir.is_dir() {
            collect_skills_from_dir(&global_dir, "global", &mut skills);
        }
    }

    // Deduplicate: workspace skills take priority over global by id
    let mut seen = std::collections::HashSet::new();
    skills.retain(|skill| seen.insert(skill.id.clone()));

    skills.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    skills
}

/// Read the full content of a skill file (SKILL.md or any auxiliary file).
pub fn read_skill_file(skill_dir: &str, relative_path: &str) -> Result<String, String> {
    let base = PathBuf::from(skill_dir);
    let target = base.join(relative_path);

    // Security: ensure the resolved path stays within the skill directory
    let canonical_base = base
        .canonicalize()
        .map_err(|e| format!("skill dir not found: {e}"))?;
    let canonical_target = target
        .canonicalize()
        .map_err(|e| format!("file not found: {e}"))?;

    if !canonical_target.starts_with(&canonical_base) {
        return Err("path traversal not allowed".to_string());
    }

    std::fs::read_to_string(&canonical_target)
        .map_err(|e| format!("failed to read {}: {e}", relative_path))
}

/// Build a combined skills context string for injection into agent system prompts.
///
/// Only includes skills that are `enabled`.  Returns an empty string if no
/// skills are active.
pub fn build_skills_context(skills: &[WorkspaceSkill]) -> String {
    let active: Vec<&WorkspaceSkill> = skills.iter().filter(|s| s.enabled).collect();
    if active.is_empty() {
        return String::new();
    }

    let mut out = String::from("\n\n# Active Skills\n\nThe following skills are loaded and their instructions should be followed when relevant:\n");

    for skill in &active {
        out.push_str(&format!(
            "\n---\n## Skill: {}\n\n{}\n\n{}\n",
            skill.name,
            if skill.description.is_empty() {
                String::new()
            } else {
                format!("_{}_\n", skill.description)
            },
            skill.content,
        ));
    }

    out
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn collect_skills_from_dir(dir: &Path, source: &str, out: &mut Vec<WorkspaceSkill>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }

        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let raw = match std::fs::read_to_string(&skill_md) {
            Ok(content) => content,
            Err(_) => continue,
        };

        let (frontmatter, body) = parse_frontmatter(&raw);

        // Collect auxiliary files
        let files = list_relative_files(&path, &path);

        let skill = WorkspaceSkill {
            id: dir_name.clone(),
            name: frontmatter.name.unwrap_or_else(|| title_case(&dir_name)),
            description: frontmatter.description.unwrap_or_default(),
            content: body,
            skill_dir: path.to_string_lossy().to_string(),
            skill_file: skill_md.to_string_lossy().to_string(),
            source: source.to_string(),
            files,
            tags: frontmatter.tags,
            enabled: true,
        };

        out.push(skill);
    }
}

/// Parse YAML-style frontmatter delimited by `---` at the top of a markdown file.
fn parse_frontmatter(raw: &str) -> (SkillFrontmatter, String) {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return (SkillFrontmatter::default(), raw.to_string());
    }

    // Find the closing ---
    let after_first = &trimmed[3..];
    let close_idx = after_first.find("\n---");
    let Some(close_idx) = close_idx else {
        return (SkillFrontmatter::default(), raw.to_string());
    };

    let frontmatter_block = &after_first[..close_idx].trim();
    let body_start = 3 + close_idx + 4; // skip past "\n---"
    let body = if body_start < trimmed.len() {
        trimmed[body_start..].trim_start().to_string()
    } else {
        String::new()
    };

    let fm = parse_simple_yaml(frontmatter_block);
    (fm, body)
}

/// Minimal YAML-like key: value parser for frontmatter fields.
/// Supports: name, description, tags (comma-separated or YAML list).
fn parse_simple_yaml(block: &str) -> SkillFrontmatter {
    let mut fm = SkillFrontmatter::default();

    let mut current_key = String::new();
    let mut list_items: Vec<String> = Vec::new();
    let mut in_list = false;

    for line in block.lines() {
        let trimmed = line.trim();

        // YAML list item
        if trimmed.starts_with("- ") && in_list {
            let value = trimmed[2..].trim().to_string();
            if !value.is_empty() {
                list_items.push(value);
            }
            continue;
        }

        // Flush any pending list
        if in_list {
            apply_frontmatter_field(&mut fm, &current_key, &list_items.join(", "));
            fm.tags = if current_key == "tags" {
                list_items.clone()
            } else {
                fm.tags.clone()
            };
            list_items.clear();
            in_list = false;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().to_string();

            if value.is_empty() {
                // Might be a YAML list following
                current_key = key;
                in_list = true;
            } else {
                apply_frontmatter_field(&mut fm, &key, &value);
            }
        }
    }

    // Flush final list
    if in_list && !list_items.is_empty() {
        if current_key == "tags" {
            fm.tags = list_items;
        } else {
            apply_frontmatter_field(&mut fm, &current_key, &list_items.join(", "));
        }
    }

    // If tags were comma-separated inline
    if fm.tags.is_empty() {
        // Already handled by apply_frontmatter_field
    }

    fm
}

fn apply_frontmatter_field(fm: &mut SkillFrontmatter, key: &str, value: &str) {
    match key {
        "name" => fm.name = Some(value.to_string()),
        "description" => fm.description = Some(value.to_string()),
        "tags" => {
            fm.tags = value
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
        }
        _ => {} // Ignore unknown fields
    }
}

fn list_relative_files(base: &Path, current: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let entries = match std::fs::read_dir(current) {
        Ok(entries) => entries,
        Err(_) => return files,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(list_relative_files(base, &path));
        } else if let Ok(relative) = path.strip_prefix(base) {
            let rel_str = relative.to_string_lossy().replace('\\', "/");
            files.push(rel_str);
        }
    }

    files.sort();
    files
}

fn title_case(s: &str) -> String {
    s.split(|c: char| c == '-' || c == '_')
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

fn global_skills_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            return Some(PathBuf::from(app_data).join("Orchestrix").join("skills"));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return Some(
                PathBuf::from(home)
                    .join(".config")
                    .join("orchestrix")
                    .join("skills"),
            );
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_basic() {
        let input = r#"---
name: my-skill
description: Does cool things
tags: rust, ai, tools
---

# My Skill

This is the body."#;

        let (fm, body) = parse_frontmatter(input);
        assert_eq!(fm.name.as_deref(), Some("my-skill"));
        assert_eq!(fm.description.as_deref(), Some("Does cool things"));
        assert_eq!(fm.tags, vec!["rust", "ai", "tools"]);
        assert!(body.starts_with("# My Skill"));
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let input = "# Just markdown\n\nNo frontmatter here.";
        let (fm, body) = parse_frontmatter(input);
        assert!(fm.name.is_none());
        assert_eq!(body, input);
    }

    #[test]
    fn test_title_case() {
        assert_eq!(title_case("hello-world"), "Hello World");
        assert_eq!(title_case("frontend-design"), "Frontend Design");
        assert_eq!(title_case("my_cool_skill"), "My Cool Skill");
    }

    #[test]
    fn test_build_skills_context_empty() {
        let skills: Vec<WorkspaceSkill> = vec![];
        assert_eq!(build_skills_context(&skills), "");
    }

    #[test]
    fn test_build_skills_context_disabled() {
        let skills = vec![WorkspaceSkill {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "A test skill".to_string(),
            content: "# Test\nDo things.".to_string(),
            skill_dir: "/tmp/test".to_string(),
            skill_file: "/tmp/test/SKILL.md".to_string(),
            source: "workspace".to_string(),
            files: vec![],
            tags: vec![],
            enabled: false,
        }];
        assert_eq!(build_skills_context(&skills), "");
    }
}
