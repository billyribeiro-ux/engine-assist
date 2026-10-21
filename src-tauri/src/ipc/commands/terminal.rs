//! Terminal (`docs/contracts/ipc.md` §4.7; F-009, F-010; tickets M3-10…M3-15): the PTY
//! Channels, live-only writes (SR-24), sanitized paste (SR-28), Terminate and the re-arm, whose
//! idle assertion and Terminate are native dialogs (SR-13, SR-24a). Only these handlers create a
//! `UserAction` (`rearm::only_from_user_action`).

use std::cell::RefCell;

use foreman_core::rearm::{IdleSource, UserAction};
use foreman_core::schema::run::RunKind;
use foreman_sys::launch::PrepareOutcome as SysOutcome;
use foreman_sys::pty::{self, PtyError, PtyStatus};
use foreman_sys::rearm::{self, Hooks, RearmError, RearmOutcome as SysRearm, RearmUi};
use tauri::ipc::{Channel, Response};

use crate::convert;
use crate::ipc::IpcError;
use crate::ipc::dto::launch::{
    AttachInfo, LaunchProgress, LinkOutcome, PasteText, RearmOutcome, RunStarted,
};
use crate::ipc::dto::requests::{
    PtyAttachReq, PtyFlowReq, PtyInterruptReq, PtyPasteReq, PtyResizeReq, PtyTerminateReq,
    PtyWriteReq, RearmContinueReq, RearmStartReq, TerminalLinkOpenReq,
};
use crate::ipc::error::StaleWhat;
use crate::ipc::types::{ChainId, RunId, RunStatus};
use crate::launch::{self, HeldPlan};
use crate::os::Confirm;
use crate::project;
use crate::runs::LiveRun;
use crate::state::App;

fn live(app: &App, run_id: &str) -> Result<std::sync::Arc<LiveRun>, IpcError> {
    app.runs()
        .get(run_id)
        .ok_or_else(|| IpcError::not_found("That session isn't open in this Foreman."))
}

fn not_live() -> IpcError {
    IpcError::not_allowed("pty-not-live", "The session isn't running.")
}

fn pty_error(e: &PtyError) -> IpcError {
    match e {
        PtyError::NotLive => not_live(),
        PtyError::TooLarge => IpcError::invalid("data", "ptyData"),
        PtyError::BadSize => IpcError::invalid("req", "size"),
        other => IpcError::io(other.to_string()),
    }
}

fn status_of(app: &App, r: &LiveRun) -> RunStatus {
    match r.session.status() {
        // P2: the sensor's idle/blocked for a live session (display only).
        PtyStatus::Running => crate::sensor_ingest::live_status(app, &r.run_id),
        PtyStatus::Exited(i) if i.code.is_some() => RunStatus::Exited,
        PtyStatus::Exited(_) => RunStatus::Errored,
    }
}

/// `pty_attach` (P1): replays the last 2,000 lines, then streams live (§5: raw bytes).
///
/// # Errors
/// `NotFound`.
pub fn pty_attach(
    app: &App,
    req: PtyAttachReq,
    output: Channel<Response>,
) -> Result<AttachInfo, IpcError> {
    let r = live(app, req.run_id.as_str())?;
    let (tail, lines) = r.session.tail();
    if !tail.is_empty() {
        let _ = output.send(Response::new(tail));
    }
    r.outputs.attach(output);
    Ok(AttachInfo {
        status: status_of(app, &r),
        replayed_lines: lines,
        tab_name: r.tab_name.clone(),
    })
}

/// `pty_write` (P1): only into a live session (SR-24; the residual RR-03).
///
/// # Errors
/// `NotAllowed("pty-not-live")`, `NotFound`.
pub fn pty_write(app: &App, req: PtyWriteReq) -> Result<(), IpcError> {
    let r = live(app, req.run_id.as_str())?;
    r.session
        .write(req.data.as_str().as_bytes())
        .map_err(|e| pty_error(&e))
}

/// `pty_paste` (P1): the clipboard as text, controls removed with a count (SR-28, SR-56); the
/// webview pastes it with `term.paste`.
///
/// # Errors
/// `Clipboard`, `NotAllowed("pty-not-live")`, `NotFound`.
pub fn pty_paste(app: &App, req: PtyPasteReq) -> Result<PasteText, IpcError> {
    let r = live(app, req.run_id.as_str())?;
    if !r.session.is_live() {
        return Err(not_live());
    }
    let text = app.os().clipboard_read().map_err(IpcError::clipboard)?;
    let clean = pty::sanitize_paste(&text);
    Ok(PasteText {
        text: clean.text,
        removed_controls: u32::try_from(clean.removed).unwrap_or(u32::MAX),
    })
}

/// `pty_resize` (P1): 2–1000 columns, 2–500 rows.
///
/// # Errors
/// `Invalid`, `NotFound`, `NotAllowed("pty-not-live")`.
pub fn pty_resize(app: &App, req: PtyResizeReq) -> Result<(), IpcError> {
    let size = pty::size(req.cols, req.rows).map_err(|e| pty_error(&e))?;
    let r = live(app, req.run_id.as_str())?;
    r.session.resize(size).map_err(|e| pty_error(&e))
}

/// `pty_flow` (P1): pause or resume output (NFR-08 back-pressure).
///
/// # Errors
/// `NotFound`.
pub fn pty_flow(app: &App, req: PtyFlowReq) -> Result<(), IpcError> {
    live(app, req.run_id.as_str())?.session.set_flow(req.paused);
    Ok(())
}

/// `pty_interrupt` (P1): Ctrl-C into a live session.
///
/// # Errors
/// `NotAllowed("pty-not-live")`, `NotFound`.
pub fn pty_interrupt(app: &App, req: PtyInterruptReq) -> Result<(), IpcError> {
    live(app, req.run_id.as_str())?
        .session
        .interrupt()
        .map_err(|e| pty_error(&e))
}

/// `pty_terminate` (P1): after the native Terminate confirmation (SR-13, SR-24a), SIGTERM to the
/// process group and SIGKILL after 5 s. Cancel sends nothing.
///
/// # Errors
/// `Cancelled`, `NotFound`.
pub fn pty_terminate(app: &App, req: PtyTerminateReq) -> Result<(), IpcError> {
    let r = live(app, req.run_id.as_str())?;
    if !r.session.is_live() {
        return Ok(());
    }
    let ok = app.os().confirm(&Confirm::new(
        format!("End {}?", r.tab_name),
        "Claude Code stops at once; unsaved work in the session is lost. You can resume the session later.",
        "Terminate",
    ));
    if !ok {
        return Err(IpcError::cancelled());
    }
    r.session.terminate(pty::TERMINATE_GRACE);
    Ok(())
}

struct Ui<'a> {
    app: &'a App,
    progress: &'a Channel<LaunchProgress>,
    /// Billy chose Wait in the busy dialog (P2, AC-F010-14).
    waited: std::cell::Cell<bool>,
}

impl RearmUi for Ui<'_> {
    fn confirm_idle(&self) -> bool {
        self.app.os().confirm(&Confirm::new(
            "Is Claude idle?",
            "Re-arming ends this session with Ctrl-C and /exit, then starts the next step. Continue only if Claude is idle and nothing is running in the background.",
            "Continue",
        ))
    }

    fn confirm_busy(&self, tasks: u32) -> bool {
        let ok = self.app.os().confirm(&Confirm::new(
            format!(
                "{tasks} background {} still running",
                if tasks == 1 { "task" } else { "tasks" }
            ),
            "Ending the session now stops them. Wait lets them finish; press Run again when Claude is idle.",
            "End anyway",
        ));
        self.waited.set(!ok);
        ok
    }

    fn end_yourself(&self) {}

    fn progress(&self, p: foreman_core::launch::LaunchProgress) {
        let _ = self.progress.send(convert::progress(&p));
    }
}

/// Runs the re-arm driver with the app's hooks; shared by `rearm_start` and `rearm_continue`.
fn drive(
    app: &App,
    held: &HeldPlan,
    chain: &str,
    output: Channel<Response>,
    progress: &Channel<LaunchProgress>,
    first: bool,
) -> Result<RearmOutcome, IpcError> {
    let old = app.runs().of_chain(chain);
    // The old session's exit rewrites `progress.json` (DR-065), so each hook reopens the project
    // rather than holding a stale read.
    let title = project::open(app, &held.project_id)?
        .step(&held.step)
        .map(|s| s.title.clone())
        .unwrap_or_default();
    let fresh: RefCell<Option<launch::Prepared>> = RefCell::new(None);
    let started: RefCell<Option<launch::Started>> = RefCell::new(None);
    let mut prepare_again = || -> SysOutcome {
        let Ok(mut p) = project::open(app, &held.project_id) else {
            return SysOutcome::Blocked {
                blockers: Vec::new(),
                warnings: Vec::new(),
            };
        };
        match launch::prepare(
            app,
            &mut p,
            &held.step,
            held.entry,
            held.mode,
            held.resume,
            &|_| {},
        ) {
            Ok(pr) => {
                let out = match &pr.held {
                    Some(h) => SysOutcome::Ready(Box::new(h.plan.clone())),
                    None => SysOutcome::Blocked {
                        blockers: Vec::new(),
                        warnings: Vec::new(),
                    },
                };
                *fresh.borrow_mut() = Some(pr);
                out
            }
            Err(_) => SysOutcome::Blocked {
                blockers: Vec::new(),
                warnings: Vec::new(),
            },
        }
    };
    let divider = foreman_core::ids::StepKey::parse(&held.step)
        .map(|k| foreman_core::rearm::divider(&k, &title, &held.intended))
        .unwrap_or_default();
    let out_for_archive = output.clone();
    let mut archive = || {
        let _ = out_for_archive.send(Response::new(format!("\r\n{divider}\r\n").into_bytes()));
    };
    let out_cell = RefCell::new(Some(output));
    let mut relaunch = |plan: &foreman_sys::launch::LaunchPlan| -> Result<pty::PtySession, String> {
        // C2: the relaunch writes `progress.json` like any launch, under the write lock.
        let w = app.writing();
        let mut p = project::open(app, &held.project_id).map_err(|e| e.message)?;
        let mut h = held.clone();
        h.plan = plan.clone();
        let s = launch::start(
            app,
            &w,
            &mut p,
            &h,
            RunKind::Rearm,
            out_cell.borrow_mut().take(),
        )
        .map_err(|e| e.message)?;
        let session = app
            .runs()
            .get(&s.run_id)
            .map(|r| r.session.clone())
            .ok_or_else(|| "the new session isn't registered".to_owned())?;
        *started.borrow_mut() = Some(s);
        Ok(session)
    };
    let ui = Ui {
        app,
        progress,
        waited: std::cell::Cell::new(false),
    };
    // ADR-008 P2: the sensor's idle (AC-F010-13) or busy (AC-F010-14) answer, read now that Billy
    // pressed Run; without a sensor or its events, his native confirmation (AC-F011-09).
    let idle = old.as_ref().map_or(IdleSource::Billy, |r| {
        crate::sensor_ingest::idle_source(app, &r.run_id)
    });
    let mut hooks = Hooks {
        prepare_again: &mut prepare_again,
        archive: &mut archive,
        relaunch: &mut relaunch,
    };
    let result = if first {
        rearm::rearm_start(
            UserAction::from_ipc_invoke(),
            old.as_ref().map(|r| &r.session),
            idle,
            &held.plan,
            &ui,
            &mut hooks,
        )
    } else {
        rearm::rearm_continue(
            UserAction::from_ipc_invoke(),
            old.as_ref().map(|r| &r.session),
            &held.plan,
            &ui,
            &mut hooks,
        )
    };
    match result {
        Ok(SysRearm::Relaunched { .. }) => {
            let s = started.into_inner().ok_or_else(IpcError::internal)?;
            Ok(RearmOutcome::Relaunched(RunStarted {
                run_id: RunId::from_trusted(s.run_id),
                chain: ChainId::from_trusted(s.chain),
                tab_name: s.tab_name,
            }))
        }
        Ok(SysRearm::EndYourself) => {
            app.launch().rearms.insert(chain.to_owned(), held.clone());
            Ok(RearmOutcome::EndYourself {
                chain: ChainId::from_trusted(chain),
            })
        }
        Ok(SysRearm::Cancelled) if ui.waited.get() => Err(IpcError::not_allowed(
            "background-tasks-running",
            "Background tasks are still running. Press Run again when Claude is idle.",
        )),
        Ok(SysRearm::Cancelled) => Err(IpcError::cancelled()),
        Ok(SysRearm::Stale(_)) => {
            let fresh = fresh
                .into_inner()
                .map(|pr| launch::hold(app, pr))
                .and_then(|o| serde_json::to_value(o).ok());
            Err(IpcError::stale(
                StaleWhat::Plan,
                "The next launch changed while the session ended. Review it again.",
                fresh,
            ))
        }
        Err(RearmError::Rejected(_)) if !first => {
            app.launch().rearms.insert(chain.to_owned(), held.clone());
            Err(IpcError::not_allowed(
                "process-still-alive",
                "Claude is still running. End it yourself, then press Continue.",
            ))
        }
        Err(RearmError::Rejected(_)) => Err(IpcError::not_allowed(
            "no-rearm-needed",
            "This session doesn't need a re-arm.",
        )),
        Err(RearmError::Relaunch(e)) => Err(IpcError::tool_failed(
            "claude",
            None,
            false,
            crate::render::escape_visible(&e),
        )),
        Err(RearmError::Pty(e)) => Err(pty_error(&e)),
    }
}

/// `rearm_start` (P1): Billy's native idle assertion, then Ctrl-C, `/exit`, ≤ 10 s, the same
/// preparation again (a different argv or prompt is `Stale`, DR-116), then the relaunch.
///
/// # Errors
/// `Expired`, `Stale`, `Cancelled`, `NotAllowed("no-rearm-needed")`, `ToolFailed`.
pub fn rearm_start(
    app: &App,
    req: RearmStartReq,
    output: Channel<Response>,
    progress: Channel<LaunchProgress>,
) -> Result<RearmOutcome, IpcError> {
    let held = app.launch().plans.take(req.plan_id.as_str())?;
    let Some(chain) = held.rearm.clone() else {
        return Err(IpcError::not_allowed(
            "no-rearm-needed",
            "This launch doesn't continue a running session.",
        ));
    };
    drive(app, &held, &chain, output, &progress, true)
}

/// `rearm_continue` (P1): after "End it yourself, then press Continue", only once the old
/// process is gone.
///
/// # Errors
/// `Expired`, `NotAllowed("process-still-alive")`, `Stale`, `ToolFailed`.
pub fn rearm_continue(
    app: &App,
    req: RearmContinueReq,
    output: Channel<Response>,
    progress: Channel<LaunchProgress>,
) -> Result<RearmOutcome, IpcError> {
    let Some(held) = app.launch().rearms.remove(&req.chain.as_str().to_owned()) else {
        return Err(IpcError::expired());
    };
    drive(app, &held, req.chain.as_str(), output, &progress, false)
}

/// `terminal_link_open` (P1, M3-12): a link clicked in a terminal (OSC 8 or a detected URL) is
/// parsed in Rust (`foreman_core::urls::terminal_url`: `https:`, or `http:` to localhost or
/// 127.0.0.1, no userinfo); any other link is refused without a dialog. An allowed link opens in
/// the default browser through `foreman-sys`'s `open` only after Billy confirms a native dialog
/// showing the full URL with hidden characters made visible (SR-29, SR-26, SR-24a). Nothing opens
/// in the webview.
///
/// # Errors
/// `NotFound` (unknown run), `Invalid` (over 8 KB), `ToolFailed` (`open` failed).
pub fn terminal_link_open(app: &App, req: TerminalLinkOpenReq) -> Result<LinkOutcome, IpcError> {
    let r = live(app, req.run_id.as_str())?;
    if req.url.len() > foreman_core::urls::MAX_URL {
        return Err(IpcError::invalid("url", "tooLong"));
    }
    let Ok(url) = foreman_core::urls::terminal_url(&req.url) else {
        return Ok(LinkOutcome::Refused);
    };
    let ok = app.os().confirm(&Confirm::new(
        "Open this link in your browser?",
        format!(
            "A link in {} wants to open:\n\n{}\n\nOpen it only if you expected this address.",
            crate::render::escape_visible(&r.tab_name),
            crate::render::escape_visible(url.as_str())
        ),
        "Open link",
    ));
    if !ok {
        return Ok(LinkOutcome::Cancelled);
    }
    let ctx = app.spawn_ctx_now()?;
    app.os()
        .open_url(&url, &ctx)
        .map_err(|_| IpcError::tool_failed("open", None, false, String::new()))?;
    Ok(LinkOutcome::Opened)
}
