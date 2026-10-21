//! `runs/<runId>/run.json` v1 and `runs/index-YYYY-MM.jsonl` lines v1 · class (b)
//! (`docs/contracts/files.md` §4.6, §4.7).

use serde::{Deserialize, Serialize};

use super::{Migration, Versioned};
use crate::ids::{Start, StepKey};

/// Run surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunSurface {
    /// Chat.
    Chat,
    /// CLI.
    Cli,
    /// Desktop.
    Desktop,
    /// Contract probe.
    Probe,
}

/// Run kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunKind {
    /// Launch.
    Launch,
    /// Re-arm.
    Rearm,
    /// Followup copy.
    FollowupCopy,
    /// Chat copy.
    ChatCopy,
    /// Desktop.
    Desktop,
    /// Probe.
    Probe,
}

/// Observed model, effort and mode (P2 sensor).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Observed {
    /// Model.
    pub model: Option<String>,
    /// Effort.
    pub effort: Option<String>,
    /// Mode.
    pub mode: Option<String>,
}

/// Entry points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryPoint {
    /// Do next.
    DoNext,
    /// Palette.
    Palette,
    /// Stage view.
    StageView,
    /// Re-arm.
    Rearm,
    /// Repair.
    Repair,
    /// Probe.
    Probe,
}

/// Timings (ms since `wallStart`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Timings {
    /// RFC 3339 UTC.
    pub wall_start: String,
    /// Look start.
    pub look_start: Option<u64>,
    /// Prepared.
    pub prepared: Option<u64>,
    /// Confirmed.
    pub confirmed: Option<u64>,
    /// Spawned.
    pub spawned: Option<u64>,
    /// Exited.
    pub exited: Option<u64>,
}

/// Run status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    /// Running.
    Running,
    /// Exited.
    Exited,
    /// Errored.
    Errored,
    /// Interrupted.
    Interrupted,
    /// Copied.
    Copied,
}

/// `run.json` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunFile {
    /// Always 1.
    #[serde(deserialize_with = "super::version_const::check::<_, 1>")]
    pub schema_version: u32,
    /// Run.
    pub run_id: String,
    /// Project.
    pub project_id: String,
    /// Stage.
    pub stage: u8,
    /// Step.
    pub step_id: StepKey,
    /// Surface.
    pub surface: RunSurface,
    /// Kind.
    pub kind: RunKind,
    /// Chain.
    pub chain_id: Option<String>,
    /// `--name`.
    pub name: Option<String>,
    /// Argv with the prompt replaced by its hash.
    pub argv: Vec<String>,
    /// Env names only.
    pub env_names: Vec<String>,
    /// cwd kind.
    pub cwd: Option<String>,
    /// Prompt hash.
    pub prompt_sha256: Option<String>,
    /// Run settings hash.
    pub run_settings_sha256: Option<String>,
    /// Intended start.
    pub intended: Option<Start>,
    /// An override applied.
    pub r#override: bool,
    /// Observed (P2).
    pub observed: Option<Observed>,
    /// Entry point.
    pub entry_point: EntryPoint,
    /// Timings.
    pub t: Timings,
    /// Time to start.
    pub time_to_start_ms: Option<u64>,
    /// Run-input interventions.
    pub run_input_interventions: u32,
    /// Copy prompt on CLI.
    pub copy_prompt_on_cli: bool,
    /// Status.
    pub status: RunStatus,
    /// Exit code.
    pub exit_code: Option<i32>,
    /// `claude -v`.
    pub claude_version: Option<String>,
    /// Pid.
    pub pid: Option<u32>,
    /// Sensor token.
    pub sensor_token: Option<String>,
}

impl Versioned for RunFile {
    const KIND: &'static str = "run";
    const CURRENT: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}

/// Index line surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexSurface {
    /// Chat.
    Chat,
    /// CLI.
    Cli,
    /// Desktop.
    Desktop,
}

/// Index line entry points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexEntryPoint {
    /// Do next.
    DoNext,
    /// Palette.
    Palette,
    /// Stage view.
    StageView,
    /// Re-arm.
    Rearm,
    /// Repair.
    Repair,
}

/// Index line status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexStatus {
    /// Exited.
    Exited,
    /// Errored.
    Errored,
    /// Interrupted.
    Interrupted,
    /// Copied.
    Copied,
}

/// One `runs/index-YYYY-MM.jsonl` line v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunIndexLine {
    /// Always 1.
    #[serde(deserialize_with = "super::version_const::check::<_, 1>")]
    pub schema_version: u32,
    /// Run.
    pub run_id: String,
    /// Project.
    pub project_id: String,
    /// Stage.
    pub stage: u8,
    /// Step.
    pub step_id: StepKey,
    /// Surface.
    pub surface: IndexSurface,
    /// Entry point.
    pub entry_point: IndexEntryPoint,
    /// RFC 3339 UTC.
    pub wall_start: String,
    /// Time to start.
    pub time_to_start_ms: Option<u64>,
    /// Duration.
    pub duration_ms: Option<u64>,
    /// Wait.
    pub wait_ms: u64,
    /// Status.
    pub status: IndexStatus,
    /// Interventions.
    pub run_input_interventions: u32,
    /// Copy prompt on CLI.
    pub copy_prompt_on_cli: bool,
    /// Override.
    pub r#override: bool,
}

impl Versioned for RunIndexLine {
    const KIND: &'static str = "run-index-line";
    const CURRENT: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}
