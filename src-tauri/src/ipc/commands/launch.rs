//! `launch` commands (`docs/contracts/ipc.md` §4).

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "handlers take requests by value and return the IPC Result"
)]

use crate::ipc::IpcError;
use crate::ipc::dto::app::SettingsView;
use crate::ipc::dto::launch::{LaunchProgress, PrepareOutcome, RunStarted};
use crate::ipc::dto::requests::{
    AuthKeyAllowlistAddReq, ChainResumePrepareReq, ConfigAckReq, LaunchPrepareReq, LaunchStartReq,
    PrepareMode, ProjectEnvApproveReq,
};
use crate::ipc::types::EntryPoint;
use crate::state::App;

/// The launch preparation shared by `launch_prepare` and `do_next` (DR-120). The prompt-side
/// checks run now (not runnable, read-only, unresolved tokens, prompt hygiene) and return
/// `Blocked`; the launch-side phases (version, fingerprint, auth, argv, run settings, the plan)
/// are the runner tickets' (M2-06, M2-09, M2-25, M3-01…M3-05), so a step that passes the
/// prompt-side checks gets `Unsupported(notYetAvailable)` until they land (IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M2-25`), `ReadOnly`, `NotFound`.
pub fn prepare(
    app: &App,
    p: &crate::project::Opened,
    s: &foreman_core::template::Step,
    _entry: EntryPoint,
    progress: &dyn Fn(LaunchProgress),
) -> Result<PrepareOutcome, IpcError> {
    let r = if s.prompt.is_some() {
        Some(crate::ipc::commands::steps::resolve(p, s)?)
    } else {
        None
    };
    progress(LaunchProgress::Inputs);
    let blockers = crate::ipc::commands::steps::blockers(p, app, s, r.as_ref());
    if !blockers.is_empty() {
        return Ok(PrepareOutcome::Blocked {
            blockers,
            warnings: Vec::new(),
        });
    }
    Err(IpcError::not_yet_available("launch_prepare", "M2-25"))
}

/// `launch_prepare` (P1): see [`prepare`]. `freshWithContext` is only for a step that continues
/// a chain (AC-F010-09).
///
/// # Errors
/// `NotFound`, `ReadOnly`, `NotAllowed("fresh-not-continuing")`, `Unsupported` until M2-25.
pub fn launch_prepare(
    app: &App,
    req: LaunchPrepareReq,
    progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<PrepareOutcome, IpcError> {
    let p = crate::project::open(app, req.project_id.as_str())?;
    let s = p.step(req.step.as_str())?;
    if req.mode == PrepareMode::FreshWithContext
        && s.session != foreman_core::template::Session::Continue
    {
        return Err(IpcError::not_allowed(
            "fresh-not-continuing",
            "This step starts its own session, so there's no chain to restart.",
        ));
    }
    prepare(app, &p, s, req.entry, &|m| {
        let _ = progress.send(m);
    })
}

/// `chain_resume_prepare`: not built yet (M3-16; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-16`).
pub fn chain_resume_prepare(
    _app: &App,
    _req: ChainResumePrepareReq,
    _progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<PrepareOutcome, IpcError> {
    Err(IpcError::not_yet_available("chain_resume_prepare", "M3-16"))
}

/// `launch_start`: not built yet (M3-10; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-10`).
pub fn launch_start(
    _app: &App,
    _req: LaunchStartReq,
    _output: tauri::ipc::Channel<tauri::ipc::Response>,
) -> Result<RunStarted, IpcError> {
    Err(IpcError::not_yet_available("launch_start", "M3-10"))
}

/// `config_ack`: not built yet (M3-02; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-02`).
pub fn config_ack(_app: &App, _req: ConfigAckReq) -> Result<(), IpcError> {
    Err(IpcError::not_yet_available("config_ack", "M3-02"))
}

/// `auth_key_allowlist_add`: not built yet (M3-03; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-03`).
pub fn auth_key_allowlist_add(
    _app: &App,
    _req: AuthKeyAllowlistAddReq,
) -> Result<SettingsView, IpcError> {
    Err(IpcError::not_yet_available(
        "auth_key_allowlist_add",
        "M3-03",
    ))
}

/// `project_env_approve`: not built yet (M3-03; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-03`).
pub fn project_env_approve(_app: &App, _req: ProjectEnvApproveReq) -> Result<(), IpcError> {
    Err(IpcError::not_yet_available("project_env_approve", "M3-03"))
}
