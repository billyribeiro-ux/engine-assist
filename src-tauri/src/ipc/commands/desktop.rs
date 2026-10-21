//! Desktop handoff, overlay additions and the run sheet (`docs/contracts/ipc.md` §4.14; F-024,
//! F-025, F-027; tickets M4-29, M4-31, M4-32).
//!
//! Desktop (ADR-008, DR-118, DR-122): `claude --desktop` for a `surface=desktop` step and
//! `claude --desktop --resume <uuid>` for a CLI chain run through the same allowlisted program,
//! environment allowlist and in-cwd auth check as a launch (AC-F024-01), behind the `--desktop`
//! version floor (AC-F024-04). After a handoff the chain follows S-23's branch
//! ([`DESKTOP_SHARE`], the specified `shared` branch until S-23 is recorded): `handed-off`, so the
//! next continuing step forks (AC-F010-15) unless Billy confirms Desktop is closed. No network.
//!
//! The run sheet (AC-F027-01…-03) is `foreman_core::runsheet`, written only to the path Billy
//! chose in the Rust save dialog, through the export path policy `metrics_export` uses.

#![allow(
    clippy::needless_pass_by_value,
    reason = "handlers take requests by value and return the IPC Result"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use foreman_core::chain::{self, DesktopShare};
use foreman_core::ids::Surface as CoreSurface;
use foreman_core::launch::argv;
use foreman_core::resolver::PacketText;
use foreman_core::sanitize;
use foreman_core::schema::progress::{ChainStatus, ChainSurface, ItemOutcome};
use foreman_core::steps::StepEvent;
use uuid::Uuid;

use crate::ipc::IpcError;
use crate::ipc::dto::app::ProjectView;
use crate::ipc::dto::gates::{DesktopOpened, GateView};
use crate::ipc::dto::launch::LaunchProgress;
use crate::ipc::dto::records::ExportResult;
use crate::ipc::dto::requests::{
    AddItemReq, AddStepReq, DesktopHandoffReq, DesktopOpenReq, RunSheetExportReq,
};
use crate::ipc::dto::steps::{ChatChip, ChatChips};
use crate::ipc::types::ToolKind;
use crate::os::Confirm;
use crate::project::{self, Opened, mode_name, model_name};
use crate::render::escape_visible;
use crate::state::App;

/// S-23's branch (pending Billy's run on his Mac; `Shared` is the specified pass branch, like
/// `ForkIds::Pass` for S-8).
pub const DESKTOP_SHARE: DesktopShare = DesktopShare::Shared;

/// Everything a Desktop spawn needs: the launch's spawn context and environment.
struct DesktopEnv {
    ctx: foreman_sys::spawn::SpawnCtx,
    env: Vec<(String, String)>,
}

/// The version floor, the launch environment and the auth check in `cwd` (AC-F024-01, -04).
fn preflight(
    app: &App,
    cwd: &Path,
    progress: &dyn Fn(LaunchProgress),
) -> Result<DesktopEnv, IpcError> {
    let (settings, _, _) = app.settings()?;
    let (reg, _) = app.registry()?;
    let ctx = app.spawn_ctx(&settings, &reg);
    let own = app.own_env();
    // 1. `claude -v` and the floor: both Desktop actions need `--desktop` (2.1.285).
    let v = foreman_sys::claude::version(&ctx, &own)
        .map_err(|_| IpcError::tool_missing(ToolKind::Claude, None))?;
    progress(LaunchProgress::Version {
        found: Some(escape_visible(&v.raw)),
    });
    // `--desktop`'s minimum (ADR-017's table, 2.1.285).
    let floor = foreman_core::compat::feature_min("--desktop");
    match (&v.version, &floor) {
        (Some(found), Some(min)) if foreman_core::compat::meets(found, min) => {
            app.inner().claude_version = Some(escape_visible(&v.raw));
        }
        (found, _) => {
            return Err(IpcError::not_allowed(
                "version-below-floor",
                format!(
                    "Open in Desktop needs Claude Code {} or later (claude --desktop); this Mac has {}. Update Claude Code, then try again.",
                    floor
                        .as_ref()
                        .map_or_else(|| "2.1.285".to_owned(), ToString::to_string),
                    found
                        .as_ref()
                        .map_or_else(|| "an unknown version".to_owned(), ToString::to_string)
                ),
            ));
        }
    }
    // 2. The launch environment (SR-05): the same allowlist and PATH as a CLI launch.
    let sources = crate::launch::path_sources(
        app,
        settings
            .login_path
            .as_ref()
            .map(|l| l.entries.clone())
            .unwrap_or_default(),
    );
    let env = foreman_sys::launch::child_env(
        &own,
        &settings.env_passthrough,
        &sources,
        &ctx.projects,
        Uuid::new_v4(),
    )
    .0;
    // 3. Auth in the step's cwd with that environment, fail closed (SR-08).
    let auth = foreman_sys::claude::auth_status(&ctx, cwd, &env);
    progress(LaunchProgress::Auth { cached: false });
    if let Err(problem) = &auth.result {
        return Err(IpcError::not_allowed(
            "auth-failed",
            format!(
                "Claude Code's sign-in check failed in this folder: {}. Foreman opens Desktop only with your subscription login.",
                escape_visible(&foreman_sys::launch::describe(problem))
            ),
        ));
    }
    Ok(DesktopEnv { ctx, env })
}

fn spawn_desktop(d: &DesktopEnv, cwd: &Path, args: Vec<String>) -> Result<(), IpcError> {
    let out = foreman_sys::claude::desktop(&d.ctx, cwd, &d.env, args).map_err(|e| {
        IpcError::tool_failed("claude", None, false, escape_visible(&e.to_string()))
    })?;
    if out.code == Some(0) {
        Ok(())
    } else {
        let err: String = String::from_utf8_lossy(&out.stderr)
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect();
        Err(IpcError::tool_failed(
            "claude",
            out.code,
            false,
            escape_visible(&err),
        ))
    }
}

/// The chips Billy picks in Desktop: model, effort and mode (AC-F024-01).
fn desktop_chips(p: &Opened, s: &foreman_core::template::Step) -> ChatChips {
    let mut chips = vec![ChatChip {
        id: "session".into(),
        label: "New Desktop session".into(),
        strong: true,
    }];
    let start = p.project.as_ref().and_then(|pr| {
        foreman_core::policy::policy_start(s, &pr.model_policy)
            .ok()
            .map(|e| e.value)
    });
    if let Some(st) = start {
        chips.push(ChatChip {
            id: "model".into(),
            label: model_name(st.model),
            strong: false,
        });
        chips.push(ChatChip {
            id: "effort".into(),
            label: format!("{} effort", st.effort.as_str()),
            strong: false,
        });
        if let Some(m) = st.mode {
            chips.push(ChatChip {
                id: "mode".into(),
                label: mode_name(m),
                strong: false,
            });
        }
    }
    ChatChips {
        chips,
        link: None,
        link_label: None,
    }
}

/// `desktop_open` (AC-F024-01, -04): for a `surface=desktop` step, the resolved prompt is
/// copied (sanitized, SR-28) and `claude --desktop` runs in the step's cwd with no other flags,
/// after the version floor, the launch environment and the auth check. Returns the chips for
/// the model, effort and mode to pick in Desktop.
///
/// # Errors
/// `ToolMissing`, `NotAllowed("not-desktop-step" | "version-below-floor" | "auth-failed" |
/// "prompt-unresolved")`, `ToolFailed`, `ReadOnly`, `NotFound`.
pub fn desktop_open(
    app: &App,
    req: DesktopOpenReq,
    progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<DesktopOpened, IpcError> {
    let send = |m: LaunchProgress| {
        let _ = progress.send(m);
    };
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let s = p.step(req.step.as_str())?.clone();
    if s.surface != Some(CoreSurface::Desktop) {
        return Err(IpcError::not_allowed(
            "not-desktop-step",
            "This step doesn't run in Claude Desktop.",
        ));
    }
    let r = crate::ipc::commands::steps::resolve(&p, &s)?;
    let Ok(resolved) = r.result else {
        return Err(IpcError::not_allowed(
            "prompt-unresolved",
            "The prompt still has unresolved parts. Fill them in on the step first.",
        ));
    };
    let (cwd, _) = crate::launch::step_cwd(&p, &s)?;
    let d = preflight(app, &cwd, &send)?;
    let (clean, _) = sanitize::for_paste(&resolved.text);
    app.os()
        .clipboard_write(&clean)
        .map_err(|e| IpcError::io(format!("The clipboard refused the prompt: {e}")))?;
    send(LaunchProgress::Spawning);
    spawn_desktop(&d, &cwd, argv::desktop_open().as_slice().to_vec())?;
    send(LaunchProgress::Done);
    // Like Copy and open: the step is `copied` until Billy marks it done.
    {
        let _w = app.writing();
        if crate::ipc::commands::steps::apply_event(
            app,
            &mut p,
            s.key.as_str(),
            &StepEvent::Copy,
            "not-runnable",
        )
        .is_ok()
        {
            p.save_progress(app)?;
        }
    }
    Ok(DesktopOpened {
        chips: desktop_chips(&p, &s),
    })
}

/// The native question for a chain that is already handed off: is Claude Desktop closed?
#[must_use]
pub fn desktop_closed_confirm() -> Confirm {
    Confirm::new(
        "Is Claude Desktop closed?",
        "This session was handed off to Claude Desktop. If Desktop is closed, Foreman continues \
         the same session here; otherwise the next step forks it into a new session so the two \
         don't write to one transcript.",
        "Desktop is closed",
    )
}

/// `desktop_handoff` (AC-F024-02, -03, -04): `claude --desktop --resume <uuid>` for a CLI chain
/// that isn't live, after the floor, environment and auth checks in the chain's cwd; then the
/// chain follows S-23's branch (shared: `handed-off`). Called for a chain that is already handed
/// off, it asks natively whether Desktop is closed and, on OK, opens the chain again so the next
/// continuing step resumes it instead of forking (DR-110's exception, AC-F010-15).
///
/// # Errors
/// `NotFound`, `ToolMissing`, `NotAllowed("chain-live" | "not-cli-chain" |
/// "version-below-floor" | "auth-failed")`, `Cancelled`, `ToolFailed`.
pub fn desktop_handoff(app: &App, req: DesktopHandoffReq) -> Result<(), IpcError> {
    let id = req.chain.as_str();
    if app.runs().of_chain(id).is_some() {
        return Err(IpcError::not_allowed(
            "chain-live",
            "This session is still running. End it first, then open it in Desktop.",
        ));
    }
    let (reg, _) = app.registry()?;
    for e in &reg.projects {
        let Ok(mut p) = project::open(app, &e.project_id) else {
            continue;
        };
        let Some(c) = p.progress.chains.iter().find(|c| c.id == id).cloned() else {
            continue;
        };
        p.writable()?;
        if c.surface != ChainSurface::Cli {
            return Err(IpcError::not_allowed(
                "not-cli-chain",
                "Only a Claude Code session can be opened in Desktop.",
            ));
        }
        let uuid = Uuid::parse_str(&c.id)
            .map_err(|_| IpcError::not_found("That session chain is damaged."))?;
        if c.status == ChainStatus::HandedOff {
            if !app.os().confirm(&desktop_closed_confirm()) {
                return Err(IpcError::cancelled());
            }
            let _w = app.writing();
            p.progress.chains = chain::desktop_closed(&p.progress.chains, uuid);
            p.save_progress(app)?;
            return Ok(());
        }
        let root = p.root()?.path().to_path_buf();
        let cwd = foreman_sys::spawn::canonical(&match c.cwd {
            Some(foreman_core::ids::CwdKind::Spec) => root.join("spec"),
            _ => root,
        });
        let d = preflight(app, &cwd, &|_| {})?;
        let a = argv::desktop(uuid)
            .map_err(|e| IpcError::tool_failed("claude", None, false, e.to_string()))?;
        spawn_desktop(&d, &cwd, a.as_slice().to_vec())?;
        let _w = app.writing();
        let mut p = project::open(app, &e.project_id)?;
        p.progress.chains = chain::after_handoff(&p.progress.chains, uuid, DESKTOP_SHARE);
        p.save_progress(app)?;
        return Ok(());
    }
    Err(IpcError::not_found(
        "That session chain isn't in any project.",
    ))
}

/// `add_step` (M4-31): in [`super::overlay`].
///
/// # Errors
/// See [`super::overlay::add_step`].
pub fn add_step(app: &App, req: AddStepReq) -> Result<ProjectView, IpcError> {
    super::overlay::add_step(app, req)
}

/// `add_item` (M4-31): in [`super::overlay`].
///
/// # Errors
/// See [`super::overlay::add_item`].
pub fn add_item(app: &App, req: AddItemReq) -> Result<GateView, IpcError> {
    super::overlay::add_item(app, req)
}

/// The run sheet's Markdown for one stage of an open project (AC-F027-01, -03).
///
/// # Errors
/// `ReadOnly` without a template, `NotFound` for a stage the template doesn't have.
pub fn run_sheet_text(app: &App, p: &Opened, stage: u8) -> Result<String, IpcError> {
    let (t, path, project) = p.parts()?;
    if !t.stages.iter().any(|s| s.number == stage) {
        return Err(IpcError::not_found("That stage isn't in this blueprint."));
    }
    let signed = p.signed();
    let packets: BTreeMap<u8, PacketText> = p
        .packets
        .iter()
        .map(|(n, f)| {
            (
                *n,
                PacketText {
                    text: f.text.clone(),
                    signed: signed.contains(n),
                },
            )
        })
        .collect();
    let inputs: BTreeMap<String, BTreeMap<String, String>> = p
        .progress
        .run_inputs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let optional: BTreeMap<String, BTreeMap<String, bool>> = p
        .progress
        .optional_parts
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let attested_na: BTreeSet<String> = p
        .progress
        .gate_runs
        .values()
        .flat_map(|g| g.items.iter())
        .filter(|(_, r)| r.outcome == ItemOutcome::NotApplicable)
        .map(|(id, _)| id.to_string())
        .collect();
    let today: String = app.now().chars().take(10).collect();
    Ok(foreman_core::runsheet::run_sheet(
        &foreman_core::runsheet::SheetInput {
            template: t,
            project,
            path,
            packets: &packets,
            inputs: &inputs,
            remembered: &p.progress.remembered,
            optional: &optional,
            attested_na: &attested_na,
            overlay_items: &p.overlay.added_items,
            stage,
            generated_on: &today,
        },
    ))
}

/// `run_sheet_export` (AC-F027-01…-03, NFR-14): the stage's run sheet, written only to the
/// path Billy chose in the Rust save dialog, through the same export path policy as
/// `metrics_export` (`WritePurpose::UserExport`: `.md` only, never inside `.git`, `.claude` or
/// `.foreman`, never `CLAUDE.md`, never app data). Cancel writes nothing. Unresolved tokens are
/// marked, never refused.
///
/// # Errors
/// `NotFound`, `ReadOnly`, `PolicyViolation` (the chosen path), `Io`.
pub fn run_sheet_export(app: &App, req: RunSheetExportReq) -> Result<ExportResult, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    let text = run_sheet_text(app, &p, req.stage.0)?;
    let slug = p.project.as_ref().map_or("project", |pr| pr.slug.as_str());
    let name = format!("{slug}-stage-{:02}-run-sheet.md", req.stage.0);
    let Some(path) = app.os().save("Export run sheet", &name) else {
        return Ok(ExportResult::Cancelled);
    };
    foreman_core::store::export::write_user_export(&path, text.as_bytes(), &app.paths().app_data)?;
    Ok(ExportResult::Saved {
        display_path: app.display(Path::new(&path)),
    })
}
