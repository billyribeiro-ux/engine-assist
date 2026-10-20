//! Settings (`docs/contracts/ipc.md` §4.1; F-029; ticket M2-12). Security-relevant changes (the
//! env pass-through list, tool paths, allowlisted key hashes) are applied only after a native
//! confirmation (SR-24a, `settings::security_fields_require_native_confirm`).

use std::path::Path;

use foreman_core::schema::settings::{SettingsFile, ToolPath, ToolSource};
use foreman_sys::spawn::{self, Program, SpawnCtx, SpawnError};

use crate::ipc::IpcError;
use crate::ipc::dto::app::{
    AllowlistEntry, LoginPathView, RunHistorySize, SettingsView, ToolPathView, ToolPaths,
};
use crate::ipc::dto::requests::{
    EditorSetting, SettingsGetReq, SettingsPatch, SettingsPickToolReq, SettingsUpdateReq,
    TerminalPrefs,
};
use crate::ipc::error::StaleWhat;
use crate::ipc::types::{BundleId, EnvName, Rev, Sha256, Timestamp, ToolKind};
use crate::os::{Confirm, Pick};
use crate::render::escape_visible;
use crate::state::App;

fn tool_name(t: ToolKind) -> &'static str {
    match t {
        ToolKind::Claude => "claude",
        ToolKind::Git => "git",
    }
}

/// Resolves a tool (git only through `foreman_sys::git`, its single entry point).
fn resolve_tool(t: ToolKind, ctx: &SpawnCtx) -> Result<std::path::PathBuf, SpawnError> {
    match t {
        ToolKind::Claude => spawn::resolve(Program::Claude, ctx),
        ToolKind::Git => foreman_sys::git::resolve(ctx),
    }
}

/// Why a tool path was refused, in Foreman's words.
#[must_use]
pub fn spawn_problem(e: &SpawnError) -> String {
    match e {
        SpawnError::RelativePath(_) => "The path isn't absolute.".to_owned(),
        SpawnError::Unlisted(_) => "Not found where Foreman looks.".to_owned(),
        SpawnError::NotExecutable(_) => "Missing or not executable.".to_owned(),
        SpawnError::InsideProject(_) => {
            "Inside a project folder, so Foreman won't run it.".to_owned()
        }
        SpawnError::WorldWritable(_) => "In a folder anyone can write to.".to_owned(),
        other => other.to_string(),
    }
}

fn tool_view(app: &App, t: ToolKind, s: &SettingsFile, ctx: &SpawnCtx) -> ToolPathView {
    let tp = match t {
        ToolKind::Claude => &s.tools.claude,
        ToolKind::Git => &s.tools.git,
    };
    let resolved = resolve_tool(t, ctx);
    let version = match t {
        ToolKind::Claude => app.inner().claude_version.clone(),
        ToolKind::Git => None,
    };
    match resolved {
        Ok(p) => ToolPathView {
            path: Some(escape_visible(&p.display().to_string())),
            version,
            auto_detected: tp.source == ToolSource::Auto,
            problem: None,
        },
        Err(e) => ToolPathView {
            path: tp.path.as_deref().map(escape_visible),
            version: None,
            auto_detected: tp.source == ToolSource::Auto,
            problem: Some(spawn_problem(&e)),
        },
    }
}

/// Both tools as Settings and Doctor show them.
#[must_use]
pub fn tool_paths(app: &App, s: &SettingsFile, ctx: &SpawnCtx) -> ToolPaths {
    ToolPaths {
        claude: tool_view(app, ToolKind::Claude, s, ctx),
        git: tool_view(app, ToolKind::Git, s, ctx),
    }
}

/// The `SettingsView` of a settings file.
///
/// # Errors
/// `Io` when the registry is unreadable.
pub fn view(app: &App, s: &SettingsFile) -> Result<SettingsView, IpcError> {
    let (reg, _) = app.registry()?;
    let ctx = app.spawn_ctx(s, &reg);
    let (rev, screen_reader_mode) = {
        let i = app.inner();
        (i.settings_rev, i.screen_reader_mode)
    };
    Ok(SettingsView {
        rev: Rev(rev),
        dri_name: s.dri_name.clone(),
        tools: tool_paths(app, s, &ctx),
        editor_bundle_id: s.editor_bundle_id.as_deref().map(BundleId::from_trusted),
        blueprint_repo: crate::ipc::commands::first_run::cached_repo_info(app, s),
        theme: s.theme.into(),
        terminal: TerminalPrefs {
            font_size: s.terminal.font_size,
            scrollback: s.terminal.scrollback,
            mac_option_is_meta: s.terminal.mac_option_is_meta,
        },
        env_passthrough: s
            .env_passthrough
            .iter()
            .map(|n| EnvName::from_trusted(n.as_str()))
            .collect(),
        login_path: s.login_path.as_ref().map(|l| LoginPathView {
            captured_at: Timestamp(l.captured_at.clone()),
            entries: l.entries.iter().map(|e| escape_visible(e)).collect(),
        }),
        global_variables: s.global_variables.clone(),
        auth_key_allowlist: s
            .auth_key_allowlist
            .iter()
            .map(|a| AllowlistEntry {
                sha256: Sha256::from_trusted(a.sha256.as_str()),
                file: "~/.claude/settings.json".to_owned(),
                key: escape_visible(&a.key),
                added_at: Timestamp(a.added_at.clone()),
            })
            .collect(),
        screen_reader_mode,
        // M4-02 counts `runs/`; nothing is recorded before the launch pipeline (M2-10) lands.
        run_history: RunHistorySize {
            runs: 0,
            megabytes: 0,
        },
    })
}

/// `settings_get` (P1, mutates nothing).
///
/// # Errors
/// `Io`.
pub fn settings_get(app: &App, req: SettingsGetReq) -> Result<SettingsView, IpcError> {
    let SettingsGetReq {} = req;
    let (s, _, _) = app.settings()?;
    view(app, &s)
}

fn global_variable_name(k: &str) -> bool {
    let mut b = k.bytes();
    matches!(b.next(), Some(c) if c.is_ascii_uppercase())
        && k.len() <= 64
        && b.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}

/// The security-relevant parts of a patch, as the native dialog lists them (SR-24a).
fn security_lines(p: &SettingsPatch, s: &SettingsFile) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(list) = &p.env_passthrough {
        let names: Vec<&str> = list.iter().map(EnvName::as_str).collect();
        out.push(if names.is_empty() {
            "Pass no extra environment variables to Claude Code.".to_owned()
        } else {
            format!(
                "Pass these environment variables to Claude Code: {}.",
                names.join(", ")
            )
        });
    }
    if let Some(t) = p.reset_tool {
        let (name, cur) = match t {
            ToolKind::Claude => ("claude", &s.tools.claude),
            ToolKind::Git => ("git", &s.tools.git),
        };
        out.push(format!(
            "Forget the {name} path{} and find it automatically.",
            cur.path
                .as_deref()
                .map(|p| format!(" {}", escape_visible(p)))
                .unwrap_or_default()
        ));
    }
    if let Some(h) = &p.remove_allowlist_entry {
        let key = s
            .auth_key_allowlist
            .iter()
            .find(|a| a.sha256 == h.as_str())
            .map_or_else(
                || "an allowlisted key".to_owned(),
                |a| escape_visible(&a.key),
            );
        out.push(format!(
            "Stop allowing {key} in ~/.claude/settings.json. Launches block while it is set."
        ));
    }
    out
}

/// `settings_update` (P1 DRI, editor; P2 the rest). Every `null` means unchanged. A patch that
/// sets the pass-through list, resets a tool or removes an allowlisted key is applied only after
/// a native confirmation listing the change; Cancel saves nothing (SR-24a).
///
/// # Errors
/// `Stale` (with the fresh view) when `expectRev` is old; `Invalid` for out-of-range values;
/// `Cancelled`; `Conflict`; `Io`.
pub fn settings_update(app: &App, req: SettingsUpdateReq) -> Result<SettingsView, IpcError> {
    let _w = app.writing();
    let (mut s, stamp, _) = app.settings()?;
    if req.expect_rev.0 != app.inner().settings_rev {
        let fresh = serde_json::to_value(view(app, &s)?).ok();
        return Err(IpcError::stale(
            StaleWhat::ReadStamp,
            "Settings changed since this screen loaded.",
            fresh,
        ));
    }
    let p = req.patch;
    if let Some(t) = &p.terminal
        && (!(9..=32).contains(&t.font_size) || !(1_000..=100_000).contains(&t.scrollback))
    {
        return Err(IpcError::invalid("patch.terminal", "range"));
    }
    if let Some(vars) = &p.global_variables
        && !vars.keys().all(|k| global_variable_name(k))
    {
        return Err(IpcError::invalid("patch.globalVariables", "variableName"));
    }
    if let Some(h) = &p.remove_allowlist_entry
        && !s.auth_key_allowlist.iter().any(|a| a.sha256 == h.as_str())
    {
        return Err(IpcError::not_found(
            "That allowlisted key isn't in Settings.",
        ));
    }
    let lines = security_lines(&p, &s);
    if !lines.is_empty() {
        let ok = app.os().confirm(&Confirm::new(
            "Change security settings?",
            lines.join("\n"),
            "Change",
        ));
        if !ok {
            return Err(IpcError::cancelled());
        }
    }
    if let Some(n) = p.dri_name {
        let n = n.as_str().trim();
        s.dri_name = (!n.is_empty()).then(|| n.to_owned());
    }
    match p.editor {
        Some(EditorSetting::Set(e)) => s.editor_bundle_id = Some(e.bundle_id.into_string()),
        Some(EditorSetting::Clear(_)) => s.editor_bundle_id = None,
        None => {}
    }
    if let Some(t) = p.theme {
        s.theme = t.into();
    }
    if let Some(t) = p.terminal {
        s.terminal.font_size = t.font_size;
        s.terminal.scrollback = t.scrollback;
        s.terminal.mac_option_is_meta = t.mac_option_is_meta;
    }
    if let Some(list) = p.env_passthrough {
        // Stored as given; build_env still removes every always-removed name (AC-F029-05).
        let mut names: Vec<String> = list.into_iter().map(EnvName::into_string).collect();
        names.sort();
        names.dedup();
        s.env_passthrough = names;
    }
    if let Some(vars) = p.global_variables {
        s.global_variables = vars
            .into_iter()
            .map(|(k, v)| (k, v.into_string()))
            .collect();
    }
    if let Some(t) = p.reset_tool {
        let auto = ToolPath {
            path: None,
            source: ToolSource::Auto,
        };
        match t {
            ToolKind::Claude => s.tools.claude = auto,
            ToolKind::Git => s.tools.git = auto,
        }
    }
    if p.clear_login_path == Some(true) {
        s.login_path = None;
    }
    if let Some(h) = p.remove_allowlist_entry {
        s.auth_key_allowlist.retain(|a| a.sha256 != h.as_str());
    }
    if let Some(on) = p.screen_reader_mode {
        app.inner().screen_reader_mode = on;
    }
    // `osDisplay` isn't stored: it only drives the window's vibrancy (ADR-014, AC-XC-23).
    if let Some(d) = p.os_display {
        crate::window::set_reduced_effects(d.reduce_transparency || d.increase_contrast);
    }
    app.save_settings(&s, &stamp)?;
    view(app, &s)
}

/// Checks a picked tool binary and reads its version for the confirmation.
fn probe_tool(app: &App, t: ToolKind, path: &Path, ctx: &SpawnCtx) -> Result<String, IpcError> {
    let checked = match t {
        ToolKind::Claude => spawn::check_program_path(Program::Claude, path, ctx),
        ToolKind::Git => foreman_sys::git::check_path(path, ctx),
    };
    checked.map_err(|e| {
        let mut err = IpcError::tool_missing(t, Some(escape_visible(&path.display().to_string())));
        err.message = spawn_problem(&e);
        err
    })?;
    let mut probe_ctx = ctx.clone();
    let failed = |timed_out: bool, exit: Option<i32>| {
        IpcError::tool_failed(tool_name(t), exit, timed_out, String::new())
    };
    let text = match t {
        ToolKind::Claude => {
            probe_ctx.settings.retain(|(p, _)| *p != Program::Claude);
            probe_ctx
                .settings
                .push((Program::Claude, path.to_path_buf()));
            let opts = spawn::RunOpts {
                env: foreman_sys::git::env(&app.paths().home, path),
                timeout: std::time::Duration::from_secs(10),
                output_cap: 64 * 1024,
                cwd: spawn::Cwd::AppData,
            };
            let out = spawn::run(&spawn::SpawnRequest::ClaudeVersion, &opts, &probe_ctx)
                .map_err(|e| failed(matches!(e, SpawnError::Timeout(_)), None))?;
            if out.code != Some(0) {
                return Err(failed(false, out.code));
            }
            String::from_utf8_lossy(&out.stdout).into_owned()
        }
        ToolKind::Git => {
            let entry = foreman_sys::git::settings_entry(path.to_path_buf());
            probe_ctx.settings.retain(|(p, _)| *p != entry.0);
            probe_ctx.settings.push(entry);
            foreman_sys::git::version(&app.paths().home, &probe_ctx)
                .map_err(|_| failed(false, None))?
        }
    };
    Ok(escape_visible(
        text.lines().next().unwrap_or_default().trim(),
    ))
}

/// `settings_pick_tool` (P1): Rust's file dialog, the SR-01 path checks and `--version`, then a
/// native confirmation showing the resolved path and version (SR-24a). The path never comes from
/// the webview.
///
/// # Errors
/// `Cancelled` (dialog or confirmation); `ToolMissing` for a refused path; `ToolFailed`; `Io`.
pub fn settings_pick_tool(app: &App, req: SettingsPickToolReq) -> Result<ToolPaths, IpcError> {
    let t = req.tool;
    let name = tool_name(t);
    let Some(path) = app.os().pick(&Pick::Tool(name)) else {
        return Err(IpcError::cancelled());
    };
    let _w = app.writing();
    let (mut s, stamp, _) = app.settings()?;
    let (reg, _) = app.registry()?;
    let ctx = app.spawn_ctx(&s, &reg);
    let version = probe_tool(app, t, &path, &ctx)?;
    let shown = escape_visible(&path.display().to_string());
    let ok = app.os().confirm(&Confirm::new(
        format!("Use this {name}?"),
        format!("Foreman will run {shown}\n{version}"),
        format!("Use this {name}"),
    ));
    if !ok {
        return Err(IpcError::cancelled());
    }
    let chosen = ToolPath {
        path: Some(path.display().to_string()),
        source: ToolSource::Settings,
    };
    match t {
        ToolKind::Claude => {
            s.tools.claude = chosen;
            app.inner().claude_version = Some(version);
        }
        ToolKind::Git => s.tools.git = chosen,
    }
    app.save_settings(&s, &stamp)?;
    let ctx = app.spawn_ctx(&s, &reg);
    Ok(tool_paths(app, &s, &ctx))
}
