//! `app::sensor` (F-011, P2, ADR-009, SR-15, SR-53): the sensor binary's install and the per-run
//! hooks. The reading side (ingest, derived state, `run:status`) is [`crate::sensor_ingest`].
//!
//! - **Bundle.** The release build embeds the `foreman-sensor` binary in the app binary
//!   (`FOREMAN_SENSOR_BIN` at build time, `build.rs`), so the bytes and their SHA-256 are fixed
//!   when Foreman is built and signed with it. A build without it has no sensor: every feature
//!   keeps its P1 manual path (ADR-009 exit path, NFR-13).
//! - **Install.** At startup, and again whenever a check fails, the bytes are written to
//!   `<app-data>/bin/foreman-sensor-<sha256-12>` (`0700`, files.md §2 row 24) through the store;
//!   other `foreman-sensor-*` files there are removed (replaced on hash change).
//! - **Per run (SR-15).** Before each run settings file is written, the installed binary is read
//!   back and hashed. A mismatch re-installs once; if it still doesn't match, the run gets no hooks
//!   and a warning, and the P1 manual behaviors apply (AC-F011-02).
//! - **Token.** Two OS-random UUID v4 values in hex (244 random bits, ADR-009), stored only in the
//!   run settings and `run.json`. In memory: a prepared run's token for [`PREPARED_TTL`] (an
//!   unused plan's is forgotten), a started run's until its ingest thread ends after the exit
//!   (code review N2).
//! - **Retention (SR-53, AC-F011-07).** A run's events go 30 days after it ended (at the startup
//!   scan); at sign the signing code deletes the stage's events.
//!
//! Nothing here is reached from a sensor event (SR-18): these run from user commands and startup.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use foreman_core::launch::run_settings::SensorHooks;
use foreman_core::pathpolicy::{ReadClass, RepoPath};
use foreman_core::store::{Root, app_dirs, sha256_hex};
use uuid::Uuid;

use crate::state::App;

/// How long a prepared run's token is kept when the run doesn't start: a plan lives 60 s
/// (IPC-R7) and the native launch confirmation may stay open a while.
pub const PREPARED_TTL: Duration = Duration::from_secs(600);

/// `bin/foreman-sensor-` (files.md §2 row 24).
pub const BIN_PREFIX: &str = "foreman-sensor-";
/// Days a run's events are kept after it ended (SR-53).
pub const EVENT_DAYS: i64 = foreman_core::retention::EVENT_DAYS;

/// The warning a run without the sensor shows when the binary didn't match (AC-F011-02).
pub const HASH_WARNING: &str = "The session sensor didn't match its expected hash, so this run has no sensor. Foreman asks you when Claude is idle, and Capture from session is off.";

/// The sensor bytes built into this binary, if any (`build.rs`, `FOREMAN_SENSOR_BIN`).
#[cfg(foreman_sensor_bundled)]
#[must_use]
pub fn bundled() -> Option<Arc<[u8]>> {
    Some(Arc::from(
        &include_bytes!(concat!(env!("OUT_DIR"), "/foreman-sensor"))[..],
    ))
}

/// The sensor bytes built into this binary, if any (`build.rs`, `FOREMAN_SENSOR_BIN`).
#[cfg(not(foreman_sensor_bundled))]
#[must_use]
pub fn bundled() -> Option<Arc<[u8]>> {
    None
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The sensor's app-side state.
pub struct Sensor {
    /// The bundled bytes and their SHA-256 (`None`: no sensor in this build).
    bundle: Mutex<Option<(Arc<[u8]>, String)>>,
    /// Serializes installs (two prepares can't race the remove-and-write).
    install: Mutex<()>,
    /// The token of each run that got hooks, by run ID: a prepared run's with the time it is
    /// forgotten unless the run starts, a started run's with `None` (kept until it ends).
    tokens: Mutex<HashMap<String, (String, Option<Instant>)>>,
}

impl std::fmt::Debug for Sensor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sensor")
            .field("available", &self.available())
            .finish_non_exhaustive()
    }
}

impl Default for Sensor {
    fn default() -> Self {
        let s = Self {
            bundle: Mutex::new(None),
            install: Mutex::new(()),
            tokens: Mutex::new(HashMap::new()),
        };
        s.set_bundle(bundled());
        s
    }
}

impl Sensor {
    /// Sets the bundled bytes (tests install the workspace's `foreman-sensor`; `None` turns the
    /// sensor off, as a build without it).
    pub fn set_bundle(&self, bytes: Option<Arc<[u8]>>) {
        *lock(&self.bundle) = bytes.map(|b| {
            let h = sha256_hex(&b);
            (b, h)
        });
    }

    /// Whether this build has a sensor at all.
    #[must_use]
    pub fn available(&self) -> bool {
        lock(&self.bundle).is_some()
    }

    /// The expected SHA-256 (hex), when there is a sensor.
    #[must_use]
    pub fn expected_sha256(&self) -> Option<String> {
        lock(&self.bundle).as_ref().map(|(_, h)| h.clone())
    }

    /// The token a run was given (`None`: the run has no sensor, or its plan expired unused).
    #[must_use]
    pub fn token(&self, run_id: &str) -> Option<String> {
        lock(&self.tokens)
            .get(run_id)
            .filter(|(_, until)| until.is_none_or(|u| u > Instant::now()))
            .map(|(t, _)| t.clone())
    }

    /// Remembers a prepared run's token for `ttl` ([`PREPARED_TTL`]); expired ones go.
    fn remember(&self, run_id: String, token: String, ttl: Duration) {
        let now = Instant::now();
        let mut t = lock(&self.tokens);
        t.retain(|_, (_, until)| until.is_none_or(|u| u > now));
        t.insert(run_id, (token, Some(now + ttl)));
    }

    /// Keeps a started run's token until [`Sensor::forget`] (the run's ingest thread ends).
    fn pin(&self, run_id: &str) {
        if let Some((_, until)) = lock(&self.tokens).get_mut(run_id) {
            *until = None;
        }
    }

    /// Forgets a run's token (its `run.json` holds it from now on).
    pub fn forget(&self, run_id: &str) {
        lock(&self.tokens).remove(run_id);
    }

    /// How many tokens are held (tests).
    #[must_use]
    pub fn held(&self) -> usize {
        lock(&self.tokens).len()
    }
}

/// `bin/foreman-sensor-<sha256-12>` for a SHA-256.
#[must_use]
pub fn bin_rel(sha256: &str) -> String {
    format!("bin/{BIN_PREFIX}{}", sha256.get(..12).unwrap_or(sha256))
}

/// What a check of the installed binary found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// No sensor in this build.
    NotBundled,
    /// The installed binary matches (its absolute path).
    Ok(PathBuf),
    /// Missing, unreadable or a different hash, even after one re-install.
    Mismatch,
}

fn installed_matches(root: &Root, rel: &str, sha: &str) -> bool {
    RepoPath::app_data(rel)
        .ok()
        .and_then(|p| root.read(&p, ReadClass::AppState).ok())
        .is_some_and(|(bytes, _)| sha256_hex(&bytes) == sha)
}

fn install_into(root: &Root, bytes: &[u8], sha: &str) -> bool {
    let rel = bin_rel(sha);
    if app_dirs::install_executable(root, &rel, bytes).is_err() {
        return false;
    }
    // Replaced on hash change: older sensors go (never anything else in `bin/`).
    let mine = rel.trim_start_matches("bin/").to_owned();
    for n in app_dirs::list_app_data_dir(root, "bin").unwrap_or_default() {
        if n.starts_with(BIN_PREFIX) && n != mine {
            let _ = app_dirs::remove_file(root, &format!("bin/{n}"));
        }
    }
    installed_matches(root, &rel, sha)
}

/// Startup (ADR-009): installs the sensor when it's missing or differs. A failure only means the
/// first run's check tries again.
pub fn install(app: &App) -> Check {
    check(app)
}

/// SR-15: the installed binary's hash, checked before a run settings file is written; on a
/// mismatch it's re-installed once.
pub fn check(app: &App) -> Check {
    let Some((bytes, sha)) = lock(&app.sensor().bundle).clone() else {
        return Check::NotBundled;
    };
    let Ok(root) = app.app_root() else {
        return Check::Mismatch;
    };
    let _g = lock(&app.sensor().install);
    let rel = bin_rel(&sha);
    if installed_matches(&root, &rel, &sha) || install_into(&root, &bytes, &sha) {
        Check::Ok(root.path().join(rel))
    } else {
        Check::Mismatch
    }
}

/// A fresh per-run token: two OS-random UUID v4 values in hex (64 lowercase hex digits).
#[must_use]
pub fn new_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// The hooks for a new run (AC-F011-01, AC-F011-02, SR-15): the hash is checked now, the token is
/// made and remembered. `(None, None)` without a sensor in this build; `(None, Some(warning))`
/// when the binary didn't match.
#[must_use]
pub fn hooks_for_run(app: &App, run_id: Uuid) -> (Option<SensorHooks>, Option<String>) {
    match check(app) {
        Check::NotBundled => (None, None),
        Check::Mismatch => (None, Some(HASH_WARNING.to_owned())),
        Check::Ok(path) => {
            let run = run_id.hyphenated().to_string();
            let token = new_token();
            app.sensor()
                .remember(run.clone(), token.clone(), PREPARED_TTL);
            (
                Some(SensorHooks {
                    command: path.display().to_string(),
                    run_id: run,
                    token,
                }),
                None,
            )
        }
    }
}

/// Before a run with hooks spawns: its event folder `events/<runId>/` exists, empty, `0700` (the
/// sensor never creates it, AB-12). Returns the token for `run.json`.
#[must_use]
pub fn begin_run(app: &App, root: &Root, run_id: &str) -> Option<String> {
    let token = app.sensor().token(run_id)?;
    app.sensor().pin(run_id);
    app_dirs::reset_dir(root, &format!("events/{run_id}")).ok()?;
    Some(token)
}

/// SR-53, AC-F011-07: deletes the events of every run that ended more than 30 days before `now`,
/// and event folders that belong to no run record (probe markers). Live runs are kept. Returns how
/// many folders went.
#[must_use]
pub fn prune_events(app: &App) -> usize {
    let Ok(root) = app.app_root() else {
        return 0;
    };
    let now = app.os().now();
    let mut n = 0;
    for id in app_dirs::list_app_data_dir(&root, "events").unwrap_or_default() {
        if app.runs().get(&id).is_some_and(|r| r.session.is_live()) {
            continue;
        }
        let expired = match crate::runs::read_run(&root, &id) {
            Ok((f, _)) => {
                f.status != foreman_core::schema::run::RunStatus::Running
                    && ended_at(&f).is_some_and(|end| {
                        now.as_second().saturating_sub(end.as_second()) > EVENT_DAYS * 86_400
                    })
            }
            Err(_) => foreman_core::ids::is_uuid_v4(&id),
        };
        if expired && app_dirs::remove_tree(&root, &format!("events/{id}")).unwrap_or(false) {
            n += 1;
        }
    }
    n
}

/// When a run ended: its start plus the exit offset (the start alone when it has none).
fn ended_at(f: &foreman_core::schema::run::RunFile) -> Option<jiff::Timestamp> {
    let start: jiff::Timestamp = f.t.wall_start.parse().ok()?;
    let ms = i64::try_from(f.t.exited.unwrap_or(0)).unwrap_or(0);
    start
        .checked_add(jiff::SignedDuration::from_millis(ms))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// N2: a prepared run's token is forgotten when its plan isn't used; a started run's stays
    /// until it is forgotten at the end of the run.
    #[test]
    fn prepared_tokens_expire_started_ones_stay() {
        let s = Sensor::default();
        s.remember("a".into(), "ta".into(), Duration::ZERO);
        s.remember("b".into(), "tb".into(), Duration::from_secs(600));
        s.pin("b");
        assert_eq!(s.token("a"), None, "an unused plan's token is gone");
        s.remember("c".into(), "tc".into(), Duration::from_secs(600));
        assert_eq!(
            s.held(),
            2,
            "expired tokens are dropped on the next prepare"
        );
        assert_eq!(s.token("b").as_deref(), Some("tb"));
        s.forget("b");
        assert_eq!(s.token("b"), None);
    }
}
