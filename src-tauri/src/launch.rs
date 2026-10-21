//! `app::launch` (ADR-021, DR-120, design doc §3.2.3): gathers what only the app knows (the
//! resolved prompt, run inputs, chains, Settings, the project revision) into
//! `foreman_sys::launch::LaunchFacts`, runs `prepare_launch`, holds the plan under a one-time
//! `PlanId` valid 60 s (IPC-R7), and starts the session: `run.json` before spawn (SR-19), then
//! `start_session`, then the chain and step updates.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use foreman_core::chain::{
    self, ChainStartState, EffortOnResume, ForkIds, HandoffConfirm, SessionPlan,
};
use foreman_core::fingerprint::{AllowlistEntry, MANAGED_DIR};
use foreman_core::ids::{CwdKind as CoreCwd, Surface as CoreSurface};
use foreman_core::launch::env::PathSources;
use foreman_core::launch::preflight::ConfirmCtx;
use foreman_core::launch::run_settings::CaseVariants;
use foreman_core::launch::{self as cl, LaunchProgress as CoreProgress};
use foreman_core::policy;
use foreman_core::rules::Applicability as CoreApp;
use foreman_core::schema::fingerprint::SnapshotEntry;
use foreman_core::schema::progress::StoredState;
use foreman_core::schema::run::{EntryPoint as RunEntry, RunKind};
use foreman_core::steps::{StepEvent, StepState as CoreState};
use foreman_core::store::ReadStamp;
use foreman_core::store::outside::{self, OutsideRead};
use foreman_core::template::{Step, StepKind as CoreKind};
use foreman_sys::launch::{self as sl, LaunchFacts, LaunchPlan, StepFacts};
use tauri::ipc::{Channel, Response};
use uuid::Uuid;

use crate::convert;
use crate::ipc::IpcError;
use crate::ipc::dto::launch::{LaunchPreview, PrepareOutcome, RearmNeed};
use crate::ipc::dto::requests::PrepareMode;
use crate::ipc::types::{
    Blocker, ChainId, EntryPoint, EnvName, NotRunnableReason, PlanId, RunStatus, Sha256,
};
use crate::os::Confirm;
use crate::project::Opened;
use crate::runs::{self, LiveRun, Outputs, RunSink};
use crate::state::{App, Expiring, OneTime, PLAN_TTL, Writing};

/// A prepared plan held between `launch_prepare` and `launch_start`/`rearm_start`.
#[derive(Debug, Clone)]
pub struct HeldPlan {
    /// Project.
    pub project_id: String,
    /// Step key.
    pub step: String,
    /// Stage.
    pub stage: u8,
    /// The project revision it was prepared at (IPC-R7).
    pub rev: u32,
    /// How the launch was entered.
    pub entry: EntryPoint,
    /// `normal` or `freshWithContext`.
    pub mode: PrepareMode,
    /// A chain resume (`chain_resume_prepare`).
    pub resume: Option<Uuid>,
    /// The plan.
    pub plan: LaunchPlan,
    /// The chain whose live session must be re-armed first (`rearm_start`).
    pub rearm: Option<String>,
    /// The effective start (`run.json` `intended`).
    pub intended: foreman_core::ids::Start,
    /// An override changed it.
    pub over: bool,
    /// When preparation began (timings).
    pub started: Instant,
    /// A run outside the step's chain bookkeeping (Fix this, M4-16): the step and its chains
    /// aren't updated when it starts.
    pub detached: bool,
}

/// A launch whose prompt and run config come from Foreman, not the step's template text: a gate
/// item's Fix this (a new chain, AC-F015-07) or a packet repair (a re-arm of the stage's chain,
/// AC-F013-11). The prompt goes through the same `check_prompt` and sanitizing as any other.
#[derive(Debug, Clone)]
pub struct Custom {
    /// The prompt (Foreman-written; repo text in it was already escaped by the caller).
    pub prompt: String,
    /// The run config.
    pub start: foreman_core::ids::Start,
    /// `None`: a new session. `Some(chain)`: continue that chain.
    pub resume: Option<Uuid>,
    /// Leave the step's state and chains alone when it starts.
    pub detached: bool,
}

/// Launch state behind the app's mutex.
#[derive(Debug, Default)]
pub struct LaunchState {
    /// One-time plans (60 s).
    pub plans: OneTime<HeldPlan>,
    /// A changed fingerprint awaiting `config_ack`, by (project, cwd): fingerprint and snapshot.
    pub acks: Expiring<(String, String), (String, Vec<SnapshotEntry>)>,
    /// Where each SR-07 hit was found: hit ID → (project, cwd name).
    pub hits: Expiring<String, (String, String)>,
    /// "End it yourself, then press Continue": the confirmed plan per chain.
    pub rearms: Expiring<String, HeldPlan>,
}

fn cwd_name(c: CoreCwd) -> String {
    c.as_str().to_owned()
}

/// The step's cwd on disk (canonical).
///
/// # Errors
/// `ReadOnly` (folder missing).
pub fn step_cwd(p: &Opened, s: &Step) -> Result<(PathBuf, CoreCwd), IpcError> {
    let root = p.root()?.path().to_path_buf();
    let kind = s.cwd.unwrap_or(CoreCwd::Project);
    let dir = match kind {
        CoreCwd::Project => root,
        CoreCwd::Spec => root.join("spec"),
    };
    Ok((foreman_sys::spawn::canonical(&dir), kind))
}

fn lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// `PATH` sources (SR-05): `/etc/paths` through the fixed outside-read list, the fixed
/// candidates and the consented login PATH. `/etc/paths.d/*` needs a listing the outside store
/// doesn't offer yet, so it isn't read (docs/progress.md).
#[must_use]
pub fn path_sources(app: &App, login: Vec<String>) -> PathSources {
    let home = app.paths().home.clone();
    PathSources {
        etc_paths: outside::read(&OutsideRead::EtcPaths, &home, None)
            .map(|b| lines(&b))
            .unwrap_or_default(),
        etc_paths_d: Vec::new(),
        home,
        pnpm_home: None,
        login,
    }
}

fn not_runnable(p: &Opened, app: &App, s: &Step) -> Option<cl::NotRunnableReason> {
    let state = p.display_state(app, s);
    let r = match state {
        CoreState::NotApplicable(_) | CoreState::Skipped(_) => cl::NotRunnableReason::NotApplicable,
        CoreState::Undecided => cl::NotRunnableReason::Undecided,
        CoreState::NeedsAnnotation | CoreState::NeedsNewerForeman => {
            cl::NotRunnableReason::NeedsAnnotation
        }
        CoreState::Done if s.kind != CoreKind::Repeatable => cl::NotRunnableReason::Done,
        CoreState::WaitingUntil(_) | CoreState::Running => cl::NotRunnableReason::Done,
        _ if s.surface != Some(CoreSurface::Cli)
            || !matches!(
                s.kind,
                CoreKind::Prompt | CoreKind::Followup | CoreKind::Repeatable
            ) =>
        {
            cl::NotRunnableReason::NotCli
        }
        _ => return None,
    };
    Some(r)
}

fn chain_start_state(p: &Opened, s: &Step) -> ChainStartState {
    let start = s.chain.clone().unwrap_or_else(|| s.key.clone());
    match p.path.as_ref().and_then(|path| path.step(start.as_str())) {
        Some(CoreApp::NotApplicable(_)) => ChainStartState::NotApplicable,
        Some(CoreApp::Skipped(_)) => ChainStartState::Skipped,
        _ => {
            let prog = p.step_progress(start.as_str());
            if prog.run_count > 0 || prog.state != StoredState::NotStarted {
                ChainStartState::Ran
            } else {
                ChainStartState::NotRun
            }
        }
    }
}

/// The last signed packet's text before `stage` (DR-113's fresh-chain context).
fn last_signed_packet(p: &Opened, stage: u8) -> Option<String> {
    let signed = p.signed();
    (0..stage)
        .rev()
        .find(|n| signed.contains(n))
        .and_then(|n| p.packets.get(&n))
        .map(|f| f.text.clone())
}

/// What a preparation produced.
#[derive(Debug)]
pub struct Prepared {
    /// The IPC outcome.
    pub outcome: PrepareOutcome,
    /// The plan held for `launch_start` (when ready).
    pub held: Option<HeldPlan>,
}

/// Runs the whole preparation (DR-120) for a step.
///
/// # Errors
/// `ReadOnly`, `NotFound`, `Io`.
#[allow(
    clippy::too_many_lines,
    reason = "one fact-gathering pass in DR-120's order feeding prepare_launch"
)]
pub fn prepare(
    app: &App,
    p: &mut Opened,
    key: &str,
    entry: EntryPoint,
    mode: PrepareMode,
    resume: Option<Uuid>,
    progress: &dyn Fn(crate::ipc::dto::launch::LaunchProgress),
) -> Result<Prepared, IpcError> {
    prepare_inner(app, p, key, entry, mode, resume, None, progress)
}

/// [`prepare`] for a [`Custom`] launch on `key`'s cwd and project.
///
/// # Errors
/// As [`prepare`].
pub fn prepare_custom(
    app: &App,
    p: &mut Opened,
    key: &str,
    custom: &Custom,
    progress: &dyn Fn(crate::ipc::dto::launch::LaunchProgress),
) -> Result<Prepared, IpcError> {
    prepare_inner(
        app,
        p,
        key,
        EntryPoint::StageView,
        PrepareMode::Normal,
        None,
        Some(custom),
        progress,
    )
}

#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "one fact-gathering pass in DR-120's order feeding prepare_launch"
)]
fn prepare_inner(
    app: &App,
    p: &mut Opened,
    key: &str,
    entry: EntryPoint,
    mode: PrepareMode,
    resume: Option<Uuid>,
    custom: Option<&Custom>,
    progress: &dyn Fn(crate::ipc::dto::launch::LaunchProgress),
) -> Result<Prepared, IpcError> {
    let started = Instant::now();
    if let Some(ro) = &p.read_only {
        return Ok(Prepared {
            outcome: PrepareOutcome::Blocked {
                blockers: vec![Blocker::ReadOnly(ro.clone())],
                warnings: Vec::new(),
            },
            held: None,
        });
    }
    let s = p.step(key)?.clone();
    let (_, _, project) = p.parts()?;
    let project = project.clone();
    let (settings, _, _) = app.settings()?;
    let (reg, _) = app.registry()?;
    let ctx = app.spawn_ctx(&settings, &reg);
    let over = p.progress.overrides.get(&s.key).copied();
    let core_over = over.map(|o| policy::RunOverride {
        model: o.model,
        effort: o.effort,
        mode: o.mode,
    });
    let (policy_start, effective) = match custom {
        Some(c) => (
            Some(policy::Explained {
                value: c.start,
                origin: policy::ValueOrigin::Template,
                from: None,
            }),
            Some(policy::Explained {
                value: c.start,
                origin: policy::ValueOrigin::Template,
                from: None,
            }),
        ),
        None => (
            policy::policy_start(&s, &project.model_policy).ok(),
            policy::effective_start(&s, &project.model_policy, core_over.as_ref()).ok(),
        ),
    };
    let mut not_run = not_runnable(p, app, &s);
    if (resume.is_some() || custom.is_some())
        && matches!(not_run, Some(cl::NotRunnableReason::Done))
    {
        // Resuming a chain re-opens its last step, and a fix or repair runs after the step is
        // done; "done" doesn't block either.
        not_run = None;
    }
    let (Some(policy_start), Some(effective)) = (policy_start, effective) else {
        return Ok(Prepared {
            outcome: PrepareOutcome::Blocked {
                blockers: vec![Blocker::NotRunnable {
                    reason: NotRunnableReason::NotCli,
                    text: convert::not_runnable_text(NotRunnableReason::NotCli).to_owned(),
                }],
                warnings: Vec::new(),
            },
            held: None,
        });
    };
    // The prompt and its inputs-phase blockers (a bare resume sends no new prompt).
    let mut input_blockers = Vec::new();
    let mut run_input_blockers = Vec::new();
    let mut warnings = Vec::new();
    let mut prompt = custom.map(|c| c.prompt.clone());
    if resume.is_none() && custom.is_none() && s.prompt.is_some() {
        let r = crate::ipc::commands::steps::resolve(p, &s)?;
        match &r.result {
            Ok(ok) => {
                prompt = Some(ok.text.clone());
                warnings.extend(ok.notes.iter().cloned());
            }
            Err(_) => {
                for b in crate::ipc::commands::steps::blockers(p, app, &s, Some(&r)) {
                    match (&b, convert::core_blocker(&b)) {
                        (Blocker::RunInputMissing { .. }, Some(c)) => run_input_blockers.push(c),
                        (Blocker::NotRunnable { .. } | Blocker::ReadOnly(_), _) => {}
                        (_, Some(c)) => input_blockers.push(c),
                        (_, None) => {}
                    }
                }
            }
        }
        if mode == PrepareMode::FreshWithContext
            && let (Some(text), Some(packet)) = (&prompt, last_signed_packet(p, s.stage))
        {
            prompt = Some(format!("{packet}\n\n{text}"));
        }
    }
    let new_id = Uuid::new_v4();
    let session = match (custom, resume, mode) {
        (Some(c), _, _) => match c.resume {
            Some(chain) => SessionPlan::Continue { chain },
            None => SessionPlan::New { id: new_id },
        },
        (None, Some(chain), _) => SessionPlan::Continue { chain },
        (None, None, PrepareMode::FreshWithContext) => SessionPlan::FreshWithContext { id: new_id },
        (None, None, PrepareMode::Normal) => chain::plan_session(
            &s,
            &p.progress.chains,
            new_id,
            chain_start_state(p, &s),
            HandoffConfirm::Fork,
        ),
    };
    let (cwd, cwd_kind) = step_cwd(p, &s)?;
    let root_path = foreman_sys::spawn::canonical(p.root()?.path());
    let own_env: Vec<(String, String)> = app.own_env();
    let sources = path_sources(
        app,
        settings
            .login_path
            .as_ref()
            .map(|l| l.entries.clone())
            .unwrap_or_default(),
    );
    let (t, _, _) = p.parts()?;
    let floor = foreman_core::compat::floor(t, &[], &project.model_policy);
    let cache = sl::load_cache(&p.app_root, &cwd);
    let cache_stamp = cache.as_ref().map_or(ReadStamp::Absent, |(_, s)| s.clone());
    let allow: Vec<AllowlistEntry> = settings
        .auth_key_allowlist
        .iter()
        .map(|a| AllowlistEntry {
            key: a.key.clone(),
            sha256: a.sha256.clone(),
        })
        .collect();
    let run_id = Uuid::new_v4();
    // P2 (M4-08, M4-12): the sensor hooks after the per-run hash check (SR-15), and the contract
    // probes when Claude Code's version changed (AC-F008-21).
    let (sensor, sensor_warning) = crate::sensor::hooks_for_run(app, run_id);
    warnings.extend(sensor_warning);
    let (contract, contract_warnings) = crate::probe::for_launch(
        app,
        &settings,
        effective.value,
        matches!(
            session,
            SessionPlan::Continue { .. } | SessionPlan::Fork { .. }
        ),
    );
    warnings.extend(contract_warnings);
    let facts = LaunchFacts {
        spawn: &ctx,
        own_env: &own_env,
        passthrough: &settings.env_passthrough,
        path_sources: &sources,
        project_root: &root_path,
        cwd: &cwd,
        cwd_kind,
        managed_dir: Path::new(MANAGED_DIR),
        floor,
        step: StepFacts {
            key: s.key.clone(),
            title: s.title.clone(),
            stage: s.stage,
            project_slug: project.slug.clone(),
            policy_start: policy_start.value,
            effective_start: effective.value,
            overlay_step: s.key.as_str().starts_with("x-"),
            not_runnable: not_run,
        },
        session,
        fork_ids: ForkIds::Pass,
        effort_on_resume: EffortOnResume::Flag,
        prompt,
        input_blockers,
        run_input_blockers,
        warnings,
        contract,
        screen_reader: app.inner().screen_reader_mode,
        confirm: ConfirmCtx {
            overlay_step: false,
            first_launch_done: p.progress.first_launch_done,
            last_seen_version: settings.last_seen_claude_version.clone(),
            found_version: None,
        },
        cache: cache.as_ref().map(|(c, _)| c.clone()),
        allowlist: &allow,
        sensor,
        case_variants: CaseVariants::NotListed,
        run_id,
        project_id: p.id().to_owned(),
        now: app.now(),
    };
    let mut send = |m: CoreProgress| progress(convert::progress(&m));
    let prepared = sl::prepare_launch(&facts, &mut send);
    if let Some(c) = &prepared.cache {
        let _ = sl::save_cache(&p.app_root, c, &cache_stamp);
    }
    if let Some((fp, snap)) = prepared.pending_ack.clone() {
        app.launch()
            .acks
            .insert((p.id().to_owned(), cwd_name(cwd_kind)), (fp, snap));
    }
    if let Some(v) = &prepared.version {
        app.inner().claude_version = Some(crate::render::escape_visible(&v.raw));
    }
    if let Some(chain) = prepared.broken_chain {
        p.progress.chains = chain::mark_broken(&p.progress.chains, chain);
        p.save_progress(app)?;
    }
    if matches!(prepared.outcome, sl::PrepareOutcome::Blocked { .. }) {
        app.sensor().forget(&run_id.hyphenated().to_string());
    }
    match prepared.outcome {
        sl::PrepareOutcome::Blocked { blockers, warnings } => Ok(Prepared {
            outcome: PrepareOutcome::Blocked {
                blockers: blockers.iter().map(convert::blocker).collect(),
                warnings,
            },
            held: None,
        }),
        sl::PrepareOutcome::Ready(plan) => {
            let rearm = match &plan.session {
                SessionPlan::Continue { chain } => {
                    let id = chain.hyphenated().to_string();
                    app.runs().of_chain(&id).map(|_| id)
                }
                _ => None,
            };
            let held = HeldPlan {
                project_id: p.id().to_owned(),
                step: s.key.to_string(),
                stage: s.stage,
                rev: p.progress.rev,
                entry,
                mode,
                resume,
                plan: *plan,
                rearm,
                intended: effective.value,
                over: over.is_some() && custom.is_none(),
                started,
                detached: custom.is_some_and(|c| c.detached),
            };
            let segments = if let Some(c) = custom {
                vec![crate::ipc::types::PromptSegment::Text {
                    spans: crate::render::spans(&c.prompt),
                }]
            } else if held.resume.is_some() {
                Vec::new()
            } else {
                crate::ipc::commands::steps::view_of(app, p, key)
                    .map(|v| v.prompt)
                    .unwrap_or_default()
            };
            let preview = LaunchPreview {
                plan_id: PlanId::from_trusted(""),
                expires_in_ms: u32::try_from(PLAN_TTL.as_millis()).unwrap_or(u32::MAX),
                prompt: segments,
                argv: std::iter::once(held.plan.program.display().to_string())
                    .chain(held.plan.argv.iter().cloned())
                    .map(|a| crate::render::escape_visible(&a))
                    .collect(),
                env_names: held
                    .plan
                    .env
                    .0
                    .iter()
                    .map(|(k, _)| EnvName::from_trusted(k.as_str()))
                    .collect(),
                run_settings_sha256: Sha256::from_trusted(held.plan.run_settings_sha256.as_str()),
                confirm: held.plan.confirm.iter().map(convert::confirm).collect(),
                rearm: held.rearm.as_ref().map(|c| RearmNeed::IdleConfirmation {
                    chain: ChainId::from_trusted(c.as_str()),
                }),
                warnings: held.plan.warnings.clone(),
                remaps: Vec::new(),
            };
            Ok(Prepared {
                outcome: PrepareOutcome::Ready(preview),
                held: Some(held),
            })
        }
    }
}

/// Holds a ready plan and fills in its `PlanId`.
pub fn hold(app: &App, prepared: Prepared) -> PrepareOutcome {
    match (prepared.outcome, prepared.held) {
        (PrepareOutcome::Ready(mut preview), Some(held)) => {
            let id = app.launch().plans.hold(held, PLAN_TTL);
            preview.plan_id = PlanId::from_trusted(id);
            PrepareOutcome::Ready(preview)
        }
        (other, _) => other,
    }
}

/// The native launch confirmation (SR-19, SR-24a): every reason, Foreman-written.
#[must_use]
pub fn confirm_launch(app: &App, held: &HeldPlan) -> bool {
    if held.plan.confirm.is_empty() {
        return true;
    }
    let reasons: Vec<String> = held
        .plan
        .confirm
        .iter()
        .map(|c| format!("• {}", convert::confirm_text(c)))
        .collect();
    app.os().confirm(&Confirm::new(
        format!("Launch {}?", held.plan.tab_name),
        reasons.join("\n"),
        "Launch",
    ))
}

/// What a start produced.
#[derive(Debug, Clone)]
pub struct Started {
    /// Run ID.
    pub run_id: String,
    /// Chain.
    pub chain: String,
    /// Tab name.
    pub tab_name: String,
}

/// C2: the configuration fingerprint of a plan's cwd as it is now (stat-cached through the cwd's
/// fingerprint cache). `None` when a source can't be read safely now, which counts as a change.
#[must_use]
pub fn fingerprint_now(app: &App, p: &Opened, held: &HeldPlan) -> Option<String> {
    let root = foreman_sys::spawn::canonical(p.root().ok()?.path());
    let (reg, _) = app.registry().ok()?;
    let projects: Vec<PathBuf> = reg
        .projects
        .iter()
        .map(|e| PathBuf::from(&e.path))
        .collect();
    let cfg = app.config_dir_env();
    let w = foreman_sys::fingerprint::Where {
        cwd: &held.plan.cwd,
        repo_root: &root,
        home: &app.paths().home,
        config_dir: cfg.as_deref(),
        managed: Path::new(MANAGED_DIR),
        projects: &projects,
    };
    let stat = sl::load_cache(&p.app_root, &held.plan.cwd)
        .map(|(c, _)| c.stat)
        .unwrap_or_default();
    foreman_sys::fingerprint::gather(&w, &stat)
        .ok()
        .map(|g| g.fingerprint)
}

/// SR-15 again at spawn (security review N-1): the sensor binary's hash is checked right before
/// the run settings are written. A plan whose hooks run a binary that no longer matches (or that
/// moved) loses its hooks and its token; the run gets the P1 manual behaviors (AC-F011-02).
fn recheck_sensor(app: &App, plan: &mut LaunchPlan, run_id: &str) {
    let Some(cmd) = plan.sensor_command() else {
        return;
    };
    let ok = matches!(
        crate::sensor::check(app),
        crate::sensor::Check::Ok(path) if path.display().to_string() == cmd
    );
    if !ok {
        plan.drop_sensor_hooks();
        app.sensor().forget(run_id);
        plan.warnings.push(crate::sensor::HASH_WARNING.to_owned());
        tracing::warn!(target: "launch", run_id, "sensor no longer matches its hash; this run has no sensor");
    }
}

/// Starts a session from a confirmed plan: `run.json` first (SR-19), then the spawn, then the
/// chain, the step (`Spawn`), `firstLaunchDone` and `lastSeenClaudeVersion`; `run:status`.
/// The caller holds the write lock (`w`) and has re-checked the plan against the project.
///
/// Once the session is spawned this never fails (C2): the step and chain updates go to a freshly
/// opened project (retried once on a conflict), and a failure there is logged, so a live session
/// always gets its tab.
///
/// # Errors
/// Before the spawn only: `ToolFailed` (the spawn), `Conflict`, `Io`.
pub fn start(
    app: &App,
    w: &Writing<'_>,
    p: &mut Opened,
    held: &HeldPlan,
    kind: RunKind,
    output: Option<Channel<Response>>,
) -> Result<Started, IpcError> {
    let s = p.step(&held.step)?.clone();
    let run_id = held.plan.run_id.hyphenated().to_string();
    let mut plan = held.plan.clone();
    recheck_sensor(app, &mut plan, &run_id);
    let entry = match (kind, held.entry) {
        (RunKind::Rearm, _) => RunEntry::Rearm,
        (_, EntryPoint::DoNext) => RunEntry::DoNext,
        (_, EntryPoint::Palette) => RunEntry::Palette,
        (_, EntryPoint::StageView) => RunEntry::StageView,
    };
    let cwd_kind = s.cwd.unwrap_or(CoreCwd::Project);
    let mut file = runs::record(&runs::RecordInput {
        project_id: p.id(),
        stage: s.stage,
        step: s.key.clone(),
        plan: &plan,
        intended: held.intended,
        over: held.over,
        entry,
        kind,
        wall_start: app.now(),
        cwd: Some(cwd_name(cwd_kind)),
    });
    // What Billy did to this step's prompt since its last run (AC-F026-01, AC-KC-03).
    let marks = app.runs().take_marks(p.id(), held.step.as_str());
    file.run_input_interventions = marks.interventions;
    file.copy_prompt_on_cli = marks.copy_prompt;
    file.t.prepared = Some(0);
    file.t.confirmed = Some(u64::try_from(held.started.elapsed().as_millis()).unwrap_or(u64::MAX));
    // P2 (ADR-009): the run's empty event folder exists before spawn; its token goes in run.json.
    file.sensor_token = crate::sensor::begin_run(app, &p.app_root, &run_id);
    let stamp = runs::write_run(&p.app_root, &file, &ReadStamp::Absent)?;
    tracing::info!(target: "launch", run_id = run_id.as_str(), step = held.step.as_str(), "run record written");
    let outputs = Arc::new(Outputs::default());
    if let Some(ch) = output {
        outputs.attach(ch);
    }
    let sink = Arc::new(RunSink {
        run_id: run_id.clone(),
        outputs: Arc::clone(&outputs),
        app: app.weak(),
    });
    let ctx = app.spawn_ctx_now()?;
    let size = foreman_sys::pty::size(120, 32).map_err(|e| IpcError::io(e.to_string()))?;
    let session = match sl::start_session(&p.app_root, &ctx, &plan, size, sink) {
        Ok(s) => s,
        Err(e) => {
            file.status = foreman_core::schema::run::RunStatus::Errored;
            if let Err(we) = runs::write_run(&p.app_root, &file, &stamp) {
                tracing::warn!(target: "launch", run_id = run_id.as_str(), error = %we.message, "run.json not updated after a failed spawn");
            }
            app.sensor().forget(&run_id);
            tracing::warn!(target: "launch", run_id = run_id.as_str(), "spawn failed");
            runs::emit_status(app, &run_id, RunStatus::Errored);
            return Err(IpcError::tool_failed(
                "claude",
                None,
                false,
                crate::render::escape_visible(&e.to_string()),
            ));
        }
    };
    // From here on the session is live: nothing returns an error (C2).
    let spawned = u64::try_from(held.started.elapsed().as_millis()).unwrap_or(u64::MAX);
    file.t.spawned = Some(spawned);
    file.time_to_start_ms = Some(spawned);
    file.pid = session.pid();
    if let Err(e) = runs::write_run(&p.app_root, &file, &stamp) {
        tracing::warn!(target: "launch", run_id = run_id.as_str(), error = %e.message, "run.json not updated after spawn");
    }
    tracing::info!(target: "launch", run_id = run_id.as_str(), ms = spawned, "spawned");
    let chain = runs::chain_of(&plan).unwrap_or_default();
    app.runs().insert(Arc::new(LiveRun {
        run_id: run_id.clone(),
        project_id: p.id().to_owned(),
        chain: chain.clone(),
        step: held.step.clone(),
        tab_name: plan.tab_name.clone(),
        session,
        outputs,
        plan: plan.clone(),
        cwd: plan.cwd.clone(),
        config_changed: AtomicBool::new(false),
        started: held.started,
    }));
    crate::sensor_ingest::watch_run(app, &run_id);
    runs::watch_output(app, &run_id);
    // The chain, the step and the first-launch flag (DR-065, DR-110), on the project as it is
    // now. A detached run (Fix this) leaves the step and its chains as they were.
    let now = app.now();
    let recorded = crate::project::with_project_mut_locked(app, w, &held.project_id, |p| {
        if !held.detached {
            p.progress.chains =
                chain::apply_launch(&p.progress.chains, &s, &plan.session, held.intended);
            let ctx_step = p.step_ctx(app, &s);
            let cur = p.step_progress(&held.step);
            if let Ok(mut next) =
                foreman_core::steps::transition(&cur, &ctx_step, &StepEvent::Spawn)
            {
                next.last_run_id = Some(run_id.clone());
                next.started_at = Some(now.clone());
                p.progress.steps.insert(s.key.clone(), next);
            }
        }
        p.progress.first_launch_done = true;
        // DR-103: an override is for the next run only; this run used it (logged in `run.json`).
        if held.over {
            p.progress.overrides.remove(&s.key);
        }
        Ok(())
    });
    if let Err(e) = recorded {
        tracing::error!(target: "launch", run_id = run_id.as_str(), code = e.code.as_str(), "the step and chain weren't recorded for a live run");
    }
    match app.settings() {
        Ok((mut st, sstamp, _))
            if st.last_seen_claude_version.as_deref() != Some(plan.claude_version.as_str()) =>
        {
            st.last_seen_claude_version = Some(plan.claude_version.chars().take(64).collect());
            if let Err(e) = app.save_settings(&st, &sstamp) {
                tracing::warn!(target: "launch", code = e.code.as_str(), "lastSeenClaudeVersion not saved");
            }
        }
        _ => {}
    }
    runs::emit_status(app, &run_id, RunStatus::Running);
    Ok(Started {
        run_id,
        chain,
        tab_name: plan.tab_name.clone(),
    })
}
