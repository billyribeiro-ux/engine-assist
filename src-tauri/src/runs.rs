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
    marks: Mutex<HashMap<(String, String), Marks>>,
    chat_open: Mutex<HashMap<(String, String), (String, jiff::Timestamp)>>,
}

/// What Billy did to a step's prompt since its last run (F-026, AC-F026-01): the proxies for a
/// manual prompt edit (AC-KC-03).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Marks {
    /// Run inputs retyped: a value set again after it was already set for the step.
    pub interventions: u32,
    /// The prompt was copied for a CLI step (typed or pasted into the terminal by hand).
    pub copy_prompt: bool,
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

    /// Notes that a run input of a step was set again (an intervention).
    pub fn note_input_retyped(&self, project: &str, step: &str) {
        lock(&self.marks)
            .entry((project.to_owned(), step.to_owned()))
            .or_default()
            .interventions += 1;
    }

    /// Notes that a CLI step's prompt was copied.
    pub fn note_copy_prompt(&self, project: &str, step: &str) {
        lock(&self.marks)
            .entry((project.to_owned(), step.to_owned()))
            .or_default()
            .copy_prompt = true;
    }

    /// Takes (and resets) a step's marks: the next run record carries them.
    pub fn take_marks(&self, project: &str, step: &str) -> Marks {
        lock(&self.marks)
            .remove(&(project.to_owned(), step.to_owned()))
            .unwrap_or_default()
    }

    /// Forgets everything held for the run history (Clear run history).
    fn forget_history(&self) {
        lock(&self.marks).clear();
        lock(&self.chat_open).clear();
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

/// The index line of a run record (`runs/index-YYYY-MM.jsonl`, files.md §4.7).
fn index_line(f: &RunFile, status: IndexStatus) -> Option<RunIndexLine> {
    Some(RunIndexLine {
        schema_version: 1,
        run_id: f.run_id.clone(),
        project_id: f.project_id.clone(),
        stage: f.stage,
        step_id: f.step_id.clone(),
        surface: match f.surface {
            RunSurface::Chat => IndexSurface::Chat,
            RunSurface::Cli => IndexSurface::Cli,
            RunSurface::Desktop => IndexSurface::Desktop,
            RunSurface::Probe => return None,
        },
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
    })
}

/// Appends the run's index line (`runs/index-YYYY-MM.jsonl`, ≤ 4 KB, append-only).
pub fn append_index(app_root: &Root, f: &RunFile, status: IndexStatus) {
    let Some(line) = index_line(f, status) else {
        return;
    };
    append_line(app_root, &line);
}

fn append_line(app_root: &Root, line: &RunIndexLine) {
    let month: String = line.wall_start.chars().take(7).collect();
    if let (Ok(rel), Ok(text)) = (
        pathpolicy::check_app_data(&format!("runs/index-{month}.jsonl")),
        serde_json::to_string(line),
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

/// What turning run records into [`RunSummary`] rows needs, gathered once per call: project
/// names and folders, where Claude Code keeps transcripts, and step titles (projects are opened
/// lazily, once, and only for rows that are shown).
pub struct SummaryCx<'a> {
    app: &'a App,
    names: HashMap<String, String>,
    roots: HashMap<String, PathBuf>,
    config_dir: PathBuf,
    titles: HashMap<String, Option<crate::project::Opened>>,
}

impl std::fmt::Debug for SummaryCx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SummaryCx")
    }
}

impl<'a> SummaryCx<'a> {
    /// Reads the registry.
    ///
    /// # Errors
    /// `Io` when `projects.json` is unreadable.
    pub fn new(app: &'a App) -> Result<Self, IpcError> {
        let (reg, _) = app.registry()?;
        Ok(Self {
            app,
            names: reg
                .projects
                .iter()
                .map(|p| (p.project_id.clone(), p.name.clone()))
                .collect(),
            roots: reg
                .projects
                .iter()
                .map(|p| (p.project_id.clone(), PathBuf::from(&p.path)))
                .collect(),
            config_dir: app
                .config_dir_env()
                .unwrap_or_else(|| app.paths().home.join(".claude")),
            titles: HashMap::new(),
        })
    }

    fn title(&mut self, project: &str, step: &str) -> String {
        let app = self.app;
        let opened = self
            .titles
            .entry(project.to_owned())
            .or_insert_with(|| crate::project::open(app, project).ok());
        opened
            .as_ref()
            .and_then(|p| p.step(step).ok().map(|s| s.title.clone()))
            .unwrap_or_else(|| step.to_owned())
    }

    /// Whether Claude Code still has the conversation of a chain (`--resume` needs its transcript).
    fn transcript_exists(&self, f: &RunFile) -> bool {
        let (Some(root), Some(chain)) = (self.roots.get(&f.project_id), f.chain_id.as_deref())
        else {
            return false;
        };
        let Ok(id) = uuid::Uuid::parse_str(chain) else {
            return false;
        };
        let cwd = match f.cwd.as_deref() {
            Some("spec") => root.join("spec"),
            _ => root.clone(),
        };
        let cwd = foreman_sys::spawn::canonical(&cwd);
        foreman_core::store::outside::transcript_exists(&foreman_core::chain::transcript_path(
            &self.config_dir,
            &cwd,
            id,
        ))
    }

    /// One run as the run log shows it.
    pub fn summary(&mut self, f: &RunFile) -> RunSummary {
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
        let cli = f.surface == RunSurface::Cli;
        let (resumable, why) = if !cli {
            (false, Some("This step continues in its chat.".to_owned()))
        } else if f.chain_id.is_none() {
            (false, Some("This run has no session to resume.".to_owned()))
        } else if f.status == FileStatus::Running {
            (false, None)
        } else if self.transcript_exists(f) {
            (true, None)
        } else {
            (
                false,
                Some("Claude Code no longer has this conversation.".to_owned()),
            )
        };
        let title = self.title(&f.project_id, f.step_id.as_str());
        RunSummary {
            run_id: RunId::from_trusted(f.run_id.as_str()),
            chain: ChainId::from_trusted(f.chain_id.clone().unwrap_or_default()),
            project: ProjectId::from_trusted(f.project_id.as_str()),
            project_name: self.names.get(&f.project_id).cloned().unwrap_or_default(),
            step: Some(StepKey::from_trusted(f.step_id.as_str())),
            step_title: title,
            tab_name: f.name.clone().unwrap_or_default(),
            ran_with,
            substituted: None,
            status,
            exit_code: f.exit_code,
            error: (f.status == FileStatus::Errored && f.exit_code.is_none())
                .then(|| "Claude Code didn't start or ended without an exit code.".to_owned()),
            started_at: Timestamp(f.t.wall_start.clone()),
            time_to_start_ms: f
                .time_to_start_ms
                .map(|v| u32::try_from(v).unwrap_or(u32::MAX)),
            argv: f.argv.clone(),
            resumable,
            why_not_resumable: why,
        }
    }
}

/// Startup (SR-57, ADR-010 §6): every `run.json` still `running` with no live session in this
/// Foreman becomes `interrupted` (once per app start); returns the interrupted runs, which are
/// listed and never relaunched. The first scan also prunes output tails past 90 days (SR-53).
///
/// # Errors
/// `Io`.
pub fn interrupted(app: &App) -> Result<Vec<RunSummary>, IpcError> {
    let root = app.app_root()?;
    let first = !app.runs().scanned.swap(true, Ordering::SeqCst);
    let mut cx = SummaryCx::new(app)?;
    let mut found = Vec::new();
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
            found.push(f);
        }
    }
    found.sort_by(|a, b| b.t.wall_start.cmp(&a.t.wall_start));
    let out = found.iter().map(|f| cx.summary(f)).collect();
    if first {
        // A failed prune keeps the tails and tries again at the next start.
        let _ = prune_tails(app);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// The index, metrics and search (F-026)
// ---------------------------------------------------------------------------------------------

fn is_index_file(n: &str) -> bool {
    n.strip_prefix("index-")
        .and_then(|r| r.strip_suffix(".jsonl"))
        .is_some_and(|m| {
            m.len() == 7
                && m.bytes().enumerate().all(|(i, b)| {
                    if i == 4 {
                        b == b'-'
                    } else {
                        b.is_ascii_digit()
                    }
                })
        })
}

/// Every index line of every month, the last line per run (ADR-018 §2). Torn lines are skipped.
///
/// # Errors
/// `Io` when a month file can't be read.
pub fn read_index(app: &App) -> Result<Vec<RunIndexLine>, IpcError> {
    let root = app.app_root()?;
    let names = foreman_core::store::app_dirs::list_app_data_dir(&root, "runs")?;
    let mut all = Vec::new();
    for n in names.iter().filter(|n| is_index_file(n)) {
        let rel = pathpolicy::check_app_data(&format!("runs/{n}"))?;
        let (bytes, _) = root.read(&rel, ReadClass::AppState)?;
        all.extend(foreman_core::runlog::parse_index(&String::from_utf8_lossy(
            &bytes,
        )));
    }
    Ok(foreman_core::runlog::merge_latest(all))
}

/// The by-hand baseline: `docs/metrics-baseline.md` of the first registered project that has a
/// filled-in log, else of the first one that has the file at all, else `None`.
#[must_use]
pub fn read_baseline(app: &App) -> Option<foreman_core::runlog::Baseline> {
    let (reg, _) = app.registry().ok()?;
    let rel = pathpolicy::check_read("docs/metrics-baseline.md", ReadClass::Decision).ok()?;
    let mut found = None;
    for e in &reg.projects {
        let Ok(root) = Root::open(
            std::path::Path::new(&e.path),
            foreman_core::store::RootKind::Project,
        ) else {
            continue;
        };
        let Ok((Some(bytes), _)) = root.read_opt(&rel, ReadClass::Decision) else {
            continue;
        };
        let b = foreman_core::runlog::Baseline::parse(&String::from_utf8_lossy(&bytes));
        if !b.is_empty() {
            return Some(b);
        }
        found.get_or_insert(b);
    }
    found
}

/// The index narrowed to one project (`None` = all).
#[must_use]
pub fn of_project(index: Vec<RunIndexLine>, project: Option<&str>) -> Vec<RunIndexLine> {
    match project {
        Some(p) => index.into_iter().filter(|l| l.project_id == p).collect(),
        None => index,
    }
}

/// What a chat step's Copy and open recorded (M3-23 calls [`record_chat_copy`]).
#[derive(Debug)]
pub struct ChatCopy<'a> {
    /// Project.
    pub project_id: &'a str,
    /// Stage.
    pub stage: u8,
    /// Step.
    pub step: foreman_core::ids::StepKey,
    /// The chat chain the step belongs to, when it has one.
    pub chain: Option<String>,
    /// How the copy was started.
    pub entry: RunEntry,
    /// `ChatCopy` for a step, `FollowupCopy` for a follow-up.
    pub kind: RunKind,
    /// What the step was meant to run with.
    pub intended: Option<foreman_core::ids::Start>,
    /// SHA-256 of the prompt text; the text itself is never stored (SR-52).
    pub prompt_sha256: Option<String>,
    /// Milliseconds from the start of looking (preparing the step) to now: the filled prompt is on
    /// the clipboard and the target chat is open. Pressing Send is not part of it.
    pub time_to_start_ms: u64,
}

fn back(app: &App, ms: u64) -> String {
    let d = jiff::SignedDuration::from_millis(i64::try_from(ms).unwrap_or(i64::MAX));
    let t = app
        .os()
        .now()
        .checked_sub(d)
        .unwrap_or_else(|_| app.os().now());
    crate::os::rfc3339(t)
}

/// A chat step's run record: `run.json` (status `copied`) and its index line, written when Copy and
/// open returns, which is where the chat clock stops (AC-F026-02, AC-KC-01). Returns the run ID.
///
/// # Errors
/// `Conflict`, `Io`.
pub fn record_chat_copy(app: &App, i: &ChatCopy<'_>) -> Result<String, IpcError> {
    let root = app.app_root()?;
    let marks = app.runs().take_marks(i.project_id, i.step.as_str());
    let run_id = uuid::Uuid::new_v4().hyphenated().to_string();
    let f = RunFile {
        schema_version: 1,
        run_id: run_id.clone(),
        project_id: i.project_id.to_owned(),
        stage: i.stage,
        step_id: i.step.clone(),
        surface: RunSurface::Chat,
        kind: i.kind,
        chain_id: i.chain.clone(),
        name: None,
        argv: Vec::new(),
        env_names: Vec::new(),
        cwd: None,
        prompt_sha256: i.prompt_sha256.clone(),
        run_settings_sha256: None,
        intended: i.intended,
        r#override: false,
        observed: None,
        entry_point: i.entry,
        t: Timings {
            wall_start: back(app, i.time_to_start_ms),
            look_start: Some(0),
            prepared: Some(0),
            confirmed: None,
            spawned: Some(i.time_to_start_ms),
            exited: None,
        },
        time_to_start_ms: Some(i.time_to_start_ms),
        run_input_interventions: marks.interventions,
        copy_prompt_on_cli: false,
        status: FileStatus::Copied,
        exit_code: None,
        claude_version: None,
        pid: None,
        sensor_token: None,
    };
    write_run(&root, &f, &ReadStamp::Absent)?;
    append_index(&root, &f, IndexStatus::Copied);
    lock(&app.runs().chat_open).insert(
        (i.project_id.to_owned(), i.step.to_string()),
        (run_id.clone(), app.os().now()),
    );
    Ok(run_id)
}

/// A step is done (packet saved, pasted back, Mark done): the step's open chat run gets its
/// duration, as a second index line with the same run ID (readers keep the last one). Does nothing
/// when no chat run is open for the step (a CLI step already has its duration from the exit).
pub fn note_step_done(app: &App, project_id: &str, step: &str) {
    let Some((run_id, copied_at)) =
        lock(&app.runs().chat_open).remove(&(project_id.to_owned(), step.to_owned()))
    else {
        return;
    };
    let Ok(root) = app.app_root() else {
        return;
    };
    let Ok((mut f, stamp)) = read_run(&root, &run_id) else {
        return;
    };
    // From Copy and open returning to now: the time after the chat clock stopped.
    let since = app.os().now().as_millisecond() - copied_at.as_millisecond();
    let spawned = f.t.spawned.unwrap_or(0);
    f.t.exited = Some(spawned.saturating_add(u64::try_from(since).unwrap_or(0)));
    if write_run(&root, &f, &stamp).is_ok() {
        append_index(&root, &f, IndexStatus::Copied);
    }
}

const SEARCH_SCAN_CAP: usize = 3_000;

struct Candidate {
    run_id: String,
    project_id: String,
    wall_start: String,
    step: String,
    status: &'static str,
    surface: &'static str,
}

fn matches(hay: &[String], needles: &[String]) -> bool {
    needles
        .iter()
        .all(|n| hay.iter().any(|h| h.contains(n.as_str())))
}

/// `runs_search`: runs newest first, narrowed to a project and to the words of `query`, which are
/// matched (case-insensitively, all of them) against the step, project name, session name, outcome
/// and argv. Live runs that have no index line yet come first. An empty query pages the index
/// without opening a run record per row beyond the page; a query reads the newest 3,000 run
/// records (older runs are found by Clear/export, not search).
///
/// # Errors
/// `NotFound` (project), `Io`.
pub fn search(
    app: &App,
    query: &str,
    project: Option<&str>,
    cursor: Option<&str>,
    limit: usize,
) -> Result<(Vec<RunSummary>, Option<String>, usize), IpcError> {
    let root = app.app_root()?;
    let mut cx = SummaryCx::new(app)?;
    if let Some(p) = project
        && !cx.names.contains_key(p)
    {
        return Err(IpcError::not_found("That project isn't registered."));
    }
    let index = of_project(read_index(app)?, project);
    let mut seen: std::collections::HashSet<&str> =
        index.iter().map(|l| l.run_id.as_str()).collect();
    let mut cands: Vec<Candidate> = Vec::with_capacity(index.len() + 4);
    for l in &index {
        cands.push(Candidate {
            run_id: l.run_id.clone(),
            project_id: l.project_id.clone(),
            wall_start: l.wall_start.clone(),
            step: l.step_id.to_string(),
            status: match l.status {
                IndexStatus::Exited => "exited",
                IndexStatus::Errored => "errored failed",
                IndexStatus::Interrupted => "interrupted",
                IndexStatus::Copied => "copied",
            },
            surface: match l.surface {
                IndexSurface::Chat => "chat",
                IndexSurface::Cli => "cli",
                IndexSurface::Desktop => "desktop",
            },
        });
    }
    let live = app.runs().live();
    let mut extra = Vec::new();
    for r in live {
        if project.is_some_and(|p| p != r.project_id) || seen.contains(r.run_id.as_str()) {
            continue;
        }
        if let Ok((f, _)) = read_run(&root, &r.run_id) {
            extra.push(Candidate {
                run_id: f.run_id.clone(),
                project_id: f.project_id.clone(),
                wall_start: f.t.wall_start.clone(),
                step: f.step_id.to_string(),
                status: "running",
                surface: "cli",
            });
        }
    }
    seen.clear();
    cands.extend(extra);
    cands.sort_by(|a, b| {
        b.wall_start
            .cmp(&a.wall_start)
            .then(b.run_id.cmp(&a.run_id))
    });

    let offset = cursor.and_then(|c| c.parse::<usize>().ok()).unwrap_or(0);
    let needles: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut rows: Vec<RunFile> = Vec::new();
    let total;
    if needles.is_empty() {
        total = cands.len();
        for c in cands.iter().skip(offset).take(limit) {
            if let Ok((f, _)) = read_run(&root, &c.run_id) {
                rows.push(f);
            }
        }
    } else {
        let mut hits: Vec<RunFile> = Vec::new();
        for c in cands.iter().take(SEARCH_SCAN_CAP) {
            let Ok((f, _)) = read_run(&root, &c.run_id) else {
                continue;
            };
            let mut hay = vec![
                c.step.to_lowercase(),
                c.status.to_owned(),
                c.surface.to_owned(),
                cx.names
                    .get(&c.project_id)
                    .map_or_else(String::new, |n| n.to_lowercase()),
                f.name.clone().unwrap_or_default().to_lowercase(),
                f.argv.join(" ").to_lowercase(),
                format!("stage {}", f.stage),
            ];
            hay.push(cx.title(&c.project_id, &c.step).to_lowercase());
            if matches(&hay, &needles) {
                hits.push(f);
            }
        }
        total = hits.len();
        rows = hits.into_iter().skip(offset).take(limit).collect();
    }
    let next = (offset + limit < total).then(|| (offset + limit).to_string());
    let out = rows.iter().map(|f| cx.summary(f)).collect();
    Ok((out, next, total))
}

// ---------------------------------------------------------------------------------------------
// Retention and Clear run history (PR-05, SR-53, M4-02)
// ---------------------------------------------------------------------------------------------

/// Deletes the output tail of every run older than 90 days (SR-53); the run record and its index
/// line stay for the metrics. Returns how many tails went.
///
/// # Errors
/// `Io`.
pub fn prune_tails(app: &App) -> Result<usize, IpcError> {
    let root = app.app_root()?;
    let now = app.os().now();
    let mut n = 0;
    for l in read_index(app)? {
        if foreman_core::retention::tail_expired(&l.wall_start, now)
            && foreman_core::store::app_dirs::remove_file(
                &root,
                &foreman_core::retention::tail_path(&l.run_id),
            )?
        {
            n += 1;
        }
    }
    Ok(n)
}

/// Run records and their size: Settings → Data.
#[must_use]
pub fn history_size(app: &App) -> (u32, u32) {
    let Ok(root) = app.app_root() else {
        return (0, 0);
    };
    let runs = foreman_core::store::app_dirs::list_app_data_dir(&root, "runs")
        .map(|ids| {
            ids.iter()
                .filter(|n| foreman_core::ids::is_uuid_v4(n))
                .count()
        })
        .unwrap_or(0);
    let bytes = foreman_core::retention::HISTORY_TREES
        .iter()
        .map(|t| foreman_core::store::app_dirs::tree_size(&root, t).unwrap_or(0))
        .sum::<u64>();
    (
        u32::try_from(runs).unwrap_or(u32::MAX),
        u32::try_from(bytes.div_ceil(1024 * 1024)).unwrap_or(u32::MAX),
    )
}

/// `runs_clear` after the native confirmation: deletes `runs/` and `events/`.
///
/// # Errors
/// `NotAllowed("runs-live")`, `Cancelled`, `Io`.
pub fn clear_history(app: &App) -> Result<(), IpcError> {
    if !app.runs().live().is_empty() {
        return Err(IpcError::not_allowed(
            "runs-live",
            "A Claude Code session is still running. End it first.",
        ));
    }
    let (runs, mb) = history_size(app);
    let ok = app.os().confirm(&crate::os::Confirm::new(
        "Clear run history?",
        format!(
            "Foreman deletes its record of {runs} runs (about {mb} MB): run records, saved terminal output and sensor events. \
             The numbers for Export metrics go with them. Your repos, packets and stage records are not touched."
        ),
        "Clear run history",
    ));
    if !ok {
        return Err(IpcError::cancelled());
    }
    let root = app.app_root()?;
    for t in foreman_core::retention::HISTORY_TREES {
        foreman_core::store::app_dirs::remove_tree(&root, t)?;
    }
    app.runs().forget_history();
    Ok(())
}
