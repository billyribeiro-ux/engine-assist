//! Core launch shapes → IPC DTOs (ADR-004: the app owns the IPC shapes; `foreman_core::launch`
//! carries the same wire form, ipc.md §2.3, §2.4, §5). Repo-derived text gets visible escapes.

use foreman_core::fingerprint as cfp;
use foreman_core::launch as cl;
use foreman_core::probe::ProbeResult;

use crate::ipc::dto::launch::{ContractOutcome, EndingStep, LaunchProgress};
use crate::ipc::types::{
    AuthProblem, Blocker, ChainId, ChangeKind, ConfirmReason, HitId, NotRunnableReason, ReadOnly,
    ReadOnlyReason, RedactedDiff, RedactedEntry, Requirement, Sha256, Stage, ToolPathProblem,
};
use crate::render::escape_visible;

/// Foreman's sentence for a reason a step can't run.
#[must_use]
pub fn not_runnable_text(r: NotRunnableReason) -> &'static str {
    match r {
        NotRunnableReason::NotApplicable => "This step doesn't apply on this project's path.",
        NotRunnableReason::Undecided => "A condition this step depends on is undecided.",
        NotRunnableReason::NeedsAnnotation => {
            "This step has no annotation, so Foreman can't run it."
        }
        NotRunnableReason::Done => "This step is done and doesn't repeat.",
        NotRunnableReason::NeedsNewerForeman => {
            "The blueprint needs a newer Foreman for this step."
        }
        NotRunnableReason::Waiting => "This step is waiting for its date.",
        NotRunnableReason::Running => "This step is running.",
        NotRunnableReason::NotCli => "This step doesn't run in Claude Code.",
        NotRunnableReason::ChainNotStarted => {
            "This step continues a session that hasn't started. Run the chain's first step."
        }
    }
}

fn reason(r: cl::NotRunnableReason) -> NotRunnableReason {
    use cl::NotRunnableReason as C;
    match r {
        C::NotApplicable => NotRunnableReason::NotApplicable,
        C::Undecided => NotRunnableReason::Undecided,
        C::NeedsAnnotation => NotRunnableReason::NeedsAnnotation,
        C::Done => NotRunnableReason::Done,
        C::NotCli => NotRunnableReason::NotCli,
        C::ChainNotStarted => NotRunnableReason::ChainNotStarted,
    }
}

/// The DTO reason as the core's (the core has no `needsNewerForeman`, `waiting`, `running`; those
/// are `needsAnnotation`/`done` there).
#[must_use]
pub fn core_reason(r: NotRunnableReason) -> cl::NotRunnableReason {
    use cl::NotRunnableReason as C;
    match r {
        NotRunnableReason::NotApplicable => C::NotApplicable,
        NotRunnableReason::Undecided => C::Undecided,
        NotRunnableReason::NeedsAnnotation | NotRunnableReason::NeedsNewerForeman => {
            C::NeedsAnnotation
        }
        NotRunnableReason::Done | NotRunnableReason::Waiting | NotRunnableReason::Running => {
            C::Done
        }
        NotRunnableReason::NotCli => C::NotCli,
        NotRunnableReason::ChainNotStarted => C::ChainNotStarted,
    }
}

fn read_only_reason(r: cl::ReadOnlyReason) -> ReadOnlyReason {
    use cl::ReadOnlyReason as C;
    match r {
        C::LockMismatch => ReadOnlyReason::LockMismatch,
        C::BlueprintMissing => ReadOnlyReason::BlueprintMissing,
        C::DecisionFileInvalid => ReadOnlyReason::DecisionFileInvalid,
        C::NewerSchema => ReadOnlyReason::NewerSchema,
        C::GitMissing => ReadOnlyReason::GitMissing,
        C::FolderMissing => ReadOnlyReason::FolderMissing,
    }
}

fn tool_problem(p: cl::ToolPathProblem) -> ToolPathProblem {
    use cl::ToolPathProblem as C;
    match p {
        C::Relative => ToolPathProblem::Relative,
        C::Missing => ToolPathProblem::Missing,
        C::NotExecutable => ToolPathProblem::NotExecutable,
        C::InProject => ToolPathProblem::InProject,
        C::WorldWritable => ToolPathProblem::WorldWritable,
    }
}

fn auth(p: &cl::AuthProblem) -> AuthProblem {
    use cl::AuthProblem as C;
    match p {
        C::WrongMethod { method } => AuthProblem::WrongMethod {
            method: escape_visible(method),
        },
        C::UnknownMethod => AuthProblem::UnknownMethod,
        C::Unparsable => AuthProblem::Unparsable,
        C::Exit { code } => AuthProblem::Exit { code: *code },
        C::Timeout => AuthProblem::Timeout,
    }
}

/// A fingerprint diff (SR-55; values already redacted by the core).
#[must_use]
pub fn redacted(d: &cfp::RedactedDiff) -> RedactedDiff {
    RedactedDiff {
        source: escape_visible(&d.source),
        entries: d
            .entries
            .iter()
            .map(|e| RedactedEntry {
                key_path: escape_visible(&e.key_path),
                change: match e.change {
                    cfp::ChangeKind::Added => ChangeKind::Added,
                    cfp::ChangeKind::Removed => ChangeKind::Removed,
                    cfp::ChangeKind::ValueChanged => ChangeKind::ValueChanged,
                },
                before: e.before.as_deref().map(escape_visible),
                after: e.after.as_deref().map(escape_visible),
            })
            .collect(),
    }
}

/// A core blocker as the DTO.
#[must_use]
pub fn blocker(b: &cl::Blocker) -> Blocker {
    use cl::Blocker as C;
    match b {
        C::NotRunnable { reason: r } => {
            let r = reason(*r);
            Blocker::NotRunnable {
                reason: r,
                text: not_runnable_text(r).to_owned(),
            }
        }
        C::ReadOnly(i) => Blocker::ReadOnly(ReadOnly {
            reason: read_only_reason(i.reason),
            detail: escape_visible(&i.detail),
        }),
        C::ClaudeMissing { path, why } => Blocker::ClaudeMissing {
            path: path.as_deref().map(escape_visible),
            why: tool_problem(*why),
        },
        C::VersionBelowFloor {
            found,
            required,
            requirements,
        } => Blocker::VersionBelowFloor {
            found: escape_visible(found),
            required: required.clone(),
            requirements: requirements
                .iter()
                .map(|r| Requirement {
                    feature: r.feature.clone(),
                    min: r.min.clone(),
                    source: r.source.clone(),
                })
                .collect(),
        },
        C::ContractFailed { feature, version } => Blocker::ContractFailed {
            feature: feature.clone(),
            version: escape_visible(version),
        },
        C::ConfigChanged {
            cwd,
            fingerprint,
            diffs,
        } => Blocker::ConfigChanged {
            cwd: (*cwd).into(),
            fingerprint: Sha256::from_trusted(fingerprint.as_str()),
            diffs: diffs.iter().map(redacted).collect(),
        },
        C::AuthKeyBlocked {
            file,
            key,
            hit,
            allowlistable,
        } => Blocker::AuthKeyBlocked {
            file: escape_visible(file),
            key: escape_visible(key),
            hit: HitId::from_trusted(hit.as_str()),
            allowlistable: *allowlistable,
        },
        C::ConfigTooLarge { file, bytes } => Blocker::ConfigTooLarge {
            file: escape_visible(file),
            bytes: *bytes,
        },
        C::ProjectEnvUnapproved { file, sha256 } => Blocker::ProjectEnvUnapproved {
            file: escape_visible(file),
            sha256: Sha256::from_trusted(sha256.as_str()),
        },
        C::AuthFailed { why } => Blocker::AuthFailed { why: auth(why) },
        C::TranscriptMissing { chain } => Blocker::TranscriptMissing {
            chain: ChainId::from_trusted(chain.as_str()),
        },
        C::SignedPacketMissing { stage } => Blocker::SignedPacketMissing {
            stage: Stage(*stage),
        },
        C::PacketUnsigned { stage } => Blocker::PacketUnsigned {
            stage: Stage(*stage),
        },
        C::RunInputMissing { name } => Blocker::RunInputMissing {
            name: escape_visible(name),
        },
        C::UnresolvedToken { token } => Blocker::UnresolvedToken {
            token: escape_visible(token),
        },
        C::PromptOptionLike { first } => Blocker::PromptOptionLike {
            first: first.to_string(),
        },
        C::PromptControlChar { offset, codepoint } => Blocker::PromptControlChar {
            offset: *offset,
            codepoint: *codepoint,
        },
        C::RunSettingsPathUnsafe { path } => Blocker::RunSettingsPathUnsafe {
            path: escape_visible(path),
        },
        C::CwdNotDirectory { cwd } => Blocker::CwdNotDirectory { cwd: (*cwd).into() },
    }
}

/// A DTO blocker the resolver produced, as the core's (the launch pipeline's inputs phases).
#[must_use]
pub fn core_blocker(b: &Blocker) -> Option<cl::Blocker> {
    Some(match b {
        Blocker::RunInputMissing { name } => cl::Blocker::RunInputMissing { name: name.clone() },
        Blocker::UnresolvedToken { token } => cl::Blocker::UnresolvedToken {
            token: token.clone(),
        },
        Blocker::PacketUnsigned { stage } => cl::Blocker::PacketUnsigned { stage: stage.0 },
        Blocker::SignedPacketMissing { stage } => {
            cl::Blocker::SignedPacketMissing { stage: stage.0 }
        }
        Blocker::NotRunnable { reason: r, .. } => cl::Blocker::NotRunnable {
            reason: core_reason(*r),
        },
        _ => return None,
    })
}

/// A confirmation reason as the DTO.
#[must_use]
pub fn confirm(c: &cl::ConfirmReason) -> ConfirmReason {
    use cl::ConfirmReason as C;
    match c {
        C::OverlayStep => ConfirmReason::OverlayStep,
        C::ModeOrModelChanged { from, to } => ConfirmReason::ModeOrModelChanged {
            from: (*from).into(),
            to: (*to).into(),
        },
        C::FirstLaunchInProject => ConfirmReason::FirstLaunchInProject,
        C::AutoOrDontAsk { mode } => ConfirmReason::AutoOrDontAsk {
            mode: (*mode).into(),
        },
        C::ClaudeVersionChanged { from, to } => ConfirmReason::ClaudeVersionChanged {
            from: escape_visible(from),
            to: escape_visible(to),
        },
    }
}

/// The native dialog's line for a confirmation reason (SR-19).
#[must_use]
pub fn confirm_text(c: &cl::ConfirmReason) -> String {
    use cl::ConfirmReason as C;
    match c {
        C::OverlayStep => {
            "This step was added to this project, not taken from the blueprint.".into()
        }
        C::ModeOrModelChanged { from, to } => {
            format!("It runs as {to}, not the blueprint's {from}.")
        }
        C::FirstLaunchInProject => "It's the first Claude Code launch in this project.".into(),
        C::AutoOrDontAsk { mode } => format!(
            "Permission mode {}: Claude Code acts without asking you first.",
            mode.as_str()
        ),
        C::ClaudeVersionChanged { from, to } => format!(
            "Claude Code updated {} → {}.",
            escape_visible(from),
            escape_visible(to)
        ),
    }
}

/// A progress message as the DTO (no prompt text, env or config values; ipc.md §5).
#[must_use]
pub fn progress(p: &cl::LaunchProgress) -> LaunchProgress {
    use cl::LaunchProgress as C;
    match p {
        C::Version { found } => LaunchProgress::Version {
            found: found.as_deref().map(escape_visible),
        },
        C::Contract { outcome } => LaunchProgress::Contract {
            outcome: match outcome {
                ProbeResult::Pass => ContractOutcome::Pass,
                ProbeResult::FlagFailure => ContractOutcome::FlagFailure,
                ProbeResult::Inconclusive => ContractOutcome::Inconclusive,
                ProbeResult::Skipped => ContractOutcome::Skipped,
            },
        },
        C::Fingerprint { first_scan } => LaunchProgress::Fingerprint {
            first_scan: *first_scan,
        },
        C::Auth { cached } => LaunchProgress::Auth { cached: *cached },
        C::Transcript => LaunchProgress::Transcript,
        C::Inputs => LaunchProgress::Inputs,
        C::Ending { step } => LaunchProgress::Ending {
            step: match step {
                cl::blocker::EndingStep::CtrlC => EndingStep::CtrlC,
                cl::blocker::EndingStep::Exit => EndingStep::Exit,
                cl::blocker::EndingStep::Waiting => EndingStep::Waiting,
            },
        },
        C::Exited { code } => LaunchProgress::Exited { code: *code },
        C::Spawning => LaunchProgress::Spawning,
        C::Done => LaunchProgress::Done,
    }
}
