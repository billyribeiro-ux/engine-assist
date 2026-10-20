//! The preflight order and its dependencies (DR-120, ADR-021 §1), and the confirmation reasons
//! (SR-19, DR-106, DR-124).
//!
//! `foreman-sys::launch::prepare_launch` walks [`Phase::ORDER`], asks [`Phase::runs_after`]
//! whether a phase may run given the phases that already blocked, and collects every blocker of
//! the phases that ran. A phase that depends on a blocked phase is skipped: `auth status` never
//! runs when the fingerprint scan found an auth-changing key.

use serde::{Deserialize, Serialize};

use crate::ids::{Mode, Start};
use crate::launch::{Blocker, ConfirmReason};

/// One preflight phase, in DR-120's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// `claude` resolves and `claude -v` meets the floor.
    Version,
    /// Contract probes on a version change (P2).
    Contract,
    /// Fingerprint + SR-07 auth-key scan + caps.
    Fingerprint,
    /// The fingerprint equals the acknowledged one.
    Acknowledgement,
    /// `claude auth status` (cached), SR-08.
    Auth,
    /// Continuing steps: the transcript exists.
    Transcript,
    /// Referenced repo paths and signed packets.
    Inputs,
    /// Run inputs filled.
    RunInputs,
    /// SR-04 / SR-28 on the resolved prompt.
    PromptHygiene,
    /// Argv and environment.
    ArgvEnv,
    /// The run settings file can be written.
    RunSettings,
    /// Confirmation reasons (never a blocker).
    Confirmation,
}

impl Phase {
    /// DR-120's fixed order.
    pub const ORDER: [Self; 12] = [
        Self::Version,
        Self::Contract,
        Self::Fingerprint,
        Self::Acknowledgement,
        Self::Auth,
        Self::Transcript,
        Self::Inputs,
        Self::RunInputs,
        Self::PromptHygiene,
        Self::ArgvEnv,
        Self::RunSettings,
        Self::Confirmation,
    ];

    /// The phases this one depends on: if any of them blocked, this one doesn't run.
    #[must_use]
    pub const fn depends_on(self) -> &'static [Self] {
        match self {
            Self::Version
            | Self::Fingerprint
            | Self::Transcript
            | Self::Inputs
            | Self::RunSettings => &[],
            Self::Contract => &[Self::Version],
            Self::Acknowledgement => &[Self::Fingerprint],
            // SR-08: only after SR-06 and SR-07 pass, and it needs a working `claude`.
            Self::Auth => &[Self::Version, Self::Fingerprint, Self::Acknowledgement],
            Self::RunInputs => &[Self::Inputs],
            Self::PromptHygiene => &[Self::Inputs, Self::RunInputs],
            Self::ArgvEnv => &[Self::PromptHygiene],
            Self::Confirmation => &[
                Self::Version,
                Self::Contract,
                Self::Fingerprint,
                Self::Acknowledgement,
                Self::Auth,
                Self::Transcript,
                Self::Inputs,
                Self::RunInputs,
                Self::PromptHygiene,
                Self::ArgvEnv,
                Self::RunSettings,
            ],
        }
    }

    /// Whether this phase runs, given the phases that blocked so far.
    #[must_use]
    pub fn runs_after(self, blocked: &[Self]) -> bool {
        !self.depends_on().iter().any(|d| blocked.contains(d))
    }
}

/// Collected outcome of the phases (what `evaluate_preflight` judges).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreflightFacts {
    /// Blockers per phase, in the order they were found.
    pub blockers: Vec<(Phase, Blocker)>,
    /// Phases that were skipped because a dependency blocked.
    pub skipped: Vec<Phase>,
}

impl PreflightFacts {
    /// The phases that blocked.
    #[must_use]
    pub fn blocked_phases(&self) -> Vec<Phase> {
        let mut v: Vec<Phase> = self.blockers.iter().map(|(p, _)| *p).collect();
        v.dedup();
        v
    }

    /// Records a phase's blockers.
    pub fn add(&mut self, phase: Phase, bs: impl IntoIterator<Item = Blocker>) {
        self.blockers.extend(bs.into_iter().map(|b| (phase, b)));
    }

    /// Whether `phase` may run now; records it as skipped when not.
    pub fn may_run(&mut self, phase: Phase) -> bool {
        let ok = phase.runs_after(&self.blocked_phases());
        if !ok {
            self.skipped.push(phase);
        }
        ok
    }
}

/// `Ok` when nothing blocked; otherwise the blockers in DR-120's order.
///
/// # Errors
/// The blockers.
pub fn evaluate_preflight(f: &PreflightFacts) -> Result<(), Vec<Blocker>> {
    if f.blockers.is_empty() {
        return Ok(());
    }
    let mut v = f.blockers.clone();
    v.sort_by_key(|(p, _)| *p);
    Err(v.into_iter().map(|(_, b)| b).collect())
}

/// What the confirmation reasons depend on (DR-124).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfirmCtx {
    /// The step was added by the overlay.
    pub overlay_step: bool,
    /// `progress.json` `firstLaunchDone`.
    pub first_launch_done: bool,
    /// `settings.json` `lastSeenClaudeVersion`.
    pub last_seen_version: Option<String>,
    /// The version `claude -v` reports now.
    pub found_version: Option<String>,
}

/// SR-19 / DR-124: every reason that applies, in the contract's order. `policy` is the
/// confirmation baseline (template + alternative + substitution, DR-106) and `effective` the start
/// after overrides.
#[must_use]
pub fn needs_confirmation(
    policy: &Start,
    effective: &Start,
    ctx: &ConfirmCtx,
) -> Vec<ConfirmReason> {
    let mut v = Vec::new();
    if ctx.overlay_step {
        v.push(ConfirmReason::OverlayStep);
    }
    if policy.model != effective.model || policy.mode != effective.mode {
        v.push(ConfirmReason::ModeOrModelChanged {
            from: *policy,
            to: *effective,
        });
    }
    if !ctx.first_launch_done {
        v.push(ConfirmReason::FirstLaunchInProject);
    }
    if let Some(m @ (Mode::Auto | Mode::DontAsk)) = effective.mode {
        v.push(ConfirmReason::AutoOrDontAsk { mode: m });
    }
    if let (Some(from), Some(to)) = (&ctx.last_seen_version, &ctx.found_version)
        && from != to
    {
        v.push(ConfirmReason::ClaudeVersionChanged {
            from: from.clone(),
            to: to.clone(),
        });
    }
    v
}
