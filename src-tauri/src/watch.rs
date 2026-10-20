//! `app::watch` (ADR-013, ADR-021 §7, ADR-022; M3-19, M3-21, M3-22): turns file hints into
//! invalidation events. It never acts (SR-18, IPC-R6): it re-reads, re-validates, updates the
//! pending-review state the way opening a project would, and emits §6 events.
//!
//! - **Projects.** One `foreman_sys::watch::ProjectWatcher` per opened project root, started by
//!   `project_open`. Its [`Change`]s map to events: a packet or attachments file →
//!   `packet:changed` (the packet view re-reads and re-validates it, AC-F013-06, -07); a decision
//!   file → re-classified, so a pending review appears without a restart (`decision:pending`,
//!   AC-F022-04); a refused root → the watcher stops and Doctor warns (AC-F013-10). `docs/lessons.md`
//!   and `out` paths have no P1 reader (F-016 and the round tracker are P2), so they emit nothing.
//!   Missed events are tolerated: the interest set is re-stat'ed on window focus ([`on_focus`]).
//! - **Config of live runs.** A `HintWatcher` per live run over its fingerprint sources (the cwd,
//!   repo root and config folder for the named files, the source folders recursively, and
//!   `~/.claude.json` and ancestor `CLAUDE.md` files directly). A hint on a source re-fingerprints
//!   that run; a changed fingerprint marks it "Config changed since launch" and emits
//!   `config:changed` once (a warning only). Sources a hint can't cover (a file created where
//!   nothing was watched, a symlinked folder, imports and programs) are re-checked every
//!   [`FALLBACK`], or every [`TICK`] for a run whose watcher couldn't start.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

use foreman_core::fingerprint::{MANAGED_DIR, SourceSpec, sources};
use foreman_core::interest::Interest;
use foreman_core::store::{ReadStamp, Root, RootKind};
use foreman_sys::fingerprint::{Where, gather};
use foreman_sys::watch::{Change, HintWatcher, ProjectWatcher, restat};

use crate::ipc::dto::events::{ConfigChanged, DoctorStatus, Event, PacketChanged};
use crate::ipc::types::{CwdKind, DoctorLevel, ProjectId, Rev, Stage};
use crate::project::Opened;
use crate::state::App;

/// How often the config thread wakes (and re-checks runs without a working hint watcher).
pub const TICK: Duration = Duration::from_secs(5);
/// How often a watched run is re-checked anyway, for sources a hint can't cover.
pub const FALLBACK: Duration = Duration::from_secs(60);
/// How long a project watcher thread waits for a change before checking for commands.
const POLL: Duration = Duration::from_millis(100);

enum Cmd {
    SetOuts(Vec<String>),
}

struct ProjectHandle {
    root: PathBuf,
    cmd: Sender<Cmd>,
}

/// The watchers of one app.
#[derive(Default)]
pub struct Watchers {
    projects: Mutex<HashMap<String, ProjectHandle>>,
    /// Project ID → why its watcher isn't running (Doctor's "file watching unavailable").
    failures: Mutex<BTreeMap<String, String>>,
    /// The stamps the last re-stat saw, per project.
    known: Mutex<HashMap<String, HashMap<String, ReadStamp>>>,
}

impl std::fmt::Debug for Watchers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watchers").finish_non_exhaustive()
    }
}

fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Watchers {
    /// Projects whose watcher couldn't start or stopped, with the reason.
    #[must_use]
    pub fn failures(&self) -> BTreeMap<String, String> {
        guard(&self.failures).clone()
    }

    /// Whether a project's watcher is running.
    #[must_use]
    pub fn is_watching(&self, project_id: &str) -> bool {
        guard(&self.projects).contains_key(project_id)
    }
}

/// The next `packet:changed` revision (events carry a revision, never content; ipc.md §6).
#[must_use]
pub fn next_packet_rev(app: &App) -> Rev {
    let mut i = app.inner();
    i.packet_rev = i.packet_rev.wrapping_add(1);
    Rev(i.packet_rev)
}

/// Emits `packet:changed` for a project's stage.
pub fn emit_packet_changed(app: &App, project_id: &str, stage: u8) {
    let rev = next_packet_rev(app);
    app.emit(Event::PacketChanged(PacketChanged {
        project_id: ProjectId::from_trusted(project_id),
        stage: Stage(stage),
        rev,
    }));
}

fn record_failure(app: &App, project_id: &str, why: String) {
    guard(&app.watchers().failures).insert(project_id.to_owned(), why);
    let level = {
        let mut i = app.inner();
        let l = i
            .doctor
            .unwrap_or(DoctorLevel::Ok)
            .max(DoctorLevel::Warning);
        i.doctor = Some(l);
        l
    };
    app.emit(Event::DoctorStatus(DoctorStatus { level }));
}

/// Maps one watcher change to state and events. Returns whether anything was emitted.
pub fn handle(app: &App, project_id: &str, change: &Change) -> bool {
    let interest = match change {
        Change::Changed { interest, .. } | Change::Refused { interest, .. } => interest,
        Change::RootRefused { root } => {
            record_failure(
                app,
                project_id,
                format!(
                    "file watching stopped: {} is a symlink or under one. Packets appear when you focus Foreman.",
                    crate::render::escape_visible(&app.display(root))
                ),
            );
            return false;
        }
    };
    match interest {
        Interest::Packet { stage } | Interest::Attachments { stage } => {
            emit_packet_changed(app, project_id, *stage);
            true
        }
        Interest::Decision { .. } => crate::project::refresh_decisions(app, project_id).is_ok(),
        // No P1 reader: lessons (F-016) and `out` paths (round tracker) are P2.
        Interest::Lessons | Interest::Out { .. } | Interest::SensorEvent { .. } => false,
    }
}

/// Starts (or updates) the watcher of an opened project (`project_open`). A failure to start is
/// recorded for Doctor; steps still run and packets are validated when the stage is opened
/// (AC-F013-10).
pub fn ensure_project(app: &App, p: &Opened) {
    let Some(root) = p.root.as_ref().map(|r| r.path().to_path_buf()) else {
        return;
    };
    let id = p.id().to_owned();
    let outs = p.watch_outs();
    let mut projects = guard(&app.watchers().projects);
    if let Some(h) = projects.get(&id) {
        if h.root == root && h.cmd.send(Cmd::SetOuts(outs.clone())).is_ok() {
            return;
        }
        projects.remove(&id);
    }
    let watcher = match ProjectWatcher::start(&root, outs) {
        Ok(w) => w,
        Err(e) => {
            drop(projects);
            record_failure(
                app,
                &id,
                format!(
                    "file watching unavailable: packets appear when you focus Foreman ({})",
                    crate::render::escape_visible(&e.to_string())
                ),
            );
            return;
        }
    };
    let (tx, rx) = channel();
    let weak = app.weak();
    let thread_id = id.clone();
    let spawned = std::thread::Builder::new()
        .name(format!(
            "fm-watch-{}",
            id.chars().take(8).collect::<String>()
        ))
        .spawn(move || run_project(&weak, &thread_id, &watcher, &rx));
    if spawned.is_ok() {
        projects.insert(id.clone(), ProjectHandle { root, cmd: tx });
        drop(projects);
        guard(&app.watchers().failures).remove(&id);
    }
}

fn run_project(app: &Weak<App>, id: &str, w: &ProjectWatcher, rx: &Receiver<Cmd>) {
    loop {
        for cmd in rx.try_iter() {
            match cmd {
                Cmd::SetOuts(outs) => w.set_outs(outs),
            }
        }
        let change = w.next(POLL);
        let Some(a) = app.upgrade() else { return };
        if !guard(&a.watchers().projects).contains_key(id) {
            return;
        }
        if let Some(c) = change {
            let stop = matches!(c, Change::RootRefused { .. });
            handle(&a, id, &c);
            if stop {
                guard(&a.watchers().projects).remove(id);
                return;
            }
        }
    }
}

/// Stops a project's watcher (`project_remove`).
pub fn stop_project(app: &App, project_id: &str) {
    guard(&app.watchers().projects).remove(project_id);
    guard(&app.watchers().known).remove(project_id);
}

/// The re-stat of a project's interest set (ADR-013): the first call records the stamps, later
/// ones report what changed since, so a coalesced or missed event can't hide a change past the
/// next interaction. Returns the number of changes handled.
pub fn restat_project(app: &App, p: &Opened) -> usize {
    let Some(root_path) = p.root.as_ref().map(|r| r.path().to_path_buf()) else {
        return 0;
    };
    let Ok(root) = Root::open(&root_path, RootKind::Project) else {
        return 0;
    };
    let stages: Vec<u8> = (0..=17).collect();
    let outs = p.watch_outs();
    let first;
    let changes = {
        let mut known_all = guard(&app.watchers().known);
        first = !known_all.contains_key(p.id());
        let known = known_all.entry(p.id().to_owned()).or_default();
        restat(&root, &stages, &outs, known)
    };
    if first {
        return 0;
    }
    changes.iter().filter(|c| handle(app, p.id(), c)).count()
}

/// Window focus (ADR-013): re-stat every watched project and re-check live runs' config.
pub fn on_focus(app: &App) {
    let ids: Vec<String> = guard(&app.watchers().projects).keys().cloned().collect();
    for id in ids {
        if let Ok(p) = crate::project::open(app, &id) {
            restat_project(app, &p);
        }
    }
    let _ = recheck_live_runs(app);
}

/// One live run's fingerprint inputs.
struct RunWhere {
    run_id: String,
    cwd: PathBuf,
    root: PathBuf,
}

fn live_runs(app: &App) -> Vec<RunWhere> {
    let Ok((reg, _)) = app.registry() else {
        return Vec::new();
    };
    app.runs()
        .live()
        .into_iter()
        .filter_map(|r| {
            let e = reg.projects.iter().find(|p| p.project_id == r.project_id)?;
            Some(RunWhere {
                run_id: r.run_id.clone(),
                cwd: r.cwd.clone(),
                root: PathBuf::from(&e.path),
            })
        })
        .collect()
}

/// Re-fingerprints the live runs `pick` selects; marks each run whose fingerprint differs from
/// its plan's and emits `config:changed` once per run. Returns the newly marked run IDs.
#[must_use]
pub fn recheck_runs(app: &App, pick: &dyn Fn(&str) -> bool) -> Vec<String> {
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
        if r.config_changed.load(Ordering::SeqCst) || !pick(&r.run_id) {
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

/// Re-fingerprints every live run (window focus, the fallback).
#[must_use]
pub fn recheck_live_runs(app: &App) -> Vec<String> {
    recheck_runs(app, &|_| true)
}

/// What a run's hint watcher watches, and which hinted paths are sources.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConfigTargets {
    /// `(path, recursive)` to watch.
    pub watch: Vec<(PathBuf, bool)>,
    /// Source files: a hint on exactly one of these is relevant.
    pub files: HashSet<PathBuf>,
    /// Source folders: a hint below one of these is relevant (minus `exclude`).
    pub dirs: Vec<(PathBuf, Vec<String>)>,
}

impl ConfigTargets {
    /// Whether a hinted path is one of the run's sources.
    #[must_use]
    pub fn relevant(&self, p: &Path) -> bool {
        self.files.contains(p)
            || self.dirs.iter().any(|(d, exclude)| {
                p.strip_prefix(d).is_ok_and(|rest| {
                    rest.components()
                        .next()
                        .is_none_or(|c| !exclude.iter().any(|x| c.as_os_str() == x.as_str()))
                })
            })
    }
}

/// The watch targets for a run in `cwd` of the project at `root` (ADR-021 §2's static sources).
/// A file source is watched through its folder when that folder is the cwd, the repo root, the
/// config folder or the managed folder (so creating it is seen too), otherwise directly (only if
/// it exists: an ancestor `CLAUDE.md` above the repo, `~/.claude.json`). Folder sources are
/// watched recursively.
#[must_use]
pub fn config_targets(
    cwd: &Path,
    root: &Path,
    home: &Path,
    cfg: Option<&Path>,
    managed: &Path,
) -> ConfigTargets {
    let cfg_dir = cfg.map_or_else(|| home.join(".claude"), Path::to_path_buf);
    let parents = [cwd, root, cfg_dir.as_path(), managed];
    let mut t = ConfigTargets::default();
    let add = |p: PathBuf, rec: bool, t: &mut ConfigTargets| {
        if !t.watch.iter().any(|(x, r)| *x == p && *r == rec) {
            t.watch.push((p, rec));
        }
    };
    for s in sources(cwd, root, home, cfg, managed) {
        match s {
            SourceSpec::File { path, .. } | SourceSpec::ClaudeJson { path, .. } => {
                match path.parent() {
                    Some(dir) if parents.contains(&dir) => add(dir.to_path_buf(), false, &mut t),
                    _ => add(path.clone(), false, &mut t),
                }
                t.files.insert(path);
            }
            SourceSpec::Dir { path, exclude, .. } => {
                add(path.clone(), true, &mut t);
                t.dirs.push((path, exclude));
            }
        }
    }
    t
}

struct RunWatch {
    hints: Option<HintWatcher>,
    targets: ConfigTargets,
    armed: usize,
    last_check: Instant,
}

/// How many targets exist now and aren't symlinks (what a `HintWatcher` would arm).
fn armable(t: &ConfigTargets) -> usize {
    t.watch
        .iter()
        .filter(|(p, _)| {
            rustix::fs::lstat(p).is_ok_and(|st| {
                rustix::fs::FileType::from_raw_mode(st.st_mode) != rustix::fs::FileType::Symlink
            })
        })
        .count()
}

fn arm(targets: ConfigTargets) -> RunWatch {
    let hints = HintWatcher::start(&targets.watch)
        .ok()
        .filter(|h| h.watched() > 0);
    RunWatch {
        armed: hints.as_ref().map_or(0, HintWatcher::watched),
        hints,
        targets,
        last_check: Instant::now(),
    }
}

/// Starts the config thread; it ends when the app is gone.
pub fn start(app: Weak<App>) {
    let _ = std::thread::Builder::new()
        .name("fm-config-watch".into())
        .spawn(move || config_loop(&app));
}

fn config_loop(app: &Weak<App>) {
    let mut runs: HashMap<String, RunWatch> = HashMap::new();
    let mut last_tick = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(250));
        let Some(a) = app.upgrade() else { break };
        let live = live_runs(&a);
        runs.retain(|id, _| live.iter().any(|r| r.run_id == *id));
        let cfg = a.config_dir_env();
        for r in &live {
            runs.entry(r.run_id.clone()).or_insert_with(|| {
                arm(config_targets(
                    &r.cwd,
                    &r.root,
                    &a.paths().home,
                    cfg.as_deref(),
                    Path::new(MANAGED_DIR),
                ))
            });
        }
        let tick = last_tick.elapsed() >= TICK;
        if tick {
            last_tick = Instant::now();
        }
        let mut due: HashSet<String> = HashSet::new();
        for (id, w) in &mut runs {
            let hinted = w
                .hints
                .as_ref()
                .is_some_and(|h| h.drain().iter().any(|p| w.targets.relevant(p)));
            let fallback = tick && (w.hints.is_none() || w.last_check.elapsed() >= FALLBACK);
            if hinted || fallback {
                // A source folder that appeared (or went) since arming: arm again so what is
                // written inside it is seen too.
                if armable(&w.targets) != w.armed {
                    let last = w.last_check;
                    *w = arm(w.targets.clone());
                    w.last_check = last;
                }
                w.last_check = Instant::now();
                due.insert(id.clone());
            }
        }
        if !due.is_empty() {
            let _ = recheck_runs(&a, &|id| due.contains(id));
        }
    }
}
