//! `doctor` commands (`docs/contracts/ipc.md` §4).

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "handlers take requests by value and return the IPC Result"
)]

use std::time::Duration;

use foreman_sys::spawn::{self, Cwd, Program, RunOpts, SpawnRequest};

use crate::ipc::IpcError;
use crate::ipc::dto::app::SettingsView;
use crate::ipc::dto::doctor::{
    CapabilityRow, CheckGroup, ContractResult, DoctorCheck, DoctorReport, LoginPathPreview,
};
use crate::ipc::dto::events::{DoctorStatus, Event};
use crate::ipc::dto::launch::{LaunchProgress, RunStarted};
use crate::ipc::dto::requests::{
    DoctorCaptureLoginPathReq, DoctorLoginPathApplyReq, DoctorRunReq, ProbeFolderOpenReq,
    ProbeRunReq,
};
use crate::ipc::types::{DoctorLevel, Timestamp};
use crate::render::{escape_visible, spans};
use crate::state::App;

/// The Claude Code floor (CLAUDE.md: ≥ 2.1.293; the highest minimum in the compat table).
fn floor() -> String {
    foreman_core::compat::TABLE
        .iter()
        .map(|(_, v)| *v)
        .max_by(|a, b| version_key(a).cmp(&version_key(b)))
        .unwrap_or("0.0.0")
        .to_owned()
}

fn version_key(v: &str) -> Vec<u64> {
    v.split(['.', '-', '+'])
        .take(3)
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

fn run_version(app: &App, req: &SpawnRequest, git: &std::path::Path) -> Result<String, String> {
    let ctx = app.spawn_ctx_now().map_err(|e| e.message)?;
    let opts = RunOpts {
        env: foreman_sys::git::env(&app.paths().home, git),
        timeout: Duration::from_secs(10),
        output_cap: 64 * 1024,
        cwd: Cwd::AppData,
    };
    match spawn::run(req, &opts, &ctx) {
        Ok(out) if out.code == Some(0) => Ok(escape_visible(
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or_default()
                .trim(),
        )),
        Ok(out) => Err(format!(
            "exited with {}",
            out.code.map_or("a signal".to_owned(), |c| c.to_string())
        )),
        Err(e) => Err(super::settings::spawn_problem(&e)),
    }
}

fn check(
    id: &str,
    name: &str,
    level: DoctorLevel,
    summary: Option<String>,
    detail: &str,
    found: Option<String>,
) -> DoctorCheck {
    DoctorCheck {
        id: id.to_owned(),
        group: CheckGroup::Claude,
        name: name.to_owned(),
        level,
        summary,
        detail: spans(detail),
        found_mono: found.is_some(),
        found,
        action: None,
    }
}

/// `doctor_run` (P1: `claude` path and version against the floor, `git --version`; P2 adds the
/// toolchain, auth, the capability probes and the login PATH). Failures are rows, never errors.
/// `claude auth status` runs only after the configuration fingerprint passes (DR-120), which
/// lands with M3-01/M3-04, so its row says when it will be checked. Emits `doctor:status`.
///
/// # Errors
/// None in practice (`Io` if settings are unreadable).
pub fn doctor_run(app: &App, req: DoctorRunReq) -> Result<DoctorReport, IpcError> {
    let DoctorRunReq { scope: _ } = req;
    let ctx = app.spawn_ctx_now()?;
    let mut checks = Vec::new();
    let floor = floor();
    let claude = spawn::resolve(Program::Claude, &ctx);
    let git = foreman_sys::git::resolve(&ctx);
    let git_path = git
        .clone()
        .unwrap_or_else(|_| std::path::PathBuf::from("/usr/bin/git"));
    let mut version = None;
    match &claude {
        Err(e) => checks.push(check(
            "claude-path",
            "Claude Code",
            DoctorLevel::Failing,
            Some(super::settings::spawn_problem(e)),
            "Install Claude Code, or set its path in Settings.",
            None,
        )),
        Ok(p) => match run_version(app, &SpawnRequest::ClaudeVersion, &git_path) {
            Ok(v) => {
                let parsed = foreman_core::compat::parse_version_output(&v);
                let ok = parsed
                    .as_ref()
                    .is_some_and(|x| version_key(&x.to_string()) >= version_key(&floor));
                version = parsed.map(|x| x.to_string());
                checks.push(check(
                    "claude-version",
                    "Claude Code",
                    if ok {
                        DoctorLevel::Ok
                    } else {
                        DoctorLevel::Failing
                    },
                    (!ok).then(|| format!("Needs {floor} or newer.")),
                    &format!("{} at {}", v, escape_visible(&app.display(p))),
                    Some(v.clone()),
                ));
            }
            Err(why) => checks.push(check(
                "claude-version",
                "Claude Code",
                DoctorLevel::Failing,
                Some(format!("claude -v failed: {why}")),
                "Reinstall Claude Code, or set its path in Settings.",
                None,
            )),
        },
    }
    checks.push(check(
        "claude-auth",
        "Sign-in",
        DoctorLevel::Unknown,
        Some("Checked before the first launch.".to_owned()),
        "Foreman runs claude auth status only after the configuration check passes (subscription sign-in only).",
        None,
    ));
    match &git {
        Err(e) => checks.push(DoctorCheck {
            group: CheckGroup::Toolchain,
            ..check(
                "git",
                "git",
                DoctorLevel::Failing,
                Some(super::settings::spawn_problem(e)),
                "Install git (xcode-select --install), or set its path in Settings.",
                None,
            )
        }),
        Ok(_) => {
            let r = foreman_sys::git::version(&app.paths().home, &ctx)
                .map(|v| escape_visible(&v))
                .map_err(|e| escape_visible(&e.to_string()));
            checks.push(DoctorCheck {
                group: CheckGroup::Toolchain,
                ..check(
                    "git",
                    "git",
                    if r.is_ok() {
                        DoctorLevel::Ok
                    } else {
                        DoctorLevel::Failing
                    },
                    r.as_ref().err().cloned(),
                    "No version floor: Foreman only shows it.",
                    r.ok(),
                )
            });
        }
    }
    let level = checks
        .iter()
        .map(|c| c.level)
        .filter(|l| *l != DoctorLevel::Unknown)
        .max()
        .unwrap_or(DoctorLevel::Unknown);
    let capabilities = foreman_core::compat::TABLE
        .iter()
        .map(|(feature, min)| {
            let found = version
                .as_deref()
                .is_some_and(|v| version_key(v) >= version_key(min));
            CapabilityRow {
                feature: (*feature).to_owned(),
                code: true,
                minimum: (*min).to_owned(),
                found,
                blocks: (!found).then(|| format!("Needs Claude Code {min}")),
            }
        })
        .collect();
    {
        let mut i = app.inner();
        i.doctor = Some(level);
        if version.is_some() {
            i.claude_version.clone_from(&version);
        }
    }
    app.emit(Event::DoctorStatus(DoctorStatus { level }));
    Ok(DoctorReport {
        level,
        checks,
        capabilities,
        probes: Vec::new(),
        probe_note: None,
        ran_at: Timestamp(app.now()),
    })
}

/// `doctor_capture_login_path`: not built yet (M4-14; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-14`).
pub fn doctor_capture_login_path(
    _app: &App,
    _req: DoctorCaptureLoginPathReq,
) -> Result<LoginPathPreview, IpcError> {
    Err(IpcError::not_yet_available(
        "doctor_capture_login_path",
        "M4-14",
    ))
}

/// `doctor_login_path_apply`: not built yet (M4-14; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-14`).
pub fn doctor_login_path_apply(
    _app: &App,
    _req: DoctorLoginPathApplyReq,
) -> Result<SettingsView, IpcError> {
    Err(IpcError::not_yet_available(
        "doctor_login_path_apply",
        "M4-14",
    ))
}

/// `probe_folder_open`: not built yet (M3-18; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-18`).
pub fn probe_folder_open(
    _app: &App,
    _req: ProbeFolderOpenReq,
    _output: tauri::ipc::Channel<tauri::ipc::Response>,
) -> Result<RunStarted, IpcError> {
    Err(IpcError::not_yet_available("probe_folder_open", "M3-18"))
}

/// `probe_run`: not built yet (M4-12; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-12`).
pub fn probe_run(
    _app: &App,
    _req: ProbeRunReq,
    _progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<ContractResult, IpcError> {
    Err(IpcError::not_yet_available("probe_run", "M4-12"))
}
