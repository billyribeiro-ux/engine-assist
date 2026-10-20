//! `app::launch` run table and run log (ADR-018; files.md §4.6, §4.7; SR-19, SR-52, SR-57):
//! `run.json` written before spawn, updated at exit, one index line per finished run; the live
//! sessions with their output Channels; Interrupted runs found at startup (never relaunched).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Instant;

use foreman_core::pathpolicy::{self, ReadClass};
use foreman_core::schema::run::{
    EntryPoint as RunEntry, IndexEntryPoint, IndexStatus, IndexSurface, RunFile, RunIndexLine,
    RunKind, RunStatus as FileStatus, RunSurface, Timings,
};
use foreman_core::schema::{self, Loaded};
use foreman_core::store::{ReadStamp, Root};
use foreman_sys::launch::LaunchPlan;
use foreman_sys::pty::{ExitInfo, PtySession, PtyStatus, Sink};
use tauri::ipc::{Channel, Response};

use crate::ipc::IpcError;
use crate::ipc::dto::app::RunSummary;
use crate::ipc::dto::events::{Event, RunStatusChanged};
use crate::ipc::types::{ChainId, ProjectId, Rev, RunId, RunStatus, StepKey, Timestamp};
use crate::state::App;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `runs/<runId>/run.json`.
#[must_use]
pub fn run_path(run_id: &str) -> String {
    format!("runs/{run_id}/run.json")
}

/// The output Channels attached to one run (one per open terminal tab; §5 PTY output).
#[derive(Default)]
pub struct Outputs {
    channels: Mutex<Vec<Channel<Response>>>,
}

impl std::fmt::Debug for Outputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Outputs")
    }
}

impl Outputs {
    /// Attaches a Channel.
    pub fn attach(&self, ch: Channel<Response>) {
        lock(&self.channels).push(ch);
    }

    /// Sends raw bytes (never JSON, §5) to every attached Channel; a closed one is dropped.
    pub fn send(&self, bytes: &[u8]) {
        lock(&self.channels).retain(|c| c.send(Response::new(bytes.to_vec())).is_ok());
    }
}

/// The sink a session streams into: the attached Channels, then the exit handler.
pub struct RunSink {
    /// The run.
    pub run_id: String,
    /// The Channels.
    pub outputs: Arc<Outputs>,
    /// The app (weak: the run table owns the session that owns this sink).
    pub app: Weak<App>,
}

impl std::fmt::Debug for RunSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunSink")
            .field("run_id", &self.run_id)
            .finish()
    }
}

impl Sink for RunSink {
    fn chunk(&self, bytes: &[u8]) {
        self.outputs.send(bytes);
    }

    fn exited(&self, info: ExitInfo) {
        if let Some(app) = self.app.upgrade() {
            on_exit(&app, &self.run_id, info);
        }
    }
}

/// A live (or just ended) run.
#[derive(Debug)]
pub struct LiveRun {
    /// Run ID.
    pub run_id: String,
    /// Project.
    pub project_id: String,
    /// Chain.
    pub chain: String,
    /// Step key.
    pub step: String,
    /// `fm-…` tab name.
    pub tab_name: String,
    /// The PTY session.
    pub session: PtySession,
    /// The Channels.
    pub outputs: Arc<Outputs>,
    /// The plan it was spawned from (re-arm compares against it, DR-116).
    pub plan: LaunchPlan,
    /// The step cwd (config:changed marking, SR-06).
    pub cwd: PathBuf,
    /// "Config changed since launch" (SR-06, ADR-021 §7).
    pub config_changed: AtomicBool,
    /// When it was prepared (timings).
    pub started: Instant,
}

/// The run table.
#[derive(Debug, Default)]
pub struct Runs {
    live: Mutex<HashMap<String, Arc<LiveRun>>>,
    rev: Mutex<u32>,
    scanned: AtomicBool,
}

impl Runs {
    /// A run by ID.
    #[must_use]
    pub fn get(&self, run_id: &str) -> Option<Arc<LiveRun>> {
        lock(&self.live).get(run_id).cloned()
    }

    /// The live run of a chain.
    #[must_use]
    pub fn of_chain(&self, chain: &str) -> Option<Arc<LiveRun>> {
        lock(&self.live)
            .values()
            .find(|r| r.chain == chain && r.session.is_live())
            .cloned()
    }

    /// Every run still live.
    #[must_use]
    pub fn live(&self) -> Vec<Arc<LiveRun>> {
        lock(&self.live)
            .values()
            .filter(|r| r.session.is_live())
            .cloned()
            .collect()
    }

    /// Registers a run.
    pub fn insert(&self, r: Arc<LiveRun>) {
        lock(&self.live).insert(r.run_id.clone(), r);
    }

    /// The next `run:status` revision.
    pub fn next_rev(&self) -> u32 {
        let mut r = lock(&self.rev);
        *r += 1;
        *r
    }

    /// Hangs up every live session (quit, after the native quit guard; SR-57).
    pub fn hangup_all(&self) {
        for r in self.live() {
            r.session.hangup();
        }
    }
}

/// Emits `run:status`.
pub fn emit_status(app: &App, run_id: &str, status: RunStatus) {
    let rev = app.runs().next_rev();
    app.emit(Event::RunStatus(RunStatusChanged {
        run_id: RunId::from_trusted(run_id),
        status,
        rev: Rev(rev),
    }));
}

/// Writes `run.json` by CAS (FILE-R4), `0600` in app data (SR-53).
///
/// # Errors
/// `Conflict`, `Io`.
pub fn write_run(app_root: &Root, f: &RunFile, stamp: &ReadStamp) -> Result<ReadStamp, IpcError> {
    let rel = pathpolicy::check_app_data(&run_path(&f.run_id))?;
    let text = schema::to_file(f).map_err(|e| IpcError::io(e.to_string()))?;
    Ok(app_root.write(&rel, text.as_bytes(), stamp)?)
}

/// Reads `run.json`.
///
/// # Errors
/// `NotFound`, `Io`.
pub fn read_run(app_root: &Root, run_id: &str) -> Result<(RunFile, ReadStamp), IpcError> {
    let rel = pathpolicy::check_app_data(&run_path(run_id))?;
    let (bytes, stamp) = app_root.read(&rel, ReadClass::AppState)?;
    match schema::load::<RunFile>(&bytes) {
        Ok(Loaded::Current(f) | Loaded::Migrated { value: f, .. }) => Ok((f, stamp)),
        _ => Err(IpcError::io(format!("{rel} is unreadable."))),
    }
}

/// What a run record needs.
#[derive(Debug)]
pub struct RecordInput<'a> {
    /// Project.
    pub project_id: &'a str,
    /// Stage.
    pub stage: u8,
    /// Step.
    pub step: foreman_core::ids::StepKey,
    /// The plan.
    pub plan: &'a LaunchPlan,
    /// The effective start.
    pub intended: foreman_core::ids::Start,
    /// An override applied.
    pub over: bool,
    /// Entry point.
    pub entry: RunEntry,
    /// Kind.
    pub kind: RunKind,
    /// RFC 3339 UTC.
    pub wall_start: String,
    /// `project` or `spec`.
    pub cwd: Option<String>,
}

/// The run record written before spawn (SR-19, `runlog::record_before_spawn`): the argv with
/// the prompt hashed and environment names only (SR-52).
#[must_use]
pub fn record(i: &RecordInput<'_>) -> RunFile {
    let plan = i.plan;
    let mut env_names: Vec<String> = plan.env.0.iter().map(|(k, _)| k.clone()).collect();
    env_names.sort();
    env_names.dedup();
    RunFile {
        schema_version: 1,
        run_id: plan.run_id.hyphenated().to_string(),
        project_id: i.project_id.to_owned(),
        stage: i.stage,
        step_id: i.step.clone(),
        surface: RunSurface::Cli,
        kind: i.kind,
        chain_id: chain_of(plan),
        name: Some(plan.tab_name.clone()),
        argv: std::iter::once(plan.program.display().to_string())
            .chain(plan.argv_redacted.iter().cloned())
            .collect(),
        env_names,
        cwd: i.cwd.clone(),
        prompt_sha256: plan.prompt_sha256.clone(),
        run_settings_sha256: Some(plan.run_settings_sha256.clone()),
        intended: Some(i.intended),
        r#override: i.over,
        observed: None,
        entry_point: i.entry,
        t: Timings {
            wall_start: i.wall_start.clone(),
            look_start: Some(0),
            prepared: Some(0),
            confirmed: None,
            spawned: None,
            exited: None,
        },
        time_to_start_ms: None,
        run_input_interventions: 0,
        copy_prompt_on_cli: false,
        status: FileStatus::Running,
        exit_code: None,
        claude_version: Some(plan.claude_version.chars().take(64).collect()),
        pid: None,
        sensor_token: None,
    }
}

/// The chain a plan runs in.
#[must_use]
pub fn chain_of(plan: &LaunchPlan) -> Option<String> {
    use foreman_core::chain::SessionPlan as S;
    match &plan.session {
        S::New { id } | S::FreshWithContext { id } | S::Fork { new_id: id, .. } => {
            Some(id.hyphenated().to_string())
        }
        S::Continue { chain } | S::Broken { chain } => Some(chain.hyphenated().to_string()),
        S::NeedsStart { .. } => None,
    }
}

fn ms_since(t: Instant) -> u64 {
    u64::try_from(t.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Appends the run's index line (`runs/index-YYYY-MM.jsonl`, ≤ 4 KB, append-only).
fn append_index(app_root: &Root, f: &RunFile, status: IndexStatus) {
    let month: String = f.t.wall_start.chars().take(7).collect();
    let line = RunIndexLine {
        schema_version: 1,
        run_id: f.run_id.clone(),
        project_id: f.project_id.clone(),
        stage: f.stage,
        step_id: f.step_id.clone(),
        surface: IndexSurface::Cli,
        entry_point: match f.entry_point {
            RunEntry::Palette => IndexEntryPoint::Palette,
            RunEntry::StageView => IndexEntryPoint::StageView,
            RunEntry::Rearm => IndexEntryPoint::Rearm,
            RunEntry::Repair => IndexEntryPoint::Repair,
            RunEntry::DoNext | RunEntry::Probe => IndexEntryPoint::DoNext,
        },
        wall_start: f.t.wall_start.clone(),
        time_to_start_ms: f.time_to_start_ms,
        duration_ms: f
            .t
            .exited
            .zip(f.t.spawned)
            .map(|(e, s)| e.saturating_sub(s)),
        wait_ms: 0,
        status,
        run_input_interventions: f.run_input_interventions,
        copy_prompt_on_cli: f.copy_prompt_on_cli,
        r#override: f.r#override,
    };
    if let (Ok(rel), Ok(text)) = (
        pathpolicy::check_app_data(&format!("runs/index-{month}.jsonl")),
        serde_json::to_string(&line),
    ) {
        let _ = app_root.append_line(&rel, format!("{text}\n").as_bytes());
    }
}

/// The exit handler (reader thread): `run.json` final status, the index line, the step's
/// `Exited` event (DR-065: exit 0 finishes a CLI step), `run:status`.
pub fn on_exit(app: &App, run_id: &str, info: ExitInfo) {
    let Some(live) = app.runs().get(run_id) else {
        return;
    };
    let status = if info.code.is_some() {
        RunStatus::Exited
    } else {
        RunStatus::Errored
    };
    if let Ok(root) = app.app_root()
        && let Ok((mut f, stamp)) = read_run(&root, run_id)
    {
        f.status = if status == RunStatus::Exited {
            FileStatus::Exited
        } else {
            FileStatus::Errored
        };
        f.exit_code = info.code;
        f.t.exited = Some(ms_since(live.started));
        if write_run(&root, &f, &stamp).is_ok() {
            append_index(
                &root,
                &f,
                if status == RunStatus::Exited {
                    IndexStatus::Exited
                } else {
                    IndexStatus::Errored
                },
            );
        }
    }
    let _w = app.writing();
    if let Ok(mut p) = crate::project::open(app, &live.project_id)
        && let Ok(s) = p.step(&live.step).cloned()
    {
        let ctx = p.step_ctx(app, &s);
        let cur = p.step_progress(&live.step);
        let ev = match info.code {
            Some(c) => foreman_core::steps::StepEvent::Exited(c),
            None => foreman_core::steps::StepEvent::Errored,
        };
        if let Ok(next) = foreman_core::steps::transition(&cur, &ctx, &ev) {
            p.progress.steps.insert(s.key.clone(), next);
            let _ = p.save_progress(app);
        }
    }
    emit_status(app, run_id, status);
}

fn summary_of(app: &App, f: &RunFile, names: &HashMap<String, String>) -> RunSummary {
    let status = match f.status {
        FileStatus::Running => RunStatus::Running,
        FileStatus::Exited | FileStatus::Copied => RunStatus::Exited,
        FileStatus::Errored => RunStatus::Errored,
        FileStatus::Interrupted => RunStatus::Interrupted,
    };
    let ran_with = f.intended.map_or_else(String::new, |s| {
        let mut out = format!(
            "{}, {}",
            crate::project::model_name(s.model),
            s.effort.as_str()
        );
        if let Some(m) = s.mode {
            out.push_str(", ");
            out.push_str(&crate::project::mode_name(m));
        }
        out
    });
    let _ = app;
    RunSummary {
        run_id: RunId::from_trusted(f.run_id.as_str()),
        chain: ChainId::from_trusted(f.chain_id.clone().unwrap_or_default()),
        project: ProjectId::from_trusted(f.project_id.as_str()),
        project_name: names.get(&f.project_id).cloned().unwrap_or_default(),
        step: Some(StepKey::from_trusted(f.step_id.as_str())),
        step_title: f.step_id.to_string(),
        tab_name: f.name.clone().unwrap_or_default(),
        ran_with,
        substituted: None,
        status,
        exit_code: f.exit_code,
        error: None,
        started_at: Timestamp(f.t.wall_start.clone()),
        time_to_start_ms: f
            .time_to_start_ms
            .map(|v| u32::try_from(v).unwrap_or(u32::MAX)),
        argv: f.argv.clone(),
        resumable: f.chain_id.is_some() && status != RunStatus::Running,
        why_not_resumable: f
            .chain_id
            .is_none()
            .then(|| "This run has no session to resume.".to_owned()),
    }
}

/// Startup (SR-57, ADR-010 §6): every `run.json` still `running` with no live session in this
/// Foreman becomes `interrupted` (once per app start); returns the interrupted runs, which are
/// listed and never relaunched.
///
/// # Errors
/// `Io`.
pub fn interrupted(app: &App) -> Result<Vec<RunSummary>, IpcError> {
    let root = app.app_root()?;
    let first = !app.runs().scanned.swap(true, Ordering::SeqCst);
    let (reg, _) = app.registry()?;
    let names: HashMap<String, String> = reg
        .projects
        .iter()
        .map(|p| (p.project_id.clone(), p.name.clone()))
        .collect();
    let mut out = Vec::new();
    let ids = foreman_core::store::app_dirs::list_app_data_dir(&root, "runs")?;
    for id in ids.iter().filter(|n| foreman_core::ids::is_uuid_v4(n)) {
        let Ok((mut f, stamp)) = read_run(&root, id) else {
            continue;
        };
        let live = app
            .runs()
            .get(id)
            .is_some_and(|r| r.session.status() == PtyStatus::Running);
        if first && f.status == FileStatus::Running && !live {
            f.status = FileStatus::Interrupted;
            if write_run(&root, &f, &stamp).is_ok() {
                append_index(&root, &f, IndexStatus::Interrupted);
            }
        }
        if f.status == FileStatus::Interrupted {
            out.push(summary_of(app, &f, &names));
        }
    }
    out.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(out)
}
