//! Re-arm as a pure state machine (F-010, ADR-008, DR-114–DR-116, SR-13, SR-18).
//!
//! `foreman-sys::rearm` drives it against a live `PtySession`: it feeds [`Event`]s in and performs
//! the [`Action`]s that come out. The machine has no action that signals a process: ending a
//! session is only ever Ctrl-C then `/exit`, and only after the idle assertion (P1: Billy's native
//! dialog; P2: the sensor). On a timeout it asks Billy to end the session himself; Terminate is a
//! separate path with its own native confirmation (ADR-006). The only way in is a
//! [`UserAction`], which only an IPC command handler constructs (SR-18).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ids::{Effort, Mode, Model, Start, StepKey};

/// Ctrl-C to `/exit` pause (ADR-008).
pub const CTRL_C_PAUSE: Duration = Duration::from_millis(150);
/// How long Foreman waits for the process to exit after `/exit` (ADR-008, P1).
pub const EXIT_WAIT: Duration = Duration::from_secs(10);
/// The bytes of the ending sequence.
pub const CTRL_C: &[u8] = b"\x03";
/// `/exit` and Enter.
pub const EXIT: &[u8] = b"/exit\r";

/// Proof that an action started from Billy's click or keystroke (SR-18). Constructed only by
/// `foreman-app`'s IPC command handlers (`rearm_start`, `rearm_continue`); the test
/// `rearm::only_from_user_action` checks that no other module calls [`UserAction::from_ipc_invoke`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserAction {
    _private: (),
}

impl UserAction {
    /// Called by an IPC command handler for a user-invoked command, nowhere else.
    #[must_use]
    pub const fn from_ipc_invoke() -> Self {
        Self { _private: () }
    }
}

/// How idleness is established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum IdleSource {
    /// P1: Billy asserts it in a native dialog.
    Billy,
    /// P2: the sensor's derived state.
    Sensor {
        /// The sensor reports idle (last event `Stop`, nothing outstanding).
        idle: bool,
        /// Outstanding subagents, tasks and background shells.
        tasks: u32,
    },
}

/// The machine's states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    /// Nothing started.
    Ready,
    /// The native idle dialog is up (P1).
    AwaitingIdle,
    /// The sensor reports background work; the native "Wait / End anyway" dialog is up (P2).
    AwaitingBusyChoice,
    /// `\x03` is to be written.
    SendCtrlC,
    /// Pausing 150 ms.
    Pause,
    /// `/exit\r` is to be written.
    SendExit,
    /// Waiting ≤ 10 s for the exit.
    WaitingExit,
    /// The old process is gone; prepare again (DR-116).
    Rechecking,
    /// The plan still matches: archive, divider, spawn.
    Relaunching,
    /// Relaunched.
    Relaunched,
    /// "Claude didn't exit. End it yourself, then press Continue."
    EndYourself,
    /// Billy cancelled; nothing was sent.
    Cancelled,
    /// The re-prepared plan differs from the confirmed one; nothing spawned.
    Stale,
}

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// `rearm_start`: the chain's process is alive or not.
    Start {
        /// Proof of a user action.
        by: UserAction,
        /// The old process is still running.
        process_alive: bool,
        /// How idle is established.
        idle: IdleSource,
    },
    /// The native idle dialog's answer (Continue = true).
    IdleAnswer(bool),
    /// The busy dialog's answer: End anyway = true, Wait = false.
    BusyAnswer(bool),
    /// An action finished.
    Done,
    /// The old process exited.
    Exited,
    /// The exit wait ran out.
    Timeout,
    /// `rearm_continue` after "End it yourself": whether the process is still alive.
    Continue {
        /// Proof of a user action.
        by: UserAction,
        /// The old process is still running.
        process_alive: bool,
    },
    /// The re-prepared plan's argv and prompt hash equal the confirmed plan's.
    Rechecked {
        /// Equal.
        same: bool,
    },
}

/// What the driver must do next. There is deliberately no signal here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum Action {
    /// Show the native "Claude is idle and nothing is running in the background" dialog.
    ShowIdleDialog,
    /// Show the native "N background tasks still running" dialog (Wait / End anyway).
    ShowBusyDialog {
        /// Outstanding tasks.
        tasks: u32,
    },
    /// Write `\x03`.
    WriteCtrlC,
    /// Sleep [`CTRL_C_PAUSE`].
    Pause,
    /// Write `/exit\r`.
    WriteExit,
    /// Wait up to [`EXIT_WAIT`] for the exit.
    WaitExit,
    /// Run `launch::prepare` again and compare (DR-116).
    PrepareAgain,
    /// Archive the old buffer and write the divider.
    ArchiveAndDivider,
    /// Spawn the new session.
    Spawn,
    /// Show "Claude didn't exit. End it yourself, then press Continue."
    ShowEndYourself,
}

/// An event that doesn't fit the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("re-arm: {event} in state {state:?}")]
pub struct Rejected {
    /// The state.
    pub state: State,
    /// The event.
    pub event: &'static str,
}

fn ending() -> (State, Vec<Action>) {
    (State::SendCtrlC, vec![Action::WriteCtrlC])
}

/// One transition.
///
/// # Errors
/// [`Rejected`] when the event doesn't fit the state.
pub fn step(state: State, event: Event) -> Result<(State, Vec<Action>), Rejected> {
    use Event as E;
    use State as S;
    let rej = |event: &'static str| Rejected { state, event };
    Ok(match (state, event) {
        (
            S::Ready,
            E::Start {
                process_alive: false,
                ..
            },
        )
        | (S::SendCtrlC | S::Pause | S::SendExit | S::WaitingExit, E::Exited)
        | (
            S::EndYourself,
            E::Continue {
                process_alive: false,
                ..
            },
        ) => (S::Rechecking, vec![Action::PrepareAgain]),
        (
            S::Ready,
            E::Start {
                idle: IdleSource::Billy,
                ..
            },
        ) => (S::AwaitingIdle, vec![Action::ShowIdleDialog]),
        (
            S::Ready,
            E::Start {
                idle:
                    IdleSource::Sensor {
                        idle: true,
                        tasks: 0,
                    },
                ..
            },
        ) => ending(),
        (
            S::Ready,
            E::Start {
                idle: IdleSource::Sensor { tasks, .. },
                ..
            },
        ) => (
            S::AwaitingBusyChoice,
            vec![Action::ShowBusyDialog { tasks }],
        ),
        (S::AwaitingIdle | S::AwaitingBusyChoice, E::IdleAnswer(true) | E::BusyAnswer(true)) => {
            ending()
        }
        (S::AwaitingIdle | S::AwaitingBusyChoice, E::IdleAnswer(false) | E::BusyAnswer(false)) => {
            (S::Cancelled, vec![])
        }
        (S::SendCtrlC, E::Done) => (S::Pause, vec![Action::Pause]),
        (S::Pause, E::Done) => (S::SendExit, vec![Action::WriteExit]),
        (S::SendExit, E::Done) => (S::WaitingExit, vec![Action::WaitExit]),
        (S::WaitingExit, E::Timeout)
        | (
            S::EndYourself,
            E::Continue {
                process_alive: true,
                ..
            },
        ) => (S::EndYourself, vec![Action::ShowEndYourself]),
        (S::Rechecking, E::Rechecked { same: true }) => (
            S::Relaunching,
            vec![Action::ArchiveAndDivider, Action::Spawn],
        ),
        (S::Rechecking, E::Rechecked { same: false }) => (S::Stale, vec![]),
        (S::Relaunching, E::Done) => (S::Relaunched, vec![]),
        (_, E::Start { .. }) => return Err(rej("start")),
        (_, E::Continue { .. }) => return Err(rej("continue")),
        (_, E::IdleAnswer(_)) => return Err(rej("idleAnswer")),
        (_, E::BusyAnswer(_)) => return Err(rej("busyAnswer")),
        (_, E::Done) => return Err(rej("done")),
        (_, E::Exited) => return Err(rej("exited")),
        (_, E::Timeout) => return Err(rej("timeout")),
        (_, E::Rechecked { .. }) => return Err(rej("rechecked")),
    })
}

/// Display name of a model alias in the divider.
#[must_use]
pub const fn model_label(m: Model) -> &'static str {
    match m {
        Model::Fable => "Fable 5.1",
        Model::Opus => "Opus 5.5",
        Model::Sonnet => "Sonnet 5.5",
        Model::Haiku => "Haiku 5.5",
        Model::Opusplan => "Opus 5.5 (opusplan)",
    }
}

/// Display name of a mode in the divider (the blueprint's words).
#[must_use]
pub const fn mode_label(m: Option<Mode>) -> &'static str {
    match m {
        Some(Mode::Plan) => "Plan",
        Some(Mode::Default) => "Manual",
        Some(Mode::AcceptEdits) => "Accept edits",
        Some(Mode::Auto) => "Auto",
        Some(Mode::DontAsk) => "dontAsk",
        None => "",
    }
}

/// DR-115: `── <step> · <model> · <effort> · <mode> · permission grants reset ──`.
#[must_use]
pub fn divider(step: &StepKey, title: &str, start: &Start) -> String {
    let effort = match start.effort {
        Effort::Adaptive => "adaptive",
        e => e.as_str(),
    };
    let head = if title.is_empty() {
        step.to_string()
    } else {
        format!("{step} {title}")
    };
    format!(
        "── {head} · {} · {effort} · {} · permission grants reset ──",
        model_label(start.model),
        mode_label(start.mode)
    )
}
