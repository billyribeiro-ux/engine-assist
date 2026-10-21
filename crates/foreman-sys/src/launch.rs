//! The launch pipeline's imperative half (F-008, ADR-021 §1, DR-120): [`prepare_launch`] gathers
//! the facts (`claude -v` in parallel with the fingerprint reads, `auth status` in the step cwd
//! only after the fingerprint and acknowledgement pass, the transcript `stat`) and calls the pure
//! core functions in DR-120's order; [`write_run_settings`] writes the fresh `0600` settings file;
//! [`start_session`] spawns the PTY. The fingerprint cache is loaded and saved here too.
//!
//! `foreman-app` adds what only it knows (resolver output, run inputs, the project revision, the
//! one-time plan ID, native dialogs) and owns the IPC shapes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use foreman_core::chain::{self, EffortCarry, EffortOnResume, ForkIds, SessionPlan};
use foreman_core::compat::{self, Requirement};
use foreman_core::fingerprint::{self, AllowlistEntry};
use foreman_core::ids::{CwdKind, Start, StepKey};
use foreman_core::launch::argv::{self, LaunchInputs, SessionArgs, SessionName};
use foreman_core::launch::env::{self as cenv, ChildEnv, PathSources};
use foreman_core::launch::preflight::{self, ConfirmCtx, Phase, PreflightFacts};
use foreman_core::launch::run_settings::{self, CaseVariants, RunSettingsOpts, SensorHooks};
use foreman_core::launch::{
    AuthProblem, Blocker, ConfirmReason, LaunchProgress, NotRunnableReason, ToolPathProblem,
};
use foreman_core::pathpolicy::RepoPath;
use foreman_core::probe::ProbeResult;
use foreman_core::schema::fingerprint::{Acked, AuthCache, FingerprintCacheFile, SnapshotEntry};
use foreman_core::store::{self, ReadStamp, Root, StoreError};
use semver::Version;
use uuid::Uuid;

use crate::claude::{self, VersionInfo};
use crate::fingerprint::{self as fp, Where};
use crate::pty::{PtyError, PtySession, Sink};
use crate::spawn::{self, Program, SpawnCtx, SpawnError};

/// The step being launched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepFacts {
    /// Step key.
    pub key: StepKey,
    /// Title (for the divider).
    pub title: String,
    /// Stage.
    pub stage: u8,
    /// The project slug for `--name` (DR-121).
    pub project_slug: String,
    /// The policy start (template + alternative + substitution; DR-106's baseline).
    pub policy_start: Start,
    /// The effective start (after an override).
    pub effective_start: Start,
    /// The step was added by the overlay.
    pub overlay_step: bool,
    /// Why it can't run at all (n/a, undecided, needs annotation, done, not a CLI step).
    pub not_runnable: Option<NotRunnableReason>,
}

/// Everything `prepare_launch` needs (gathered by `foreman-app`).
#[derive(Debug, Clone)]
pub struct LaunchFacts<'a> {
    /// Program resolution facts (app data, projects, Settings tool paths).
    pub spawn: &'a SpawnCtx,
    /// Foreman's own environment.
    pub own_env: &'a [(String, String)],
    /// Settings `envPassthrough`.
    pub passthrough: &'a [String],
    /// `PATH` sources.
    pub path_sources: &'a PathSources,
    /// Canonical repo root.
    pub project_root: &'a Path,
    /// Canonical step cwd.
    pub cwd: &'a Path,
    /// Which cwd.
    pub cwd_kind: CwdKind,
    /// Managed settings folder (`fingerprint::MANAGED_DIR` in production).
    pub managed_dir: &'a Path,
    /// The computed floor and its requirements (`compat::floor`).
    pub floor: (Version, Vec<Requirement>),
    /// The step.
    pub step: StepFacts,
    /// `chain::plan_session` before the transcript check.
    pub session: SessionPlan,
    /// S-8's branch.
    pub fork_ids: ForkIds,
    /// S-3's branch.
    pub effort_on_resume: EffortOnResume,
    /// The resolved prompt (`None` for a bare chain resume).
    pub prompt: Option<String>,
    /// Inputs-phase blockers from the resolver (referenced paths, signed packets).
    pub input_blockers: Vec<Blocker>,
    /// Run-inputs blockers.
    pub run_input_blockers: Vec<Blocker>,
    /// Inputs warnings (shown, never blocking).
    pub warnings: Vec<String>,
    /// Contract outcome when probes ran (P2); `None` = not due.
    pub contract: Option<(ProbeResult, Vec<Blocker>)>,
    /// Settings → Screen reader mode.
    pub screen_reader: bool,
    /// Confirmation context (overlay flag is taken from `step`).
    pub confirm: ConfirmCtx,
    /// The cwd's fingerprint cache (`load_cache`).
    pub cache: Option<FingerprintCacheFile>,
    /// Settings `authKeyAllowlist`.
    pub allowlist: &'a [AllowlistEntry],
    /// P2 sensor hooks (hash-verified by the caller).
    pub sensor: Option<SensorHooks>,
    /// S-1′ side result.
    pub case_variants: CaseVariants,
    /// The new run's ID.
    pub run_id: Uuid,
    /// The project ID (for the cache file).
    pub project_id: String,
    /// Now, RFC 3339 UTC (cache timestamps).
    pub now: String,
}

/// A ready launch (`LaunchPreview` plus what `launch_start` needs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    /// Run ID.
    pub run_id: Uuid,
    /// The resolved `claude` (re-resolved again at spawn).
    pub program: PathBuf,
    /// Arguments after the program.
    pub argv: Vec<String>,
    /// Arguments with the prompt replaced by its hash (run log, SR-52).
    pub argv_redacted: Vec<String>,
    /// The child environment (values never leave Rust).
    pub env: ChildEnv,
    /// The step cwd.
    pub cwd: PathBuf,
    /// The settings file path (`<app-data>/runs/<runId>/settings.json`).
    pub run_settings_path: PathBuf,
    /// Its bytes.
    pub run_settings: Vec<u8>,
    /// Its SHA-256 (recorded in `run.json`).
    pub run_settings_sha256: String,
    /// Native confirmation reasons.
    pub confirm: Vec<ConfirmReason>,
    /// The session plan after the transcript check.
    pub session: SessionPlan,
    /// Effort on resume (DR-117), including a `/effort` line to show.
    pub effort: EffortCarry,
    /// `claude -v`.
    pub claude_version: String,
    /// The fingerprint the plan was made against.
    pub fingerprint: String,
    /// The tab name (`--name`).
    pub tab_name: String,
    /// SHA-256 of the prompt (DR-116's comparison).
    pub prompt_sha256: Option<String>,
    /// Warnings.
    pub warnings: Vec<String>,
}

impl LaunchPlan {
    /// The sensor binary the plan's run settings hooks run (`None`: no hooks).
    #[must_use]
    pub fn sensor_command(&self) -> Option<String> {
        run_settings::sensor_command(&self.run_settings)
    }

    /// Drops the sensor hooks from the run settings (SR-15 re-check at spawn, security review
    /// N-1): the bytes and their hash change; argv points at the same settings path.
    pub fn drop_sensor_hooks(&mut self) {
        if let Some((bytes, sha)) = run_settings::without_hooks(&self.run_settings) {
            self.run_settings = bytes;
            self.run_settings_sha256 = sha;
        }
    }

    /// DR-116: same argv (prompt hashed) and same prompt hash.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        let strip = |v: &[String]| -> Vec<String> {
            v.iter()
                .map(|a| {
                    // The run settings path contains the run ID, which differs per plan.
                    if a.ends_with("/settings.json") && a.contains("/runs/") {
                        "<settings>".to_owned()
                    } else {
                        a.clone()
                    }
                })
                .collect()
        };
        strip(&self.argv_redacted) == strip(&other.argv_redacted)
            && self.prompt_sha256 == other.prompt_sha256
            && self.cwd == other.cwd
    }
}

/// `PrepareOutcome` (ipc.md §4.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareOutcome {
    /// Ready.
    Ready(Box<LaunchPlan>),
    /// Blocked, with the blockers in DR-120's order.
    Blocked {
        /// Blockers.
        blockers: Vec<Blocker>,
        /// Warnings.
        warnings: Vec<String>,
    },
}

/// The result plus what the caller persists.
#[derive(Debug, Clone, PartialEq)]
pub struct Prepared {
    /// The outcome.
    pub outcome: PrepareOutcome,
    /// The updated cache (stat and auth parts; `acked` unchanged) to save with [`save_cache`].
    pub cache: Option<FingerprintCacheFile>,
    /// A changed fingerprint awaiting `config_ack`: the fingerprint and its redacted snapshot.
    pub pending_ack: Option<(String, Vec<SnapshotEntry>)>,
    /// A chain whose transcript is missing (mark it broken, DR-113).
    pub broken_chain: Option<Uuid>,
    /// What `claude -v` said.
    pub version: Option<VersionInfo>,
    /// The phases that ran, in order (for `launch::progress_phase_order`).
    pub phases: Vec<Phase>,
}

fn tool_problem(e: &SpawnError) -> ToolPathProblem {
    match e {
        SpawnError::RelativePath(_) => ToolPathProblem::Relative,
        SpawnError::NotExecutable(_) => ToolPathProblem::NotExecutable,
        SpawnError::InsideProject(_) => ToolPathProblem::InProject,
        SpawnError::WorldWritable(_) => ToolPathProblem::WorldWritable,
        _ => ToolPathProblem::Missing,
    }
}

/// `<app-data>/runs/<runId>/settings.json`.
#[must_use]
pub fn run_settings_path(app_data: &Path, run_id: Uuid) -> PathBuf {
    app_data
        .join("runs")
        .join(run_id.hyphenated().to_string())
        .join("settings.json")
}

/// The child environment for a launch (SR-05): PATH from its sources, sanitized.
#[must_use]
pub fn child_env(
    own: &[(String, String)],
    passthrough: &[String],
    sources: &PathSources,
    projects: &[PathBuf],
    run_id: Uuid,
) -> ChildEnv {
    let path = cenv::build_path(&sources.candidates(), projects, &mut |p| {
        spawn::path_dir_facts(p)
    });
    cenv::build_env(own, passthrough, &path, &run_id.hyphenated().to_string())
}

fn config_dir(own: &[(String, String)], home: &Path) -> PathBuf {
    own.iter()
        .find(|(k, _)| k == "CLAUDE_CONFIG_DIR")
        .map_or_else(|| home.join(".claude"), |(_, v)| PathBuf::from(v))
}

/// Runs the preflight (DR-120) and builds the plan. Never spawns the session.
#[allow(
    clippy::too_many_lines,
    reason = "one linear pipeline in DR-120's order; each phase is a few lines"
)]
pub fn prepare_launch(f: &LaunchFacts<'_>, progress: &mut dyn FnMut(LaunchProgress)) -> Prepared {
    let mut facts = PreflightFacts::default();
    let mut phases = Vec::new();
    let mut warnings = f.warnings.clone();
    let mut cache = f.cache.clone();
    let mut pending_ack = None;
    let mut broken_chain = None;
    let blocked = |facts: PreflightFacts, w: Vec<String>| PrepareOutcome::Blocked {
        blockers: preflight::evaluate_preflight(&facts)
            .err()
            .unwrap_or_default(),
        warnings: w,
    };
    if let Some(reason) = f.step.not_runnable {
        return Prepared {
            outcome: PrepareOutcome::Blocked {
                blockers: vec![Blocker::NotRunnable { reason }],
                warnings,
            },
            cache,
            pending_ack,
            broken_chain,
            version: None,
            phases,
        };
    }
    let home = f.spawn.home.clone();
    let env = child_env(
        f.own_env,
        f.passthrough,
        f.path_sources,
        &f.spawn.projects,
        f.run_id,
    );
    let cfg_env = f
        .own_env
        .iter()
        .find(|(k, _)| k == "CLAUDE_CONFIG_DIR")
        .map(|(_, v)| PathBuf::from(v));

    // 1 + 3 in parallel: `claude -v` and the fingerprint reads (NFR-04/05).
    let where_ = Where {
        cwd: f.cwd,
        repo_root: f.project_root,
        home: &home,
        config_dir: cfg_env.as_deref(),
        managed: f.managed_dir,
        projects: &f.spawn.projects,
    };
    let old_stat = cache.as_ref().map(|c| c.stat.clone()).unwrap_or_default();
    let (version_r, gathered) = std::thread::scope(|s| {
        let v = s.spawn(|| -> Result<VersionInfo, SpawnError> {
            spawn::resolve(Program::Claude, f.spawn)?;
            claude::version(f.spawn, &env.0)
        });
        let g = fp::gather(&where_, &old_stat);
        let v = v
            .join()
            .unwrap_or_else(|_| Err(SpawnError::Spawn("version thread".into())));
        (v, g)
    });

    // Phase 1: version.
    phases.push(Phase::Version);
    let version = match version_r {
        Ok(v) => Some(v),
        Err(e) => {
            facts.add(
                Phase::Version,
                [Blocker::ClaudeMissing {
                    path: f
                        .spawn
                        .settings
                        .iter()
                        .find(|(p, _)| *p == Program::Claude)
                        .map(|(_, p)| p.display().to_string()),
                    why: tool_problem(&e),
                }],
            );
            None
        }
    };
    progress(LaunchProgress::Version {
        found: version.as_ref().map(|v| v.raw.clone()),
    });
    if let Some(v) = &version {
        match &v.version {
            Some(found) if compat::meets(found, &f.floor.0) => {}
            found => facts.add(
                Phase::Version,
                [Blocker::VersionBelowFloor {
                    found: found
                        .as_ref()
                        .map_or_else(|| v.raw.clone(), ToString::to_string),
                    required: f.floor.0.to_string(),
                    requirements: f.floor.1.clone(),
                }],
            ),
        }
    }
    let found_version = version.as_ref().map(|v| v.raw.clone());

    // Phase 2: contract (P2; run by the caller when due).
    if facts.may_run(Phase::Contract) {
        phases.push(Phase::Contract);
        let (outcome, bs) = f
            .contract
            .clone()
            .unwrap_or((ProbeResult::Skipped, Vec::new()));
        progress(LaunchProgress::Contract { outcome });
        facts.add(Phase::Contract, bs);
    }

    // Phase 3: fingerprint + SR-07 scan.
    let mut fingerprint_now = String::new();
    let mut snapshot_now = Vec::new();
    phases.push(Phase::Fingerprint);
    match gathered {
        Ok(g) => {
            progress(LaunchProgress::Fingerprint {
                first_scan: f.cache.is_none(),
            });
            let hits = fingerprint::auth_key_hits(&g.files, f.allowlist);
            facts.add(
                Phase::Fingerprint,
                hits.into_iter().map(|h| Blocker::AuthKeyBlocked {
                    file: h.file,
                    key: h.key,
                    hit: h.id,
                    allowlistable: h.allowlistable,
                }),
            );
            fingerprint_now.clone_from(&g.fingerprint);
            snapshot_now = fingerprint::snapshot(&g.files);
            let c = cache.get_or_insert_with(|| FingerprintCacheFile {
                schema_version: 1,
                project_id: f.project_id.clone(),
                cwd: f.cwd.display().to_string(),
                acked: None,
                stat: vec![],
                auth: None,
            });
            c.stat = g.stat;
        }
        Err(bs) => {
            progress(LaunchProgress::Fingerprint {
                first_scan: f.cache.is_none(),
            });
            facts.add(Phase::Fingerprint, bs);
        }
    }

    // Phase 4: acknowledgement.
    if facts.may_run(Phase::Acknowledgement) {
        phases.push(Phase::Acknowledgement);
        let acked = f.cache.as_ref().and_then(|c| c.acked.as_ref());
        if acked.map(|a| a.fingerprint.as_str()) != Some(fingerprint_now.as_str()) {
            let prev: Vec<SnapshotEntry> = acked.map(|a| a.snapshot.clone()).unwrap_or_default();
            facts.add(
                Phase::Acknowledgement,
                [Blocker::ConfigChanged {
                    cwd: f.cwd_kind,
                    fingerprint: fingerprint_now.clone(),
                    diffs: fingerprint::diff(&prev, &snapshot_now),
                }],
            );
            pending_ack = Some((fingerprint_now.clone(), snapshot_now.clone()));
        }
    }

    // Phase 5: auth (SR-08), cached on (fingerprint, version, env).
    if facts.may_run(Phase::Auth) {
        phases.push(Phase::Auth);
        let env_key = env.key();
        let ver = found_version.clone().unwrap_or_default();
        let hit = cache
            .as_ref()
            .and_then(|c| c.auth.as_ref())
            .is_some_and(|a| {
                a.fingerprint == fingerprint_now
                    && a.claude_version == ver
                    && a.env_key == env_key
                    && a.exit == 0
                    && a.auth_method
                        .as_deref()
                        .is_some_and(|m| claude::ALLOWED_METHODS.contains(&m))
            });
        progress(LaunchProgress::Auth { cached: hit });
        if !hit {
            let check = claude::auth_status(f.spawn, f.cwd, &env.0);
            match &check.result {
                Ok(_) => {
                    if let Some(c) = cache.as_mut() {
                        c.auth = Some(AuthCache {
                            fingerprint: fingerprint_now.clone(),
                            claude_version: ver,
                            env_key,
                            auth_method: check.method.clone(),
                            exit: check.exit.unwrap_or(-1),
                            at: f.now.clone(),
                        });
                    }
                }
                Err(why) => {
                    if let Some(c) = cache.as_mut() {
                        c.auth = None;
                    }
                    facts.add(Phase::Auth, [Blocker::AuthFailed { why: why.clone() }]);
                }
            }
        }
    }

    // Phase 6: transcript (continuing steps).
    phases.push(Phase::Transcript);
    let cfg = config_dir(f.own_env, &home);
    let session = match &f.session {
        SessionPlan::Continue { chain } | SessionPlan::Fork { from: chain, .. } => {
            let exists =
                store::outside::transcript_exists(&chain::transcript_path(&cfg, f.cwd, *chain));
            chain::check_transcript(f.session.clone(), exists)
        }
        other => other.clone(),
    };
    match &session {
        SessionPlan::Broken { chain } => {
            broken_chain = Some(*chain);
            facts.add(
                Phase::Transcript,
                [Blocker::TranscriptMissing {
                    chain: chain.hyphenated().to_string(),
                }],
            );
        }
        SessionPlan::NeedsStart { .. } => facts.add(
            Phase::Transcript,
            [Blocker::NotRunnable {
                reason: NotRunnableReason::ChainNotStarted,
            }],
        ),
        _ => {}
    }
    progress(LaunchProgress::Transcript);

    // Phases 7–8: inputs and run inputs (from the resolver).
    phases.push(Phase::Inputs);
    facts.add(Phase::Inputs, f.input_blockers.clone());
    progress(LaunchProgress::Inputs);
    if facts.may_run(Phase::RunInputs) {
        phases.push(Phase::RunInputs);
        facts.add(Phase::RunInputs, f.run_input_blockers.clone());
    }

    // Phase 9: prompt hygiene (SR-04, SR-28).
    if facts.may_run(Phase::PromptHygiene) {
        phases.push(Phase::PromptHygiene);
        if let Some(p) = &f.prompt
            && let Err(b) = foreman_core::sanitize::check_prompt(p)
        {
            facts.add(Phase::PromptHygiene, [Blocker::from(b)]);
        }
    }

    // Phase 10: argv and env.
    let settings_path = run_settings_path(&f.spawn.app_data, f.run_id);
    let name = SessionName::for_step(&f.step.project_slug, f.step.stage, f.step.key.as_str());
    let args = chain::session_args(&session, f.fork_ids);
    let effort = args.as_ref().map_or_else(EffortCarry::default, |a| {
        chain::effort_carry(a, f.step.effective_start.effort, f.effort_on_resume)
    });
    if let Some(line) = &effort.slash_line {
        warnings.push(format!(
            "Type {line} in the session yourself; Claude Code saves it as your default effort."
        ));
    }
    let mut built: Option<argv::Argv> = None;
    if facts.may_run(Phase::ArgvEnv) {
        phases.push(Phase::ArgvEnv);
        if let Some(a) = args {
            let inputs = LaunchInputs {
                session: a,
                start: f.step.effective_start,
                pass_effort: effort.flag,
                name: name.clone(),
                settings: settings_path.clone(),
                screen_reader: f.screen_reader,
                prompt: f.prompt.clone(),
            };
            match argv::build_argv(&inputs) {
                Ok(v) => built = Some(v),
                Err(argv::ArgvError::Prompt(b)) => facts.add(Phase::ArgvEnv, [Blocker::from(b)]),
                Err(_) => facts.add(
                    Phase::ArgvEnv,
                    [Blocker::NotRunnable {
                        reason: NotRunnableReason::NeedsAnnotation,
                    }],
                ),
            }
        }
    }

    // Phase 11: run settings.
    phases.push(Phase::RunSettings);
    let rs = run_settings::build(
        f.project_root,
        &f.spawn.app_data,
        &RunSettingsOpts {
            effort_level: effort.settings_level,
            sensor: f.sensor.clone(),
            case_variants: f.case_variants,
        },
    );
    let rs = match rs {
        Ok(r) => Some(run_settings::to_bytes(&r)),
        Err(b) => {
            facts.add(Phase::RunSettings, [b]);
            None
        }
    };
    if !f.cwd.is_dir() {
        facts.add(
            Phase::RunSettings,
            [Blocker::CwdNotDirectory { cwd: f.cwd_kind }],
        );
    }

    // Phase 12: confirmation reasons.
    let mut confirm_ctx = f.confirm.clone();
    confirm_ctx.overlay_step = f.step.overlay_step;
    confirm_ctx.found_version.clone_from(&found_version);
    let outcome = match (
        preflight::evaluate_preflight(&facts),
        built,
        rs,
        version.as_ref(),
    ) {
        (Ok(()), Some(a), Some((bytes, sha)), Some(v)) => {
            phases.push(Phase::Confirmation);
            let confirm = preflight::needs_confirmation(
                &f.step.policy_start,
                &f.step.effective_start,
                &confirm_ctx,
            );
            let program = spawn::resolve(Program::Claude, f.spawn).unwrap_or_default();
            progress(LaunchProgress::Done);
            PrepareOutcome::Ready(Box::new(LaunchPlan {
                run_id: f.run_id,
                program,
                argv_redacted: a.redacted(f.prompt.is_some()),
                argv: a.0,
                env,
                cwd: f.cwd.to_path_buf(),
                run_settings_path: settings_path,
                run_settings: bytes,
                run_settings_sha256: sha,
                confirm,
                session,
                effort,
                claude_version: v.raw.clone(),
                fingerprint: fingerprint_now,
                tab_name: name.as_str().to_owned(),
                prompt_sha256: f.prompt.as_ref().map(|p| store::sha256_hex(p.as_bytes())),
                warnings: warnings.clone(),
            }))
        }
        _ => blocked(facts, warnings),
    };
    Prepared {
        outcome,
        cache,
        pending_ack,
        broken_chain,
        version,
        phases,
    }
}

/// `cache/fingerprints/<cwdHash>.json`: the first 32 hex of SHA-256 over the canonical cwd.
#[must_use]
pub fn cache_rel(cwd: &Path) -> String {
    let h = store::sha256_hex(cwd.display().to_string().as_bytes());
    format!("cache/fingerprints/{}.json", &h[..32])
}

/// Loads the cwd's fingerprint cache (`None` when missing or unreadable: a fresh scan).
#[must_use]
pub fn load_cache(app_data: &Root, cwd: &Path) -> Option<(FingerprintCacheFile, ReadStamp)> {
    let p = RepoPath::app_data(&cache_rel(cwd)).ok()?;
    let (bytes, stamp) = app_data
        .read_opt(&p, foreman_core::pathpolicy::ReadClass::AppState)
        .ok()?;
    let f: FingerprintCacheFile = foreman_core::json::parse(&bytes?).ok()?;
    Some((f, stamp))
}

/// Saves the cache (`0600`, never clobbering a newer write: SR-33).
///
/// # Errors
/// [`StoreError`].
pub fn save_cache(
    app_data: &Root,
    cache: &FingerprintCacheFile,
    expect: &ReadStamp,
) -> Result<ReadStamp, StoreError> {
    let p = RepoPath::app_data(&cache_rel(Path::new(&cache.cwd)))?;
    let text = foreman_core::json::to_canonical(cache).map_err(|e| StoreError::Io {
        name: p.to_string(),
        message: e.to_string(),
        errno: 0,
    })?;
    app_data.write(&p, text.as_bytes(), expect)
}

/// `config_ack` after Billy's native confirmation: stores the redacted snapshot (SR-55).
#[must_use]
pub fn acknowledge(
    cache: FingerprintCacheFile,
    fingerprint: &str,
    snapshot: Vec<SnapshotEntry>,
    at: &str,
) -> FingerprintCacheFile {
    FingerprintCacheFile {
        acked: Some(Acked {
            fingerprint: fingerprint.to_owned(),
            at: at.to_owned(),
            snapshot,
        }),
        ..cache
    }
}

/// Writes the run settings file fresh (SR-09): `runs/<runId>/settings.json`, `0600`, refusing to
/// replace an existing file. Returns the absolute path.
///
/// # Errors
/// [`StoreError`] (a `Conflict` if the file exists).
pub fn write_run_settings(app_data: &Root, plan: &LaunchPlan) -> Result<PathBuf, StoreError> {
    let rel = format!("runs/{}/settings.json", plan.run_id.hyphenated());
    let p = RepoPath::app_data(&rel)?;
    app_data.write(&p, &plan.run_settings, &ReadStamp::Absent)?;
    Ok(app_data.path().join(rel))
}

/// Why a session couldn't start.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    /// The run settings file couldn't be written.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The PTY or the spawn checks refused.
    #[error(transparent)]
    Pty(#[from] PtyError),
    /// The written file isn't what the plan recorded.
    #[error("the run settings file changed before spawn")]
    SettingsMismatch,
}

/// `launch_start`'s spawn half: writes the run settings (fresh) and spawns the PTY with exactly
/// the plan's argv, env and cwd. The caller writes `run.json` before calling this (SR-19).
///
/// # Errors
/// [`StartError`].
pub fn start_session(
    app_data: &Root,
    ctx: &SpawnCtx,
    plan: &LaunchPlan,
    size: portable_pty::PtySize,
    sink: Arc<dyn Sink>,
) -> Result<PtySession, StartError> {
    let written = write_run_settings(app_data, plan)?;
    if written != plan.run_settings_path
        && store::canonical(&written) != store::canonical(&plan.run_settings_path)
    {
        return Err(StartError::SettingsMismatch);
    }
    Ok(PtySession::spawn(
        ctx,
        &plan.argv,
        &plan.env.0,
        &plan.cwd,
        size,
        sink,
    )?)
}

/// The probe folder's settings (files.md §4.11): the run-settings shape with exactly one hook,
/// `SessionStart`, the sensor writing into `events/<probeRunId>/` (the marker).
///
/// # Errors
/// [`Blocker::RunSettingsPathUnsafe`].
pub fn probe_settings(app_data: &Path, sensor: &SensorHooks) -> Result<(Vec<u8>, String), Blocker> {
    let probe_dir = app_data.join("probe");
    let mut f = run_settings::build(
        &probe_dir,
        app_data,
        &RunSettingsOpts {
            sensor: Some(sensor.clone()),
            ..RunSettingsOpts::default()
        },
    )?;
    if let Some(h) = f.hooks.as_mut() {
        h.retain(|k, _| k == "SessionStart");
    }
    Ok(run_settings::to_bytes(&f))
}

/// Resets `<app-data>/probe/` (`0700`, emptied) and writes its settings file and the probe run's
/// event folder (`events/<probeRunId>/`, `0700`). Returns the settings path.
///
/// # Errors
/// [`StoreError`].
pub fn reset_probe_dir(
    app_data: &Root,
    settings: &[u8],
    probe_run: Uuid,
) -> Result<PathBuf, StoreError> {
    store::app_dirs::reset_dir(app_data, "probe")?;
    app_data.write(
        &RepoPath::app_data("probe/settings.json")?,
        settings,
        &ReadStamp::Absent,
    )?;
    store::app_dirs::reset_dir(app_data, &format!("events/{}", probe_run.hyphenated()))?;
    Ok(app_data.path().join("probe/settings.json"))
}

/// Whether the probe's `SessionStart` marker (an event file with the probe token) exists.
#[must_use]
pub fn probe_marker(app_data: &Root, probe_run: Uuid, token: &str) -> bool {
    crate::sensor::read_events(app_data, &probe_run.hyphenated().to_string()).is_ok_and(|evs| {
        evs.iter().any(|(_, e)| {
            e.event == "SessionStart" && foreman_core::session_state::token_matches(token, &e.token)
        })
    })
}

/// Maps an argv error that can't be a blocker (an internal inconsistency) to text.
#[must_use]
pub fn describe(e: &AuthProblem) -> String {
    match e {
        AuthProblem::WrongMethod { method } => format!("Claude Code is signed in with {method}"),
        AuthProblem::UnknownMethod => "auth status didn't name a sign-in method".into(),
        AuthProblem::Unparsable => "auth status output wasn't JSON".into(),
        AuthProblem::Exit { code } => format!("auth status exited with {code:?}"),
        AuthProblem::Timeout => "auth status didn't answer within 10 s".into(),
    }
}

/// `SessionArgs` for display (the argv preview shows the real argv; this is for logs).
#[must_use]
pub fn session_kind(a: &SessionArgs) -> &'static str {
    match a {
        SessionArgs::New { .. } => "new",
        SessionArgs::Resume { .. } => "resume",
        SessionArgs::Fork { .. } => "fork",
    }
}
