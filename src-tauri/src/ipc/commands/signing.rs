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
    // ADR-013: the interest set is re-stat'ed before every sign.
    crate::watch::restat_before_acting(app, req.project_id.as_str());
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
    let held = match app.inner().previews.take(req.preview_id.as_str())? {
        Preview::Sign(h) => h,
        _ => return Err(IpcError::expired()),
    };
    // ADR-013: re-stat before signing (outside the write lock: a decision change takes it).
    crate::watch::restat_before_acting(app, &held.project_id);
    let _w = app.writing();
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
    // `view_of` carries the tamper diff (none once the committed text is back).
    crate::ipc::commands::packets::view_of(&p, n)
}

/// `packet_repair_prepare` (AC-F013-11; M4-23): prepares a re-arm of the stage's chain with
/// "Fix only these problems in docs/handoff/stage-NN.md: … Do not change anything else." The
/// problems come from Rust's own validation of the current packet, never from the webview; the
/// prompt is sanitized and passes `check_prompt` in the preparation. Nothing starts here: Billy
/// continues with `rearm_start` (its native idle dialog) or `launch_start` when the chain's
/// session has ended.
///
/// # Errors
/// `NotFound` (no packet, or the chain never ran), `ReadOnly`, `NotAllowed("no-problems" |
/// "not-cli-packet" | "chain-handed-off")`.
pub fn packet_repair_prepare(
    app: &App,
    req: crate::ipc::dto::requests::PacketRepairPrepareReq,
    progress: tauri::ipc::Channel<crate::ipc::dto::launch::LaunchProgress>,
) -> Result<crate::ipc::dto::launch::PrepareOutcome, IpcError> {
    use foreman_core::ids::Surface;
    use foreman_core::schema::progress::ChainStatus;
    let n = req.stage.0;
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let Some(file) = p.packets.get(&n).cloned() else {
        return Err(IpcError::not_found("That packet doesn't exist yet."));
    };
    let problems = p.packet_problems(n, &file.text);
    if problems.is_empty() {
        return Err(IpcError::not_allowed(
            "no-problems",
            "The packet has no problems to fix.",
        ));
    }
    let (t, path, _) = p.parts()?;
    let Some(producer) = foreman_core::rules::packet_producer(t, path, n).cloned() else {
        return Err(IpcError::not_found(
            "This stage has no packet-producing step.",
        ));
    };
    if producer.surface != Some(Surface::Cli) {
        return Err(IpcError::not_allowed(
            "not-cli-packet",
            "A chat packet is fixed in the chat: use Copy fix request.",
        ));
    }
    let start_key = producer
        .chain
        .clone()
        .unwrap_or_else(|| producer.key.clone());
    let Some(chain) = p
        .progress
        .chains
        .iter()
        .rev()
        .find(|c| c.start == start_key && c.status != ChainStatus::Superseded)
        .cloned()
    else {
        return Err(IpcError::not_found(
            "The stage's chain hasn't run, so there's no session to re-arm.",
        ));
    };
    if chain.status == ChainStatus::HandedOff {
        return Err(IpcError::not_allowed(
            "chain-handed-off",
            "The chain was handed off to Desktop. Fix the packet there or in your editor.",
        ));
    }
    let chain_id = uuid::Uuid::parse_str(&chain.id)
        .map_err(|_| IpcError::io("The chain's ID is unreadable."))?;
    let list: Vec<String> = problems
        .iter()
        .map(|x| format!("line {}: {}", x.line, x.message))
        .collect();
    let prompt = foreman_core::sanitize::for_paste(&format!(
        "Fix only these problems in {}: {} Do not change anything else.",
        foreman_core::packet::file_name(n),
        list.join("; ") + if list.is_empty() { "" } else { "." }
    ))
    .0;
    let start = chain
        .last_start
        .or(producer.start)
        .ok_or_else(|| IpcError::not_found("The chain has no run config."))?;
    let custom = crate::launch::Custom {
        prompt,
        start,
        resume: Some(chain_id),
        detached: false,
    };
    let send = |m: crate::ipc::dto::launch::LaunchProgress| {
        let _ = progress.send(m);
    };
    let prepared =
        crate::launch::prepare_custom(app, &mut p, producer.key.as_str(), &custom, &send)?;
    Ok(crate::launch::hold(app, prepared))
}
