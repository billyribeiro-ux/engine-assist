//! `desktop` commands (`docs/contracts/ipc.md` §4).

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "handlers take requests by value and return the IPC Result"
)]

use crate::ipc::IpcError;
use crate::ipc::dto::app::ProjectView;
use crate::ipc::dto::gates::{DesktopOpened, GateView};
use crate::ipc::dto::launch::LaunchProgress;
use crate::ipc::dto::records::ExportResult;
use crate::ipc::dto::requests::{
    AddItemReq, AddStepReq, DesktopHandoffReq, DesktopOpenReq, RunSheetExportReq,
};
use crate::state::App;

/// `desktop_open`: not built yet (M4-29; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-29`).
pub fn desktop_open(
    _app: &App,
    _req: DesktopOpenReq,
    _progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<DesktopOpened, IpcError> {
    Err(IpcError::not_yet_available("desktop_open", "M4-29"))
}

/// `desktop_handoff`: not built yet (M4-29; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-29`).
pub fn desktop_handoff(_app: &App, _req: DesktopHandoffReq) -> Result<(), IpcError> {
    Err(IpcError::not_yet_available("desktop_handoff", "M4-29"))
}

/// `add_step`: not built yet (M4-31; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-31`).
pub fn add_step(_app: &App, _req: AddStepReq) -> Result<ProjectView, IpcError> {
    Err(IpcError::not_yet_available("add_step", "M4-31"))
}

/// `add_item`: not built yet (M4-31; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-31`).
pub fn add_item(_app: &App, _req: AddItemReq) -> Result<GateView, IpcError> {
    Err(IpcError::not_yet_available("add_item", "M4-31"))
}

/// `run_sheet_export`: not built yet (M4-32; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-32`).
pub fn run_sheet_export(_app: &App, _req: RunSheetExportReq) -> Result<ExportResult, IpcError> {
    Err(IpcError::not_yet_available("run_sheet_export", "M4-32"))
}
