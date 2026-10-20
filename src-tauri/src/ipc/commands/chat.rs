//! `chat` commands (`docs/contracts/ipc.md` §4).

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "handlers take requests by value and return the IPC Result"
)]

use crate::ipc::IpcError;
use crate::ipc::dto::requests::{
    ChatCopyAndOpenReq, ChatLinkSetReq, PasteBackApplyReq, PasteBackPreviewReq,
};
use crate::ipc::dto::steps::{ChatCopyResult, PasteApplyResult, PastePreview};
use crate::ipc::types::Rev;
use crate::state::App;

/// `chat_copy_and_open`: not built yet (M3-23; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-23`).
pub fn chat_copy_and_open(
    _app: &App,
    _req: ChatCopyAndOpenReq,
) -> Result<ChatCopyResult, IpcError> {
    Err(IpcError::not_yet_available("chat_copy_and_open", "M3-23"))
}

/// `chat_link_set`: not built yet (M3-23; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-23`).
pub fn chat_link_set(_app: &App, _req: ChatLinkSetReq) -> Result<Rev, IpcError> {
    Err(IpcError::not_yet_available("chat_link_set", "M3-23"))
}

/// `paste_back_preview`: not built yet (M3-24; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-24`).
pub fn paste_back_preview(_app: &App, _req: PasteBackPreviewReq) -> Result<PastePreview, IpcError> {
    Err(IpcError::not_yet_available("paste_back_preview", "M3-24"))
}

/// `paste_back_apply`: not built yet (M3-24; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-24`).
pub fn paste_back_apply(_app: &App, _req: PasteBackApplyReq) -> Result<PasteApplyResult, IpcError> {
    Err(IpcError::not_yet_available("paste_back_apply", "M3-24"))
}
