use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command as TokioCommand;

const AGENT_SKILLS_SOURCE: &str = "vercel-labs/agent-skills";
const AGENT_SKILLS_SEARCH_API: &str = "https://skills.sh/api/search";
const AGENT_SKILLS_GITHUB_CONTENTS_API: &str =
    "https://api.github.com/repos/vercel-labs/agent-skills/contents/skills?ref=main";
const AGENT_SKILLS_RAW_BASE_URL: &str =
    "https://raw.githubusercontent.com/vercel-labs/agent-skills/main/skills";
const AGENT_SKILLS_USER_AGENT: &str = "Orchestrix/0.1 (+https://github.com/orchestrix)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCatalogItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub install_command: String,
    pub url: String,
    pub source: String,
    pub tags: Vec<String>,
    pub is_custom: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewCustomSkill {
    pub id: Option<String>,
    pub title: String,
    pub description: String,
    pub install_command: String,
    pub url: String,
    pub source: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkillSearchItem {
    pub skill_name: String,
    pub title: String,
    pub description: String,
    pub source: String,
    pub installs: u64,
    pub url: String,
    pub install_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkillInstallResult {
    pub skill_name: String,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

pub fn list_all_skills() -> Vec<SkillCatalogItem> {
    let mut all = built_in_skills();
    let mut custom = load_custom_skills();
    all.append(&mut custom);

    let mut dedup = std::collections::HashMap::<String, SkillCatalogItem>::new();
    for skill in all {
        dedup.insert(skill.id.clone(), skill);
    }

    let mut values: Vec<_> = dedup.into_values().collect();
    values.sort_by(|a, b| {
        a.title
            .to_ascii_lowercase()
            .cmp(&b.title.to_ascii_lowercase())
    });
    values
}

pub fn search_skills(query: &str, source: Option<&str>, limit: usize) -> Vec<SkillCatalogItem> {
    let query = query.trim().to_ascii_lowercase();
    let source = source.map(|value| value.trim().to_ascii_lowercase());

    let mut items = list_all_skills()
        .into_iter()
        .filter(|item| match source.as_ref() {
            Some(selected) if !selected.is_empty() => item.source.eq_ignore_ascii_case(selected),
            _ => true,
        })
        .collect::<Vec<_>>();

    if !query.is_empty() {
        let query_terms = query
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();

        items.retain(|item| {
            let id = item.id.to_ascii_lowercase();
            let title = item.title.to_ascii_lowercase();
            let description = item.description.to_ascii_lowercase();
            let tags = item
                .tags
                .iter()
                .map(|tag| tag.to_ascii_lowercase())
                .collect::<Vec<_>>();

            id.contains(&query)
                || title.contains(&query)
                || description.contains(&query)
                || tags.iter().any(|tag| tag.contains(&query))
                || query_terms.iter().all(|term| {
                    id.contains(term)
                        || title.contains(term)
                        || description.contains(term)
                        || tags.iter().any(|tag| tag.contains(term))
                })
        });

        if items.is_empty() {
            let fallback = list_all_skills()
                .into_iter()
                .filter(|item| item.id == "find-skills")
                .collect::<Vec<_>>();
            if !fallback.is_empty() {
                items = fallback;
            }
        }
    }

    items.sort_by_key(|item| item.title.to_ascii_lowercase());
    items.into_iter().take(limit).collect()
}

pub fn add_custom_skill(input: NewCustomSkill) -> Result<SkillCatalogItem, String> {
    if input.title.trim().is_empty() {
        return Err("title is required".to_string());
    }
    if input.install_command.trim().is_empty() {
        return Err("install_command is required".to_string());
    }
    if input.url.trim().is_empty() {
        return Err("url is required".to_string());
    }

    let id = input
        .id
        .as_deref()
        .map(sanitize_skill_id)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| sanitize_skill_id(&input.title));

    if id.is_empty() {
        return Err("could not derive a valid skill id".to_string());
    }

    let source = input
        .source
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("custom")
        .to_string();

    let tags = input
        .tags
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let skill = SkillCatalogItem {
        id,
        title: input.title.trim().to_string(),
        description: input.description.trim().to_string(),
        install_command: input.install_command.trim().to_string(),
        url: input.url.trim().to_string(),
        source,
        tags,
        is_custom: true,
    };

    upsert_custom_skill(skill.clone())?;
    Ok(skill)
}

pub fn remove_custom_skill(skill_id: &str) -> Result<bool, String> {
    let target = skill_id.trim();
    if target.is_empty() {
        return Ok(false);
    }

    let mut current = load_custom_skills();
    let before = current.len();
    current.retain(|item| item.id != target);
    if current.len() == before {
        return Ok(false);
    }

    save_custom_skills(&current)?;
    Ok(true)
}

pub fn upsert_custom_skill(skill: SkillCatalogItem) -> Result<(), String> {
    let mut current = load_custom_skills();
    current.retain(|item| item.id != skill.id);
    current.push(skill);
    save_custom_skills(&current)
}

pub fn import_context7_skill(
    library_id: &str,
    title: Option<&str>,
) -> Result<SkillCatalogItem, String> {
    let normalized = library_id.trim();
    if !normalized.starts_with('/') {
        return Err("context7 library id must start with '/'".to_string());
    }

    let segments = normalized
        .split('/')
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if segments.len() < 2 {
        return Err("context7 library id must be in /org/project format".to_string());
    }

    let fallback_title = format!("Context7: {}/{}", segments[0], segments[1]);
    let skill = SkillCatalogItem {
        id: format!("context7-{}", sanitize_skill_id(&segments.join("-"))),
        title: title
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&fallback_title)
            .to_string(),
        description: format!("Query Context7 docs for `{normalized}`."),
        install_command: format!(
            "Use MCP Context7 tools resolve-library-id then query-docs (for example mcp.context7.resolve-library-id and mcp.context7.query-docs) with libraryId='{normalized}'"
        ),
        url: format!("https://context7.com{normalized}"),
        source: "context7".to_string(),
        tags: vec!["docs".to_string(), "context7".to_string()],
        is_custom: true,
    };

    upsert_custom_skill(skill.clone())?;
    Ok(skill)
}

pub fn import_vercel_skill(skill_name: &str) -> Result<SkillCatalogItem, String> {
    let slug = normalize_agent_skill_name(skill_name)?;

    let id = sanitize_skill_id(&slug);
    let title = slug
        .split('-')
        .filter(|value| !value.is_empty())
        .map(|value| {
            let mut chars = value.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let skill = SkillCatalogItem {
        id,
        title: if title.is_empty() {
            "Vercel Agent Skill".to_string()
        } else {
            format!("Vercel: {title}")
        },
        description: "Install a skill from vercel-labs/agent-skills.".to_string(),
        install_command: project_scope_install_command(&slug),
        url: format!("https://skills.sh/vercel-labs/agent-skills/{slug}"),
        source: "vercel".to_string(),
        tags: vec!["vercel".to_string(), "agent-skills".to_string()],
        is_custom: true,
    };

    upsert_custom_skill(skill.clone())?;
    Ok(skill)
}

pub async fn search_agent_skills(
    query: &str,
    limit: usize,
) -> Result<Vec<AgentSkillSearchItem>, String> {
    let query = query.trim();
    let limit = limit.clamp(1, 100);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| format!("failed to initialize HTTP client: {e}"))?;

    let canonical = fetch_agent_skills_metadata(&client)
        .await
        .unwrap_or_default();

    let mut seen = HashSet::new();
    let mut out = Vec::new();

    // 1. If query is present, try the skills.sh search API first for popularity ranking
    if !query.is_empty() {
        if let Ok(response) = client
            .get(AGENT_SKILLS_SEARCH_API)
            .query(&[("q", query), ("limit", &limit.to_string())])
            .send()
            .await
        {
            if response.status().is_success() {
                if let Ok(payload) = response.json::<SkillsShSearchResponse>().await {
                    for item in payload.skills {
                        // Allow any source, not just vercel-labs/agent-skills
                        let source = if item.source.trim().is_empty() {
                            AGENT_SKILLS_SOURCE.to_string()
                        } else {
                            item.source.clone()
                        };

                        let candidate_name = if !item.skill_id.trim().is_empty() {
                            item.skill_id.as_str()
                        } else if !item.name.trim().is_empty() {
                            item.name.as_str()
                        } else {
                            item.id.rsplit('/').next().unwrap_or(item.id.as_str())
                        };

                        // Relax normalization for search results since we trust the API to some extent
                        // or at least we shouldn't filter them out aggressively if they look weird.
                        let normalized = candidate_name.trim().to_string();
                        let normalized_lc = normalized.to_ascii_lowercase();

                        // For vercel-labs/agent-skills, we can validate against canonical list.
                        // For others, we accept them as-is from the search API.
                        let is_default_repo = source.eq_ignore_ascii_case(AGENT_SKILLS_SOURCE);
                        let mut title = humanize_skill_title(&normalized);
                        let mut description = format!("Install `{normalized}` from {source}.");

                        if is_default_repo {
                            if !canonical.is_empty() && !canonical.contains_key(&normalized_lc) {
                                continue;
                            }
                            if let Some(meta) = canonical.get(&normalized_lc) {
                                title = meta.display_name.clone();
                                description = meta.description.clone();
                            }
                        }

                        // Deduplicate based on source + skill_name
                        let unique_key = format!("{}::{}", source.to_lowercase(), normalized_lc);
                        if !seen.insert(unique_key) {
                            continue;
                        }

                        out.push(AgentSkillSearchItem {
                            skill_name: normalized.clone(),
                            title,
                            description,
                            source: source.clone(),
                            installs: item.installs,
                            url: if is_default_repo {
                                agent_skill_url(&normalized)
                            } else {
                                format!("https://skills.sh/{source}/{normalized}")
                            },
                            install_command: format!(
                                "bunx skills add {source} --skill {normalized} --agent opencode -y"
                            ),
                        });
                    }
                }
            }
        }
    }

    // 2. Fallback/Fill: Add anything from canonical that matches (or all if query empty)
    if !canonical.is_empty() {
        let query_lc = query.to_ascii_lowercase();
        let mut remaining: Vec<_> = canonical
            .values()
            .filter(|meta| {
                if query.is_empty() {
                    return true;
                }
                let name_match = meta.canonical_name.to_ascii_lowercase().contains(&query_lc);
                let desc_match = meta.description.to_ascii_lowercase().contains(&query_lc);
                name_match || desc_match
            })
            .collect();

        // Sort alphabetically since we don't have install counts for these
        remaining.sort_by(|a, b| a.canonical_name.cmp(&b.canonical_name));

        for metadata in remaining {
            // Dedupe for default repo
            let unique_key = format!(
                "{}::{}",
                AGENT_SKILLS_SOURCE.to_lowercase(),
                metadata.canonical_name.to_ascii_lowercase()
            );
            if !seen.insert(unique_key) {
                continue;
            }
            out.push(AgentSkillSearchItem {
                skill_name: metadata.canonical_name.clone(),
                title: metadata.display_name.clone(),
                description: metadata.description.clone(),
                source: AGENT_SKILLS_SOURCE.to_string(),
                installs: 0,
                url: agent_skill_url(&metadata.canonical_name),
                install_command: project_scope_install_command(&metadata.canonical_name),
            });
        }
    }

    out.truncate(limit);
    Ok(out)
}

pub async fn install_agent_skill(
    workspace_root: &Path,
    skill_name: &str,
    repo_url: Option<String>,
) -> Result<AgentSkillInstallResult, String> {
    if !workspace_root.is_dir() {
        return Err(format!(
            "workspace root is not a directory: {}",
            workspace_root.display()
        ));
    }

    let normalized_skill_name = normalize_agent_skill_name(skill_name)?;
    let target_repo = repo_url
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(AGENT_SKILLS_SOURCE);

    // If using the default vercel-labs/agent-skills repo, validate against canonical list
    if target_repo == AGENT_SKILLS_SOURCE {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(|e| format!("failed to initialize HTTP client: {e}"))?;
        let canonical = fetch_agent_skills_metadata(&client)
            .await
            .unwrap_or_default();

        if !canonical.is_empty()
            && !canonical.contains_key(&normalized_skill_name.to_ascii_lowercase())
        {
            return Err(format!(
                "unknown agent skill `{}` for {}",
                normalized_skill_name, AGENT_SKILLS_SOURCE
            ));
        }
    }

    let args = build_project_scope_install_args(&normalized_skill_name, target_repo);
    let (output, command) = run_skills_cli(workspace_root, &args).await?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code();

    if !output.status.success() {
        let detail = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        return Err(format!(
            "failed to install `{}` from `{}` (code {:?}): {}",
            normalized_skill_name, target_repo, exit_code, detail
        ));
    }

    // Keep the local skill catalog in sync with what we just installed IF it's from the default repo
    // (Custom repo installs won't have a catalog entry auto-created unless we add logic for it,
    // but they will appear in workspace skills)
    if target_repo == AGENT_SKILLS_SOURCE {
        import_vercel_skill(&normalized_skill_name)?;
    } else {
        // Create a custom catalog entry for this external skill so we remember it
        // We can't fetch metadata easily for arbitrary repos without more complex logic,
        // so we'll use a basic entry.
        let id = sanitize_skill_id(&format!("{}-{}", target_repo, normalized_skill_name));
        let title = humanize_skill_title(&normalized_skill_name);
        
        let skill = SkillCatalogItem {
            id,
            title: format!("External: {title}"),
            description: format!("Installed from {target_repo}"),
            install_command: format!("bunx skills add {target_repo} --skill {normalized_skill_name} --agent opencode -y"),
            url: if target_repo.starts_with("http") {
                format!("{}/tree/main/skills/{}", target_repo, normalized_skill_name)
            } else {
                format!("https://github.com/{}/tree/main/skills/{}", target_repo, normalized_skill_name)
            },
            source: "external".to_string(),
            tags: vec!["external".to_string(), "custom".to_string()],
            is_custom: true,
        };
        upsert_custom_skill(skill)?;
    }

    Ok(AgentSkillInstallResult {
        skill_name: normalized_skill_name,
        command,
        stdout,
        stderr,
        exit_code,
    })
}

fn project_scope_install_command(skill_name: &str) -> String {
    format!("bunx skills add {AGENT_SKILLS_SOURCE} --skill {skill_name} --agent opencode -y")
}

fn build_project_scope_install_args(skill_name: &str, repo: &str) -> Vec<String> {
    vec![
        "skills".to_string(),
        "add".to_string(),
        repo.to_string(),
        "--skill".to_string(),
        skill_name.to_string(),
        "--agent".to_string(),
        "opencode".to_string(),
        "-y".to_string(),
    ]
}

fn agent_skill_url(skill_name: &str) -> String {
    format!("https://skills.sh/{AGENT_SKILLS_SOURCE}/{skill_name}")
}

fn normalize_agent_skill_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("skill_name is required".to_string());
    }

    let mut candidate = trimmed;

    if let Some(rest) = candidate.strip_prefix("https://skills.sh/") {
        candidate = rest;
    }

    if let Some(idx) = candidate.rfind('@') {
        candidate = &candidate[idx + 1..];
    }

    if candidate.contains('/') {
        candidate = candidate.rsplit('/').next().unwrap_or(candidate);
    }

    let candidate = candidate.trim().trim_matches('/').to_ascii_lowercase();
    if candidate.is_empty() {
        return Err("invalid agent skill name".to_string());
    }
    if candidate.chars().any(|ch| ch.is_ascii_whitespace()) {
        return Err("agent skill name must not contain spaces".to_string());
    }
    if candidate
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':')))
    {
        return Err("agent skill name contains unsupported characters".to_string());
    }

    Ok(candidate)
}

fn humanize_skill_title(skill_name: &str) -> String {
    let words = skill_name
        .split(|ch: char| matches!(ch, '-' | '_' | '.' | ':'))
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>();

    if words.is_empty() {
        skill_name.to_string()
    } else {
        words.join(" ")
    }
}

async fn run_skills_cli(
    workspace_root: &Path,
    args: &[String],
) -> Result<(std::process::Output, String), String> {
    let bunx_display = format!("bunx {}", args.join(" "));

    let bunx_result = TokioCommand::new("bunx")
        .args(args)
        .current_dir(workspace_root)
        .env("DISABLE_TELEMETRY", "1")
        .env("DO_NOT_TRACK", "1")
        .env("CI", "1")
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0")
        .output()
        .await;

    match bunx_result {
        Ok(output) => Ok((output, bunx_display)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut bun_args = vec!["x".to_string()];
            bun_args.extend(args.iter().cloned());
            let bun_display = format!("bun {}", bun_args.join(" "));
            let output = TokioCommand::new("bun")
                .args(&bun_args)
                .current_dir(workspace_root)
                .env("DISABLE_TELEMETRY", "1")
                .env("DO_NOT_TRACK", "1")
                .env("CI", "1")
                .env("NO_COLOR", "1")
                .env("FORCE_COLOR", "0")
                .output()
                .await
                .map_err(|e| format!("failed to execute `{bun_display}`: {e}"))?;
            Ok((output, bun_display))
        }
        Err(error) => Err(format!("failed to execute `{bunx_display}`: {error}")),
    }
}

#[derive(Debug, Deserialize)]
struct SkillsShSearchResponse {
    #[serde(default)]
    skills: Vec<SkillsShSearchItem>,
}

#[derive(Debug, Deserialize)]
struct SkillsShSearchItem {
    #[serde(default)]
    id: String,
    #[serde(default, rename = "skillId")]
    skill_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    installs: u64,
    #[serde(default)]
    source: String,
}

#[derive(Debug, Deserialize)]
struct GitHubContentsItem {
    name: String,
    #[serde(rename = "type")]
    item_type: String,
}

#[derive(Debug, Clone)]
struct AgentSkillMetadata {
    canonical_name: String,
    display_name: String,
    description: String,
}

async fn fetch_agent_skills_metadata(
    client: &reqwest::Client,
) -> Result<HashMap<String, AgentSkillMetadata>, String> {
    let response = client
        .get(AGENT_SKILLS_GITHUB_CONTENTS_API)
        .header(reqwest::header::USER_AGENT, AGENT_SKILLS_USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("failed to fetch agent-skills index: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "agent-skills index returned status {}",
            response.status()
        ));
    }

    let items: Vec<GitHubContentsItem> = response
        .json()
        .await
        .map_err(|e| format!("failed to decode agent-skills index: {e}"))?;

    let mut out = HashMap::new();

    for item in items {
        if item.item_type != "dir" {
            continue;
        }

        let fallback_name = item.name.trim().to_ascii_lowercase();
        if fallback_name.is_empty() {
            continue;
        }

        let skill_md_url = format!("{AGENT_SKILLS_RAW_BASE_URL}/{}/SKILL.md", item.name);
        let mut display_name = fallback_name.clone();
        let mut description = format!("Install `{}` from {AGENT_SKILLS_SOURCE}.", fallback_name);

        if let Ok(response) = client
            .get(&skill_md_url)
            .header(reqwest::header::USER_AGENT, AGENT_SKILLS_USER_AGENT)
            .send()
            .await
        {
            if response.status().is_success() {
                if let Ok(raw) = response.text().await {
                    let (name, desc) = parse_skill_frontmatter(&raw);
                    if let Some(name) = name {
                        display_name = name.to_ascii_lowercase();
                    }
                    if let Some(desc) = desc {
                        description = desc;
                    }
                }
            }
        }

        let metadata = AgentSkillMetadata {
            canonical_name: display_name.clone(),
            display_name: humanize_skill_title(&display_name),
            description,
        };

        out.insert(fallback_name.clone(), metadata.clone());
        out.insert(display_name, metadata);
    }

    Ok(out)
}

fn parse_skill_frontmatter(raw: &str) -> (Option<String>, Option<String>) {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return (None, None);
    }

    let after_open = &trimmed[3..];
    let Some(close_idx) = after_open.find("\n---") else {
        return (None, None);
    };

    let frontmatter = &after_open[..close_idx];
    let mut name = None;
    let mut description = None;

    for line in frontmatter.lines() {
        let trimmed = line.trim();
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim()
            .to_string();
        if value.is_empty() {
            continue;
        }

        match key.as_str() {
            "name" => name = Some(value.to_ascii_lowercase()),
            "description" => description = Some(value),
            _ => {}
        }
    }

    (name, description)
}

fn built_in_skills() -> Vec<SkillCatalogItem> {
    vec![
        SkillCatalogItem {
            id: "vercel-react-best-practices".to_string(),
            title: "Vercel React Best Practices".to_string(),
            description: "React and Next.js optimization/playbook from Vercel.".to_string(),
            install_command: project_scope_install_command("vercel-react-best-practices"),
            url: "https://skills.sh/vercel-labs/agent-skills/vercel-react-best-practices"
                .to_string(),
            source: "vercel".to_string(),
            tags: vec![
                "react".to_string(),
                "nextjs".to_string(),
                "vercel".to_string(),
            ],
            is_custom: false,
        },
        SkillCatalogItem {
            id: "find-skills".to_string(),
            title: "Find Skills".to_string(),
            description: "Search and install skills with bunx skills find/add.".to_string(),
            install_command: "bunx skills find <query>".to_string(),
            url: "https://skills.sh/".to_string(),
            source: "builtin".to_string(),
            tags: vec!["catalog".to_string(), "search".to_string()],
            is_custom: false,
        },
    ]
}

fn load_custom_skills() -> Vec<SkillCatalogItem> {
    let path = skills_store_path();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    let parsed: Vec<SkillCatalogItem> = serde_json::from_str(&raw).unwrap_or_default();
    parsed
        .into_iter()
        .map(|mut item| {
            item.is_custom = true;
            item
        })
        .collect()
}

fn save_custom_skills(skills: &[SkillCatalogItem]) -> Result<(), String> {
    let path = skills_store_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create skills directory: {e}"))?;
    }

    let cleaned: Vec<SkillCatalogItem> = skills
        .iter()
        .cloned()
        .map(|mut item| {
            item.is_custom = true;
            item
        })
        .collect();

    let body = serde_json::to_string_pretty(&cleaned)
        .map_err(|e| format!("failed to serialize skills: {e}"))?;
    std::fs::write(&path, body).map_err(|e| format!("failed to save skills: {e}"))
}

fn skills_store_path() -> PathBuf {
    if let Ok(path) = std::env::var("ORCHESTRIX_SKILLS_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            return PathBuf::from(app_data)
                .join("Orchestrix")
                .join("custom-skills-v1.json");
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".orchestrix")
            .join("custom-skills-v1.json");
    }

    if let Ok(home) = std::env::var("USERPROFILE") {
        return PathBuf::from(home)
            .join(".orchestrix")
            .join("custom-skills-v1.json");
    }

    PathBuf::from(".orchestrix").join("custom-skills-v1.json")
}

fn sanitize_skill_id(raw: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in raw.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests;
