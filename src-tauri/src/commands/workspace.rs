use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::agent_presets;
use crate::db::queries;
use crate::{load_workspace_root, AppError, AppState, ArtifactContentView, WorkspaceRootView};

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkspaceReferenceCandidate {
    pub kind: String,
    pub value: String,
    pub display: String,
    pub description: String,
    pub group: String,
}

const MAX_SCAN_ENTRIES: usize = 3000;

#[tauri::command]
pub fn set_workspace_root(
    state: tauri::State<'_, AppState>,
    workspace_root: String,
) -> Result<(), AppError> {
    if !Path::new(&workspace_root).exists() {
        return Err(AppError::Other(format!(
            "workspace root does not exist: {workspace_root}"
        )));
    }
    queries::upsert_setting(
        &state.db,
        "workspace_root",
        &workspace_root,
        &chrono::Utc::now().to_rfc3339(),
    )?;
    state
        .orchestrator
        .set_workspace_root(PathBuf::from(&workspace_root));
    Ok(())
}

#[tauri::command]
pub fn get_workspace_root(
    state: tauri::State<'_, AppState>,
) -> Result<WorkspaceRootView, AppError> {
    let path = load_workspace_root(&state.db);
    Ok(WorkspaceRootView {
        workspace_root: path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn search_workspace_references(
    state: tauri::State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<WorkspaceReferenceCandidate>, AppError> {
    let workspace_root = load_workspace_root(&state.db);
    let max = limit.unwrap_or(20).clamp(1, 50);
    let normalized = query.trim().replace('\\', "/");
    let query_lower = normalized.to_ascii_lowercase();
    let (base, needle) = split_base_and_needle(&normalized);

    let mut ranked: Vec<(i64, WorkspaceReferenceCandidate)> = Vec::new();

    let agents = agent_presets::scan_agent_presets(&workspace_root);
    for agent in agents {
        if !agent.enabled {
            continue;
        }
        let mention = format!("agent:{}", agent.id);
        let agent_haystack = format!("{} {} {}", mention, agent.name, agent.description);
        let score = fuzzy_score(&query_lower, &agent_haystack);
        if score >= 0 {
            ranked.push((
                score,
                WorkspaceReferenceCandidate {
                    kind: "agent".to_string(),
                    value: mention,
                    display: format!("{} ({})", agent.name, agent.id),
                    description: if agent.description.trim().is_empty() {
                        "agent preset".to_string()
                    } else {
                        agent.description
                    },
                    group: "Agents".to_string(),
                },
            ));
        }
    }

    let skills = crate::core::workspace_skills::scan_workspace_skills(&workspace_root);
    for skill in skills {
        if !skill.enabled {
            continue;
        }
        let mention = format!("skill:{}", skill.id);
        let skill_haystack = format!("{} {} {}", mention, skill.name, skill.description);
        let score = fuzzy_score(&query_lower, &skill_haystack);
        if score >= 0 {
            ranked.push((
                score,
                WorkspaceReferenceCandidate {
                    kind: "skill".to_string(),
                    value: mention,
                    display: format!("{} ({})", skill.name, skill.id),
                    description: if skill.description.trim().is_empty() {
                        "workspace skill".to_string()
                    } else {
                        skill.description
                    },
                    group: "Skills".to_string(),
                },
            ));
        }
    }

    let base_dir = workspace_root.join(&base);
    if base_dir.exists() && base_dir.is_dir() {
        let mut entries = collect_workspace_entries(&base_dir, &workspace_root, MAX_SCAN_ENTRIES);
        entries.sort();

        for rel_str in entries {
            if !needle.is_empty() {
                let basename = rel_str
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let parent = rel_str.rsplit_once('/').map(|(p, _)| p).unwrap_or("(root)");
                let path_haystack = format!("{} {} {}", rel_str, basename, parent);
                let score = fuzzy_score(&query_lower, &path_haystack);
                if score < 0 {
                    continue;
                }
                let kind = if workspace_root.join(&rel_str).is_dir() {
                    "directory"
                } else {
                    "file"
                };
                ranked.push((
                    score,
                    WorkspaceReferenceCandidate {
                        kind: kind.to_string(),
                        value: rel_str.clone(),
                        display: rel_str.clone(),
                        description: format!("workspace {kind}"),
                        group: parent.to_string(),
                    },
                ));
                continue;
            }

            let kind = if workspace_root.join(&rel_str).is_dir() {
                "directory"
            } else {
                "file"
            };
            let parent = rel_str
                .rsplit_once('/')
                .map(|(p, _)| p.to_string())
                .unwrap_or_else(|| "(root)".to_string());
            ranked.push((
                0,
                WorkspaceReferenceCandidate {
                    kind: kind.to_string(),
                    value: rel_str.clone(),
                    display: rel_str,
                    description: format!("workspace {kind}"),
                    group: parent,
                },
            ));
        }
    }

    ranked.sort_by(|(a_score, a), (b_score, b)| {
        b_score
            .cmp(a_score)
            .then_with(|| a.group.cmp(&b.group))
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.value.len().cmp(&b.value.len()))
            .then_with(|| a.value.cmp(&b.value))
    });

    let mut candidates = ranked.into_iter().map(|(_, item)| item).collect::<Vec<_>>();
    candidates.dedup_by(|a, b| a.value == b.value && a.kind == b.kind);
    candidates.truncate(max);

    Ok(candidates)
}

#[tauri::command]
pub fn read_artifact_content(path: String) -> Result<ArtifactContentView, AppError> {
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(AppError::Other(format!("artifact not found: {path}")));
    }

    let content = std::fs::read_to_string(&file_path)
        .map_err(|e| AppError::Other(format!("failed to read artifact: {e}")))?;

    let ext = file_path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.to_ascii_lowercase())
        .unwrap_or_default();
    let is_markdown = matches!(ext.as_str(), "md" | "markdown" | "mdx");

    Ok(ArtifactContentView {
        path,
        content,
        is_markdown,
    })
}

#[tauri::command]
pub fn open_local_path(path: String) -> Result<(), AppError> {
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(AppError::Other(format!("path not found: {path}")));
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", &path])
            .spawn()
            .map_err(|e| AppError::Other(format!("failed to open path: {e}")))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| AppError::Other(format!("failed to open path: {e}")))?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| AppError::Other(format!("failed to open path: {e}")))?;
    }

    Ok(())
}

fn split_base_and_needle(query: &str) -> (String, String) {
    let trimmed = query.trim().trim_start_matches("./").trim_matches('/');
    if trimmed.is_empty() {
        return (".".to_string(), String::new());
    }
    if let Some((base, needle)) = trimmed.rsplit_once('/') {
        let base = if base.is_empty() {
            ".".to_string()
        } else {
            base.to_string()
        };
        return (base, needle.to_string());
    }
    (".".to_string(), trimmed.to_string())
}

fn collect_workspace_entries(base_dir: &Path, workspace_root: &Path, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![base_dir.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            continue;
        };

        for entry in read_dir.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if should_skip_dir(&name, &path) {
                continue;
            }

            let Ok(relative) = path.strip_prefix(workspace_root) else {
                continue;
            };
            let rel_str = relative.to_string_lossy().replace('\\', "/");
            out.push(rel_str);
            if out.len() >= limit {
                return out;
            }

            if path.is_dir() {
                stack.push(path);
            }
        }
    }

    out
}

fn should_skip_dir(name: &str, path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }

    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | ".orchestrix"
    )
}

fn fuzzy_score(needle: &str, haystack: &str) -> i64 {
    if needle.trim().is_empty() {
        return 0;
    }

    let needle = needle.to_ascii_lowercase();
    let haystack = haystack.to_ascii_lowercase();

    if haystack == needle {
        return 400;
    }

    let basename = haystack.rsplit('/').next().unwrap_or_default();

    let mut score = 0i64;
    if haystack.starts_with(&needle) {
        score += 140;
    }
    if basename.starts_with(&needle) {
        score += 120;
    }
    if haystack.contains(&needle) {
        score += 80;
    }

    let mut last_index = None;
    let mut current_pos = 0usize;
    for ch in needle.chars() {
        let Some(found_rel) = haystack[current_pos..].find(ch) else {
            return -1;
        };
        let found = current_pos + found_rel;

        score += 10;
        if let Some(last) = last_index {
            if found == last + 1 {
                score += 12;
            } else if found <= last + 3 {
                score += 4;
            }
        }

        if found == 0 || haystack.as_bytes().get(found.wrapping_sub(1)) == Some(&b'/') {
            score += 8;
        }

        last_index = Some(found);
        current_pos = found + 1;
    }

    score - (haystack.len() as i64 / 8)
}
