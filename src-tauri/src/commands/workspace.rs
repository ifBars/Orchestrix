use std::path::{Path, PathBuf};
use std::process::Command;

use crate::db::queries;
use crate::{load_workspace_root, AppError, AppState, ArtifactContentView, WorkspaceRootView};

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
