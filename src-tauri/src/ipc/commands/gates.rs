//! `gates` commands (`docs/contracts/ipc.md` §4).

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "handlers take requests by value and return the IPC Result"
)]

use crate::ipc::IpcError;
use crate::ipc::dto::gates::{GateView, ImportProposal};
use crate::ipc::dto::launch::{LaunchProgress, PrepareOutcome};
use crate::ipc::dto::requests::{
    GateFixStartReq, GateImportConfirmReq, GateImportResultsReq, GateItemAttestReq, GateViewReq,
};
use crate::state::App;

/// `gate_view`: not built yet (M4-16; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-16`).
pub fn gate_view(_app: &App, _req: GateViewReq) -> Result<GateView, IpcError> {
    Err(IpcError::not_yet_available("gate_view", "M4-16"))
}

/// `gate_item_attest`: not built yet (M4-16; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-16`).
pub fn gate_item_attest(_app: &App, _req: GateItemAttestReq) -> Result<GateView, IpcError> {
    Err(IpcError::not_yet_available("gate_item_attest", "M4-16"))
}

/// `gate_import_results`: not built yet (M4-16; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-16`).
pub fn gate_import_results(
    _app: &App,
    _req: GateImportResultsReq,
) -> Result<Vec<ImportProposal>, IpcError> {
    Err(IpcError::not_yet_available("gate_import_results", "M4-16"))
}

/// `gate_import_confirm`: not built yet (M4-16; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-16`).
pub fn gate_import_confirm(_app: &App, _req: GateImportConfirmReq) -> Result<GateView, IpcError> {
    Err(IpcError::not_yet_available("gate_import_confirm", "M4-16"))
}

/// `gate_fix_start`: not built yet (M4-16; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-16`).
pub fn gate_fix_start(
    _app: &App,
    _req: GateFixStartReq,
    _progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<PrepareOutcome, IpcError> {
    Err(IpcError::not_yet_available("gate_fix_start", "M4-16"))
}
