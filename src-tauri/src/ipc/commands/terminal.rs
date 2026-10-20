//! `terminal` commands (`docs/contracts/ipc.md` §4).

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "handlers take requests by value and return the IPC Result"
)]

use crate::ipc::IpcError;
use crate::ipc::dto::launch::{AttachInfo, LaunchProgress, LinkOutcome, PasteText, RearmOutcome};
use crate::ipc::dto::requests::{
    PtyAttachReq, PtyFlowReq, PtyInterruptReq, PtyPasteReq, PtyResizeReq, PtyTerminateReq,
    PtyWriteReq, RearmContinueReq, RearmStartReq, TerminalLinkOpenReq,
};
use crate::state::App;

/// `pty_attach`: not built yet (M3-10; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-10`).
pub fn pty_attach(
    _app: &App,
    _req: PtyAttachReq,
    _output: tauri::ipc::Channel<tauri::ipc::Response>,
) -> Result<AttachInfo, IpcError> {
    Err(IpcError::not_yet_available("pty_attach", "M3-10"))
}

/// `pty_write`: not built yet (M3-10; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-10`).
pub fn pty_write(_app: &App, _req: PtyWriteReq) -> Result<(), IpcError> {
    Err(IpcError::not_yet_available("pty_write", "M3-10"))
}

/// `pty_paste`: not built yet (M3-12; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-12`).
pub fn pty_paste(_app: &App, _req: PtyPasteReq) -> Result<PasteText, IpcError> {
    Err(IpcError::not_yet_available("pty_paste", "M3-12"))
}

/// `pty_resize`: not built yet (M3-10; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-10`).
pub fn pty_resize(_app: &App, _req: PtyResizeReq) -> Result<(), IpcError> {
    Err(IpcError::not_yet_available("pty_resize", "M3-10"))
}

/// `pty_flow`: not built yet (M3-10; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-10`).
pub fn pty_flow(_app: &App, _req: PtyFlowReq) -> Result<(), IpcError> {
    Err(IpcError::not_yet_available("pty_flow", "M3-10"))
}

/// `pty_interrupt`: not built yet (M3-13; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-13`).
pub fn pty_interrupt(_app: &App, _req: PtyInterruptReq) -> Result<(), IpcError> {
    Err(IpcError::not_yet_available("pty_interrupt", "M3-13"))
}

/// `pty_terminate`: not built yet (M3-13; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-13`).
pub fn pty_terminate(_app: &App, _req: PtyTerminateReq) -> Result<(), IpcError> {
    Err(IpcError::not_yet_available("pty_terminate", "M3-13"))
}

/// `rearm_start`: not built yet (M3-15; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-15`).
pub fn rearm_start(
    _app: &App,
    _req: RearmStartReq,
    _output: tauri::ipc::Channel<tauri::ipc::Response>,
    _progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<RearmOutcome, IpcError> {
    Err(IpcError::not_yet_available("rearm_start", "M3-15"))
}

/// `rearm_continue`: not built yet (M3-15; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-15`).
pub fn rearm_continue(
    _app: &App,
    _req: RearmContinueReq,
    _output: tauri::ipc::Channel<tauri::ipc::Response>,
    _progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<RearmOutcome, IpcError> {
    Err(IpcError::not_yet_available("rearm_continue", "M3-15"))
}

/// `terminal_link_open`: not built yet (M3-12; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-12`).
pub fn terminal_link_open(_app: &App, _req: TerminalLinkOpenReq) -> Result<LinkOutcome, IpcError> {
    Err(IpcError::not_yet_available("terminal_link_open", "M3-12"))
}
