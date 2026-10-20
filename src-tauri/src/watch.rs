//! Config changes under live runs (ADR-021 §7, SR-06, M3-22): a fingerprinted file that changes
//! while a run in that cwd is live marks the run "Config changed since launch" and emits
//! `config:changed` (a warning only; the next launch in that cwd is the blocking check).
//!
//! The project watcher (`foreman_sys::watch`) has no config interest (the sources sit outside the
//! project too: `~/.claude`, managed settings, ancestors), so the live runs are re-fingerprinted on
//! a slow tick while any run is live.

use std::path::{Path, PathBuf};
use std::sync::Weak;
use std::sync::atomic::Ordering;
use std::time::Duration;

use foreman_core::fingerprint::MANAGED_DIR;
use foreman_sys::fingerprint::{Where, gather};

use crate::ipc::dto::events::{ConfigChanged, Event};
use crate::ipc::types::{CwdKind, ProjectId};
use crate::state::App;

/// How often live runs are re-fingerprinted.
pub const TICK: Duration = Duration::from_secs(5);

/// Re-fingerprints every live run's cwd; marks each run whose fingerprint differs from its plan's
/// and emits `config:changed` once per run. Returns the newly marked run IDs.
#[must_use]
pub fn recheck_live_runs(app: &App) -> Vec<String> {
    let live = app.runs().live();
    if live.is_empty() {
        return Vec::new();
    }
    let Ok((reg, _)) = app.registry() else {
        return Vec::new();
    };
    let projects: Vec<PathBuf> = reg
        .projects
        .iter()
        .map(|p| PathBuf::from(&p.path))
        .collect();
    let config_dir = app.config_dir_env();
    let mut marked = Vec::new();
    for r in live {
        if r.config_changed.load(Ordering::SeqCst) {
            continue;
        }
        let Some(entry) = reg.projects.iter().find(|p| p.project_id == r.project_id) else {
            continue;
        };
        let root = Path::new(&entry.path);
        let w = Where {
            cwd: &r.cwd,
            repo_root: root,
            home: &app.paths().home,
            config_dir: config_dir.as_deref(),
            managed: Path::new(MANAGED_DIR),
            projects: &projects,
        };
        // A source that can't be read now (too large, an unsafe symlink) is a change too.
        let changed = gather(&w, &[]).map_or(true, |g| g.fingerprint != r.plan.fingerprint);
        if changed && !r.config_changed.swap(true, Ordering::SeqCst) {
            let cwd = if r.cwd == root {
                CwdKind::Project
            } else {
                CwdKind::Spec
            };
            app.os().emit(&Event::ConfigChanged(ConfigChanged {
                project_id: ProjectId::from_trusted(&r.project_id),
                cwd,
            }));
            marked.push(r.run_id.clone());
        }
    }
    marked
}

/// Starts the tick on its own thread; it ends when the app is gone.
pub fn start(app: Weak<App>) {
    let _ = std::thread::Builder::new()
        .name("fm-config-watch".into())
        .spawn(move || {
            loop {
                std::thread::sleep(TICK);
                let Some(a) = app.upgrade() else { break };
                let _ = recheck_live_runs(&a);
            }
        });
}
