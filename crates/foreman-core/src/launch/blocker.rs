//! What a launch can return instead of a plan (`docs/contracts/ipc.md` §2.3 `Blocker`,
//! `ConfirmReason`; §5 `LaunchProgress`). Blockers are values inside `PrepareOutcome`, never
//! thrown errors. `foreman-app` re-exports these shapes over IPC (ADR-004).

use serde::{Deserialize, Serialize};

use crate::compat::Requirement;
use crate::fingerprint::RedactedDiff;
use crate::ids::{CwdKind, Mode, Start};
use crate::probe::ProbeResult;
use crate::sanitize;

/// Why a step can't run at all (`Blocker::NotRunnable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotRunnableReason {
    /// Not applicable on this project's path.
    NotApplicable,
    /// A condition it depends on is undecided.
    Undecided,
    /// The blueprint step has no annotation.
    NeedsAnnotation,
    /// Done and not repeatable.
    Done,
    /// A chat or desktop step (no `claude` argv in v1, DR-122).
    NotCli,
    /// A continuing step whose chain start hasn't run yet.
    ChainNotStarted,
}

/// `ReadOnlyReason` (ipc.md §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReadOnlyReason {
    /// The lock doesn't match the blueprint.
    LockMismatch,
    /// The pinned blueprint is missing.
    BlueprintMissing,
    /// A decision file is invalid.
    DecisionFileInvalid,
    /// A file has a newer schema.
    NewerSchema,
    /// git is missing.
    GitMissing,
    /// The project folder is missing.
    FolderMissing,
}

/// `ReadOnly` (ipc.md §2.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadOnlyInfo {
    /// Why.
    pub reason: ReadOnlyReason,
    /// Detail for display.
    pub detail: String,
}

/// Why the `claude` path was refused (`Blocker::ClaudeMissing`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolPathProblem {
    /// Not absolute.
    Relative,
    /// No such file (or no candidate exists).
    Missing,
    /// Not an executable regular file.
    NotExecutable,
    /// Inside (or resolving into) a registered project.
    InProject,
    /// In a world-writable folder.
    WorldWritable,
}

/// Why the auth check failed (SR-08, fail closed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AuthProblem {
    /// A known method other than the subscription login (`api_key`, `api_key_helper`,
    /// `third_party`, …), or a forced non-subscription login method.
    WrongMethod {
        /// The method `auth status` reported.
        method: String,
    },
    /// `authMethod` missing, `null`, or not a string.
    UnknownMethod,
    /// stdout wasn't JSON.
    Unparsable,
    /// Non-zero exit.
    Exit {
        /// The exit code (`None` = killed by a signal).
        code: Option<i32>,
    },
    /// Didn't finish in 10 s.
    Timeout,
}

/// A launch blocker (ipc.md §2.3). `#[serde(tag = "kind")]` gives TS a discriminated union.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum Blocker {
    /// n/a, undecided, needs annotation, done (non-repeatable).
    NotRunnable {
        /// Why.
        reason: NotRunnableReason,
    },
    /// The project is read-only.
    ReadOnly(ReadOnlyInfo),
    /// `claude` can't be resolved or failed the SR-01 checks.
    ClaudeMissing {
        /// The configured path, if any.
        path: Option<String>,
        /// Why.
        why: ToolPathProblem,
    },
    /// `claude -v` is below the computed floor (ADR-017).
    VersionBelowFloor {
        /// Found version (or the raw output when it didn't parse).
        found: String,
        /// The floor.
        required: String,
        /// The requirements behind it.
        requirements: Vec<Requirement>,
    },
    /// A contract probe disabled the feature this step needs (P2).
    ContractFailed {
        /// The feature.
        feature: String,
        /// The probed version.
        version: String,
    },
    /// The fingerprint differs from the acknowledged one (needs `config_ack`).
    ConfigChanged {
        /// Which cwd.
        cwd: CwdKind,
        /// The new fingerprint.
        fingerprint: String,
        /// The redacted diffs per source (SR-55).
        diffs: Vec<RedactedDiff>,
    },
    /// SR-07: an auth-changing key in a settings source.
    AuthKeyBlocked {
        /// Display path of the file.
        file: String,
        /// The key (`apiKeyHelper`, `env.ANTHROPIC_API_KEY`, `forceLoginMethod`, …).
        key: String,
        /// Stable ID of this hit (`foreman-app` maps it to a one-time `HitId`).
        hit: String,
        /// User settings only (SR-07 exception).
        allowlistable: bool,
    },
    /// A fingerprint source over its cap (256 KB; `~/.claude.json` 64 MB), or one that can't be
    /// read safely (a symlink into a project, a directory symlink, a non-regular file).
    ConfigTooLarge {
        /// Display path.
        file: String,
        /// Size in bytes (0 when the problem isn't size).
        bytes: u32,
    },
    /// S-7′ fail branch only: a project `env` block awaiting approval.
    ProjectEnvUnapproved {
        /// Display path.
        file: String,
        /// SHA-256 of the block.
        sha256: String,
    },
    /// SR-08.
    AuthFailed {
        /// Why.
        why: AuthProblem,
    },
    /// The chain's transcript is gone (DR-113): offers a fresh chain with context.
    TranscriptMissing {
        /// The chain.
        chain: String,
    },
    /// A referenced signed packet is missing.
    SignedPacketMissing {
        /// Stage.
        stage: u8,
    },
    /// "Sign Stage N first".
    PacketUnsigned {
        /// Stage.
        stage: u8,
    },
    /// A required run input is empty.
    RunInputMissing {
        /// Its name.
        name: String,
    },
    /// A token the resolver couldn't fill.
    UnresolvedToken {
        /// The token.
        token: String,
    },
    /// SR-04.
    PromptOptionLike {
        /// The first character.
        first: char,
    },
    /// SR-28.
    PromptControlChar {
        /// Byte offset.
        offset: u32,
        /// The code point.
        codepoint: u32,
    },
    /// A deny-rule path with glob metacharacters that can't be escaped (ADR-021 §6).
    RunSettingsPathUnsafe {
        /// The path.
        path: String,
    },
    /// The step cwd isn't a directory reached without symlinks (ADR-006: no `$HOME` fallback).
    CwdNotDirectory {
        /// Which cwd.
        cwd: CwdKind,
    },
}

impl From<sanitize::Blocker> for Blocker {
    fn from(b: sanitize::Blocker) -> Self {
        match b {
            sanitize::Blocker::PromptOptionLike { first } => Self::PromptOptionLike { first },
            sanitize::Blocker::PromptControlChar { offset, codepoint } => {
                Self::PromptControlChar { offset, codepoint }
            }
        }
    }
}

/// A reason the launch needs Billy's native confirmation (SR-19, DR-124).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum ConfirmReason {
    /// The step was added by the project overlay.
    OverlayStep,
    /// An override changed the model or mode (DR-106: compared with the policy start).
    ModeOrModelChanged {
        /// The policy start.
        from: Start,
        /// The effective start.
        to: Start,
    },
    /// `firstLaunchDone` is false.
    FirstLaunchInProject,
    /// `auto` or `dontAsk` in an interactive session.
    AutoOrDontAsk {
        /// The mode.
        mode: Mode,
    },
    /// The first launch after `claude -v` changed.
    ClaudeVersionChanged {
        /// The last seen version.
        from: String,
        /// The version now.
        to: String,
    },
}

/// Where a re-arm's ending sequence is (`LaunchProgress::Ending`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EndingStep {
    /// `\x03` written.
    CtrlC,
    /// `/exit\r` written.
    Exit,
    /// Waiting for the process to exit.
    Waiting,
}

/// One message on the launch progress Channel (ipc.md §5). Never carries prompt text, env values
/// or config values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "phase"
)]
pub enum LaunchProgress {
    /// `claude -v` read.
    Version {
        /// The version found (`None` when it didn't run or parse).
        found: Option<String>,
    },
    /// Contract probes (P2; `skipped` when not due).
    Contract {
        /// The outcome.
        outcome: ProbeResult,
    },
    /// Fingerprint computed.
    Fingerprint {
        /// No stat cache existed for this cwd.
        first_scan: bool,
    },
    /// Auth checked.
    Auth {
        /// A cache hit (no spawn).
        cached: bool,
    },
    /// Transcript checked.
    Transcript,
    /// Inputs checked.
    Inputs,
    /// Re-arm ending sequence.
    Ending {
        /// Which step.
        step: EndingStep,
    },
    /// The old process exited.
    Exited {
        /// Its exit code.
        code: Option<i32>,
    },
    /// Spawning.
    Spawning,
    /// Done.
    Done,
}
