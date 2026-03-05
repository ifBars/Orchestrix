use crate::db::{queries, Database};

pub fn collect_markdown_artifact_bundle(db: &Database, task_id: &str) -> String {
    let artifacts = queries::list_markdown_artifacts_for_task(db, task_id).unwrap_or_default();
    if artifacts.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for artifact in artifacts {
        let path = std::path::PathBuf::from(&artifact.uri_or_content);
        if !path.exists() {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            out.push_str(&format!(
                "\n\n---\nArtifact: {}\n\n{}",
                artifact.uri_or_content, content
            ));
        }
    }

    out
}
