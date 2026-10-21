//! Launch (`docs/contracts/ipc.md` §4.6; F-008, F-010; tickets M2-25, M3-01…M3-05, M3-10,
//! M3-16): preparation through `app::launch`, one-time plans (IPC-R7), native confirmations
//! (SR-19, SR-06, SR-07, SR-24a) and the session start.

use std::path::Path;

use foreman_core::fingerprint::{self, AllowlistEntry, MANAGED_DIR};
use foreman_core::ids::CwdKind as CoreCwd;
use foreman_core::schema::progress::ChainStatus;
use foreman_core::schema::run::RunKind;
use foreman_core::schema::settings::AuthKeyAllow;
use tauri::ipc::{Channel, Response};
use uuid::Uuid;

use crate::ipc::IpcError;
use crate::ipc::dto::app::SettingsView;
use crate::ipc::dto::launch::{LaunchProgress, PrepareOutcome, RunStarted};
use crate::ipc::dto::requests::{
    AuthKeyAllowlistAddReq, ChainResumePrepareReq, ConfigAckReq, LaunchPrepareReq, LaunchStartReq,
    PrepareMode, ProjectEnvApproveReq,
};
use crate::ipc::error::StaleWhat;
use crate::ipc::types::{Blocker, ChainId, EntryPoint, RunId};
use crate::launch::{self, HeldPlan};
use crate::os::Confirm;
use crate::project;
use crate::render::escape_visible;
use crate::state::App;

fn sender(progress: &Channel<LaunchProgress>) -> impl Fn(LaunchProgress) + '_ {
    move |m| {
        let _ = progress.send(m);
    }
}

/// Remembers which project and cwd each SR-07 hit came from (`auth_key_allowlist_add` re-reads
/// it there).
fn note_hits(app: &App, project_id: &str, cwd: CoreCwd, outcome: &PrepareOutcome) {
    if let PrepareOutcome::Blocked { blockers, .. } = outcome {
        let mut l = app.launch();
        for b in blockers {
            if let Blocker::AuthKeyBlocked { hit, .. } = b {
                l.hits.insert(
                    hit.as_str().to_owned(),
                    (project_id.to_owned(), cwd.as_str().to_owned()),
                );
            }
        }
    }
}

/// The preparation shared by `launch_prepare` and `do_next` (DR-120).
///
/// # Errors
/// `NotFound`, `ReadOnly`, `Io`.
pub fn prepare(
    app: &App,
    p: &mut project::Opened,
    key: &str,
    entry: EntryPoint,
    mode: PrepareMode,
    progress: &dyn Fn(LaunchProgress),
) -> Result<PrepareOutcome, IpcError> {
    let cwd = p.step(key)?.cwd.unwrap_or(CoreCwd::Project);
    let prepared = launch::prepare(app, p, key, entry, mode, None, progress)?;
    let out = launch::hold(app, prepared);
    note_hits(app, p.id(), cwd, &out);
    Ok(out)
}

/// `launch_prepare` (P1). `freshWithContext` is only for a step that continues a chain
/// (AC-F010-09). Nothing launches here.
///
/// # Errors
/// `NotFound`, `ReadOnly`, `NotAllowed("fresh-not-continuing")`, `Io`.
pub fn launch_prepare(
    app: &App,
    req: LaunchPrepareReq,
    progress: Channel<LaunchProgress>,
) -> Result<PrepareOutcome, IpcError> {
    // ADR-013: the interest set is re-stat'ed before every launch.
    crate::watch::restat_before_acting(app, req.project_id.as_str());
    let mut p = project::open(app, req.project_id.as_str())?;
    let s = p.step(req.step.as_str())?;
    if req.mode == PrepareMode::FreshWithContext
        && s.session != foreman_core::template::Session::Continue
    {
        return Err(IpcError::not_allowed(
            "fresh-not-continuing",
            "This step starts its own session, so there's no chain to restart.",
        ));
    }
    prepare(
        app,
        &mut p,
        req.step.as_str(),
        req.entry,
        req.mode,
        &sender(&progress),
    )
}

/// `chain_resume_prepare` (P1): a plan for the chain's last step with `--resume`, no new
/// prompt; the full preflight runs (DR-120).
///
/// # Errors
/// `NotFound`, `ReadOnly`, `NotAllowed("chain-live" | "chain-handed-off")`, `Io`.
pub fn chain_resume_prepare(
    app: &App,
    req: ChainResumePrepareReq,
    progress: Channel<LaunchProgress>,
) -> Result<PrepareOutcome, IpcError> {
    let id = req.chain.as_str();
    if app.runs().of_chain(id).is_some() {
        return Err(IpcError::not_allowed(
            "chain-live",
            "This session is still running. Use its tab.",
        ));
    }
    let (reg, _) = app.registry()?;
    for e in &reg.projects {
        let Ok(mut p) = project::open(app, &e.project_id) else {
            continue;
        };
        let Some(c) = p.progress.chains.iter().find(|c| c.id == id).cloned() else {
            continue;
        };
        if crate::watch::restat_before_acting(app, &e.project_id) > 0 {
            p = project::open(app, &e.project_id)?;
        }
        if c.status == ChainStatus::HandedOff {
            return Err(IpcError::not_allowed(
                "chain-handed-off",
                "This session was handed off to Claude Desktop.",
            ));
        }
        let last = c.steps.last().cloned().unwrap_or(c.start.clone());
        let chain = Uuid::parse_str(&c.id)
            .map_err(|_| IpcError::not_found("That session chain is damaged."))?;
        let prepared = launch::prepare(
            app,
            &mut p,
            last.as_str(),
            EntryPoint::StageView,
            PrepareMode::Normal,
            Some(chain),
            &sender(&progress),
        )?;
        return Ok(launch::hold(app, prepared));
    }
    Err(IpcError::not_found(
        "That session chain isn't in any project.",
    ))
}

/// The project for a held plan, refused when it can't be written or its revision moved
/// (IPC-R7: `Stale` with a fresh preview).
fn current_plan(app: &App, held: &HeldPlan) -> Result<project::Opened, IpcError> {
    let p = project::open(app, &held.project_id)?;
    p.writable()?;
    if p.progress.rev != held.rev {
        return Err(IpcError::stale(
            StaleWhat::ProjectRev,
            "The project changed since this preview.",
            fresh(app, held),
        ));
    }
    Ok(p)
}

/// Re-prepares a plan for `Stale` (IPC-R7: the fresh preview goes in the error).
fn fresh(app: &App, held: &HeldPlan) -> Option<serde_json::Value> {
    let mut p = project::open(app, &held.project_id).ok()?;
    let prepared = launch::prepare(
        app,
        &mut p,
        &held.step,
        held.entry,
        held.mode,
        held.resume,
        &|_| {},
    )
    .ok()?;
    serde_json::to_value(launch::hold(app, prepared)).ok()
}

/// `launch_start` (P1): the plan is single-use and valid 60 s; a changed project revision or
/// configuration fingerprint is `Stale` with a fresh preview, checked before the native dialog
/// and again after it under the write lock (IPC-R7, C2); the plan's confirmation reasons are a
/// native dialog (SR-19, SR-24a; Cancel spawns nothing and writes no run record); `run.json` is
/// written before spawn. Once the session is spawned the call succeeds.
///
/// # Errors
/// `Expired`, `Stale`, `Cancelled`, `NotAllowed("needs-rearm")`, `ToolFailed`, `Io`.
pub fn launch_start(
    app: &App,
    req: LaunchStartReq,
    output: Channel<Response>,
) -> Result<RunStarted, IpcError> {
    let held = app.launch().plans.take(req.plan_id.as_str())?;
    // ADR-013: a change a missed event hid lands now, so the revision check below sees it.
    crate::watch::restat_before_acting(app, &held.project_id);
    // A plan that is already stale never asks.
    current_plan(app, &held)?;
    if held.rearm.is_some() {
        return Err(IpcError::not_allowed(
            "needs-rearm",
            "This session is still running. Re-arm it instead.",
        ));
    }
    if !launch::confirm_launch(app, &held) {
        return Err(IpcError::cancelled());
    }
    // C2: the dialog may have been open a long time. A session may have exited (rewriting
    // `progress.json`), a decision file or the configuration may have changed: everything is
    // checked again under the write lock, before anything spawns.
    crate::watch::restat_before_acting(app, &held.project_id);
    let w = app.writing();
    let mut p = current_plan(app, &held)?;
    if launch::fingerprint_now(app, &p, &held).as_deref() != Some(held.plan.fingerprint.as_str()) {
        return Err(IpcError::stale(
            StaleWhat::Fingerprint,
            "The Claude Code configuration changed since this preview.",
            fresh(app, &held),
        ));
    }
    let started = launch::start(app, &w, &mut p, &held, RunKind::Launch, Some(output))?;
    Ok(RunStarted {
        run_id: RunId::from_trusted(started.run_id),
        chain: ChainId::from_trusted(started.chain),
        tab_name: started.tab_name,
    })
}

/// `config_ack` (P1): after the native acknowledgement naming the changed sources (SR-06,
/// SR-24a), the redacted snapshot is stored in the cwd's fingerprint cache (SR-55).
///
/// # Errors
/// `Cancelled`, `Stale` (the fingerprint moved since the banner), `NotFound`, `Io`.
pub fn config_ack(app: &App, req: ConfigAckReq) -> Result<(), IpcError> {
    let cwd_kind: CoreCwd = req.cwd.into();
    let key = (
        req.project_id.as_str().to_owned(),
        cwd_kind.as_str().to_owned(),
    );
    let Some((fp, snapshot)) = app.launch().acks.get(&key).cloned() else {
        return Err(IpcError::stale(
            StaleWhat::Fingerprint,
            "Check the configuration again: nothing is waiting for acknowledgement.",
            None,
        ));
    };
    if fp != req.fingerprint.as_str() {
        return Err(IpcError::stale(
            StaleWhat::Fingerprint,
            "The configuration changed again. Review it before acknowledging.",
            None,
        ));
    }
    let sources: std::collections::BTreeSet<String> =
        snapshot.iter().map(|e| escape_visible(&e.source)).collect();
    let ok = app.os().confirm(&Confirm::new(
        "Acknowledge the Claude Code configuration?",
        format!(
            "These sources changed since the last launch in {}:\n{}",
            cwd_kind.as_str(),
            sources.into_iter().collect::<Vec<_>>().join("\n")
        ),
        "Acknowledge",
    ));
    if !ok {
        return Err(IpcError::cancelled());
    }
    let p = project::open(app, req.project_id.as_str())?;
    let root = p.root()?.path();
    let dir = match cwd_kind {
        CoreCwd::Project => root.to_path_buf(),
        CoreCwd::Spec => root.join("spec"),
    };
    let cwd = foreman_sys::spawn::canonical(&dir);
    let (cache, stamp) = foreman_sys::launch::load_cache(&p.app_root, &cwd).unwrap_or_else(|| {
        (
            foreman_core::schema::fingerprint::FingerprintCacheFile {
                schema_version: 1,
                project_id: p.id().to_owned(),
                cwd: cwd.display().to_string(),
                acked: None,
                stat: Vec::new(),
                auth: None,
            },
            foreman_core::store::ReadStamp::Absent,
        )
    });
    let acked = foreman_sys::launch::acknowledge(cache, &fp, snapshot, &app.now());
    foreman_sys::launch::save_cache(&p.app_root, &acked, &stamp)?;
    app.launch().acks.remove(&key);
    Ok(())
}

/// `auth_key_allowlist_add` (P1): Rust re-reads the configuration where the hit was found,
/// confirms in a native dialog naming the file and key (SR-07, SR-24a), and stores only
/// `sha256("<key>=<value>")` in Settings. Project and managed keys can't be allowlisted.
///
/// # Errors
/// `Expired` (hit unknown or gone), `Cancelled`, `NotAllowed("not-user-settings")`.
pub fn auth_key_allowlist_add(
    app: &App,
    req: AuthKeyAllowlistAddReq,
) -> Result<SettingsView, IpcError> {
    let hit_id = req.hit.as_str().to_owned();
    let Some((project_id, cwd)) = app.launch().hits.get(&hit_id).cloned() else {
        return Err(IpcError::expired());
    };
    let p = project::open(app, &project_id)?;
    let root = foreman_sys::spawn::canonical(p.root()?.path());
    let dir = if cwd == "spec" {
        root.join("spec")
    } else {
        root.clone()
    };
    let dir = foreman_sys::spawn::canonical(&dir);
    let (settings, _, _) = app.settings()?;
    let reg = app.registry()?.0;
    let ctx = app.spawn_ctx(&settings, &reg);
    let cfg = app.config_dir_env();
    let w = foreman_sys::fingerprint::Where {
        cwd: &dir,
        repo_root: &root,
        home: &app.paths().home,
        config_dir: cfg.as_deref(),
        managed: Path::new(MANAGED_DIR),
        projects: &ctx.projects,
    };
    // N4: a source that can't be read now is an I/O problem, not an expired hit.
    let gathered = foreman_sys::fingerprint::gather(&w, &[]).map_err(|_| {
        IpcError::io(
            "Foreman couldn't read the Claude Code configuration again to confirm this key. Run the step again to see what blocks it.",
        )
    })?;
    let allow: Vec<AllowlistEntry> = settings
        .auth_key_allowlist
        .iter()
        .map(|a| AllowlistEntry {
            key: a.key.clone(),
            sha256: a.sha256.clone(),
        })
        .collect();
    let Some(hit) = fingerprint::auth_key_hits(&gathered.files, &allow)
        .into_iter()
        .find(|h| h.id == req.hit.as_str())
    else {
        return Err(IpcError::expired());
    };
    if !hit.allowlistable {
        return Err(IpcError::not_allowed(
            "not-user-settings",
            "Only keys in your own ~/.claude/settings.json can be allowed. Remove this one instead.",
        ));
    }
    let ok = app.os().confirm(&Confirm::new(
        "Allow this key?",
        format!(
            "Foreman will launch while {} sets {}. Its value is remembered by hash only; a different value blocks again.",
            escape_visible(&hit.file),
            escape_visible(&hit.key)
        ),
        "Allow key",
    ));
    if !ok {
        return Err(IpcError::cancelled());
    }
    let _w = app.writing();
    let (mut s, stamp, _) = app.settings()?;
    if !s
        .auth_key_allowlist
        .iter()
        .any(|a| a.sha256 == hit.value_hash)
    {
        s.auth_key_allowlist.push(AuthKeyAllow {
            key: hit.key.clone(),
            sha256: hit.value_hash.clone(),
            added_at: app.now(),
        });
    }
    app.save_settings(&s, &stamp)?;
    app.launch().hits.remove(&hit_id);
    crate::ipc::commands::settings::view(app, &s)
}

/// `project_env_approve` (only on the S-7′ fail branch, which isn't taken: Claude Code doesn't
/// apply a project's `env` before the trust prompt, so nothing needs approving).
///
/// # Errors
/// `Unsupported` (`s7-passed`).
pub fn project_env_approve(_app: &App, _req: ProjectEnvApproveReq) -> Result<(), IpcError> {
    Err(IpcError::unsupported(
        "project_env_approve",
        "s7-passed",
        "Project env approval isn't needed with this Claude Code (S-7′).",
    ))
}
