//! `signing` commands (`docs/contracts/ipc.md` §4.12; F-014; M4-19…M4-22). The logic is in
//! [`crate::signing`]: these handlers validate, take one-time previews and stream
//! `Channel<SignProgress>`.

#![allow(
    clippy::needless_pass_by_value,
    reason = "handlers take requests by value and return the IPC Result"
)]

use crate::ipc::IpcError;
use crate::ipc::dto::gates::{SignPreview, SignResult};
use crate::ipc::dto::launch::SignProgress;
use crate::ipc::dto::records::PacketView;
use crate::ipc::dto::requests::{
    SignApplyReq, SignBranch, SignPreviewReq, SignRetryCommitReq, TamperRestoreReq,
};
use crate::ipc::types::PreviewId;
use crate::preview::Preview;
use crate::project;
use crate::signing::{self, PreviewInput, Seams};
use crate::state::App;

/// `sign_preview` (AC-F014-01…-03, AC-F004-13, AC-F013-15): the name and date, the branch, the
/// added fields as a diff, the content hash, the paths and (Stage 2) the conditions to confirm.
/// Writes nothing; the preview is held 10 minutes for one `sign_apply`.
///
/// # Errors
/// `NotAllowed("not-signable" | "detached-head")`, `ToolMissing(git)`, `ReadOnly`, `Invalid`,
/// `NotFound`.
pub fn sign_preview(app: &App, req: SignPreviewReq) -> Result<SignPreview, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    let inp = PreviewInput {
        stage: req.stage.0,
        other: match &req.branch {
            SignBranch::Current(_) => None,
            SignBranch::Other(o) => Some(o.branch.as_str().to_owned()),
        },
        name: req.name.as_str().to_owned(),
        date: req.date.as_str().to_owned(),
    };
    let (mut pv, held) = signing::preview(app, &p, &inp)?;
    pv.preview_id = PreviewId::from_trusted(signing::hold(app, held));
    Ok(pv)
}

/// [`sign_apply`] with test seams (fault injection; the IPC handler passes none).
///
/// # Errors
/// As [`sign_apply`].
pub fn sign_apply_with(
    app: &App,
    req: SignApplyReq,
    progress: tauri::ipc::Channel<SignProgress>,
    seams: &Seams<'_>,
) -> Result<SignResult, IpcError> {
    let _w = app.writing();
    let held = match app.inner().previews.take(req.preview_id.as_str())? {
        Preview::Sign(h) => h,
        _ => return Err(IpcError::expired()),
    };
    let send = |m: SignProgress| {
        let _ = progress.send(m);
    };
    signing::apply(app, *held, &send, seams)
}

/// `sign_apply` (AC-F014-04…-14, SR-24a, SR-42…SR-47): the native sign dialog, then phase 1
/// (APPROVED BY, the added fields, the stage record, pending decision files), then the plumbing
/// commit with read-back, then the stamp.
///
/// # Errors
/// `Expired`, `Cancelled`, `Stale` (packet changed since review; the fresh preview in `detail`),
/// `Conflict`, `ToolFailed`, `NotAllowed`.
pub fn sign_apply(
    app: &App,
    req: SignApplyReq,
    progress: tauri::ipc::Channel<SignProgress>,
) -> Result<SignResult, IpcError> {
    sign_apply_with(app, req, progress, &Seams::default())
}

/// [`sign_retry_commit`] with test seams.
///
/// # Errors
/// As [`sign_retry_commit`].
pub fn sign_retry_commit_with(
    app: &App,
    req: SignRetryCommitReq,
    progress: tauri::ipc::Channel<SignProgress>,
    seams: &Seams<'_>,
) -> Result<SignResult, IpcError> {
    let _w = app.writing();
    let send = |m: SignProgress| {
        let _ = progress.send(m);
    };
    signing::retry(app, req.project_id.as_str(), req.stage.0, &send, seams)
}

/// `sign_retry_commit` (AC-F014-11, -13): phase 2 again for a `signed (commit pending)` stage.
///
/// # Errors
/// `NotAllowed("not-commit-pending" | "packet-changed")`, `ToolFailed`, `ReadOnly`.
pub fn sign_retry_commit(
    app: &App,
    req: SignRetryCommitReq,
    progress: tauri::ipc::Channel<SignProgress>,
) -> Result<SignResult, IpcError> {
    sign_retry_commit_with(app, req, progress, &Seams::default())
}

/// `tamper_restore` (AC-F014-15, SR-46): the committed packet written back by compare-and-swap.
///
/// # Errors
/// `Stale`, `Conflict`, `NotAllowed("not-tampered")`, `ReadOnly`.
pub fn tamper_restore(app: &App, req: TamperRestoreReq) -> Result<PacketView, IpcError> {
    let _w = app.writing();
    let n = req.stage.0;
    let p = signing::restore(app, req.project_id.as_str(), n, req.seen_sha256.as_str())?;
    let mut v = crate::ipc::commands::packets::view_of(&p, n)?;
    v.tamper = signing::tamper_diff(&p, n);
    Ok(v)
}
