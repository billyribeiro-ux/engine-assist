//! `projects/<projectId>/progress.json` v1 · class (b) (`docs/contracts/files.md` §4.3). Derived
//! states are never stored (DR-050).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Migration, Versioned};
use crate::ids::{CwdKind, Effort, ItemId, Mode, Model, Start, StepKey};

/// Stored step states (DR-065).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoredState {
    /// Not started.
    NotStarted,
    /// Prompt copied.
    Copied,
    /// Running.
    Running,
    /// Done.
    Done,
}

/// One step's stored progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StepProgress {
    /// Stored state.
    pub state: StoredState,
    /// Runs so far.
    pub run_count: u32,
    /// Last run.
    pub last_run_id: Option<String>,
    /// When it started.
    pub started_at: Option<String>,
    /// When it was copied.
    pub copied_at: Option<String>,
}

/// `chat` or `cli`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChainSurface {
    /// Chat.
    Chat,
    /// CLI.
    Cli,
}

/// Chain status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChainStatus {
    /// Open.
    Open,
    /// Closed at sign.
    Closed,
    /// Replaced by a re-run.
    Superseded,
    /// Handed off to Desktop.
    HandedOff,
    /// Transcript missing.
    Broken,
}

/// A session chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Chain {
    /// Chain UUID.
    pub id: String,
    /// Surface.
    pub surface: ChainSurface,
    /// Working directory.
    pub cwd: Option<CwdKind>,
    /// Start step.
    pub start: StepKey,
    /// Steps run in it.
    pub steps: Vec<StepKey>,
    /// Status.
    pub status: ChainStatus,
    /// Fork parent.
    pub forked_from: Option<String>,
    /// Last effective start.
    pub last_start: Option<Start>,
}

/// A per-run override (DR-103).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredOverride {
    /// Model.
    pub model: Option<Model>,
    /// Effort.
    pub effort: Option<Effort>,
    /// Mode.
    pub mode: Option<Mode>,
}

/// Gate states (DR-070).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GateState {
    /// Not ready.
    NotReady,
    /// Gate A ran.
    A,
    /// Results confirmed.
    Results,
    /// Gate B ran.
    B,
    /// Packet valid.
    PacketValid,
    /// Gate C ran.
    C,
    /// Lessons entry found.
    LessonsFound,
    /// Signable.
    Signable,
}

/// Item outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemOutcome {
    /// Attested done.
    Done,
    /// Not applicable with a reason.
    NotApplicable,
    /// Gate A pass.
    Pass,
    /// Gate A fail.
    Fail,
}

/// One item result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemResult {
    /// Outcome.
    pub outcome: ItemOutcome,
    /// Note or reason.
    pub text: String,
    /// Optional evidence link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// RFC 3339 UTC.
    pub at: String,
}

/// One stage's gate run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GateRun {
    /// State.
    pub state: GateState,
    /// Item results.
    pub items: BTreeMap<ItemId, ItemResult>,
    /// Failure counts per item.
    pub failures: BTreeMap<String, u32>,
}

/// Decision file names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DecisionFileKind {
    /// `.foreman/project.json`.
    Project,
    /// `.foreman/overlay.json`.
    Overlay,
    /// `.foreman/blueprint.lock.json`.
    BlueprintLock,
}

impl DecisionFileKind {
    /// Every decision file.
    pub const ALL: [Self; 3] = [Self::Project, Self::Overlay, Self::BlueprintLock];

    /// The repo path.
    #[must_use]
    pub const fn repo_path(self) -> &'static str {
        match self {
            Self::Project => ".foreman/project.json",
            Self::Overlay => ".foreman/overlay.json",
            Self::BlueprintLock => ".foreman/blueprint.lock.json",
        }
    }

    /// The approved copy's file name in `approvals/<projectId>/`.
    #[must_use]
    pub const fn approved_name(self) -> &'static str {
        match self {
            Self::Project => "project.json",
            Self::Overlay => "overlay.json",
            Self::BlueprintLock => "blueprint.lock.json",
        }
    }
}

/// Change kinds in a redacted diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    /// Key added.
    Added,
    /// Key removed.
    Removed,
    /// Value changed.
    ValueChanged,
}

/// One diff entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiffEntry {
    /// Dotted key path.
    pub key_path: String,
    /// Change.
    pub change: ChangeKind,
    /// Before (redacted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// After (redacted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// A pending outside change (ADR-022).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingOutside {
    /// Change ID.
    pub id: String,
    /// Which file.
    pub file: DecisionFileKind,
    /// Hash seen on disk.
    pub seen_sha256: String,
    /// RFC 3339 UTC.
    pub detected_at: String,
    /// Redacted diff.
    pub diff: Vec<DiffEntry>,
}

/// Foreman's own change waiting for the next signing commit (from Stage 5 on).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingOwn {
    /// Which file.
    pub file: DecisionFileKind,
    /// Hash of the content to commit.
    pub sha256: String,
    /// RFC 3339 UTC.
    pub at: String,
}

/// A stored read stamp (ADR-010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredStamp {
    /// Device.
    pub dev: u64,
    /// Inode.
    pub ino: u64,
    /// Size.
    pub size: u64,
    /// Modification time (ns).
    pub mtime_ns: i64,
    /// SHA-256.
    pub sha256: String,
}

/// Signing states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SigningState {
    /// Read-back pending.
    CommitPending,
    /// Signed on the current branch.
    Signed,
    /// Signed on another branch.
    SignedOtherBranch,
    /// Signed by hand and confirmed (P1).
    HandSigned,
}

/// One signing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Signing {
    /// State.
    pub state: SigningState,
    /// The commit OID (null when hand-signed).
    pub commit: Option<String>,
    /// Branch.
    pub branch: String,
    /// Packet hash.
    pub packet_sha256: String,
    /// RFC 3339 UTC.
    pub at: String,
}

/// An unfinished real-index refresh (ADR-012).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IndexRefreshPending {
    /// Commit.
    pub commit: String,
    /// Paths.
    pub paths: Vec<String>,
    /// Since.
    pub since: String,
}

/// `progress.json` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgressFile {
    /// Always 1.
    #[serde(deserialize_with = "super::version_const::check::<_, 1>")]
    pub schema_version: u32,
    /// Project.
    pub project_id: String,
    /// Monotonic revision.
    pub rev: u32,
    /// Step progress.
    pub steps: BTreeMap<StepKey, StepProgress>,
    /// Chains.
    pub chains: Vec<Chain>,
    /// Run inputs per step.
    pub run_inputs: BTreeMap<StepKey, BTreeMap<String, String>>,
    /// Remembered inputs.
    pub remembered: BTreeMap<String, String>,
    /// Optional part toggles.
    pub optional_parts: BTreeMap<StepKey, BTreeMap<String, bool>>,
    /// Overrides for the next run.
    pub overrides: BTreeMap<StepKey, StoredOverride>,
    /// Gate runs by stage number.
    pub gate_runs: BTreeMap<String, GateRun>,
    /// Pending outside changes.
    pub pending_outside: Vec<PendingOutside>,
    /// Pending own changes.
    pub pending_own: Vec<PendingOwn>,
    /// Read stamps by repo path.
    pub read_stamps: BTreeMap<String, StoredStamp>,
    /// First launch confirmed.
    pub first_launch_done: bool,
    /// Signings by stage number.
    pub signing: BTreeMap<String, Signing>,
    /// Wait dates by step.
    pub waits: BTreeMap<StepKey, String>,
    /// Pending index refresh.
    pub index_refresh_pending: Option<IndexRefreshPending>,
}

impl ProgressFile {
    /// A fresh progress file.
    #[must_use]
    pub fn new(project_id: &str) -> Self {
        Self {
            schema_version: 1,
            project_id: project_id.to_owned(),
            rev: 0,
            steps: BTreeMap::new(),
            chains: Vec::new(),
            run_inputs: BTreeMap::new(),
            remembered: BTreeMap::new(),
            optional_parts: BTreeMap::new(),
            overrides: BTreeMap::new(),
            gate_runs: BTreeMap::new(),
            pending_outside: Vec::new(),
            pending_own: Vec::new(),
            read_stamps: BTreeMap::new(),
            first_launch_done: false,
            signing: BTreeMap::new(),
            waits: BTreeMap::new(),
            index_refresh_pending: None,
        }
    }
}

impl Versioned for ProgressFile {
    const KIND: &'static str = "progress";
    const CURRENT: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}
