//! Sensor ingest, the reading side (F-011, P2, ADR-009, SR-15, SR-18): lists
//! `events/<runId>/`, re-reads each event file through the app-data store (no symlinks, `0600`
//! files written by `foreman-sensor`), drops wrong-token events and folds the rest into
//! `foreman_core::session_state::SessionState`. It returns state; it calls nothing else.

use foreman_core::pathpolicy::{ReadClass, RepoPath};
use foreman_core::schema::sensor_event::SensorEventFile;
use foreman_core::session_state::{self, SessionState};
use foreman_core::store::{Root, StoreError, app_dirs};

/// Every parseable event file of a run, with its `seq`, in `seq` order. Unparseable or
/// newer-schema files are skipped (files.md §4.8: "ignored with a log line").
///
/// # Errors
/// [`StoreError`] when the folder can't be listed safely.
pub fn read_events(
    app_data: &Root,
    run_id: &str,
) -> Result<Vec<(String, SensorEventFile)>, StoreError> {
    if !foreman_core::ids::is_uuid_v4(run_id) {
        return Err(StoreError::NotFound(run_id.to_owned()));
    }
    let dir = format!("events/{run_id}");
    let mut out = Vec::new();
    for name in app_dirs::list_app_data_dir(app_data, &dir)? {
        let Some(seq) = session_state::event_file_seq(&name) else {
            continue;
        };
        let Ok(p) = RepoPath::app_data(&format!("{dir}/{name}")) else {
            continue;
        };
        let Ok((bytes, _)) = app_data.read(&p, ReadClass::AppState) else {
            continue;
        };
        if let Ok(ev) = foreman_core::json::parse::<SensorEventFile>(&bytes) {
            out.push((seq.to_owned(), ev));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Folds the run's new events into `state` (only events after `state.last_seq`; wrong tokens are
/// counted in `discarded`). Display state only (SR-18).
///
/// # Errors
/// As [`read_events`].
pub fn ingest(
    app_data: &Root,
    run_id: &str,
    token: &str,
    mut state: SessionState,
) -> Result<SessionState, StoreError> {
    let evs = read_events(app_data, run_id)?;
    session_state::ingest(&mut state, token, evs.iter().map(|(s, e)| (s.as_str(), e)));
    Ok(state)
}
