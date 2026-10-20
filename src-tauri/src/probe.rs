//! `app::probe` (F-019, P2, ADR-017 as amended 2026-10-20, SR-17, NFR-44): the zero-token
//! contract probes and the stored contract (`settings.json` `contract`).
//!
//! - **Where.** Only in `<app-data>/probe/`, recreated `0700` with Foreman's files only before each
//!   run (`foreman_sys::launch::reset_probe_dir`); `foreman_sys::spawn` forces that cwd for every
//!   probe request and allows only `--init-only` argv (AC-F019-17).
//! - **What.** Probe A (`--init-only --settings … --session-id … --model … --effort …
//!   --permission-mode … --name fm-probe`, a `SessionStart` marker written by the hash-checked
//!   sensor) and probe B (B1 the fork form, B2 the refused control), 20 s each.
//! - **When.** `probe_run` (Doctor) any time; before a launch when the found version differs from
//!   the verified one, once Billy has done the probe-folder trust step and this build has the
//!   sensor (AC-F008-21, AC-F019-15). Without either, probes stay off and Doctor says why.
//! - **Outcome (AC-F019-16).** Pass stores "contract verified on X.Y.Z"; a flag failure disables
//!   only its dependent feature and names it; inconclusive keeps the last verified state, warns,
//!   and the launch proceeds.

use foreman_core::ids::{Effort, Mode, Model, Start};
use foreman_core::launch::Blocker;
use foreman_core::launch::argv;
use foreman_core::launch::run_settings::{self, RunSettingsOpts, SensorHooks};
use foreman_core::probe::{self as cp, ProbeOutcome, ProbeResult};
use foreman_core::schema::settings::{
    Contract, ProbeOutcome as StoredOutcome, ProbeOutcomes, ProbeResult as StoredResult,
    SettingsFile,
};
use foreman_core::store::app_dirs;
use foreman_sys::launch as sl;
use uuid::Uuid;

use crate::ipc::IpcError;
use crate::state::App;

/// The run config Doctor's probe uses (any valid values do; a launch uses the step's own).
pub const DOCTOR_START: Start = Start {
    model: Model::Opus,
    effort: Effort::High,
    mode: Some(Mode::Plan),
};

/// What one probe pass produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    /// The version found (semver when it parsed, else the raw `claude -v` line).
    pub found: String,
    /// Probe A this time.
    pub a: ProbeOutcome,
    /// Probe B this time.
    pub b: ProbeOutcome,
    /// The contract stored after merging (SR-17).
    pub stored: Contract,
}

fn to_stored(o: &ProbeOutcome) -> StoredOutcome {
    StoredOutcome {
        result: match o.result {
            ProbeResult::Pass => StoredResult::Pass,
            ProbeResult::FlagFailure => StoredResult::FlagFailure,
            ProbeResult::Inconclusive | ProbeResult::Skipped => StoredResult::Inconclusive,
        },
        flag: o.flag.clone(),
    }
}

/// A stored outcome as the core's type.
#[must_use]
pub fn from_stored(o: &StoredOutcome) -> ProbeOutcome {
    ProbeOutcome {
        result: match o.result {
            StoredResult::Pass => ProbeResult::Pass,
            StoredResult::FlagFailure => ProbeResult::FlagFailure,
            StoredResult::Inconclusive => ProbeResult::Inconclusive,
        },
        flag: o.flag.clone(),
    }
}

/// The features the stored contract disables (only those, SR-17).
#[must_use]
pub fn disabled(c: Option<&Contract>) -> Vec<&'static str> {
    c.map(|c| {
        cp::disabled_features(&[
            from_stored(&c.outcomes.probe_a),
            from_stored(&c.outcomes.probe_b),
        ])
    })
    .unwrap_or_default()
}

/// `claude -v` as the version string probes compare (semver when it parses).
///
/// # Errors
/// `ToolMissing` when `claude` can't be resolved or run.
pub fn found_version(app: &App) -> Result<String, IpcError> {
    let (settings, _, _) = app.settings()?;
    let (reg, _) = app.registry()?;
    let ctx = app.spawn_ctx(&settings, &reg);
    let missing = || {
        IpcError::tool_missing(
            crate::ipc::types::ToolKind::Claude,
            settings.tools.claude.path.clone(),
        )
    };
    foreman_sys::spawn::resolve(foreman_sys::spawn::Program::Claude, &ctx)
        .map_err(|_| missing())?;
    let env = env(app, &settings, Uuid::new_v4());
    let v = foreman_sys::claude::version(&ctx, &env).map_err(|_| missing())?;
    Ok(v.version.map_or(v.raw, |s| s.to_string()))
}

fn env(app: &App, settings: &SettingsFile, run: Uuid) -> Vec<(String, String)> {
    let ctx = app.spawn_ctx_now().unwrap_or_default();
    let sources = crate::launch::path_sources(
        app,
        settings
            .login_path
            .as_ref()
            .map(|l| l.entries.clone())
            .unwrap_or_default(),
    );
    sl::child_env(
        &app.own_env(),
        &settings.env_passthrough,
        &sources,
        &ctx.projects,
        run,
    )
    .0
}

/// Runs both probes in a fresh probe folder (AC-F019-15, AC-F019-17) and stores the merged
/// contract (AC-F019-16). A passing probe A also records that the probe folder is trusted.
///
/// # Errors
/// `ToolMissing` (no `claude`), `Io`.
pub fn run(app: &App, start: Start) -> Result<Ran, IpcError> {
    let found = found_version(app)?;
    let root = app.app_root()?;
    let (settings, _, _) = app.settings()?;
    let (reg, _) = app.registry()?;
    let ctx = app.spawn_ctx(&settings, &reg);
    let probe_run = Uuid::new_v4();
    let token = crate::sensor::new_token();
    // The marker hook needs the hash-checked sensor; without it probe A can't pass (inconclusive).
    let bytes = match crate::sensor::check(app) {
        crate::sensor::Check::Ok(path) => sl::probe_settings(
            root.path(),
            &SensorHooks {
                command: path.display().to_string(),
                run_id: probe_run.hyphenated().to_string(),
                token: token.clone(),
            },
        )
        .map(|(b, _)| b),
        _ => run_settings::build(
            &root.path().join("probe"),
            root.path(),
            &RunSettingsOpts::default(),
        )
        .map(|f| run_settings::to_bytes(&f).0),
    }
    .map_err(|_| IpcError::io("The probe folder's path can't be written into a settings rule."))?;
    let settings_path = sl::reset_probe_dir(&root, &bytes, probe_run)
        .map_err(|e| IpcError::io(format!("Foreman couldn't reset its probe folder: {e}")))?;
    let a =
        argv::probe_a(&settings_path, Uuid::new_v4(), start).map_err(|_| IpcError::internal())?;
    let (b1, b2) =
        argv::probe_b(Uuid::new_v4(), Uuid::new_v4()).map_err(|_| IpcError::internal())?;
    let env = env(app, &settings, probe_run);
    let report = foreman_sys::claude::run_probes(&ctx, &env, a.0, (b1.0, b2), &|| {
        sl::probe_marker(&root, probe_run, &token)
    });
    let _ = app_dirs::remove_tree(&root, &format!("events/{}", probe_run.hyphenated()));
    store(app, &found, report.a, report.b)
}

fn store(app: &App, found: &str, a: ProbeOutcome, b: ProbeOutcome) -> Result<Ran, IpcError> {
    let (mut s, stamp, _) = app.settings()?;
    let last = s.contract.as_ref().map(|c| {
        (
            c.version.clone(),
            from_stored(&c.outcomes.probe_a),
            from_stored(&c.outcomes.probe_b),
        )
    });
    let (version, ma, mb) = cp::merge(
        last.as_ref().map(|(v, la, lb)| (v.as_str(), la, lb)),
        found,
        a.clone(),
        b.clone(),
    );
    let inconclusive = |o: &ProbeOutcome| o.result == ProbeResult::Inconclusive;
    let stored = if last.is_some() || !(inconclusive(&ma) || inconclusive(&mb)) {
        let advanced = last.as_ref().is_none_or(|(v, ..)| *v != version);
        let at = if advanced || !(inconclusive(&a) || inconclusive(&b)) {
            app.now()
        } else {
            s.contract
                .as_ref()
                .map_or_else(|| app.now(), |c| c.verified_at.clone())
        };
        Some(Contract {
            version,
            verified_at: at,
            outcomes: ProbeOutcomes {
                probe_a: to_stored(&ma),
                probe_b: to_stored(&mb),
            },
        })
    } else {
        // Nothing verified yet and this run was inconclusive: nothing to keep.
        None
    };
    if a.result == ProbeResult::Pass {
        s.probe_folder_trusted = true;
    }
    let shown = stored.clone().unwrap_or(Contract {
        version: found.to_owned(),
        verified_at: app.now(),
        outcomes: ProbeOutcomes {
            probe_a: to_stored(&a),
            probe_b: to_stored(&b),
        },
    });
    s.contract = stored;
    app.save_settings(&s, &stamp)?;
    Ok(Ran {
        found: found.to_owned(),
        a,
        b,
        stored: shown,
    })
}

/// The worst result of a pass, for `LaunchProgress::Contract`.
#[must_use]
pub fn worst(a: &ProbeOutcome, b: &ProbeOutcome) -> ProbeResult {
    let rank = |r: ProbeResult| match r {
        ProbeResult::FlagFailure => 3,
        ProbeResult::Inconclusive => 2,
        ProbeResult::Pass => 1,
        ProbeResult::Skipped => 0,
    };
    if rank(a.result) >= rank(b.result) {
        a.result
    } else {
        b.result
    }
}

/// Why probes don't run before launches yet, if they don't (Doctor's note).
#[must_use]
pub fn off_reason(app: &App, s: &SettingsFile) -> Option<&'static str> {
    if !app.sensor().available() {
        Some("Contract probes need the session sensor, which this build doesn't include.")
    } else if !s.probe_folder_trusted {
        Some(
            "Contract probes are off until you trust the probe folder (first run, or Open probe folder below).",
        )
    } else {
        None
    }
}

/// The contract phase of a launch (DR-120 phase 2, AC-F008-21): runs the probes first when the
/// found version differs from the verified one, then turns the stored contract into blockers for
/// the features this step needs and warnings for the rest. `None` = not due (`skipped`).
#[must_use]
pub fn for_launch(
    app: &App,
    s: &SettingsFile,
    start: Start,
    continues: bool,
) -> (Option<(ProbeResult, Vec<Blocker>)>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut outcome = None;
    let mut contract = s.contract.clone();
    if off_reason(app, s).is_none()
        && let Ok(found) = found_version(app)
        && cp::due(&found, contract.as_ref().map(|c| c.version.as_str()))
    {
        match run(app, start) {
            Ok(r) => {
                let w = worst(&r.a, &r.b);
                if w == ProbeResult::Inconclusive {
                    warnings.push(format!(
                        "The contract probes for Claude Code {} were inconclusive; Foreman keeps the last verified state{}.",
                        crate::render::escape_visible(&found),
                        contract
                            .as_ref()
                            .map(|c| format!(" ({})", crate::render::escape_visible(&c.version)))
                            .unwrap_or_default()
                    ));
                }
                outcome = Some(w);
                contract = Some(r.stored);
            }
            Err(_) => outcome = Some(ProbeResult::Inconclusive),
        }
    }
    let version = contract
        .as_ref()
        .map(|c| c.version.clone())
        .unwrap_or_default();
    let mut blockers = Vec::new();
    for f in disabled(contract.as_ref()) {
        let needed = match f {
            "launch" | "chains" => true,
            "re-arm" => continues,
            _ => false,
        };
        if needed {
            blockers.push(Blocker::ContractFailed {
                feature: f.to_owned(),
                version: version.clone(),
            });
        } else if f == "effort" {
            warnings.push(format!(
                "Claude Code {} didn't accept --effort in the contract probe; the step runs, and Claude Code may use its default effort.",
                crate::render::escape_visible(&version)
            ));
        }
    }
    let result = match (outcome, blockers.is_empty()) {
        (Some(r), _) => Some((r, blockers)),
        (None, false) => Some((ProbeResult::FlagFailure, blockers)),
        (None, true) => None,
    };
    (result, warnings)
}
