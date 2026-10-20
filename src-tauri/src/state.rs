//! `app::state` (design doc §3.4): the app-data root, settings, the project registry, one-time
//! previews and plans (IPC-R7), and the revisions events carry. Commands get it as `&App`.
//!
//! Every file goes through `foreman_core::store` (FILE-R4); nothing here touches `std::fs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use foreman_core::pathpolicy::{self, ReadClass};
use foreman_core::schema::projects::ProjectsFile;
use foreman_core::schema::settings::{SettingsFile, Terminal, Theme, ToolPath, ToolSource, Tools};
use foreman_core::schema::{self, Loaded, Versioned};
use foreman_core::store::{ReadStamp, Root};
use foreman_sys::spawn::{Program, SpawnCtx};

use crate::ipc::IpcError;
use crate::ipc::dto::events::{Event, ProjectChanged, ProjectsChanged, SettingsChanged};
use crate::ipc::types::{DoctorLevel, ProjectId, Rev};
use crate::os::{Os, rfc3339};

/// Previews and picks are valid 10 minutes (IPC-R7).
pub const PREVIEW_TTL: Duration = Duration::from_secs(600);
/// Launch plans are valid 60 s (IPC-R7, ADR-021).
pub const PLAN_TTL: Duration = Duration::from_secs(60);

/// Where Foreman's files are.
#[derive(Debug, Clone)]
pub struct Paths {
    /// `~/Library/Application Support/<bundle id>/`.
    pub app_data: PathBuf,
    /// The user's home folder (tool candidates, the blueprint candidates, SR-31a).
    pub home: PathBuf,
}

/// Something held under a one-time ID.
#[derive(Debug)]
pub struct Held<T> {
    /// The payload.
    pub value: T,
    /// When it stops being valid.
    pub until: Instant,
}

/// One-time payloads by ID (IPC-R7). A taken or expired ID is `Expired`; an unknown one too
/// (the webview can't tell the difference, which is the point).
#[derive(Debug)]
pub struct OneTime<T> {
    items: HashMap<String, Held<T>>,
}

impl<T> Default for OneTime<T> {
    fn default() -> Self {
        Self {
            items: HashMap::new(),
        }
    }
}

impl<T> OneTime<T> {
    /// Holds a value under a fresh UUID v4.
    pub fn hold(&mut self, value: T, ttl: Duration) -> String {
        let now = Instant::now();
        self.items.retain(|_, h| h.until > now);
        let id = uuid::Uuid::new_v4().to_string();
        self.items.insert(
            id.clone(),
            Held {
                value,
                until: now + ttl,
            },
        );
        id
    }

    /// Takes a value once.
    ///
    /// # Errors
    /// `Expired` when unknown, already taken or past its time.
    pub fn take(&mut self, id: &str) -> Result<T, IpcError> {
        match self.items.remove(id) {
            Some(h) if h.until > Instant::now() => Ok(h.value),
            _ => Err(IpcError::expired()),
        }
    }

    /// Reads a value without taking it.
    ///
    /// # Errors
    /// `Expired` when unknown or past its time.
    pub fn peek(&self, id: &str) -> Result<&T, IpcError> {
        match self.items.get(id) {
            Some(h) if h.until > Instant::now() => Ok(&h.value),
            _ => Err(IpcError::expired()),
        }
    }
}

/// In-memory state behind the app's mutex.
#[derive(Debug, Default)]
pub struct Inner {
    /// `SettingsView.rev`: bumped on every settings write Foreman makes or sees.
    pub settings_rev: u32,
    /// `projects:changed` revision.
    pub projects_rev: u32,
    /// The last Doctor level (`unknown` until Doctor runs).
    pub doctor: Option<DoctorLevel>,
    /// The `claude -v` Doctor last saw.
    pub claude_version: Option<String>,
    /// Settings → "Screen reader mode" (AC-XC-22). Kept in memory until `settings.json` v2 adds
    /// `screenReaderMode` (files.md §4.1 note; docs/progress.md).
    pub screen_reader_mode: bool,
    /// The blueprint repo as `blueprint_pick_repo` last read it (Settings shows it).
    pub blueprint_info: Option<(String, crate::ipc::dto::app::BlueprintRepoInfo)>,
    /// The last full summary of each project opened this run (for the light summaries).
    pub summaries: HashMap<String, crate::ipc::dto::app::ProjectSummary>,
    /// Folders picked in the Rust dialog (`setup_pick_folder`).
    pub picks: OneTime<PathBuf>,
    /// Two-phase previews (`*_preview` → `*_apply`).
    pub previews: OneTime<crate::preview::Preview>,
    /// The last `packet:changed` revision (ipc.md §6).
    pub packet_rev: u32,
    /// The inbox items already notified (kind and identifiers), so each notifies once while it
    /// stays in the inbox (AC-F017-09).
    pub notified: std::collections::HashSet<String>,
    /// `run()` switches this on: events then schedule an inbox sync (badge, notifications).
    /// Tests leave it off and call `inbox::sync` themselves.
    pub inbox_sync: bool,
    /// An inbox sync is already scheduled.
    pub inbox_scheduled: bool,
}

/// The app state.
pub struct App {
    os: Arc<dyn Os>,
    paths: Paths,
    me: std::sync::Weak<App>,
    runs: crate::runs::Runs,
    watchers: crate::watch::Watchers,
    launch: Mutex<crate::launch::LaunchState>,
    sensor: crate::sensor::Sensor,
    sessions: crate::sensor_ingest::Sessions,
    inner: Mutex<Inner>,
    /// Serializes mutating commands (one writer at a time; CAS still guards every file).
    write_lock: Mutex<()>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("paths", &self.paths)
            .finish_non_exhaustive()
    }
}

fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The default `settings.json` (files.md §4.1: missing file → defaults and auto-detection).
#[must_use]
pub fn default_settings() -> SettingsFile {
    let auto = || ToolPath {
        path: None,
        source: ToolSource::Auto,
    };
    SettingsFile {
        schema_version: SettingsFile::CURRENT,
        blueprint_repo: None,
        dri_name: None,
        tools: Tools {
            claude: auto(),
            git: auto(),
        },
        editor_bundle_id: None,
        theme: Theme::System,
        terminal: Terminal {
            font_size: 13,
            scrollback: 10_000,
            mac_option_is_meta: true,
        },
        env_passthrough: Vec::new(),
        login_path: None,
        global_variables: std::collections::BTreeMap::new(),
        auth_key_allowlist: Vec::new(),
        contract: None,
        last_seen_claude_version: None,
        probe_folder_trusted: false,
    }
}

/// `~/…` for display (paths inside home are shown relative to it).
#[must_use]
pub fn display_path(home: &Path, p: &Path) -> String {
    match p.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

impl App {
    /// The state for an app-data folder and home.
    #[must_use]
    pub fn new(os: Arc<dyn Os>, paths: Paths) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            os,
            paths,
            me: me.clone(),
            runs: crate::runs::Runs::default(),
            watchers: crate::watch::Watchers::default(),
            launch: Mutex::new(crate::launch::LaunchState::default()),
            sensor: crate::sensor::Sensor::default(),
            sessions: crate::sensor_ingest::Sessions::default(),
            inner: Mutex::new(Inner {
                settings_rev: 1,
                projects_rev: 1,
                ..Inner::default()
            }),
            write_lock: Mutex::new(()),
        })
    }

    /// A weak handle to this app (exit handlers on reader threads).
    #[must_use]
    pub fn weak(&self) -> std::sync::Weak<App> {
        self.me.clone()
    }

    /// The run table.
    #[must_use]
    pub fn runs(&self) -> &crate::runs::Runs {
        &self.runs
    }

    /// The file watchers (ADR-013).
    #[must_use]
    pub fn watchers(&self) -> &crate::watch::Watchers {
        &self.watchers
    }

    /// Plans, pending acknowledgements and re-arms (IPC-R7).
    pub fn launch(&self) -> MutexGuard<'_, crate::launch::LaunchState> {
        guard(&self.launch)
    }

    /// The P2 sensor's install and per-run tokens (ADR-009).
    #[must_use]
    pub fn sensor(&self) -> &crate::sensor::Sensor {
        &self.sensor
    }

    /// The sensor's derived session states (display only, SR-18).
    #[must_use]
    pub fn sessions(&self) -> &crate::sensor_ingest::Sessions {
        &self.sessions
    }

    /// The OS seam.
    #[must_use]
    pub fn os(&self) -> &dyn Os {
        self.os.as_ref()
    }

    /// Paths.
    #[must_use]
    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    /// Foreman's own environment as a launch sees it (SR-05's input): `HOME` is the home folder
    /// Foreman uses, so the preflight and the child agree on `~/.claude`; a `CLAUDE_CONFIG_DIR`
    /// set for a different home doesn't carry over.
    #[must_use]
    pub fn own_env(&self) -> Vec<(String, String)> {
        let home = self.paths.home.display().to_string();
        let same_home = std::env::var("HOME").is_ok_and(|h| h == home);
        std::env::vars()
            .filter(|(k, _)| k != "HOME" && (same_home || k != "CLAUDE_CONFIG_DIR"))
            .chain(std::iter::once(("HOME".to_owned(), home)))
            .collect()
    }

    /// `CLAUDE_CONFIG_DIR` from [`App::own_env`].
    #[must_use]
    pub fn config_dir_env(&self) -> Option<PathBuf> {
        self.own_env()
            .into_iter()
            .find(|(k, _)| k == "CLAUDE_CONFIG_DIR")
            .map(|(_, v)| PathBuf::from(v))
    }

    /// The in-memory state.
    pub fn inner(&self) -> MutexGuard<'_, Inner> {
        guard(&self.inner)
    }

    /// Held for the whole of a mutating command.
    pub fn writing(&self) -> MutexGuard<'_, ()> {
        guard(&self.write_lock)
    }

    /// Now as RFC 3339 UTC.
    #[must_use]
    pub fn now(&self) -> String {
        rfc3339(self.os.now())
    }

    /// Emits a §6 event.
    pub fn emit(&self, e: Event) {
        let wakes_inbox = matches!(
            e,
            Event::PacketChanged(_)
                | Event::DecisionPending(_)
                | Event::TamperDetected(_)
                | Event::RunStatus(_)
                | Event::ProjectChanged(_)
        );
        self.os.emit(&e);
        if wakes_inbox {
            crate::inbox::schedule(self);
        }
    }

    /// Emits `project:changed` with a project's new revision.
    pub fn project_changed(&self, id: &str, rev: u32) {
        self.emit(Event::ProjectChanged(ProjectChanged {
            project_id: ProjectId::from_trusted(id),
            rev: Rev(rev),
        }));
    }

    /// Bumps and emits `projects:changed`.
    pub fn projects_changed(&self) {
        let rev = {
            let mut i = self.inner();
            i.projects_rev += 1;
            i.projects_rev
        };
        self.emit(Event::ProjectsChanged(ProjectsChanged { rev: Rev(rev) }));
    }

    /// Bumps and emits `settings:changed`; returns the new revision.
    pub fn settings_changed(&self) -> u32 {
        let rev = {
            let mut i = self.inner();
            i.settings_rev += 1;
            i.settings_rev
        };
        self.emit(Event::SettingsChanged(SettingsChanged { rev: Rev(rev) }));
        rev
    }

    /// The app-data root (created `0700`, SR-53).
    ///
    /// # Errors
    /// `Io` when it can't be created or opened.
    pub fn app_root(&self) -> Result<Root, IpcError> {
        Root::open_app_data(&self.paths.app_data)
            .map(|r| r.with_recovery(&self.paths.app_data, "app"))
            .map_err(|e| IpcError::io(format!("Foreman's app data folder: {e}")))
    }

    fn read_versioned<T: Versioned>(
        &self,
        rel: &str,
    ) -> Result<(Option<Loaded<T>>, ReadStamp), IpcError> {
        let root = self.app_root()?;
        let rel = pathpolicy::check_app_data(rel)?;
        let (bytes, stamp) = root.read_opt(&rel, ReadClass::AppState)?;
        let Some(bytes) = bytes else {
            return Ok((None, stamp));
        };
        let loaded = schema::load::<T>(&bytes)
            .map_err(|e| IpcError::io(format!("{rel} couldn't be read: {e}")))?;
        Ok((Some(loaded), stamp))
    }

    fn write_versioned<T: Versioned>(
        &self,
        rel: &str,
        v: &T,
        stamp: &ReadStamp,
    ) -> Result<ReadStamp, IpcError> {
        let root = self.app_root()?;
        let rel = pathpolicy::check_app_data(rel)?;
        let text = schema::to_file(v).map_err(|e| IpcError::io(e.to_string()))?;
        Ok(root.write(&rel, text.as_bytes(), stamp)?)
    }

    /// `settings.json`, or the defaults when it doesn't exist yet. A newer file is read-only.
    ///
    /// # Errors
    /// `Io` when the file is unreadable.
    pub fn settings(&self) -> Result<(SettingsFile, ReadStamp, bool), IpcError> {
        match self.read_versioned::<SettingsFile>("settings.json")? {
            (None, stamp) => Ok((default_settings(), stamp, false)),
            (Some(l), stamp) => match l.value() {
                Some(s) => Ok((s, stamp, true)),
                None => Err(IpcError::io(
                    "settings.json was written by a newer Foreman. Update Foreman to change settings.",
                )),
            },
        }
    }

    /// Writes `settings.json` by CAS and emits `settings:changed`; returns the new revision.
    ///
    /// # Errors
    /// `Conflict` when the file changed since it was read; `Io` otherwise.
    pub fn save_settings(&self, s: &SettingsFile, stamp: &ReadStamp) -> Result<u32, IpcError> {
        self.write_versioned("settings.json", s, stamp)?;
        Ok(self.settings_changed())
    }

    /// `projects.json` (empty when it doesn't exist).
    ///
    /// # Errors
    /// `Io` when unreadable or newer.
    pub fn registry(&self) -> Result<(ProjectsFile, ReadStamp), IpcError> {
        match self.read_versioned::<ProjectsFile>("projects.json")? {
            (None, stamp) => Ok((
                ProjectsFile {
                    schema_version: ProjectsFile::CURRENT,
                    projects: Vec::new(),
                },
                stamp,
            )),
            (Some(l), stamp) => l
                .value()
                .map(|p| (p, stamp))
                .ok_or_else(|| IpcError::io("projects.json was written by a newer Foreman.")),
        }
    }

    /// Writes `projects.json` (sorted by `projectId`, FILE-R1) and emits `projects:changed`.
    ///
    /// # Errors
    /// `Conflict` or `Io`.
    pub fn save_registry(&self, p: &mut ProjectsFile, stamp: &ReadStamp) -> Result<(), IpcError> {
        p.projects.sort_by(|a, b| a.project_id.cmp(&b.project_id));
        self.write_versioned("projects.json", p, stamp)?;
        self.projects_changed();
        Ok(())
    }

    /// The spawn context: home, app data, every registered project (tool paths inside a project
    /// are refused, SR-01) and the tool paths set in Settings.
    #[must_use]
    pub fn spawn_ctx(&self, s: &SettingsFile, reg: &ProjectsFile) -> SpawnCtx {
        let mut settings = Vec::new();
        if let (Some(p), ToolSource::Settings) = (&s.tools.claude.path, s.tools.claude.source) {
            settings.push((Program::Claude, PathBuf::from(p)));
        }
        if let (Some(p), ToolSource::Settings) = (&s.tools.git.path, s.tools.git.source) {
            settings.push(foreman_sys::git::settings_entry(PathBuf::from(p)));
        }
        SpawnCtx {
            home: self.paths.home.clone(),
            app_data: self.paths.app_data.clone(),
            projects: reg
                .projects
                .iter()
                .map(|p| PathBuf::from(&p.path))
                .collect(),
            settings,
        }
    }

    /// The spawn context from the files on disk.
    ///
    /// # Errors
    /// `Io` when settings or the registry are unreadable.
    pub fn spawn_ctx_now(&self) -> Result<SpawnCtx, IpcError> {
        let (s, _, _) = self.settings()?;
        let (reg, _) = self.registry()?;
        Ok(self.spawn_ctx(&s, &reg))
    }

    /// `~/…` display form.
    #[must_use]
    pub fn display(&self, p: &Path) -> String {
        display_path(&self.paths.home, p)
    }
}
