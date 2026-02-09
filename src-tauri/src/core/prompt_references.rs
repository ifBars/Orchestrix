use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::core::workspace_skills;

const MAX_FILE_CHARS: usize = 12_000;
const MAX_DIR_ENTRIES: usize = 80;

pub fn expand_prompt_references(prompt: &str, workspace_root: &Path) -> String {
    let refs = extract_mentions(prompt);
    if refs.is_empty() {
        return prompt.to_string();
    }

    let mut sections: Vec<String> = Vec::new();
    let workspace_skills = workspace_skills::scan_workspace_skills(workspace_root);

    for token in refs {
        if let Some(skill_ref) = token.strip_prefix("skill:") {
            if let Some(section) = resolve_skill_reference(skill_ref, &workspace_skills) {
                sections.push(section);
            }
            continue;
        }

        if let Some(section) = resolve_path_reference(&token, workspace_root) {
            sections.push(section);
        }
    }

    if sections.is_empty() {
        return prompt.to_string();
    }

    format!(
        "{}\n\n# Workspace References\n\n{}\n\nUse these references as authoritative workspace context when relevant.",
        prompt,
        sections.join("\n\n---\n\n")
    )
}

fn extract_mentions(prompt: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    let chars: Vec<char> = prompt.chars().collect();
    let len = chars.len();

    let mut i = 0usize;
    while i < len {
        if chars[i] != '@' {
            i += 1;
            continue;
        }

        let boundary_ok = if i == 0 {
            true
        } else {
            chars[i - 1].is_whitespace() || matches!(chars[i - 1], '(' | '[' | '{' | '"' | '\'')
        };
        if !boundary_ok {
            i += 1;
            continue;
        }

        let mut j = i + 1;
        while j < len {
            let ch = chars[j];
            if ch.is_whitespace()
                || matches!(ch, ')' | ']' | '}' | ',' | ';' | '!' | '?' | '"' | '\'')
            {
                break;
            }
            j += 1;
        }

        if j > i + 1 {
            let raw: String = chars[i + 1..j].iter().collect();
            let token = raw.trim_end_matches(&['.', ':'][..]).replace('\\', "/");
            if !token.is_empty() && seen.insert(token.clone()) {
                out.push(token);
            }
        }

        i = j;
    }

    out
}

fn resolve_skill_reference(
    skill_ref: &str,
    skills: &[workspace_skills::WorkspaceSkill],
) -> Option<String> {
    let needle = skill_ref.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return None;
    }

    let skill = skills
        .iter()
        .find(|s| s.id.to_ascii_lowercase() == needle || s.name.to_ascii_lowercase() == needle)?;

    Some(format!(
        "Reference `@skill:{}` (workspace skill):\n\n## {}\n\n{}",
        skill.id, skill.name, skill.content
    ))
}

fn resolve_path_reference(token: &str, workspace_root: &Path) -> Option<String> {
    let rel = token.trim().trim_start_matches("./").trim_matches('/');
    if rel.is_empty() {
        return None;
    }

    let full = workspace_root.join(rel);
    if !full.exists() {
        return None;
    }
    if !is_within_workspace(&full, workspace_root) {
        return None;
    }

    if full.is_file() {
        let content = std::fs::read_to_string(&full).ok()?;
        let truncated = if content.chars().count() > MAX_FILE_CHARS {
            let snippet: String = content.chars().take(MAX_FILE_CHARS).collect();
            format!("{}\n\n...[truncated]", snippet)
        } else {
            content
        };

        return Some(format!(
            "Reference `{}` (file):\n```text\n{}\n```",
            rel, truncated
        ));
    }

    if full.is_dir() {
        let mut entries = list_directory_entries(&full, workspace_root, MAX_DIR_ENTRIES);
        let truncated = entries.len() >= MAX_DIR_ENTRIES;
        if truncated {
            entries.truncate(MAX_DIR_ENTRIES);
        }
        let listing = if entries.is_empty() {
            "(empty directory)".to_string()
        } else {
            entries
                .into_iter()
                .map(|e| format!("- {}", e))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let suffix = if truncated { "\n- ...[truncated]" } else { "" };

        return Some(format!(
            "Reference `{}` (directory):\n{}{}",
            rel, listing, suffix
        ));
    }

    None
}

fn list_directory_entries(dir: &Path, workspace_root: &Path, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        let read_dir = match std::fs::read_dir(&current) {
            Ok(v) => v,
            Err(_) => continue,
        };

        for item in read_dir.flatten() {
            let path = item.path();
            if !is_within_workspace(&path, workspace_root) {
                continue;
            }

            if let Ok(rel) = path.strip_prefix(workspace_root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
            if out.len() >= limit {
                return out;
            }

            if path.is_dir() {
                stack.push(path);
            }
        }
    }

    out.sort();
    out
}

fn is_within_workspace(path: &Path, workspace_root: &Path) -> bool {
    let root = match workspace_root.canonicalize() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let candidate = match path.canonicalize() {
        Ok(v) => v,
        Err(_) => return false,
    };
    candidate.starts_with(root)
}
