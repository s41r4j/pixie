//! Core data model (BUILD-SPEC §4). Behaviour arrives in M5; the shapes are fixed now so
//! events and storage agree from day one.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type JobId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Offered,
    WaitingOnUser,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub title: String,
    pub state: JobState,
    pub cwd: PathBuf,
    pub instructions: String,
    /// Unix millis.
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Pending,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub n: u32,
    pub text: String,
    pub images: Vec<PathBuf>,
    /// Unix millis when the turn started.
    pub clock: i64,
    pub status: TurnStatus,
    pub error: Option<String>,
}

/// `note.md` is the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub path_or_url: String,
    pub kind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    InProgress,
    Done,
}

/// 3–5 steps, exactly one `in_progress` (validated in M5 by `update_plan`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub label: String,
    pub status: StepStatus,
}
