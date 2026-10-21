//! `app::sensor_ingest` (F-011, P2, ADR-009, SR-15, SR-18): the reading side of the sensor.
//!
//! While a run with hooks is live, a thread wakes on a hint watcher over `events/<runId>/` (and
//! every 500 ms without one), folds the new event files into the run's
//! [`SessionState`] (wrong tokens are dropped and logged by count), keeps the newest 20 files, and
//! emits `run:status` when the shown state changes (`running`, `idle`, `blocked`).
//!
//! **Events only update displayed state and enable buttons (SR-18, AC-F011-08).** This module
//! reads, folds and emits; it never launches, re-arms, interrupts, terminates, signs or writes a
//! record (`sensor::events_only_update_state` checks its dependencies). The re-arm reads
//! [`idle_source`] only when Billy presses Run.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use foreman_core::ids::Start;
use foreman_core::rearm::IdleSource;
use foreman_core::schema::run::Observed;
use foreman_core::session_state::{Activity, SessionState, event_file_seq};
use foreman_core::store::{Root, app_dirs};

use crate::ipc::types::RunStatus;
use crate::state::App;

/// Event files kept per run (SR-53).
pub const EVENTS_PER_RUN: usize = 20;
/// The wake-up interval without a hint.
const TICK: Duration = Duration::from_millis(500);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The derived state of each run with hooks, in memory (display only).
#[derive(Debug, Default)]
pub struct Sessions {
    states: Mutex<HashMap<String, (SessionState, Option<RunStatus>)>>,
}

/// The shown status for a derived state (`None`: nothing to show beyond the process status).
#[must_use]
pub fn shown(st: &SessionState) -> Option<RunStatus> {
    match st.activity {
        Activity::Working => Some(RunStatus::Running),
        Activity::Idle if st.outstanding() == 0 => Some(RunStatus::Idle),
        Activity::Idle => Some(RunStatus::Running),
        Activity::Blocked { .. } => Some(RunStatus::Blocked),
        Activity::Unknown | Activity::Ended { .. } => None,
    }
}

/// Keeps the newest [`EVENTS_PER_RUN`] event files of a run (the sensor prunes too; this covers a
/// sensor that was killed before it could).
fn prune(root: &Root, run_id: &str) {
    let dir = format!("events/{run_id}");
    let mut names: Vec<String> = app_dirs::list_app_data_dir(root, &dir)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| event_file_seq(n).is_some())
        .collect();
    if names.len() <= EVENTS_PER_RUN {
        return;
    }
    names.sort();
    let excess = names.len() - EVENTS_PER_RUN;
    for n in names.iter().take(excess) {
        let _ = app_dirs::remove_file(root, &format!("{dir}/{n}"));
    }
}

/// Folds the run's new events into its state; emits `run:status` when the shown state changes
/// and the run is still live. Returns the state.
pub fn ingest_once(app: &App, run_id: &str) -> Option<SessionState> {
    let token = app.sensor().token(run_id)?;
    let root = app.app_root().ok()?;
    let prev = lock(&app.sessions().states).get(run_id).cloned();
    let (state, last_shown) = prev.unwrap_or_default();
    // Wrong-token events are dropped and counted in `state.discarded` (the diagnostics log that
    // records the count is M4-01's).
    let state = foreman_sys::sensor::ingest(&root, run_id, &token, state).ok()?;
    prune(&root, run_id);
    let now = shown(&state);
    let live = app.runs().get(run_id).is_some_and(|r| r.session.is_live());
    let changed = live && now.is_some() && now != last_shown;
    lock(&app.sessions().states).insert(
        run_id.to_owned(),
        (state.clone(), if changed { now } else { last_shown }),
    );
    if changed && let Some(s) = now {
        crate::runs::emit_status(app, run_id, s);
    }
    Some(state)
}

/// Starts the run's ingest thread (a run without hooks has none). It ends after the session
/// exits, with one last pass for `SessionEnd`.
pub fn watch_run(app: &App, run_id: &str) {
    if app.sensor().token(run_id).is_none() {
        return;
    }
    let weak = app.weak();
    let run = run_id.to_owned();
    let dir = app.paths().app_data.join("events").join(run_id);
    let _ = std::thread::Builder::new()
        .name("fm-sensor-ingest".into())
        .spawn(move || {
            let hints = foreman_sys::watch::HintWatcher::start(&[(dir, false)]).ok();
            loop {
                match &hints {
                    Some(h) if h.watched() > 0 => {
                        let _ = h.next(TICK);
                    }
                    _ => std::thread::sleep(TICK),
                }
                let Some(app) = weak.upgrade() else {
                    return;
                };
                let live = app.runs().get(&run).is_some_and(|r| r.session.is_live());
                let _ = ingest_once(&app, &run);
                if !live {
                    // N2: the run is over (its last pass read `SessionEnd`): its token and
                    // derived state go; `run.json` keeps the token for later reads.
                    app.sensor().forget(&run);
                    lock(&app.sessions().states).remove(&run);
                    return;
                }
            }
        });
}

/// The live derived state of a run (display only).
#[must_use]
pub fn live_state(app: &App, run_id: &str) -> Option<SessionState> {
    lock(&app.sessions().states)
        .get(run_id)
        .map(|(s, _)| s.clone())
}

/// The status a live run shows: the sensor's `idle`/`blocked`, else `running`.
#[must_use]
pub fn live_status(app: &App, run_id: &str) -> RunStatus {
    lock(&app.sessions().states)
        .get(run_id)
        .and_then(|(s, _)| shown(s))
        .unwrap_or(RunStatus::Running)
}

/// How the re-arm establishes idle (ADR-008, AC-F010-13, AC-F010-14, AC-F011-09), read when Billy
/// presses Run: the sensor's answer when it has one, Billy's native confirmation otherwise (no
/// sensor, no events yet, or a session that is working or blocked with nothing outstanding).
#[must_use]
pub fn idle_source(app: &App, run_id: &str) -> IdleSource {
    let Some(st) = ingest_once(app, run_id) else {
        return IdleSource::Billy;
    };
    if st.is_idle() {
        IdleSource::Sensor {
            idle: true,
            tasks: 0,
        }
    } else if st.outstanding() > 0 {
        IdleSource::Sensor {
            idle: st.activity == Activity::Idle,
            tasks: st.outstanding(),
        }
    } else {
        IdleSource::Billy
    }
}

/// The derived state of any run from its event files and the token in its `run.json` (`None`
/// without a sensor token or events). Reads only. This is the read for paste-back's "Capture from
/// session" (`last_assistant_message`, AC-F013-14) and gate import.
#[must_use]
pub fn session_state(root: &Root, run_id: &str) -> Option<SessionState> {
    let (f, _) = crate::runs::read_run(root, run_id).ok()?;
    let token = f.sensor_token?;
    let st = foreman_sys::sensor::ingest(root, run_id, &token, SessionState::default()).ok()?;
    (st.last_seq.is_some()).then_some(st)
}

/// The sensor's last assistant message for a run (paste-back "Capture from session").
#[must_use]
pub fn last_assistant_message(root: &Root, run_id: &str) -> Option<String> {
    session_state(root, run_id)?.last_assistant_message
}

/// What the run observed (`run.json` `observed`, files.md §4.6): `None` without the sensor.
#[must_use]
pub fn observed(root: &Root, run_id: &str) -> Option<Observed> {
    let st = session_state(root, run_id)?;
    Some(Observed {
        model: st.observed_model.map(|m| m.chars().take(100).collect()),
        effort: st.effort.map(|e| e.chars().take(20).collect()),
        mode: st.permission_mode.map(|m| m.chars().take(20).collect()),
    })
}

/// (M4-11) The live mode of a live run with the sensor, for the Now/Next strip: its display
/// name, and the expected mode's name when the live one is neither `intended`'s mode nor one of
/// `allowed` (the step's switch targets).
#[must_use]
pub fn live_mode(
    app: &App,
    run_id: &str,
    intended: Start,
    allowed: &[foreman_core::ids::Mode],
) -> Option<crate::ipc::dto::steps::LiveMode> {
    if !app.runs().get(run_id).is_some_and(|r| r.session.is_live()) {
        return None;
    }
    let raw = live_state(app, run_id)?.permission_mode?;
    let parsed = foreman_core::ids::Mode::parse(&raw);
    let ok = parsed.is_some_and(|m| intended.mode == Some(m) || allowed.contains(&m));
    Some(crate::ipc::dto::steps::LiveMode {
        mode: parsed.map_or_else(
            || crate::render::escape_visible(&raw.chars().take(20).collect::<String>()),
            crate::project::mode_name,
        ),
        expected: (!ok)
            .then(|| intended.mode.map(crate::project::mode_name))
            .flatten(),
    })
}

/// The mode guard (AC-F011-06): a warning when the live mode or effort differs from the step's.
/// Shown only; nothing is enforced.
#[must_use]
pub fn mode_guard(app: &App, run_id: &str, intended: Start) -> Option<String> {
    let st = live_state(app, run_id)?;
    if !app.runs().get(run_id).is_some_and(|r| r.session.is_live()) {
        return None;
    }
    let mut out = Vec::new();
    if let (Some(live), Some(want)) = (st.permission_mode.as_deref(), intended.mode)
        && live != want.as_str()
    {
        out.push(format!(
            "Claude Code is in {} mode; this step expects {}.",
            crate::render::escape_visible(live),
            crate::project::mode_name(want)
        ));
    }
    if let Some(live) = st.effort.as_deref()
        && live != intended.effort.as_str()
    {
        out.push(format!(
            "Claude Code runs at {} effort; this step expects {}.",
            crate::render::escape_visible(live),
            intended.effort.as_str()
        ));
    }
    (!out.is_empty()).then(|| format!("{} Foreman doesn't change it.", out.join(" ")))
}
