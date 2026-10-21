//! Step kinds and states (F-007, DR-065–DR-069). Stored states are only what Billy's actions
//! produced (`notStarted`, `copied`, `running`, `done`); derived states (skipped, not applicable,
//! undecided, waiting, needs annotation) override display and refuse every event.

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

pub mod actions;
pub mod closeout;

use crate::ids::Surface;
use crate::rules::Applicability;
use crate::schema::progress::{StepProgress, StoredState};
use crate::template::StepKind;

/// Every displayed state (`docs/contracts/ipc.md` `StepState`). There is no `adopted`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "state", content = "detail")]
pub enum StepState {
    /// Not started.
    NotStarted,
    /// Prompt copied (chat, followup).
    Copied,
    /// Running (CLI run, manual step started).
    Running,
    /// Done.
    Done,
    /// Skipped with a reason.
    Skipped(String),
    /// Not applicable with a reason.
    NotApplicable(String),
    /// Waiting until a date (DR-067).
    WaitingUntil(String),
    /// Waits for conditions.
    Undecided,
    /// No annotation.
    NeedsAnnotation,
    /// A newer grammar.
    NeedsNewerForeman,
}

/// Events (Billy's actions and run outcomes; no event comes from content).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "value")]
pub enum StepEvent {
    /// Copy and open (chat) or copy into the live session (followup).
    Copy,
    /// A CLI/Desktop launch (or a followup's relaunch).
    Spawn,
    /// The run exited with a code. The code is recorded in the run record (DR-066); exit 0 makes
    /// a prompt step done, any other code returns it to `notStarted` (DR-065).
    Exited(i32),
    /// The run failed to start or crashed.
    Errored,
    /// Foreman restarted while it ran.
    Interrupted,
    /// ⌘D (chat, manual, wait after its date).
    MarkDone,
    /// A valid paste-back of the step's `out`.
    PasteBackValid,
    /// A declared `out` file changed after the step started (manual).
    OutChanged,
    /// An action step applied successfully.
    ActionApplied,
    /// A wait step started with its end date (`YYYY-MM-DD`).
    StartWait(String),
    /// Start a manual step.
    Start,
}

/// Facts a transition needs.
#[derive(Debug, Clone)]
pub struct StepCtx {
    /// The step kind.
    pub kind: StepKind,
    /// The surface (prompt kinds).
    pub surface: Option<Surface>,
    /// The derived state from the applicable path.
    pub applicability: Applicability,
    /// Today (local date, passed in).
    pub today: Date,
    /// The wait date, if one was set.
    pub wait_until: Option<Date>,
    /// When the event happens (RFC 3339 UTC).
    pub at: String,
}

/// A refused transition, with the reason shown to Billy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Rejected(pub String);

/// The displayed state (derived states override, DR-050/DR-065).
#[must_use]
pub fn display_state(p: &StepProgress, ctx: &StepCtx) -> StepState {
    match &ctx.applicability {
        Applicability::Skipped(r) => return StepState::Skipped(r.clone()),
        Applicability::NotApplicable(r) => return StepState::NotApplicable(r.clone()),
        Applicability::Undecided(_) => return StepState::Undecided,
        Applicability::NeedsAnnotation | Applicability::Invalid => {
            return StepState::NeedsAnnotation;
        }
        Applicability::NeedsNewerForeman => return StepState::NeedsNewerForeman,
        Applicability::Applicable | Applicability::Skippable => {}
    }
    if let Some(d) = ctx.wait_until
        && ctx.kind == StepKind::Wait
        && p.state != StoredState::Done
        && ctx.today < d
    {
        return StepState::WaitingUntil(d.to_string());
    }
    match p.state {
        StoredState::NotStarted => StepState::NotStarted,
        StoredState::Copied => StepState::Copied,
        StoredState::Running => StepState::Running,
        StoredState::Done => StepState::Done,
    }
}

/// A step that runs in a Claude Code session Foreman spawns. A Desktop step isn't one: Foreman
/// copies its prompt and opens Desktop, so it is `copied` until Mark done (code review W4).
fn cli(ctx: &StepCtx) -> bool {
    matches!(ctx.surface, Some(Surface::Cli))
}

fn with(p: &StepProgress, state: StoredState) -> StepProgress {
    StepProgress { state, ..p.clone() }
}

/// Applies an event (DR-065–DR-069). Every (state, event) pair is legal or rejected with a reason.
///
/// # Errors
/// [`Rejected`] with the reason.
#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "the whole transition table in one place (DR-065)"
)]
pub fn transition(
    p: &StepProgress,
    ctx: &StepCtx,
    e: &StepEvent,
) -> Result<StepProgress, Rejected> {
    let shown = display_state(p, ctx);
    let reject = |why: &str| Err(Rejected(why.to_owned()));
    match &shown {
        StepState::NeedsAnnotation => return reject("needs annotation"),
        StepState::NeedsNewerForeman => return reject("needs a newer Foreman"),
        StepState::NotApplicable(r) => return Err(Rejected(format!("not applicable: {r}"))),
        StepState::Skipped(r) => return Err(Rejected(format!("skipped: {r}"))),
        StepState::Undecided => return reject("decide the conditions first"),
        StepState::WaitingUntil(d) => return Err(Rejected(format!("waiting until {d}"))),
        _ => {}
    }
    let s = p.state;
    match (ctx.kind, e) {
        // Chat prompts and followups: copy; chat finishes by Mark done or a valid paste-back.
        (StepKind::Prompt | StepKind::Repeatable, StepEvent::Copy) if !cli(ctx) => match s {
            StoredState::NotStarted | StoredState::Copied | StoredState::Done => {
                let mut n = with(p, StoredState::Copied);
                n.copied_at = Some(ctx.at.clone());
                Ok(n)
            }
            StoredState::Running => reject("already running"),
        },
        (StepKind::Followup, StepEvent::Copy) => {
            if s == StoredState::Running {
                reject("already running")
            } else {
                let mut n = with(p, StoredState::Copied);
                n.copied_at = Some(ctx.at.clone());
                Ok(n)
            }
        }
        (
            StepKind::Prompt | StepKind::Followup | StepKind::Repeatable,
            StepEvent::MarkDone | StepEvent::PasteBackValid,
        ) if !cli(ctx) => match s {
            StoredState::Copied => Ok(with(p, StoredState::Done)),
            StoredState::NotStarted => reject("copy the prompt first"),
            StoredState::Done => reject("already done"),
            StoredState::Running => reject("still running"),
        },
        (StepKind::Prompt | StepKind::Followup | StepKind::Repeatable, StepEvent::MarkDone) => {
            reject("a CLI step finishes when its run exits; it can't be marked done by hand")
        }
        // CLI runs.
        (StepKind::Prompt | StepKind::Followup | StepKind::Repeatable, StepEvent::Spawn)
            if cli(ctx) =>
        {
            match s {
                StoredState::Running => reject("already running"),
                StoredState::Done if ctx.kind == StepKind::Repeatable => unreachable_done(),
                _ => {
                    let mut n = with(p, StoredState::Running);
                    n.started_at = Some(ctx.at.clone());
                    Ok(n)
                }
            }
        }
        // DR-066: the run ends on any exit code (the code is recorded in the run record).
        // DR-065: only exit 0 of a prompt step makes it done; any other exit returns it to
        // notStarted (run again). A repeatable step is never done (DR-069).
        (StepKind::Prompt | StepKind::Followup | StepKind::Repeatable, StepEvent::Exited(code))
            if cli(ctx) =>
        {
            match s {
                StoredState::Running => {
                    let next = if *code == 0 && ctx.kind != StepKind::Repeatable {
                        StoredState::Done
                    } else {
                        StoredState::NotStarted
                    };
                    let mut n = with(p, next);
                    n.run_count = p.run_count.saturating_add(1);
                    Ok(n)
                }
                _ => reject("no run is running"),
            }
        }
        (
            StepKind::Prompt | StepKind::Followup | StepKind::Repeatable,
            StepEvent::Errored | StepEvent::Interrupted,
        ) if cli(ctx) => match s {
            StoredState::Running => Ok(with(p, StoredState::NotStarted)),
            _ => reject("no run is running"),
        },
        // Manual steps.
        (StepKind::Manual, StepEvent::Start) => match s {
            StoredState::NotStarted | StoredState::Done => {
                let mut n = with(p, StoredState::Running);
                n.started_at = Some(ctx.at.clone());
                Ok(n)
            }
            _ => reject("already started"),
        },
        (StepKind::Manual, StepEvent::OutChanged | StepEvent::MarkDone) => match s {
            StoredState::Running => Ok(with(p, StoredState::Done)),
            StoredState::NotStarted if *e == StepEvent::MarkDone => Ok(with(p, StoredState::Done)),
            StoredState::Done => reject("already done"),
            _ => reject("start the step first"),
        },
        // Wait steps (DR-067).
        (StepKind::Wait, StepEvent::StartWait(d)) => match s {
            StoredState::NotStarted => {
                if Date::strptime("%Y-%m-%d", d).is_err() {
                    return reject("the wait needs a date (YYYY-MM-DD)");
                }
                let mut n = with(p, StoredState::Running);
                n.started_at = Some(ctx.at.clone());
                Ok(n)
            }
            _ => reject("already waiting or done"),
        },
        (StepKind::Wait, StepEvent::MarkDone) => match s {
            StoredState::Running => Ok(with(p, StoredState::Done)),
            _ => reject("start the wait first"),
        },
        // Action steps (DR-066).
        (StepKind::Action, StepEvent::ActionApplied) => match s {
            StoredState::NotStarted | StoredState::Done => Ok(with(p, StoredState::Done)),
            _ => reject("not available"),
        },
        (kind, ev) => Err(Rejected(format!(
            "{ev:?} doesn't apply to a {} step in state {s:?}",
            kind.as_str()
        ))),
    }
}

fn unreachable_done() -> Result<StepProgress, Rejected> {
    Err(Rejected("a repeatable step is never done".into()))
}

/// What a followup does (DR-068).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FollowupAction {
    /// Copy the sanitized prompt and focus the chain's tab; nothing is typed into the PTY.
    CopyAndFocus,
    /// The chain's process exited: a continuing launch (re-arm relaunch).
    Relaunch,
}

/// DR-068: copy into the live session, or relaunch when the chain's process has exited.
#[must_use]
pub fn followup_action(chain_process_live: bool) -> FollowupAction {
    if chain_process_live {
        FollowupAction::CopyAndFocus
    } else {
        FollowupAction::Relaunch
    }
}

/// DR-067: the step after a waiting step is blocked until the date ("Waiting until …").
#[must_use]
pub fn blocked_by_wait(
    prev_kind: StepKind,
    prev_wait: Option<Date>,
    prev_done: bool,
    today: Date,
) -> Option<String> {
    match (prev_kind, prev_wait) {
        (StepKind::Wait, Some(d)) if !prev_done && today < d => Some(format!("Waiting until {d}")),
        _ => None,
    }
}

/// A fresh progress entry.
#[must_use]
pub fn fresh() -> StepProgress {
    StepProgress {
        state: StoredState::NotStarted,
        run_count: 0,
        last_run_id: None,
        started_at: None,
        copied_at: None,
    }
}
