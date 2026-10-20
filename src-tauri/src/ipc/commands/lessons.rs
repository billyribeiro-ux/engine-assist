//! `lessons_status` (`docs/contracts/ipc.md` §4.13; F-016, M4-18; DR-074). Appending goes through
//! `paste_back_preview { kind: "lessons" }` / `paste_back_apply` with
//! `foreman_core::lessons::append` (DR-138).

#![allow(
    clippy::needless_pass_by_value,
    reason = "handlers take requests by value and return the IPC Result"
)]

use foreman_core::lessons;

use crate::ipc::IpcError;
use crate::ipc::dto::gates::{LessonRef, LessonsStatus};
use crate::ipc::dto::requests::LessonsStatusReq;
use crate::project;
use crate::render::escape_visible;
use crate::state::App;

/// `lessons_status` (mutates nothing): whether `docs/lessons.md` has an entry for the stage
/// (`## L-NNN · Stage N · …`; "clean stage" counts; another stage's doesn't), its entries, the next
/// free ID, and whether the file exists.
///
/// # Errors
/// `NotFound` (unknown project or a stage above 17), `ReadOnly` (folder missing), `Io`.
pub fn lessons_status(app: &App, req: LessonsStatusReq) -> Result<LessonsStatus, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    let n = req.stage.0;
    if n > 17 {
        return Err(IpcError::not_found(
            "That stage has no lessons requirement.",
        ));
    }
    let root = p.root()?;
    let Some((text, _)) = lessons::read(root)? else {
        return Ok(LessonsStatus {
            met: false,
            entries: Vec::new(),
            next_id: "L-001".to_owned(),
            file_present: false,
        });
    };
    let s = lessons::status(&text, n);
    Ok(LessonsStatus {
        met: s.met,
        entries: s
            .entries
            .iter()
            .map(|e| LessonRef {
                id: e.id.clone(),
                title: escape_visible(&e.title),
            })
            .collect(),
        next_id: s.next_id,
        file_present: true,
    })
}
