//! Files (`docs/contracts/ipc.md` §4.8; tickets M2-13, M4-33): opening a repo file in Billy's
//! editor by bundle ID and revealing it in Finder, through `foreman-sys::spawn` (SR-25a). The file
//! is read through the store first, so symlinks, hard links and non-regular files are refused
//! before anything opens it (SR-31).

use std::path::Path;

use foreman_core::pathpolicy::{self, ReadClass};
use foreman_sys::spawn::SpawnRequest;

use crate::ipc::IpcError;
use crate::ipc::dto::requests::{FileOpenReq, FileRevealReq, TerminalOpenHereReq};
use crate::project;
use crate::render::escape_visible;
use crate::state::App;

/// SR-25a: only these open in an editor (never an executable type).
pub const OPENABLE: [&str; 3] = ["md", "json", "txt"];

fn checked(app: &App, project_id: &str, path: &str) -> Result<std::path::PathBuf, IpcError> {
    let p = project::open(app, project_id)?;
    let rel = pathpolicy::check_read(path, ReadClass::Decision)?;
    let root = p.root()?;
    if !root.exists(&rel) {
        return Err(IpcError::not_found(format!(
            "{} doesn't exist.",
            escape_visible(path)
        )));
    }
    // The same walk as every read: no symlink at any component, regular file, one link.
    root.read(&rel, ReadClass::Decision)?;
    Ok(root.path().join(rel.as_str()))
}

fn opener(app: &App, req: &SpawnRequest) -> Result<(), IpcError> {
    let ctx = app.spawn_ctx_now()?;
    app.os()
        .open_request(req, &ctx)
        .map_err(|e| IpcError::tool_failed("open", None, e.starts_with("timed out"), String::new()))
}

/// `file_open` (P1): `/usr/bin/open -b <editor> <file>` for `.md`, `.json` and `.txt` only.
///
/// # Errors
/// `PolicyViolation` (read policy, extension, symlink), `NotAllowed("no-editor-set")`,
/// `NotFound`, `ToolFailed`.
pub fn file_open(app: &App, req: FileOpenReq) -> Result<(), IpcError> {
    let ext = Path::new(req.path.as_str())
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if !ext.as_deref().is_some_and(|e| OPENABLE.contains(&e)) {
        return Err(IpcError::policy(req.path.as_str(), "open.extension"));
    }
    let (s, _, _) = app.settings()?;
    let Some(bundle) = s.editor_bundle_id else {
        return Err(IpcError::not_allowed(
            "no-editor-set",
            "Choose your editor in Settings first.",
        ));
    };
    let abs = checked(app, req.project_id.as_str(), req.path.as_str())?;
    // AC-F007-04: Open file on a manual step's file starts that step.
    {
        let _w = app.writing();
        let mut p = project::open(app, req.project_id.as_str())?;
        crate::ipc::commands::steps::manual_start_on_open(app, &mut p, req.path.as_str())?;
    }
    opener(app, &SpawnRequest::OpenWithBundle { bundle, path: abs })
}

/// `file_reveal` (P1): `/usr/bin/open -R <file>`.
///
/// # Errors
/// `PolicyViolation`, `NotFound`, `ToolFailed`.
pub fn file_reveal(app: &App, req: FileRevealReq) -> Result<(), IpcError> {
    let abs = checked(app, req.project_id.as_str(), req.path.as_str())?;
    opener(app, &SpawnRequest::OpenReveal(abs))
}

/// `terminal_open_here` (P2): Terminal, by bundle ID, at the project root
/// (`/usr/bin/open -b com.apple.Terminal <root>`; AC-F014-11, SR-25a). No shell and no command
/// is typed into it.
///
/// # Errors
/// `NotFound` (unknown project or missing folder), `ReadOnly`, `ToolFailed`.
pub fn terminal_open_here(app: &App, req: TerminalOpenHereReq) -> Result<(), IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    let dir = p.root()?.path().to_path_buf();
    if !dir.is_dir() {
        return Err(IpcError::not_found(
            "The project folder isn't there any more.",
        ));
    }
    let ctx = app.spawn_ctx_now()?;
    app.os()
        .open_terminal(&dir, &ctx)
        .map_err(|_| IpcError::tool_failed("open", None, false, String::new()))
}
