use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

const START_MARKER: &str = "<!-- PREFERENCES_START -->";
const END_MARKER: &str = "<!-- PREFERENCES_END -->";
const MEMORY_ENTRYPOINT: &str = "MEMORY.md";
const AGENTS_FILE_NAME: &str = "AGENTS.md";
const MAX_MEMORY_LINES: usize = 200;
const MAX_PREFERENCES: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceEntry {
    pub key: String,
    pub value: String,
    pub category: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PreferenceDocument {
    preferences: Vec<PreferenceEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct AutoMemorySettings {
    #[serde(rename = "autoMemoryEnabled")]
    auto_memory_enabled: Option<bool>,
}

fn orchestrix_data_dir() -> PathBuf {
    if let Ok(path) = std::env::var("ORCHESTRIX_DATA_DIR") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            return PathBuf::from(app_data).join("Orchestrix");
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".orchestrix");
    }

    if let Ok(home) = std::env::var("USERPROFILE") {
        return PathBuf::from(home).join(".orchestrix");
    }

    PathBuf::from(".orchestrix")
}

fn project_key(workspace_root: &Path) -> String {
    let canonical = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    canonical
        .to_string_lossy()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn canonical_workspace_root(path: &Path) -> PathBuf {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let parts: Vec<_> = canonical.components().collect();
    for idx in 0..parts.len() {
        if parts[idx].as_os_str() == ".orchestrix" && idx + 1 < parts.len() {
            if parts[idx + 1].as_os_str() == "worktrees" {
                let mut root = PathBuf::new();
                for component in &parts[..idx] {
                    root.push(component.as_os_str());
                }
                if !root.as_os_str().is_empty() {
                    return root;
                }
            }
        }
    }
    canonical
}

fn auto_memory_enabled(workspace_root: &Path) -> bool {
    if let Ok(value) = std::env::var("ORCHESTRIX_DISABLE_AUTO_MEMORY") {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized == "1" || normalized == "true" || normalized == "yes" {
            return false;
        }
    }

    if let Ok(value) = std::env::var("ORCHESTRIX_AUTO_MEMORY_ENABLED") {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized == "0" || normalized == "false" || normalized == "no" {
            return false;
        }
        if normalized == "1" || normalized == "true" || normalized == "yes" {
            return true;
        }
    }

    let project_settings_path = workspace_root.join(".orchestrix").join("settings.json");
    if let Ok(raw) = std::fs::read_to_string(project_settings_path) {
        if let Ok(settings) = serde_json::from_str::<AutoMemorySettings>(&raw) {
            if let Some(enabled) = settings.auto_memory_enabled {
                return enabled;
            }
        }
    }

    let global_settings_path = orchestrix_data_dir().join("settings.json");
    if let Ok(raw) = std::fs::read_to_string(global_settings_path) {
        if let Ok(settings) = serde_json::from_str::<AutoMemorySettings>(&raw) {
            if let Some(enabled) = settings.auto_memory_enabled {
                return enabled;
            }
        }
    }

    true
}

fn memory_entrypoint_path(workspace_root: &Path) -> PathBuf {
    let root = canonical_workspace_root(workspace_root);
    orchestrix_data_dir()
        .join("projects")
        .join(project_key(&root))
        .join("memory")
        .join(MEMORY_ENTRYPOINT)
}

pub fn memory_entrypoint_path_for_workspace(workspace_root: &Path) -> PathBuf {
    memory_entrypoint_path(workspace_root)
}

fn default_memory_content() -> String {
    format!(
        "# Auto Memory\n\nThis file stores concise per-project memories loaded into each run context.\n\n## Preferences\n\n{}\n{{\n  \"preferences\": []\n}}\n{}\n",
        START_MARKER, END_MARKER
    )
}

fn ensure_memory_entrypoint(workspace_root: &Path) -> Result<PathBuf, String> {
    let path = memory_entrypoint_path(workspace_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "failed to create memory directory {}: {e}",
                parent.display()
            )
        })?;
    }
    if !path.exists() {
        std::fs::write(&path, default_memory_content())
            .map_err(|e| format!("failed to create memory entrypoint {}: {e}", path.display()))?;
    }
    Ok(path)
}

fn migrate_legacy_agents_preferences(
    workspace_root: &Path,
    memory_path: &Path,
) -> Result<(), String> {
    let memory_content = std::fs::read_to_string(memory_path)
        .map_err(|e| format!("failed to read {}: {e}", memory_path.display()))?;
    let mut memory_doc = parse_document(parse_block(&memory_content));
    if !memory_doc.preferences.is_empty() {
        return Ok(());
    }

    let legacy_path = canonical_workspace_root(workspace_root).join(AGENTS_FILE_NAME);
    if !legacy_path.exists() {
        return Ok(());
    }

    let legacy_content = std::fs::read_to_string(&legacy_path)
        .map_err(|e| format!("failed to read legacy {}: {e}", legacy_path.display()))?;
    let legacy_doc = parse_document(parse_block(&legacy_content));
    if legacy_doc.preferences.is_empty() {
        return Ok(());
    }

    memory_doc.preferences = legacy_doc.preferences;
    memory_doc
        .preferences
        .sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    if memory_doc.preferences.len() > MAX_PREFERENCES {
        memory_doc.preferences.truncate(MAX_PREFERENCES);
    }

    let next_content = write_document(&memory_content, &memory_doc)?;
    std::fs::write(memory_path, next_content)
        .map_err(|e| format!("failed to write {}: {e}", memory_path.display()))?;

    Ok(())
}

fn parse_block(content: &str) -> Option<String> {
    let start = content.find(START_MARKER)?;
    let after_start = start + START_MARKER.len();
    let end_rel = content[after_start..].find(END_MARKER)?;
    let end = after_start + end_rel;
    Some(content[after_start..end].trim().to_string())
}

fn parse_document(raw_block: Option<String>) -> PreferenceDocument {
    let Some(raw) = raw_block else {
        return PreferenceDocument::default();
    };
    if raw.trim().is_empty() {
        return PreferenceDocument::default();
    }
    serde_json::from_str::<PreferenceDocument>(&raw).unwrap_or_default()
}

fn write_document(content: &str, document: &PreferenceDocument) -> Result<String, String> {
    let block = serde_json::to_string_pretty(document)
        .map_err(|e| format!("failed to serialize preferences: {e}"))?;

    if let Some(start) = content.find(START_MARKER) {
        let after_start = start + START_MARKER.len();
        if let Some(end_rel) = content[after_start..].find(END_MARKER) {
            let end = after_start + end_rel;
            let mut next = String::with_capacity(content.len() + block.len() + 32);
            next.push_str(&content[..after_start]);
            next.push('\n');
            next.push_str(&block);
            next.push('\n');
            next.push_str(&content[end..]);
            return Ok(next);
        }
    }

    let mut next = String::from(content);
    if !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str("\n## User Preferences\n\n");
    next.push_str(START_MARKER);
    next.push('\n');
    next.push_str(&block);
    next.push('\n');
    next.push_str(END_MARKER);
    next.push('\n');
    Ok(next)
}

pub fn read_preferences_block(workspace_root: &Path) -> Result<Option<String>, String> {
    if !auto_memory_enabled(workspace_root) {
        return Ok(None);
    }
    let path = ensure_memory_entrypoint(workspace_root)?;
    migrate_legacy_agents_preferences(workspace_root, &path)?;
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    Ok(parse_block(&content))
}

pub fn list_preferences(workspace_root: &Path) -> Result<Vec<PreferenceEntry>, String> {
    let block = read_preferences_block(workspace_root)?;
    let mut preferences = parse_document(block).preferences;
    preferences.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(preferences)
}

pub fn upsert_preference(
    workspace_root: &Path,
    key: &str,
    value: &str,
    category: Option<&str>,
) -> Result<PreferenceEntry, String> {
    if !auto_memory_enabled(workspace_root) {
        return Err("auto memory is disabled".to_string());
    }

    let normalized_key = key.trim();
    if normalized_key.is_empty() {
        return Err("preference key cannot be empty".to_string());
    }

    let path = ensure_memory_entrypoint(workspace_root)?;
    migrate_legacy_agents_preferences(workspace_root, &path)?;
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let mut document = parse_document(parse_block(&content));

    let now = Utc::now().to_rfc3339();
    let normalized_category = category
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);

    let mut updated = PreferenceEntry {
        key: normalized_key.to_string(),
        value: value.to_string(),
        category: normalized_category,
        updated_at: now,
    };

    if let Some(entry) = document
        .preferences
        .iter_mut()
        .find(|entry| entry.key == normalized_key)
    {
        entry.value = updated.value.clone();
        entry.category = updated.category.clone();
        entry.updated_at = updated.updated_at.clone();
        updated = entry.clone();
    } else {
        document.preferences.push(updated.clone());
    }

    document
        .preferences
        .sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    if document.preferences.len() > MAX_PREFERENCES {
        document.preferences.truncate(MAX_PREFERENCES);
    }
    let next_content = write_document(&content, &document)?;
    std::fs::write(&path, next_content)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))?;

    Ok(updated)
}

pub fn delete_preference(workspace_root: &Path, key: &str) -> Result<bool, String> {
    if !auto_memory_enabled(workspace_root) {
        return Ok(false);
    }
    let normalized_key = key.trim();
    if normalized_key.is_empty() {
        return Err("preference key cannot be empty".to_string());
    }

    let path = ensure_memory_entrypoint(workspace_root)?;
    migrate_legacy_agents_preferences(workspace_root, &path)?;
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let mut document = parse_document(parse_block(&content));
    let before = document.preferences.len();
    document
        .preferences
        .retain(|entry| entry.key != normalized_key);
    if document.preferences.len() == before {
        return Ok(false);
    }
    let next_content = write_document(&content, &document)?;
    std::fs::write(&path, next_content)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    Ok(true)
}

pub fn compact_preferences(workspace_root: &Path) -> Result<usize, String> {
    if !auto_memory_enabled(workspace_root) {
        return Ok(0);
    }
    let path = ensure_memory_entrypoint(workspace_root)?;
    migrate_legacy_agents_preferences(workspace_root, &path)?;
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let mut document = parse_document(parse_block(&content));

    document
        .preferences
        .sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let original = document.preferences.len();
    if original > MAX_PREFERENCES {
        document.preferences.truncate(MAX_PREFERENCES);
    }

    let next_content = write_document(&content, &document)?;
    std::fs::write(&path, next_content)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    Ok(original.saturating_sub(document.preferences.len()))
}

pub fn startup_memory_context(workspace_root: &Path) -> Result<String, String> {
    if !auto_memory_enabled(workspace_root) {
        return Ok(String::new());
    }
    let path = ensure_memory_entrypoint(workspace_root)?;
    migrate_legacy_agents_preferences(workspace_root, &path)?;
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let lines: Vec<&str> = content.lines().take(MAX_MEMORY_LINES).collect();
    Ok(lines.join("\n"))
}
