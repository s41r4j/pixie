//! Events streamed from Rust to the UI (one Tauri event channel, `openclicky://event`).

use crate::model::JobId;
use serde::{Deserialize, Serialize};

/// Name of the Tauri event every [`Event`] is emitted on.
pub const EVENT_CHANNEL: &str = "openclicky://event";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    /// Dotted kind, e.g. `panel.state`, `hotkey.cancel`, `job.plan`.
    pub kind: String,
    pub job_id: Option<JobId>,
    pub payload: serde_json::Value,
}

impl Event {
    pub fn new(kind: impl Into<String>, payload: serde_json::Value) -> Self {
        Self { kind: kind.into(), job_id: None, payload }
    }

    pub fn with_job(kind: impl Into<String>, job_id: impl Into<JobId>, payload: serde_json::Value) -> Self {
        Self { kind: kind.into(), job_id: Some(job_id.into()), payload }
    }

    pub fn for_job(mut self, job_id: impl Into<JobId>) -> Self {
        self.job_id = Some(job_id.into());
        self
    }
}
