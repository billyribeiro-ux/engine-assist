//! `assembler` commands (`docs/contracts/ipc.md` §4).

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "handlers take requests by value and return the IPC Result"
)]

use crate::ipc::IpcError;
use crate::ipc::dto::gates::{Assembler, LessonsStatus, Manifest};
use crate::ipc::dto::requests::{
    AssemblerPasteApplyReq, AssemblerPastePreviewReq, AssemblerViewReq, LessonsStatusReq,
    ManifestMarkReq, ManifestViewReq,
};
use crate::ipc::dto::steps::PastePreview;
use crate::state::App;

/// `lessons_status` (M4-18): built in [`crate::ipc::commands::lessons`].
///
/// # Errors
/// As [`crate::ipc::commands::lessons::lessons_status`].
pub fn lessons_status(app: &App, req: LessonsStatusReq) -> Result<LessonsStatus, IpcError> {
    crate::ipc::commands::lessons::lessons_status(app, req)
}

/// `manifest_view`: not built yet (M4-27; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-27`).
pub fn manifest_view(_app: &App, _req: ManifestViewReq) -> Result<Manifest, IpcError> {
    Err(IpcError::not_yet_available("manifest_view", "M4-27"))
}

/// `manifest_mark`: not built yet (M4-27; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-27`).
pub fn manifest_mark(_app: &App, _req: ManifestMarkReq) -> Result<Manifest, IpcError> {
    Err(IpcError::not_yet_available("manifest_mark", "M4-27"))
}

/// `assembler_view`: not built yet (M4-28; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-28`).
pub fn assembler_view(_app: &App, _req: AssemblerViewReq) -> Result<Assembler, IpcError> {
    Err(IpcError::not_yet_available("assembler_view", "M4-28"))
}

/// `assembler_paste_preview`: not built yet (M4-28; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-28`).
pub fn assembler_paste_preview(
    _app: &App,
    _req: AssemblerPastePreviewReq,
) -> Result<PastePreview, IpcError> {
    Err(IpcError::not_yet_available(
        "assembler_paste_preview",
        "M4-28",
    ))
}

/// `assembler_paste_apply`: not built yet (M4-28; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-28`).
pub fn assembler_paste_apply(
    _app: &App,
    _req: AssemblerPasteApplyReq,
) -> Result<Assembler, IpcError> {
    Err(IpcError::not_yet_available(
        "assembler_paste_apply",
        "M4-28",
    ))
}
