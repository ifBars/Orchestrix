//! Plan mode settings for controlling token limits during planning.

use crate::db::{queries, Database};

/// Default max tokens for plan mode (25,000).
/// This allows ample room for reasoning while staying within context limits.
pub const DEFAULT_PLAN_MODE_MAX_TOKENS: u32 = 25_000;

/// High limit for worker/build mode (180,000).
/// Based on MiniMax's 204,800 context window, leaving room for input tokens.
pub const WORKER_MAX_TOKENS: u32 = 180_000;

/// Settings for plan mode behavior.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanModeSettings {
    /// Maximum tokens for plan mode responses (content + reasoning + tool calls).
    pub max_tokens: u32,
}

impl Default for PlanModeSettings {
    fn default() -> Self {
        Self {
            max_tokens: DEFAULT_PLAN_MODE_MAX_TOKENS,
        }
    }
}

/// Load plan mode settings from database (or return defaults).
pub fn load_plan_mode_settings(db: &Database) -> Result<PlanModeSettings, String> {
    match queries::get_setting(db, "plan_mode_settings") {
        Ok(Some(json_str)) => serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse plan mode settings: {e}")),
        Ok(None) => Ok(PlanModeSettings::default()),
        Err(e) => Err(format!("Failed to load plan mode settings: {e}")),
    }
}

/// Save plan mode settings to database.
pub fn save_plan_mode_settings(db: &Database, settings: &PlanModeSettings) -> Result<(), String> {
    let json_str = serde_json::to_string(settings)
        .map_err(|e| format!("Failed to serialize plan mode settings: {e}"))?;

    let now = chrono::Utc::now().to_rfc3339();
    queries::upsert_setting(db, "plan_mode_settings", &json_str, &now)
        .map_err(|e| format!("Failed to save plan mode settings: {e}"))?;

    Ok(())
}

/// Get the effective max tokens for plan mode.
pub fn get_plan_mode_max_tokens(db: &Database) -> u32 {
    load_plan_mode_settings(db)
        .map(|s| s.max_tokens)
        .unwrap_or(DEFAULT_PLAN_MODE_MAX_TOKENS)
}
