use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
                .filter(|item| item.id == "find-skills" || item.id == "context7-docs")
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
            "Use context7_resolve-library-id/context7_query-docs with libraryId='{normalized}'"
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
    let raw = skill_name.trim();
    if raw.is_empty() {
        return Err("skill_name is required".to_string());
    }

    let slug = if let Some(idx) = raw.rfind('@') {
        raw[idx + 1..].trim()
    } else {
        raw
    };
    if slug.is_empty() {
        return Err("invalid vercel skill name".to_string());
    }

    let id = sanitize_skill_id(slug);
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
        install_command: format!("npx skills add vercel-labs/agent-skills@{slug} -g -y"),
        url: format!("https://skills.sh/vercel-labs/agent-skills/{slug}"),
        source: "vercel".to_string(),
        tags: vec!["vercel".to_string(), "agent-skills".to_string()],
        is_custom: true,
    };

    upsert_custom_skill(skill.clone())?;
    Ok(skill)
}

fn built_in_skills() -> Vec<SkillCatalogItem> {
    vec![
        SkillCatalogItem {
            id: "vercel-react-best-practices".to_string(),
            title: "Vercel React Best Practices".to_string(),
            description: "React and Next.js optimization/playbook from Vercel.".to_string(),
            install_command:
                "npx skills add vercel-labs/agent-skills@vercel-react-best-practices -g -y"
                    .to_string(),
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
            description: "Search and install skills with npx skills find/add.".to_string(),
            install_command: "npx skills find <query>".to_string(),
            url: "https://skills.sh/".to_string(),
            source: "builtin".to_string(),
            tags: vec!["catalog".to_string(), "search".to_string()],
            is_custom: false,
        },
        SkillCatalogItem {
            id: "context7-docs".to_string(),
            title: "Context7 Docs Lookup".to_string(),
            description: "Resolve and query up-to-date library docs from Context7 MCP.".to_string(),
            install_command: "Use context7_resolve-library-id then context7_query-docs".to_string(),
            url: "https://context7.com/".to_string(),
            source: "context7".to_string(),
            tags: vec!["docs".to_string(), "mcp".to_string()],
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
