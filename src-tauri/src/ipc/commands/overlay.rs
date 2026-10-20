//! Overlay additions (`docs/contracts/ipc.md` §4.14; ticket M4-31, rules in M4-30): a prompt or an
//! inspection item for one project. `overlay.json` is a decision file, so the write goes through
//! the same own-change path as a skip (committed by plumbing before Stage 5 is signed, pending
//! after; DR-153). Validation is the core's (DR-061).

use foreman_core::rules::overlay::{self, NewStep, OverlayError};
use foreman_core::schema::overlay::AddedTag as CoreTag;
use foreman_core::schema::progress::DecisionFileKind;

use crate::closeout;
use crate::ipc::IpcError;
use crate::ipc::commands::gates;
use crate::ipc::dto::app::ProjectView;
use crate::ipc::dto::gates::GateView;
use crate::ipc::dto::requests::{AddItemReq, AddStepReq, AddedTag};
use crate::project;
use crate::render::escape_visible;
use crate::state::App;

fn refusal(e: &OverlayError) -> IpcError {
    let rule = match e {
        OverlayError::BypassPermissions => "bypass-permissions",
        OverlayError::InvalidStart(_) => "invalid-start",
        OverlayError::Path(p) => return IpcError::from(p.clone()),
        OverlayError::UnknownStep(_) => {
            return IpcError::not_found("That step isn't in this project.");
        }
        OverlayError::WrongStage(..) => "wrong-stage",
        OverlayError::StageNotApplicable(_) => "stage-not-applicable",
        OverlayError::BadTitle => "bad-title",
        OverlayError::BadText => "bad-text",
        OverlayError::BadOut => "bad-out",
        OverlayError::TooMany => "too-many",
    };
    IpcError::not_allowed(rule, escape_visible(&e.to_string()))
}

/// `add_step` (P2): a prompt for this project only, `x-<stage>-<n>`, after the step Billy chose,
/// with the run config he edited (the insert point starts from a copied step's). It always starts
/// a new session and always confirms before running (DR-059, SR-19).
///
/// # Errors
/// `NotAllowed` (`bypass-permissions`, `invalid-start`, `stage-not-applicable`, `wrong-stage`,
/// `bad-title`, `bad-text`, `bad-out`, `too-many`), `PolicyViolation` (an `out` path), `NotFound`,
/// `ReadOnly`, `Conflict`.
pub fn add_step(app: &App, req: AddStepReq) -> Result<ProjectView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let (t, path, _) = p.parts()?;
    let new = NewStep {
        stage: req.stage.0,
        after: req.after.as_str().to_owned(),
        copied_from: req.copy_config_from.as_str().to_owned(),
        title: req.title.as_str().to_owned(),
        text: req.text.as_str().to_owned(),
        surface: req.config.surface.into(),
        cwd: req.config.cwd.map(Into::into),
        start: req.config.start.into(),
        out: req
            .config
            .out
            .iter()
            .map(|o| o.as_str().to_owned())
            .collect(),
        tag: match req.tag {
            AddedTag::Optional => CoreTag::Optional,
            AddedTag::Core => CoreTag::Core,
        },
    };
    let (next, _id) = overlay::add_step(t, path, &p.overlay, &new).map_err(|e| refusal(&e))?;
    p.own_change(app, DecisionFileKind::Overlay, &next)?;
    project::reopen_view(app, req.project_id.as_str())
}

/// `add_item` (P2): an inspection item for this project only, `xi-<stage>-<n>`, kind `attest`
/// (DR-060). Returns the stage's gate with the new item in it.
///
/// # Errors
/// `NotAllowed` (`stage-not-applicable`, `bad-text`, `bypass-permissions`, `too-many`),
/// `NotFound`, `ReadOnly`, `Conflict`.
pub fn add_item(app: &App, req: AddItemReq) -> Result<GateView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let (_, path, _) = p.parts()?;
    let (next, _id) = overlay::add_item(path, &p.overlay, req.stage.0, req.text.as_str())
        .map_err(|e| refusal(&e))?;
    p.own_change(app, DecisionFileKind::Overlay, &next)?;
    let p = project::open(app, req.project_id.as_str())?;
    let c = closeout::gather(app, &p, req.stage.0)?;
    Ok(gates::view(&p, &c))
}
