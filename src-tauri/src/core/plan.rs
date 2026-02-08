use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        };
        f.write_str(s)
    }
}

impl std::str::FromStr for StepStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "skipped" => Ok(Self::Skipped),
            _ => Err(format!("unknown step status: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub idx: u32,
    pub title: String,
    pub description: String,
    pub tool_intent: Option<String>,
    pub status: StepStatus,
    pub max_retries: u32,
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: Uuid,
    pub run_id: Uuid,
    pub goal_summary: String,
    pub steps: Vec<PlanStep>,
    pub completion_criteria: Vec<String>,
}
