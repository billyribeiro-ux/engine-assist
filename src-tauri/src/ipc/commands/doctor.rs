//! `doctor` commands (`docs/contracts/ipc.md` §4.2; F-019; M2-15, M3-18, M4-12…M4-14).
//!
//! - `doctor_run`: P1 rows (Claude Code path and version against the floor, sign-in, git) and the
//!   full Doctor (P2): the session sensor, the probe folder, user settings (`cleanupPeriodDays`
//!   with a recommendation, `disableAllHooks`, broad allow rules, the Appendix A artifacts), the
//!   toolchain from fixed paths with `--version` only, the PATH Foreman builds, the login PATH, the
//!   capability table with the steps each missing feature blocks, and the contract probes.
//!   Nothing here writes Claude Code's settings (SR-10); failures are rows, never errors.
//! - `probe_folder_open`: interactive `claude` in `<app-data>/probe/` in a terminal tab so Billy
//!   can accept the trust dialog once (AC-F019-08).
//! - `probe_run`: the contract probes on demand (`app::probe`).
//! - `doctor_capture_login_path` / `doctor_login_path_apply`: the single consented shell (SR-02,
//!   AC-F019-12): `$SHELL` must be listed in `/etc/shells`; a native dialog asks first; the
//!   fixed argv runs once in app data with a timeout; the output is parsed as data only and
//!   stored only after a second native confirmation listing the entries.

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "handlers take requests by value and return the IPC Result"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use foreman_core::pathpolicy;
use foreman_core::probe::{ProbeOutcome as CoreOutcome, ProbeResult as CoreResult};
use foreman_core::schema::settings::{Contract, LoginPath, SettingsFile};
use foreman_core::store::dirs;
use foreman_core::store::outside::{self, OutsideRead, UserClaudeEntry};
use foreman_sys::spawn::{
    self, Cwd, LoginShellConsent, Program, RunOpts, SpawnError, SpawnRequest,
};
use serde_json::Value;
use tauri::ipc::{Channel, Response};
use uuid::Uuid;

use crate::ipc::IpcError;
use crate::ipc::dto::app::SettingsView;
use crate::ipc::dto::doctor::{
    CapabilityRow, CheckAction, CheckGroup, ContractResult, DoctorCheck, DoctorReport,
    LoginPathPreview, PathEntryCheck, ProbeOutcome, ProbeResult,
};
use crate::ipc::dto::events::{DoctorStatus, Event};
use crate::ipc::dto::launch::{ContractOutcome, LaunchProgress, RunStarted};
use crate::ipc::dto::requests::{
    DoctorCaptureLoginPathReq, DoctorLoginPathApplyReq, DoctorRunReq, DoctorScope,
    ProbeFolderOpenReq, ProbeRunReq,
};
use crate::ipc::types::{ChainId, DoctorLevel, PreviewId, RunId, TextSpan, Timestamp, ToolKind};
use crate::os::Confirm;
use crate::render::{escape_visible, spans};
use crate::runs::{LiveRun, Outputs, RunSink};
use crate::state::{App, PREVIEW_TTL};

/// Claude Code's default `cleanupPeriodDays` (docs/threat-model.md §"chains", ADR-008).
const CLEANUP_DEFAULT: u64 = 30;
/// What Doctor recommends (never written, SR-10): long enough for a stage that waits weeks.
const CLEANUP_RECOMMENDED: u64 = 90;
/// Login-shell timeout (ADR-007: helpers time out).
const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(10);
/// The global agents of blueprint v2.2 Appendix A.2 (`~/.claude/agents/`).
const APPENDIX_A_AGENTS: [&str; 8] = [
    "explore.md",
    "test-runner.md",
    "implementer.md",
    "code-reviewer.md",
    "debugger.md",
    "design-reviewer.md",
    "security-reviewer.md",
    "sre-reviewer.md",
];
/// Allow rules that let a session run almost anything (AC-F019-10).
const BROAD_RULES: [&str; 6] = [
    "Bash(*)",
    "Bash",
    "Bash(pnpm exec *)",
    "Bash(npx *)",
    "Bash(pnpm dlx *)",
    "Bash(bunx *)",
];
/// Interpreters: an allow rule that starts one runs any program (AC-F019-10).
/// (One string, matched word by word: these are rule text to flag, never programs Foreman runs.)
const INTERPRETERS: &str = "python python3 node ruby perl bash sh zsh deno bun";

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

fn run_version(app: &App, req: &SpawnRequest, git: &Path) -> Result<String, String> {
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

/// The Sign-in row (AC-F019-04, SR-08): `claude auth status` in `<app-data>/probe/` with the
/// launch environment, only after the configuration fingerprint passes SR-07's scan there. Only
/// the method (`claude.ai` or `oauth_token`) and the exit code are read; a logged-out, unknown,
/// unparsable or API-key answer fails with how to fix it.
fn auth_row(
    app: &App,
    settings: &SettingsFile,
    ctx: &spawn::SpawnCtx,
    claude_ready: bool,
) -> DoctorCheck {
    let row = |level: DoctorLevel, summary: Option<String>, detail: &str, found: Option<String>| {
        let name = if level == DoctorLevel::Ok {
            "Signed in with your plan"
        } else {
            "Sign-in"
        };
        DoctorCheck {
            found_mono: false,
            ..check("auth", name, level, summary, detail, found)
        }
    };
    let how = "Run claude in a terminal and sign in with your Claude plan (/login). Foreman launches only with a subscription sign-in, never an API key.";
    if !claude_ready {
        return row(
            DoctorLevel::Unknown,
            Some("Needs Claude Code first.".to_owned()),
            "Foreman checks the sign-in once Claude Code is found at the right version.",
            None,
        );
    }
    let Ok(root) = app.app_root() else {
        return row(
            DoctorLevel::Unknown,
            Some("Foreman's data folder isn't available.".to_owned()),
            how,
            None,
        );
    };
    let probe = ctx.app_data.join("probe");
    let made = pathpolicy::check_app_data("probe")
        .ok()
        .is_some_and(|p| dirs::ensure_dir(&root, &p).is_ok());
    if !made {
        return row(
            DoctorLevel::Unknown,
            Some("The probe folder couldn't be prepared.".to_owned()),
            how,
            None,
        );
    }
    let probe = spawn::canonical(&probe);
    let own_env = app.own_env();
    let config_dir = own_env
        .iter()
        .find(|(k, _)| k == "CLAUDE_CONFIG_DIR")
        .map(|(_, v)| PathBuf::from(v));
    let home = app.paths().home.clone();
    let gathered = foreman_sys::fingerprint::gather(
        &foreman_sys::fingerprint::Where {
            cwd: &probe,
            repo_root: &probe,
            home: &home,
            config_dir: config_dir.as_deref(),
            managed: Path::new(foreman_core::fingerprint::MANAGED_DIR),
            projects: &ctx.projects,
        },
        &[],
    );
    let g = match gathered {
        Ok(g) => g,
        Err(_) => {
            return row(
                DoctorLevel::Failing,
                Some("Foreman couldn't read the Claude Code settings it checks first.".to_owned()),
                "A settings file is unreadable, too large or a symlink into a project. Fix it, then check again.",
                None,
            );
        }
    };
    let allow: Vec<foreman_core::fingerprint::AllowlistEntry> = settings
        .auth_key_allowlist
        .iter()
        .map(|a| foreman_core::fingerprint::AllowlistEntry {
            key: a.key.clone(),
            sha256: a.sha256.clone(),
        })
        .collect();
    if let Some(hit) = foreman_core::fingerprint::auth_key_hits(&g.files, &allow)
        .into_iter()
        .next()
    {
        let problem = format!(
            "{} sets {}, which would replace your plan sign-in",
            escape_visible(&hit.file),
            escape_visible(&hit.key)
        );
        return row(
            DoctorLevel::Failing,
            Some(problem.clone()),
            &format!(
                "{problem}. Remove that setting (Foreman never edits it), then check again. A launch shows the same file and key."
            ),
            None,
        );
    }
    let sources = crate::launch::path_sources(
        app,
        settings
            .login_path
            .as_ref()
            .map(|l| l.entries.clone())
            .unwrap_or_default(),
    );
    let env = foreman_sys::launch::child_env(
        &own_env,
        &settings.env_passthrough,
        &sources,
        &ctx.projects,
        Uuid::new_v4(),
    )
    .0;
    let check_ = foreman_sys::claude::auth_status(ctx, &probe, &env);
    match check_.result {
        Ok(method) => row(
            DoctorLevel::Ok,
            None,
            "Claude Code reports a subscription sign-in. Foreman reads only the method and the exit code.",
            Some(escape_visible(&method)),
        ),
        Err(why) => {
            let problem = escape_visible(&foreman_sys::launch::describe(&why));
            row(
                DoctorLevel::Failing,
                Some(problem.clone()),
                &format!("{problem}. {how}"),
                check_.method.map(|m| escape_visible(&m)),
            )
        }
    }
}

fn tool(c: DoctorCheck) -> DoctorCheck {
    DoctorCheck {
        group: CheckGroup::Toolchain,
        ..c
    }
}

/// Plain text with `code` parts (`(text, is_code)`), escaped (SR-26).
fn rich(parts: &[(&str, bool)]) -> Vec<TextSpan> {
    parts
        .iter()
        .flat_map(|(t, code)| {
            if *code {
                vec![TextSpan::Code {
                    text: escape_visible(t),
                }]
            } else {
                spans(t)
            }
        })
        .collect()
}

/// `~/.claude/settings.json` and managed settings as JSON (missing or unreadable: `None`).
fn claude_settings(app: &App) -> (Option<Value>, Option<Value>) {
    let home = app.paths().home.clone();
    let cfg = app.config_dir_env();
    let read = |r: OutsideRead| {
        outside::read(&r, &home, cfg.as_deref())
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
    };
    (
        read(OutsideRead::UserSettings),
        read(OutsideRead::ManagedSettings),
    )
}

fn settings_rows(app: &App) -> Vec<DoctorCheck> {
    let (user, managed) = claude_settings(app);
    let mut out = Vec::new();
    // Effective cleanupPeriodDays: managed settings win over user settings.
    let set = managed
        .as_ref()
        .and_then(|m| m.get("cleanupPeriodDays"))
        .or_else(|| user.as_ref().and_then(|u| u.get("cleanupPeriodDays")))
        .and_then(Value::as_u64);
    let days = set.unwrap_or(CLEANUP_DEFAULT);
    out.push(DoctorCheck {
        detail: rich(&[
            ("cleanupPeriodDays", true),
            (
                &format!(
                    " is {days}{}. Claude Code deletes conversations older than that, and a chain idle longer breaks. Foreman recommends {CLEANUP_RECOMMENDED} or more if your stages run long; set it in ",
                    if set.is_none() { " (the default)" } else { "" }
                ),
                false,
            ),
            ("~/.claude/settings.json", true),
            (" yourself. Foreman doesn't edit that file.", false),
        ]),
        ..check(
            "cleanup",
            "Transcripts kept",
            if days >= CLEANUP_RECOMMENDED {
                DoctorLevel::Ok
            } else {
                DoctorLevel::Warning
            },
            None,
            "",
            Some(format!("{days} days")),
        )
    });
    let hooks_off = |v: &Option<Value>| {
        v.as_ref()
            .and_then(|s| s.get("disableAllHooks"))
            .and_then(Value::as_bool)
            == Some(true)
    };
    let (user_off, managed_off) = (hooks_off(&user), hooks_off(&managed));
    out.push(DoctorCheck {
        detail: rich(&if managed_off {
            vec![
                ("disableAllHooks", true),
                (
                    " is set in managed settings, so the session sensor can't run. Foreman asks you when Claude is idle instead.",
                    false,
                ),
            ]
        } else if user_off {
            vec![
                ("disableAllHooks", true),
                (
                    " is set in your user settings. Foreman's run settings turn hooks back on for its own runs; other sessions stay without hooks.",
                    false,
                ),
            ]
        } else {
            vec![
                ("disableAllHooks", true),
                (" isn't set, so the session sensor can run.", false),
            ]
        }),
        found_mono: false,
        ..check(
            "hooks",
            "Hooks enabled",
            if user_off || managed_off {
                DoctorLevel::Warning
            } else {
                DoctorLevel::Ok
            },
            None,
            "",
            Some(if user_off || managed_off { "off" } else { "on" }.to_owned()),
        )
    });
    let mut broad: Vec<String> = Vec::new();
    for s in [&user, &managed].into_iter().flatten() {
        let rules = s
            .get("permissions")
            .and_then(|p| p.get("allow"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            .unwrap_or_default();
        for r in rules {
            if is_broad(r) && !broad.iter().any(|b| b == r) {
                broad.push(r.to_owned());
            }
        }
    }
    let mut parts: Vec<(String, bool)> = Vec::new();
    if broad.is_empty() {
        parts.push((
            "No allow rule in user or managed settings lets a session run any program.".into(),
            false,
        ));
    } else {
        for (i, b) in broad.iter().enumerate() {
            if i > 0 {
                parts.push((", ".into(), false));
            }
            parts.push((b.clone(), true));
        }
        parts.push((
            " lets a session run almost any program without asking. Claude Code merges allow lists across settings files, so this applies in every project.".into(),
            false,
        ));
    }
    out.push(DoctorCheck {
        detail: rich(
            &parts
                .iter()
                .map(|(t, c)| (t.as_str(), *c))
                .collect::<Vec<_>>(),
        ),
        found_mono: false,
        ..check(
            "allow",
            "Broad allow rules",
            if broad.is_empty() {
                DoctorLevel::Ok
            } else {
                DoctorLevel::Warning
            },
            None,
            "",
            Some(format!(
                "{} rule{}",
                broad.len(),
                if broad.len() == 1 { "" } else { "s" }
            )),
        )
    });
    out.push(appendix_a(app, user.is_some()));
    out
}

/// Whether an allow rule runs nearly anything: a bare `Bash`, `Bash(*)`, a general runner, or an
/// interpreter (`Bash(python3 *)`).
fn is_broad(rule: &str) -> bool {
    if BROAD_RULES.contains(&rule) {
        return true;
    }
    let Some(inner) = rule.strip_prefix("Bash(").and_then(|r| r.strip_suffix(')')) else {
        return false;
    };
    let first = inner.split([' ', ':']).next().unwrap_or_default();
    INTERPRETERS.split_whitespace().any(|i| i == first) || first == "*"
}

fn appendix_a(app: &App, settings_found: bool) -> DoctorCheck {
    let home = app.paths().home.clone();
    let cfg = app.config_dir_env();
    let has = |r: OutsideRead| outside::read(&r, &home, cfg.as_deref()).is_ok();
    let claude_md = has(OutsideRead::UserClaude(UserClaudeEntry::ClaudeMd, None));
    let missing_agents: Vec<&str> = APPENDIX_A_AGENTS
        .iter()
        .copied()
        .filter(|a| {
            !has(OutsideRead::UserClaude(
                UserClaudeEntry::Agents,
                Some((*a).to_owned()),
            ))
        })
        .collect();
    let mut missing: Vec<String> = Vec::new();
    if !settings_found {
        missing.push("~/.claude/settings.json".into());
    }
    if !claude_md {
        missing.push("~/.claude/CLAUDE.md".into());
    }
    missing.extend(
        missing_agents
            .iter()
            .map(|a| format!("~/.claude/agents/{a}")),
    );
    let total = APPENDIX_A_AGENTS.len() + 2;
    let detail = if missing.is_empty() {
        rich(&[(
            "The blueprint's one-time setup (Appendix A: user settings, global agents, CLAUDE.md) is in place.",
            false,
        )])
    } else {
        let mut v = rich(&[(
            "Missing from the blueprint's one-time setup (Appendix A): ",
            false,
        )]);
        for (i, m) in missing.iter().enumerate() {
            if i > 0 {
                v.extend(spans(", "));
            }
            v.push(TextSpan::Code { text: m.clone() });
        }
        v.extend(rich(&[
            (". Run ", false),
            ("./install.sh", true),
            (
                " in your claude-blueprint kit yourself; Foreman doesn't run scripts.",
                false,
            ),
        ]));
        v
    };
    DoctorCheck {
        detail,
        found_mono: false,
        ..check(
            "appendixA",
            "One-time setup",
            if missing.is_empty() {
                DoctorLevel::Ok
            } else {
                DoctorLevel::Warning
            },
            None,
            "",
            Some(format!("{} of {total}", total - missing.len())),
        )
    }
}

fn sensor_row(app: &App) -> DoctorCheck {
    let (level, detail, found) = match crate::sensor::check(app) {
        crate::sensor::Check::NotBundled => (
            DoctorLevel::Unknown,
            "This build doesn't include the session sensor. Foreman asks you when Claude is idle, and Capture from session is off.",
            "not in this build",
        ),
        crate::sensor::Check::Mismatch => (
            DoctorLevel::Warning,
            "The installed sensor doesn't match the one built into Foreman and couldn't be replaced, so runs start without it.",
            "hash mismatch",
        ),
        crate::sensor::Check::Ok(_) => (
            DoctorLevel::Ok,
            "Installed in Foreman's app data and checked before every run. Its events only update what Foreman shows.",
            "installed",
        ),
    };
    DoctorCheck {
        found_mono: false,
        ..check(
            "sensor",
            "Session sensor",
            level,
            None,
            detail,
            Some(found.to_owned()),
        )
    }
}

fn probe_folder_row(s: &SettingsFile) -> DoctorCheck {
    DoctorCheck {
        found_mono: false,
        action: Some(CheckAction::OpenProbeFolder),
        ..check(
            "probeFolder",
            "Probe folder trusted",
            if s.probe_folder_trusted {
                DoctorLevel::Ok
            } else {
                DoctorLevel::Warning
            },
            None,
            if s.probe_folder_trusted {
                "Contract probes run in Foreman's own folder, never in a project."
            } else {
                "Open Foreman's probe folder in Claude Code once and accept the trust question; until then contract probes stay off."
            },
            Some(if s.probe_folder_trusted { "yes" } else { "no" }.to_owned()),
        )
    }
}

/// Toolchain rows (AC-F019-11): `git`, `node`, `pnpm`, `cargo` from the fixed candidates (or
/// Settings for git) with `--version` only, in app data.
fn toolchain_rows(app: &App, ctx: &spawn::SpawnCtx, full: bool) -> Vec<DoctorCheck> {
    let mut out = Vec::new();
    let git = foreman_sys::git::resolve(ctx);
    match &git {
        Err(e) => out.push(tool(check(
            "git",
            "git",
            DoctorLevel::Failing,
            Some(super::settings::spawn_problem(e)),
            "Install git (xcode-select --install), or set its path in Settings.",
            None,
        ))),
        Ok(p) => {
            let r = foreman_sys::git::version(&app.paths().home, ctx)
                .map(|v| escape_visible(&v))
                .map_err(|e| escape_visible(&e.to_string()));
            out.push(tool(check(
                "git",
                "git",
                if r.is_ok() {
                    DoctorLevel::Ok
                } else {
                    DoctorLevel::Failing
                },
                r.as_ref().err().cloned(),
                &format!(
                    "At {}. No version floor: Foreman only shows it.",
                    escape_visible(&app.display(p))
                ),
                r.ok(),
            )));
        }
    }
    if !full {
        return out;
    }
    let git_path = git.unwrap_or_else(|_| PathBuf::from("/usr/bin/git"));
    for (program, id) in [
        (Program::Node, "node"),
        (Program::Pnpm, "pnpm"),
        (Program::Cargo, "cargo"),
    ] {
        let row = match spawn::resolve(program, ctx) {
            Err(_) => check(
                id,
                id,
                DoctorLevel::Unknown,
                Some("Not found at a fixed path.".into()),
                &format!(
                    "Foreman looks only at {}. Steps that need it fail inside their terminal tab.",
                    program.candidates().join(", ")
                ),
                None,
            ),
            Ok(p) => match run_version(app, &SpawnRequest::ToolVersion(program), &git_path) {
                Ok(v) => check(
                    id,
                    id,
                    DoctorLevel::Ok,
                    None,
                    &format!(
                        "At {}, run with --version only.",
                        escape_visible(&app.display(&p))
                    ),
                    Some(v),
                ),
                Err(why) => check(
                    id,
                    id,
                    DoctorLevel::Warning,
                    Some(format!("{id} --version failed: {why}")),
                    &format!("At {}.", escape_visible(&app.display(&p))),
                    None,
                ),
            },
        };
        out.push(tool(row));
    }
    out
}

/// The PATH a launch gets (AC-F019-14, Master Spec §6) and the login PATH row (AC-F019-12).
fn path_rows(app: &App, s: &SettingsFile, ctx: &spawn::SpawnCtx) -> Vec<DoctorCheck> {
    let login = s
        .login_path
        .as_ref()
        .map(|l| l.entries.clone())
        .unwrap_or_default();
    let sources = crate::launch::path_sources(app, login);
    let built = foreman_core::launch::env::build_path(
        &sources.candidates(),
        &spawn::project_roots(&ctx.projects),
        &mut |p| spawn::path_dir_facts(p),
    );
    let joined = escape_visible(&built.joined());
    let mut out = vec![tool(DoctorCheck {
        detail: rich(&[
            (
                "What every launch gets: /etc/paths, Homebrew, ~/.cargo/bin, pnpm, then your login PATH. Missing, relative, world-writable and in-project folders are dropped",
                false,
            ),
            (
                &if built.dropped.is_empty() {
                    ".".to_owned()
                } else {
                    format!(" ({} dropped).", built.dropped.len())
                },
                false,
            ),
        ]),
        ..check(
            "launchPath",
            "PATH for launches",
            if built.entries.is_empty() {
                DoctorLevel::Failing
            } else {
                DoctorLevel::Ok
            },
            None,
            "",
            Some(joined),
        )
    })];
    let row = match &s.login_path {
        Some(l) => DoctorCheck {
            detail: rich(&[(
                &format!(
                    "Captured from {} at {}: {}",
                    escape_visible(&l.shell),
                    l.captured_at,
                    escape_visible(&l.entries.join(":"))
                ),
                false,
            )]),
            action: Some(CheckAction::CaptureLoginPath),
            ..check(
                "loginPath",
                "Login shell PATH",
                DoctorLevel::Ok,
                None,
                "",
                None,
            )
        },
        None => DoctorCheck {
            action: Some(CheckAction::CaptureLoginPath),
            ..check(
                "loginPath",
                "Login shell PATH",
                DoctorLevel::Unknown,
                None,
                "Not captured. Capturing runs your login shell once to read PATH, shows you the result, and stores it only if you save it. It's the only shell Foreman ever starts.",
                None,
            )
        },
    };
    out.push(tool(row));
    out
}

/// The steps of the registered projects that need each feature (AC-F019-09).
fn blocked_steps(app: &App) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let Ok((reg, _)) = app.registry() else {
        return out;
    };
    for e in &reg.projects {
        let Ok(p) = crate::project::open(app, &e.project_id) else {
            continue;
        };
        let Ok((t, _, project)) = p.parts() else {
            continue;
        };
        let (_, reqs) = foreman_core::compat::floor(t, &[], &project.model_policy);
        for r in reqs {
            let what = r.source.strip_prefix("step ").unwrap_or(&r.source);
            out.entry(r.feature).or_default().push(format!(
                "{} {}",
                escape_visible(&e.name),
                escape_visible(what)
            ));
        }
    }
    out
}

fn capability_rows(app: &App, version: Option<&str>) -> Vec<CapabilityRow> {
    let steps = blocked_steps(app);
    foreman_core::compat::TABLE
        .iter()
        .map(|(feature, min)| {
            let found = version.is_some_and(|v| version_key(v) >= version_key(min));
            CapabilityRow {
                feature: (*feature).to_owned(),
                code: feature.starts_with("--"),
                minimum: (*min).to_owned(),
                found,
                blocks: (!found).then(|| {
                    steps.get(*feature).map_or_else(
                        || "No step in your projects uses it".to_owned(),
                        |v| v.join(", "),
                    )
                }),
            }
        })
        .collect()
}

/// A probe outcome as Doctor's row (§2.4 `ProbeOutcome`).
fn probe_row(which: &str, o: &CoreOutcome, verified: &str) -> ProbeOutcome {
    let probe = if which == "a" {
        rich(&[
            ("Flags parse and per-launch hooks fire (", false),
            ("--init-only", true),
            (")", false),
        ])
    } else {
        rich(&[
            ("--resume", true),
            (" and ", false),
            ("--fork-session", true),
            (" parse", false),
        ])
    };
    let (result, text, detail) = match o.result {
        CoreResult::Pass => (ProbeResult::Pass, format!("Pass on {verified}"), None),
        CoreResult::FlagFailure => {
            let flag = o.flag.clone().unwrap_or_else(|| "a flag".into());
            let off = foreman_core::probe::disabled_features(std::slice::from_ref(o)).join(", ");
            (
                ProbeResult::Fail,
                format!("{} failed", escape_visible(&flag)),
                Some(format!(
                    "Turned off: {off}. Everything else still launches."
                )),
            )
        }
        CoreResult::Inconclusive | CoreResult::Skipped => (
            ProbeResult::Inconclusive,
            "Inconclusive".to_owned(),
            Some(format!("Keeping the last verified state, {verified}.")),
        ),
    };
    ProbeOutcome {
        probe,
        result,
        text,
        detail,
    }
}

fn probe_rows(c: Option<&Contract>) -> Vec<ProbeOutcome> {
    c.map(|c| {
        let v = escape_visible(&c.version);
        vec![
            probe_row("a", &crate::probe::from_stored(&c.outcomes.probe_a), &v),
            probe_row("b", &crate::probe::from_stored(&c.outcomes.probe_b), &v),
        ]
    })
    .unwrap_or_default()
}

/// `doctor_run`: every row of Doctor (`p1`: Claude Code version and path, sign-in, git; `full`:
/// everything, AC-F019-09…-11, -14). Failures are rows, never errors. Emits `doctor:status`.
///
/// # Errors
/// `Io` when settings are unreadable.
#[allow(
    clippy::too_many_lines,
    reason = "one pass over Doctor's rows in screen order"
)]
pub fn doctor_run(app: &App, req: DoctorRunReq) -> Result<DoctorReport, IpcError> {
    let full = req.scope == DoctorScope::Full;
    let (settings, _, _) = app.settings()?;
    let ctx = app.spawn_ctx_now()?;
    let mut checks = Vec::new();
    let floor = floor();
    let claude = spawn::resolve(Program::Claude, &ctx);
    let git_path =
        foreman_sys::git::resolve(&ctx).unwrap_or_else(|_| PathBuf::from("/usr/bin/git"));
    let mut version = None;
    match &claude {
        Err(e) => checks.push(check(
            "path",
            "Claude Code",
            DoctorLevel::Failing,
            Some(super::settings::spawn_problem(e)),
            "Install Claude Code, or set its path in Settings.",
            None,
        )),
        Ok(p) => {
            checks.push(check(
                "path",
                "Found at an allowed path",
                DoctorLevel::Ok,
                None,
                "Foreman only launches this exact file, checked again before every spawn.",
                Some(escape_visible(&app.display(p))),
            ));
            match run_version(app, &SpawnRequest::ClaudeVersion, &git_path) {
                Ok(v) => {
                    let parsed = foreman_core::compat::parse_version_output(&v);
                    let ok = parsed
                        .as_ref()
                        .is_some_and(|x| version_key(&x.to_string()) >= version_key(&floor));
                    version = parsed.map(|x| x.to_string());
                    checks.push(check(
                        "version",
                        "Version",
                        if ok {
                            DoctorLevel::Ok
                        } else {
                            DoctorLevel::Failing
                        },
                        (!ok).then(|| format!("Needs {floor} or newer. Run claude update.")),
                        &if ok {
                            "Meets every model and flag the blueprint uses.".to_owned()
                        } else {
                            format!("Claude Code {v} is below {floor}; run claude update, then check again.")
                        },
                        Some(v.clone()),
                    ));
                }
                Err(why) => checks.push(check(
                    "version",
                    "Version",
                    DoctorLevel::Failing,
                    Some(format!("claude -v failed: {why}")),
                    "Reinstall Claude Code, or set its path in Settings.",
                    None,
                )),
            }
        }
    }
    let claude_ready = checks
        .iter()
        .any(|c| c.id == "version" && c.level == DoctorLevel::Ok);
    checks.push(auth_row(app, &settings, &ctx, claude_ready));
    if full {
        checks.push(sensor_row(app));
        checks.push(probe_folder_row(&settings));
        checks.extend(settings_rows(app));
    }
    checks.extend(toolchain_rows(app, &ctx, full));
    if full {
        checks.extend(path_rows(app, &settings, &ctx));
    }
    let level = checks
        .iter()
        .map(|c| c.level)
        .filter(|l| *l != DoctorLevel::Unknown)
        .max()
        .unwrap_or(DoctorLevel::Unknown);
    let capabilities = capability_rows(app, version.as_deref());
    let probe_note = if full {
        Some(
            match (crate::probe::off_reason(app, &settings), &settings.contract) {
                (Some(why), _) => why.to_owned(),
                (None, Some(c)) => format!(
                    "Contract verified on {} ({}). Probes run again when Claude Code's version changes.",
                    escape_visible(&c.version),
                    c.verified_at
                ),
                (None, None) => {
                    "No contract verified yet. Probes run before the next launch.".to_owned()
                }
            },
        )
    } else {
        None
    };
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
        probes: if full {
            probe_rows(settings.contract.as_ref())
        } else {
            Vec::new()
        },
        probe_note,
        ran_at: Timestamp(app.now()),
    })
}

// ---------------------------------------------------------------------------------------------
// Login-shell PATH (SR-02, ADR-007 §8, AC-F019-12…-14)
// ---------------------------------------------------------------------------------------------

/// A captured login PATH waiting for `doctor_login_path_apply`.
#[derive(Debug, Clone)]
pub struct HeldLoginPath {
    /// The shell that ran.
    pub shell: String,
    /// The entries that passed SR-05's checks, in order.
    pub entries: Vec<String>,
    /// Every entry with its verdict (the second dialog lists them).
    pub checks: Vec<PathEntryCheck>,
}

/// The shells `/etc/shells` lists (comments and blanks skipped).
#[must_use]
pub fn listed_shells(etc_shells: &str) -> Vec<String> {
    etc_shells
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// The child environment of the login shell: the SR-05 allowlist only (no pass-through, no
/// `CLAUDE_*`), with a minimal system `PATH` for the shell's own start-up files.
fn shell_env(app: &App, shell: &str) -> Vec<(String, String)> {
    let own = app.own_env();
    let mut env: Vec<(String, String)> = own
        .into_iter()
        .filter(|(k, _)| {
            matches!(k.as_str(), "HOME" | "USER" | "LOGNAME" | "TMPDIR" | "LANG")
                || k.starts_with("LC_")
        })
        .collect();
    env.push(("SHELL".into(), shell.to_owned()));
    env.push(("PATH".into(), "/usr/bin:/bin:/usr/sbin:/sbin".into()));
    env
}

/// Parses the shell's output as data (never run, never interpreted): the `PATH` value split on
/// `:` and checked like every PATH source (SR-05: absolute, existing, not inside a project, not
/// world-writable). At most 200 entries are considered.
#[must_use]
pub fn parse_login_path(
    stdout: &[u8],
    projects: &[PathBuf],
    facts: &mut dyn FnMut(&Path) -> foreman_core::launch::env::DirFacts,
) -> (Vec<String>, Vec<PathEntryCheck>) {
    use foreman_core::launch::env::PathDrop;
    let text = String::from_utf8_lossy(stdout);
    let raw: Vec<String> = text
        .trim_end_matches(['\n', '\r'])
        .split(':')
        .take(200)
        .map(str::to_owned)
        .collect();
    let built = foreman_core::launch::env::build_path(&raw, projects, facts);
    let mut checks = Vec::new();
    let mut kept = built.entries.iter();
    let mut next_kept = kept.next();
    let mut dropped = built.dropped.iter();
    let mut next_drop = dropped.next();
    for r in &raw {
        if next_kept.is_some_and(|k| k.as_os_str() == r.trim()) {
            checks.push(PathEntryCheck {
                entry: escape_visible(r.trim()),
                ok: true,
                why: None,
            });
            next_kept = kept.next();
        } else if let Some((d, why)) = next_drop {
            checks.push(PathEntryCheck {
                entry: escape_visible(d),
                ok: false,
                why: Some(
                    match why {
                        PathDrop::NotAbsolute => "Relative or empty entries are dropped.",
                        PathDrop::BadCharacter => "It contains a control character.",
                        PathDrop::Duplicate => "Already listed.",
                        PathDrop::InProject => "Inside a project folder.",
                        PathDrop::Missing => "The folder doesn't exist.",
                        PathDrop::WorldWritable => "Anyone can write to this folder.",
                    }
                    .to_owned(),
                ),
            });
            next_drop = dropped.next();
        }
    }
    (
        built
            .entries
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
        checks,
    )
}

/// The capture itself, with `$SHELL` given (`doctor_capture_login_path` passes Foreman's own).
///
/// # Errors
/// `NotAllowed("shell-not-listed")`, `Cancelled`, `ToolFailed`.
pub fn capture_login_path_with(
    app: &App,
    shell: Option<&str>,
) -> Result<LoginPathPreview, IpcError> {
    let not_listed = || {
        IpcError::not_allowed(
            "shell-not-listed",
            "Your login shell ($SHELL) isn't listed in /etc/shells, so Foreman won't run it.",
        )
    };
    let shell = shell
        .filter(|s| s.starts_with('/'))
        .ok_or_else(not_listed)?;
    let shells = outside::read(&OutsideRead::EtcShells, &app.paths().home, None)
        .map(|b| listed_shells(&String::from_utf8_lossy(&b)))
        .unwrap_or_default();
    if !shells.iter().any(|s| s == shell) {
        return Err(not_listed());
    }
    let shown = escape_visible(shell);
    // SR-02, SR-24a: Billy consents in a native dialog before the one shell ever starts.
    let ok = app.os().confirm(&Confirm::new(
        "Run your login shell once?",
        format!(
            "Foreman starts {shown} -l -c 'printf %s \"$PATH\"' once, in its own app data folder, to read the PATH your Terminal gets. It runs nothing else and stores nothing until you save the result."
        ),
        "Capture PATH",
    ));
    if !ok {
        return Err(IpcError::cancelled());
    }
    let ctx = app.spawn_ctx_now()?;
    let out = spawn::run(
        &SpawnRequest::LoginShellPath {
            shell: PathBuf::from(shell),
            consent: LoginShellConsent::granted_in_native_dialog(),
        },
        &RunOpts {
            env: shell_env(app, shell),
            timeout: LOGIN_SHELL_TIMEOUT,
            output_cap: 64 * 1024,
            cwd: Cwd::AppData,
        },
        &ctx,
    )
    .map_err(|e| match e {
        SpawnError::Timeout(_) => IpcError::tool_failed("login shell", None, true, String::new()),
        other => IpcError::tool_failed(
            "login shell",
            None,
            false,
            escape_visible(&super::settings::spawn_problem(&other)),
        ),
    })?;
    if out.code != Some(0) {
        return Err(IpcError::tool_failed(
            "login shell",
            out.code,
            false,
            String::new(),
        ));
    }
    let (entries, checks) = parse_login_path(
        &out.stdout,
        &spawn::project_roots(&ctx.projects),
        &mut |p| spawn::path_dir_facts(p),
    );
    let held = HeldLoginPath {
        shell: shell.to_owned(),
        entries,
        checks: checks.clone(),
    };
    let id = app.inner().previews.hold(
        crate::preview::Preview::LoginPath(Box::new(held)),
        PREVIEW_TTL,
    );
    Ok(LoginPathPreview {
        preview_id: PreviewId::from_trusted(id),
        shell: shown,
        entries: checks,
    })
}

/// `doctor_capture_login_path` (AC-F019-12): Foreman's own `$SHELL`, only if `/etc/shells` lists
/// it, after Billy's native consent; the result is shown, never stored here.
///
/// # Errors
/// `NotAllowed("shell-not-listed")`, `Cancelled`, `ToolFailed`.
pub fn doctor_capture_login_path(
    app: &App,
    _req: DoctorCaptureLoginPathReq,
) -> Result<LoginPathPreview, IpcError> {
    let shell = app
        .own_env()
        .into_iter()
        .find(|(k, _)| k == "SHELL")
        .map(|(_, v)| v);
    capture_login_path_with(app, shell.as_deref())
}

/// `doctor_login_path_apply`: a second native dialog lists the entries; OK stores them
/// (`settings.json` `loginPath`, CAS). Launches sanitize them again when PATH is built (SR-05).
///
/// # Errors
/// `Expired`, `Cancelled`, `Conflict`.
pub fn doctor_login_path_apply(
    app: &App,
    req: DoctorLoginPathApplyReq,
) -> Result<SettingsView, IpcError> {
    let _w = app.writing();
    let held = match app.inner().previews.take(req.preview_id.as_str())? {
        crate::preview::Preview::LoginPath(h) => *h,
        _ => return Err(IpcError::expired()),
    };
    let list = if held.entries.is_empty() {
        "(no usable entries)".to_owned()
    } else {
        held.entries
            .iter()
            .map(|e| format!("• {}", escape_visible(e)))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let ok = app.os().confirm(&Confirm::new(
        "Save this login PATH?",
        format!(
            "Every launch adds these folders to PATH after Foreman's own:\n\n{list}\n\n{} other entries were dropped.",
            held.checks.iter().filter(|c| !c.ok).count()
        ),
        "Save PATH",
    ));
    if !ok {
        return Err(IpcError::cancelled());
    }
    let (mut s, stamp, _) = app.settings()?;
    s.login_path = Some(LoginPath {
        entries: held.entries,
        shell: held.shell,
        captured_at: app.now(),
    });
    app.save_settings(&s, &stamp)?;
    super::settings::view(app, &s)
}

// ---------------------------------------------------------------------------------------------
// Probe folder and contract probes (M3-18, M4-12)
// ---------------------------------------------------------------------------------------------

/// `probe_folder_open` (AC-F019-08, SR-17): `<app-data>/probe/` is recreated (`0700`, Foreman's
/// files only), then interactive `claude` runs there in a terminal tab, with no prompt, so Billy
/// can accept the trust question once. Records that the trust step was done.
///
/// # Errors
/// `ToolMissing`, `Io`.
pub fn probe_folder_open(
    app: &App,
    _req: ProbeFolderOpenReq,
    output: Channel<Response>,
) -> Result<RunStarted, IpcError> {
    let started = Instant::now();
    let (settings, stamp, _) = app.settings()?;
    let ctx = app.spawn_ctx_now()?;
    let program = spawn::resolve(Program::Claude, &ctx).map_err(|_| {
        IpcError::tool_missing(ToolKind::Claude, settings.tools.claude.path.clone())
    })?;
    let root = app.app_root()?;
    let bytes = foreman_core::launch::run_settings::build(
        &root.path().join("probe"),
        root.path(),
        &foreman_core::launch::run_settings::RunSettingsOpts::default(),
    )
    .map(|f| foreman_core::launch::run_settings::to_bytes(&f))
    .map_err(|_| IpcError::io("The probe folder's path can't be written into a settings rule."))?;
    let run = Uuid::new_v4();
    foreman_sys::launch::reset_probe_dir(&root, &bytes.0, run)
        .map_err(|e| IpcError::io(format!("Foreman couldn't reset its probe folder: {e}")))?;
    let _ = foreman_core::store::app_dirs::remove_tree(&root, &format!("events/{run}"));
    let cwd = spawn::canonical(&root.path().join("probe"));
    let sources = crate::launch::path_sources(
        app,
        settings
            .login_path
            .as_ref()
            .map(|l| l.entries.clone())
            .unwrap_or_default(),
    );
    let env = foreman_sys::launch::child_env(
        &app.own_env(),
        &settings.env_passthrough,
        &sources,
        &ctx.projects,
        run,
    );
    let args = vec!["--name".to_owned(), "fm-probe".to_owned()];
    let run_id = run.hyphenated().to_string();
    let outputs = Arc::new(Outputs::default());
    outputs.attach(output);
    let sink = Arc::new(RunSink {
        run_id: run_id.clone(),
        outputs: Arc::clone(&outputs),
        app: app.weak(),
    });
    let size = foreman_sys::pty::size(120, 32).map_err(|e| IpcError::io(e.to_string()))?;
    let session = foreman_sys::pty::PtySession::spawn(&ctx, &args, &env.0, &cwd, size, sink)
        .map_err(|e| {
            IpcError::tool_failed("claude", None, false, escape_visible(&e.to_string()))
        })?;
    let chain = Uuid::new_v4().hyphenated().to_string();
    let plan = foreman_sys::launch::LaunchPlan {
        run_id: run,
        program,
        argv: args.clone(),
        argv_redacted: args,
        env,
        cwd: cwd.clone(),
        run_settings_path: root.path().join("probe/settings.json"),
        run_settings: bytes.0,
        run_settings_sha256: bytes.1,
        confirm: Vec::new(),
        session: foreman_core::chain::SessionPlan::New { id: run },
        effort: foreman_core::chain::EffortCarry::default(),
        claude_version: app.inner().claude_version.clone().unwrap_or_default(),
        fingerprint: String::new(),
        tab_name: "fm-probe".to_owned(),
        prompt_sha256: None,
        warnings: Vec::new(),
    };
    app.runs().insert(Arc::new(LiveRun {
        run_id: run_id.clone(),
        project_id: String::new(),
        chain: chain.clone(),
        step: String::new(),
        tab_name: "fm-probe".to_owned(),
        session,
        outputs,
        plan,
        cwd,
        config_changed: AtomicBool::new(false),
        started,
    }));
    // The first-run trust step is done (a later probe that meets the trust prompt is
    // inconclusive and Doctor says so).
    if !settings.probe_folder_trusted {
        let mut s = settings;
        s.probe_folder_trusted = true;
        let _ = app.save_settings(&s, &stamp);
    }
    crate::runs::emit_status(app, &run_id, crate::ipc::types::RunStatus::Running);
    Ok(RunStarted {
        run_id: RunId::from_trusted(run_id),
        chain: ChainId::from_trusted(chain),
        tab_name: "fm-probe".to_owned(),
    })
}

fn contract_outcome(r: CoreResult) -> ContractOutcome {
    match r {
        CoreResult::Pass => ContractOutcome::Pass,
        CoreResult::FlagFailure => ContractOutcome::FlagFailure,
        CoreResult::Inconclusive => ContractOutcome::Inconclusive,
        CoreResult::Skipped => ContractOutcome::Skipped,
    }
}

/// `probe_run` (AC-F019-15…-17): both probes now, in the probe folder, zero tokens; the merged
/// contract is stored (inconclusive keeps the last verified state).
///
/// # Errors
/// `ToolMissing`, `Io`.
pub fn probe_run(
    app: &App,
    _req: ProbeRunReq,
    progress: Channel<LaunchProgress>,
) -> Result<ContractResult, IpcError> {
    let found = crate::probe::found_version(app)?;
    let _ = progress.send(LaunchProgress::Version {
        found: Some(escape_visible(&found)),
    });
    let r = crate::probe::run(app, crate::probe::DOCTOR_START)?;
    let _ = progress.send(LaunchProgress::Contract {
        outcome: contract_outcome(crate::probe::worst(&r.a, &r.b)),
    });
    let _ = progress.send(LaunchProgress::Done);
    let v = escape_visible(&r.stored.version);
    Ok(ContractResult {
        version: v.clone(),
        verified_at: Timestamp(r.stored.verified_at.clone()),
        outcomes: vec![probe_row("a", &r.a, &v), probe_row("b", &r.b, &v)],
    })
}
