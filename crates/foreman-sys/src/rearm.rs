//! Drives `foreman_core::rearm`'s state machine against a live session (F-010, ADR-008,
//! DR-114–DR-116, SR-13, SR-18). Native dialogs, the re-prepare and the relaunch come from the
//! caller (`foreman-app`), so this module never decides anything security-relevant by itself and
//! has no way to signal the old process.

use std::time::Duration;

use foreman_core::launch::LaunchProgress;
use foreman_core::launch::blocker::EndingStep;
use foreman_core::rearm::{self, Action, Event, IdleSource, State, UserAction};

use crate::launch::{LaunchPlan, PrepareOutcome};
use crate::pty::{PtyError, PtySession};

/// What the caller supplies: native dialogs (SR-24a) and progress.
pub trait RearmUi {
    /// P1: "Claude is idle and nothing is running in the background" → Continue (true) / Cancel.
    fn confirm_idle(&self) -> bool;
    /// P2: "N background tasks still running" → End anyway (true) / Wait.
    fn confirm_busy(&self, tasks: u32) -> bool;
    /// "Claude didn't exit. End it yourself, then press Continue."
    fn end_yourself(&self);
    /// Progress messages (`Ending`, `Exited`, `Spawning`).
    fn progress(&self, p: LaunchProgress);
}

/// How a re-arm ended.
#[derive(Debug)]
pub enum RearmOutcome {
    /// The new session runs.
    Relaunched {
        /// The new session.
        session: PtySession,
        /// The plan it was spawned from (the re-prepared one, equal to the confirmed one).
        plan: Box<LaunchPlan>,
    },
    /// The old process didn't exit within 10 s; Billy ends it, then `rearm_continue`.
    EndYourself,
    /// Billy cancelled the idle dialog; nothing was sent.
    Cancelled,
    /// The re-prepared plan differs (DR-116); nothing spawned. Carries the fresh outcome.
    Stale(Box<PrepareOutcome>),
}

/// A driver failure.
#[derive(Debug, thiserror::Error)]
pub enum RearmError {
    /// The machine rejected an event (a bug or a racing caller).
    #[error(transparent)]
    Rejected(#[from] rearm::Rejected),
    /// The relaunch failed.
    #[error("relaunch failed: {0}")]
    Relaunch(String),
    /// Writing to the old session failed for a reason other than its exit.
    #[error(transparent)]
    Pty(#[from] PtyError),
}

/// The caller's hooks for the end of the sequence.
pub struct Hooks<'a> {
    /// Runs `launch::prepare` again (DR-116).
    pub prepare_again: &'a mut dyn FnMut() -> PrepareOutcome,
    /// Archives the old buffer and writes the divider (called before the spawn).
    pub archive: &'a mut dyn FnMut(),
    /// Spawns the new session from the plan (`launch::start_session`).
    pub relaunch: &'a mut dyn FnMut(&LaunchPlan) -> Result<PtySession, String>,
}

impl std::fmt::Debug for Hooks<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Hooks")
    }
}

/// The ending sequence's timings (ADR-008: 150 ms and 10 s; tests shorten them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timings {
    /// Ctrl-C → `/exit` pause.
    pub pause: Duration,
    /// Exit wait.
    pub exit_wait: Duration,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            pause: rearm::CTRL_C_PAUSE,
            exit_wait: rearm::EXIT_WAIT,
        }
    }
}

fn run(
    mut state: State,
    first: Event,
    old: Option<&PtySession>,
    confirmed: &LaunchPlan,
    ui: &dyn RearmUi,
    hooks: &mut Hooks<'_>,
    t: Timings,
) -> Result<RearmOutcome, RearmError> {
    let (pause, exit_wait) = (t.pause, t.exit_wait);
    let mut queue = vec![first];
    let mut fresh: Option<PrepareOutcome> = None;
    let mut spawned: Option<PtySession> = None;
    let live = || old.is_some_and(PtySession::is_live);
    while let Some(ev) = queue.pop() {
        let (next, actions) = rearm::step(state, ev)?;
        state = next;
        for a in actions {
            let ev = match a {
                Action::ShowIdleDialog => Some(Event::IdleAnswer(ui.confirm_idle())),
                Action::ShowBusyDialog { tasks } => Some(Event::BusyAnswer(ui.confirm_busy(tasks))),
                Action::WriteCtrlC | Action::WriteExit => {
                    let (bytes, step) = if a == Action::WriteCtrlC {
                        (rearm::CTRL_C, EndingStep::CtrlC)
                    } else {
                        (rearm::EXIT, EndingStep::Exit)
                    };
                    match old.map(|s| s.write(bytes)) {
                        Some(Ok(())) => {
                            ui.progress(LaunchProgress::Ending { step });
                            Some(Event::Done)
                        }
                        Some(Err(PtyError::NotLive)) | None => Some(Event::Exited),
                        Some(Err(e)) => return Err(e.into()),
                    }
                }
                Action::Pause => {
                    std::thread::sleep(pause);
                    Some(if live() { Event::Done } else { Event::Exited })
                }
                Action::WaitExit => {
                    ui.progress(LaunchProgress::Ending {
                        step: EndingStep::Waiting,
                    });
                    match old.and_then(|s| s.wait_exit(exit_wait)) {
                        Some(info) => {
                            ui.progress(LaunchProgress::Exited { code: info.code });
                            Some(Event::Exited)
                        }
                        None if !live() => Some(Event::Exited),
                        None => Some(Event::Timeout),
                    }
                }
                Action::PrepareAgain => {
                    let o = (hooks.prepare_again)();
                    let same = matches!(&o, PrepareOutcome::Ready(p) if p.same_as(confirmed));
                    fresh = Some(o);
                    Some(Event::Rechecked { same })
                }
                Action::ArchiveAndDivider => {
                    (hooks.archive)();
                    None
                }
                Action::Spawn => {
                    ui.progress(LaunchProgress::Spawning);
                    let Some(PrepareOutcome::Ready(plan)) = fresh.as_ref() else {
                        return Err(RearmError::Relaunch("no plan".into()));
                    };
                    let s = (hooks.relaunch)(plan).map_err(RearmError::Relaunch)?;
                    spawned = Some(s);
                    Some(Event::Done)
                }
                Action::ShowEndYourself => {
                    ui.end_yourself();
                    None
                }
            };
            if let Some(e) = ev {
                queue.push(e);
            }
        }
    }
    Ok(match state {
        State::Relaunched => {
            ui.progress(LaunchProgress::Done);
            let Some(PrepareOutcome::Ready(plan)) = fresh else {
                return Err(RearmError::Relaunch("no plan".into()));
            };
            RearmOutcome::Relaunched {
                session: spawned.ok_or_else(|| RearmError::Relaunch("no session".into()))?,
                plan,
            }
        }
        State::EndYourself => RearmOutcome::EndYourself,
        State::Stale => RearmOutcome::Stale(Box::new(fresh.unwrap_or(PrepareOutcome::Blocked {
            blockers: vec![],
            warnings: vec![],
        }))),
        _ => RearmOutcome::Cancelled,
    })
}

/// `rearm_start`: idle assertion, Ctrl-C, 150 ms, `/exit`, ≤ 10 s, re-prepare, relaunch. `old` is
/// the chain's live session, if any.
///
/// # Errors
/// [`RearmError`].
pub fn rearm_start(
    by: UserAction,
    old: Option<&PtySession>,
    idle: IdleSource,
    confirmed: &LaunchPlan,
    ui: &dyn RearmUi,
    hooks: &mut Hooks<'_>,
) -> Result<RearmOutcome, RearmError> {
    rearm_start_with(by, old, idle, confirmed, ui, hooks, Timings::default())
}

/// [`rearm_start`] with explicit timings (tests).
///
/// # Errors
/// [`RearmError`].
pub fn rearm_start_with(
    by: UserAction,
    old: Option<&PtySession>,
    idle: IdleSource,
    confirmed: &LaunchPlan,
    ui: &dyn RearmUi,
    hooks: &mut Hooks<'_>,
    t: Timings,
) -> Result<RearmOutcome, RearmError> {
    let process_alive = old.is_some_and(PtySession::is_live);
    run(
        State::Ready,
        Event::Start {
            by,
            process_alive,
            idle,
        },
        old,
        confirmed,
        ui,
        hooks,
        t,
    )
}

/// `rearm_continue`: after "End it yourself", only once the old process is gone.
///
/// # Errors
/// [`RearmError`] (`Rejected` while the process is still alive shows the dialog again).
pub fn rearm_continue(
    by: UserAction,
    old: Option<&PtySession>,
    confirmed: &LaunchPlan,
    ui: &dyn RearmUi,
    hooks: &mut Hooks<'_>,
) -> Result<RearmOutcome, RearmError> {
    let process_alive = old.is_some_and(PtySession::is_live);
    run(
        State::EndYourself,
        Event::Continue { by, process_alive },
        old,
        confirmed,
        ui,
        hooks,
        Timings::default(),
    )
}
