//! Agent tools (BUILD-SPEC §5) and the tool loop.
//!
//! Tools that touch only files, the shell and the web live here. Tools that need the app
//! (speech, pointer, Accessibility, plan/progress UI, asking the user, background jobs,
//! the browser, connectors) are delegated to the [`Host`].

pub mod agent;
pub mod coders;
pub mod defs;
mod exec;
pub mod guard;
pub mod notes;
pub mod portable;
pub mod web;

pub use agent::{run, AgentEvent, AgentOptions, AgentOutcome};
pub use exec::execute;

use openclicky_core::prompt::Mode;
use openclicky_llm::ToolDef;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
pub use tokio_util::sync::CancellationToken;

/// Result of one tool call, fed back to the model.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolOutput {
    pub text: String,
    /// Base64 JPEG/PNG images (with media type) to attach to the result.
    pub images: Vec<(String, String)>,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn ok(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Default::default() }
    }
    pub fn err(text: impl Into<String>) -> Self {
        Self { text: text.into(), is_error: true, ..Default::default() }
    }
}

impl From<Result<String, String>> for ToolOutput {
    fn from(r: Result<String, String>) -> Self {
        match r {
            Ok(t) => ToolOutput::ok(t),
            Err(e) => ToolOutput::err(e),
        }
    }
}

/// Where a tool call runs.
#[derive(Clone)]
pub struct ToolCtx {
    pub mode: Mode,
    /// Job folder (or the voice session folder).
    pub cwd: PathBuf,
    pub note_path: PathBuf,
    pub job_id: Option<String>,
    pub host: Arc<dyn Host>,
    pub cancel: CancellationToken,
    /// `openclicky-helper` (PDF, document text) if available.
    pub helper: Option<PathBuf>,
    /// Settings → Coding.
    pub coding: coders::Prefs,
}

#[async_trait::async_trait]
pub trait Host: Send + Sync {
    /// Run an app-side tool. `None` means "not mine".
    async fn call(&self, ctx: &ToolCtx, name: &str, input: &Value) -> Option<ToolOutput>;
    /// note.md changed on disk.
    fn note_changed(&self, _ctx: &ToolCtx) {}
    /// Extra tools (connectors) for this mode.
    fn extra_tools(&self, _mode: Mode) -> Vec<ToolDef> {
        vec![]
    }
}

/// A host that implements nothing (tests, headless).
pub struct NoHost;

#[async_trait::async_trait]
impl Host for NoHost {
    async fn call(&self, _: &ToolCtx, name: &str, _: &Value) -> Option<ToolOutput> {
        Some(ToolOutput::err(format!("{name} isn't available here.")))
    }
}
