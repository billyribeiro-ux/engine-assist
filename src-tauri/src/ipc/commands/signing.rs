//! `signing` commands (`docs/contracts/ipc.md` §4).

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "handlers take requests by value and return the IPC Result"
)]

use crate::ipc::IpcError;
use crate::ipc::dto::gates::{SignPreview, SignResult};
use crate::ipc::dto::launch::SignProgress;
use crate::ipc::dto::records::PacketView;
use crate::ipc::dto::requests::{
    SignApplyReq, SignPreviewReq, SignRetryCommitReq, TamperRestoreReq,
};
use crate::state::App;

/// `sign_preview`: not built yet (M4-19; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-19`).
pub fn sign_preview(_app: &App, _req: SignPreviewReq) -> Result<SignPreview, IpcError> {
    Err(IpcError::not_yet_available("sign_preview", "M4-19"))
}

/// `sign_apply`: not built yet (M4-20; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-20`).
pub fn sign_apply(
    _app: &App,
    _req: SignApplyReq,
    _progress: tauri::ipc::Channel<SignProgress>,
) -> Result<SignResult, IpcError> {
    Err(IpcError::not_yet_available("sign_apply", "M4-20"))
}

/// `sign_retry_commit`: not built yet (M4-21; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-21`).
pub fn sign_retry_commit(
    _app: &App,
    _req: SignRetryCommitReq,
    _progress: tauri::ipc::Channel<SignProgress>,
) -> Result<SignResult, IpcError> {
    Err(IpcError::not_yet_available("sign_retry_commit", "M4-21"))
}

/// `tamper_restore`: not built yet (M4-22; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-22`).
pub fn tamper_restore(_app: &App, _req: TamperRestoreReq) -> Result<PacketView, IpcError> {
    Err(IpcError::not_yet_available("tamper_restore", "M4-22"))
}
