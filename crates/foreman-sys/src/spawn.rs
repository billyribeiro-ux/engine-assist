//! The only module that constructs a `Command` (ADR-007, SR-01, SR-02, SR-57). Requests are typed;
//! the program is re-resolved and re-checked before every spawn; the environment is cleared and
//! rebuilt from an explicit list; non-session helpers run with their cwd in app data; children get
//! their own process group, a null stdin (unless a pipe is needed), a timeout and capped output.
#![allow(
    clippy::disallowed_methods,
    reason = "foreman-sys::spawn is the single Command site (clippy.toml, SR-02, AC-XC-01)"
)]

use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The allowlisted programs (AC-XC-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Program {
    /// `claude`
    Claude,
    /// `git`
    Git,
    /// `/usr/bin/open`
    Open,
    /// `node --version`
    Node,
    /// `pnpm --version`
    Pnpm,
    /// `cargo --version`
    Cargo,
}

impl Program {
    /// The file name the resolved program must have.
    #[must_use]
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Git => "git",
            Self::Open => "open",
            Self::Node => "node",
            Self::Pnpm => "pnpm",
            Self::Cargo => "cargo",
        }
    }

    /// The fixed candidates (home-relative entries start with `~/`).
    #[must_use]
    pub const fn candidates(self) -> &'static [&'static str] {
        match self {
            Self::Claude => &["~/.local/bin/claude", "/opt/homebrew/bin/claude"],
            Self::Git => &["/usr/bin/git", "/opt/homebrew/bin/git"],
            Self::Open => &["/usr/bin/open"],
            Self::Node => &["/opt/homebrew/bin/node", "/usr/local/bin/node"],
            Self::Pnpm => &[
                "/opt/homebrew/bin/pnpm",
                "~/Library/pnpm/pnpm",
                "~/.local/share/pnpm/pnpm",
            ],
            Self::Cargo => &["~/.cargo/bin/cargo"],
        }
    }

    /// Whether Billy may point Settings at another path (not `/usr/bin/open`).
    #[must_use]
    pub const fn settable(self) -> bool {
        !matches!(self, Self::Open)
    }
}

/// The facts the spawn checks need.
#[derive(Debug, Clone, Default)]
pub struct SpawnCtx {
    /// `$HOME`.
    pub home: PathBuf,
    /// Foreman's app-data folder (cwd of non-session helpers).
    pub app_data: PathBuf,
    /// Canonical roots of every registered project.
    pub projects: Vec<PathBuf>,
    /// Settings paths per program (`settings.json` `tools`).
    pub settings: Vec<(Program, PathBuf)>,
}

/// Where the child runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cwd {
    /// App data (every non-session spawn, SR-01 / T-038).
    AppData,
    /// A session's step cwd (a `claude` launch) or git's `-C` root.
    Dir(PathBuf),
}

/// A typed spawn request (never a string).
#[derive(Debug, Clone)]
pub enum SpawnRequest {
    /// `claude -v`
    ClaudeVersion,
    /// `claude auth status` outside a launch.
    ClaudeAuthStatus,
    /// `claude auth status` inside the launch pipeline: in the step cwd with the exact launch
    /// environment (SR-08). `RunOpts::cwd` must be `Cwd::Dir`.
    ClaudeAuthStatusInLaunch,
    /// A zero-token contract probe (`--init-only …`, ADR-017), always in `<app-data>/probe/`.
    ClaudeProbe(Vec<String>),
    /// `<tool> --version` for `node`, `pnpm`, `cargo`.
    ToolVersion(Program),
    /// git with validated arguments (built by `foreman_core::git`).
    Git {
        /// Full argument list after the program (`-C <root> -c … <op>`).
        args: Vec<String>,
        /// stdin bytes (`hash-object --stdin`).
        stdin: Option<Vec<u8>>,
        /// Extra env (`GIT_INDEX_FILE` for the temporary index only).
        index_file: Option<PathBuf>,
    },
    /// `/usr/bin/open -R <path>` (Reveal in Finder).
    OpenReveal(PathBuf),
    /// `/usr/bin/open -b <bundle id> <path>`.
    OpenWithBundle {
        /// Bundle ID.
        bundle: String,
        /// Path.
        path: PathBuf,
    },
    /// `/usr/bin/open -u <url>`: a URL that passed `foreman_core::urls` (SR-25, SR-29). `-u`
    /// opens it with the app that claims its scheme, even if the text also names a file path.
    OpenUrl(foreman_core::urls::OpenUrl),
    /// The single consented shell (SR-02, ADR-007 §8, AC-F019-12): `<shell> -l -c 'printf %s
    /// "$PATH"'` with exactly [`LOGIN_SHELL_ARGS`], in app data, after Billy's native dialog. The
    /// caller has checked that `shell` is listed in `/etc/shells`.
    LoginShellPath {
        /// `$SHELL`: absolute, an executable regular file.
        shell: PathBuf,
        /// Proof the native consent dialog was answered.
        consent: LoginShellConsent,
    },
}

/// The login shell's fixed argument vector (SR-02): a login shell that prints `$PATH` and
/// nothing else. No part of it comes from input.
pub const LOGIN_SHELL_ARGS: [&str; 3] = ["-l", "-c", "printf %s \"$PATH\""];

/// Proof that Billy consented to the login-shell run in a native dialog (SR-02, SR-24a). Only
/// `foreman-app`'s `doctor_capture_login_path` makes one, right after the dialog returns OK.
#[derive(Debug, Clone)]
pub struct LoginShellConsent(());

impl LoginShellConsent {
    /// Billy answered the native consent dialog with Capture.
    #[must_use]
    pub const fn granted_in_native_dialog() -> Self {
        Self(())
    }
}

/// How to run it.
#[derive(Debug, Clone)]
pub struct RunOpts {
    /// The child's environment (exactly these variables; `env_clear()` first).
    pub env: Vec<(String, String)>,
    /// Timeout.
    pub timeout: Duration,
    /// Output cap per stream.
    pub output_cap: usize,
    /// Working directory.
    pub cwd: Cwd,
}

/// A finished child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// Exit code (`None` if killed by a signal).
    pub code: Option<i32>,
    /// stdout.
    pub stdout: Vec<u8>,
    /// stderr.
    pub stderr: Vec<u8>,
}

/// Why a spawn was refused or failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpawnError {
    /// The configured path isn't absolute.
    #[error("{0}: not an absolute path")]
    RelativePath(String),
    /// No candidate or settings path is the program.
    #[error("{0} is not an allowlisted program")]
    Unlisted(String),
    /// Missing or not an executable regular file.
    #[error("{0}: missing or not executable")]
    NotExecutable(String),
    /// Inside (or resolving into) a registered project.
    #[error("{0} is inside a project folder")]
    InsideProject(String),
    /// In (or resolving into) a world-writable folder.
    #[error("{0} is in a world-writable folder")]
    WorldWritable(String),
    /// The arguments for a request kind are wrong (e.g. `open` with a relative path).
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// The OS refused the spawn.
    #[error("spawn failed: {0}")]
    Spawn(String),
    /// The child ran past its timeout and was killed.
    #[error("timed out after {0:?}")]
    Timeout(Duration),
    /// The child printed more than the cap.
    #[error("output over {0} bytes")]
    OutputTooLarge(usize),
}

/// The canonical form of a path (the path itself when it can't be resolved).
#[must_use]
pub fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn expand(c: &str, home: &Path) -> PathBuf {
    c.strip_prefix("~/")
        .map_or_else(|| PathBuf::from(c), |rest| home.join(rest))
}

fn inside_any(p: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|r| p.starts_with(r))
}

fn world_writable_dir(p: &Path) -> bool {
    p.ancestors().skip(1).any(|a| {
        std::fs::metadata(a).is_ok_and(|m| {
            m.is_dir()
                && m.permissions().mode() & 0o002 != 0
                && m.permissions().mode() & 0o1000 == 0
        })
    })
}

/// Checks one path for a program: absolute, the program's file name, an executable regular file,
/// no ancestor inside a registered project, and a canonical target that isn't inside a project or
/// a world-writable folder (SR-01).
///
/// # Errors
/// See [`SpawnError`].
pub fn check_program_path(
    program: Program,
    p: &Path,
    ctx: &SpawnCtx,
) -> Result<PathBuf, SpawnError> {
    let shown = p.display().to_string();
    if !p.is_absolute() {
        return Err(SpawnError::RelativePath(shown));
    }
    if p.file_name().and_then(|n| n.to_str()) != Some(program.file_name()) {
        return Err(SpawnError::Unlisted(shown));
    }
    let canon_projects: Vec<PathBuf> = ctx
        .projects
        .iter()
        .map(|r| std::fs::canonicalize(r).unwrap_or_else(|_| r.clone()))
        .chain(ctx.projects.iter().cloned())
        .collect();
    if inside_any(p, &canon_projects) {
        return Err(SpawnError::InsideProject(shown));
    }
    let target = std::fs::canonicalize(p).map_err(|_| SpawnError::NotExecutable(shown.clone()))?;
    if inside_any(&target, &canon_projects) {
        return Err(SpawnError::InsideProject(shown));
    }
    let md = std::fs::metadata(&target).map_err(|_| SpawnError::NotExecutable(shown.clone()))?;
    if !md.is_file() || md.permissions().mode() & 0o111 == 0 {
        return Err(SpawnError::NotExecutable(shown));
    }
    if world_writable_dir(&target) || world_writable_dir(p) {
        return Err(SpawnError::WorldWritable(shown));
    }
    Ok(p.to_path_buf())
}

/// Resolves a program: the Settings path when set (checked), else the first candidate that passes.
///
/// # Errors
/// The Settings path's problem, or [`SpawnError::Unlisted`] when no candidate exists.
pub fn resolve(program: Program, ctx: &SpawnCtx) -> Result<PathBuf, SpawnError> {
    if program.settable()
        && let Some((_, p)) = ctx.settings.iter().find(|(k, _)| *k == program)
    {
        return check_program_path(program, p, ctx);
    }
    program
        .candidates()
        .iter()
        .map(|c| expand(c, &ctx.home))
        .find_map(|c| check_program_path(program, &c, ctx).ok())
        .ok_or_else(|| SpawnError::Unlisted(program.file_name().to_owned()))
}

fn program_and_args(req: &SpawnRequest) -> Result<(Program, Vec<String>), SpawnError> {
    Ok(match req {
        SpawnRequest::ClaudeVersion => (Program::Claude, vec!["-v".into()]),
        SpawnRequest::ClaudeAuthStatus | SpawnRequest::ClaudeAuthStatusInLaunch => {
            (Program::Claude, vec!["auth".into(), "status".into()])
        }
        SpawnRequest::ClaudeProbe(args) => {
            check_claude_args(args)?;
            if args.first().map(String::as_str) != Some("--init-only") {
                return Err(SpawnError::InvalidRequest(
                    "a probe starts with --init-only".into(),
                ));
            }
            (Program::Claude, args.clone())
        }
        SpawnRequest::ToolVersion(p) => {
            if !matches!(p, Program::Node | Program::Pnpm | Program::Cargo) {
                return Err(SpawnError::InvalidRequest(format!("{p:?} --version")));
            }
            (*p, vec!["--version".into()])
        }
        SpawnRequest::Git { args, .. } => (Program::Git, args.clone()),
        SpawnRequest::OpenReveal(path) => {
            if !path.is_absolute() {
                return Err(SpawnError::InvalidRequest(
                    "open -R needs an absolute path".into(),
                ));
            }
            (Program::Open, vec!["-R".into(), path.display().to_string()])
        }
        SpawnRequest::OpenWithBundle { bundle, path } => {
            let ok = !bundle.starts_with('-')
                && bundle.split('.').count() >= 2
                && bundle
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-');
            if !ok || !path.is_absolute() {
                return Err(SpawnError::InvalidRequest(
                    "open -b needs a bundle ID and an absolute path".into(),
                ));
            }
            (
                Program::Open,
                vec!["-b".into(), bundle.clone(), path.display().to_string()],
            )
        }
        SpawnRequest::LoginShellPath { .. } => {
            return Err(SpawnError::InvalidRequest(
                "the login shell isn't an allowlisted program".into(),
            ));
        }
        SpawnRequest::OpenUrl(url) => {
            // Only `https:`/`http:` URLs are constructible, so the argument can't start with `-`;
            // re-checked here anyway, as every request kind is.
            if url.as_str().starts_with('-') || !url.as_str().starts_with("http") {
                return Err(SpawnError::InvalidRequest("open -u needs a URL".into()));
            }
            (Program::Open, vec!["-u".into(), url.as_str().to_owned()])
        }
    })
}

/// Checks a `claude` argument vector: every flag from the fixed list, no forbidden spelling
/// (SR-03, SR-16). Values follow their flags; the prompt (a non-flag last element) is allowed.
///
/// # Errors
/// [`SpawnError::InvalidRequest`] naming the argument.
pub fn check_claude_args(args: &[String]) -> Result<(), SpawnError> {
    use foreman_core::launch::argv::{KNOWN_FLAGS, is_forbidden};
    const TAKES_VALUE: [&str; 7] = [
        "--session-id",
        "--resume",
        "--model",
        "--effort",
        "--permission-mode",
        "--name",
        "--settings",
    ];
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if is_forbidden(a) {
            return Err(SpawnError::InvalidRequest(format!(
                "forbidden argument {a}"
            )));
        }
        if a.starts_with('-') {
            if !KNOWN_FLAGS.contains(&a.as_str()) {
                return Err(SpawnError::InvalidRequest(format!("unknown flag {a}")));
            }
            if TAKES_VALUE.contains(&a.as_str()) {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| SpawnError::InvalidRequest(format!("{a} needs a value")))?;
                if is_forbidden(v) {
                    return Err(SpawnError::InvalidRequest(format!("forbidden value {v}")));
                }
                i += 1;
            }
        } else if i + 1 != args.len() {
            return Err(SpawnError::InvalidRequest(
                "only the last argument may be positional (the prompt)".into(),
            ));
        } else {
            foreman_core::sanitize::check_prompt(a)
                .map_err(|e| SpawnError::InvalidRequest(e.to_string()))?;
        }
        i += 1;
    }
    Ok(())
}

/// A checked interactive `claude` command for a PTY (ADR-006): the re-resolved absolute program,
/// argv from the core builder, exactly the built environment (`env_clear` first), the step cwd
/// (canonical, an existing directory: portable-pty would silently fall back to `$HOME`), and
/// umask 022 (SR-57). This is the only `CommandBuilder` construction site.
///
/// # Errors
/// [`SpawnError`] for the program checks, bad args, or a cwd that isn't a canonical directory.
pub fn claude_pty_command(
    ctx: &SpawnCtx,
    args: &[String],
    env: &[(String, String)],
    cwd: &Path,
) -> Result<(portable_pty::CommandBuilder, PathBuf), SpawnError> {
    check_claude_args(args)?;
    check_session_cwd(cwd)?;
    let exe = resolve(Program::Claude, ctx)?;
    let mut cmd = portable_pty::CommandBuilder::new(&exe);
    cmd.args(args);
    cmd.env_clear();
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.cwd(cwd);
    cmd.umask(Some(0o022));
    Ok((cmd, exe))
}

/// The session cwd must be absolute, canonical (no symlink anywhere in it) and a directory.
///
/// # Errors
/// [`SpawnError::InvalidRequest`].
pub fn check_session_cwd(cwd: &Path) -> Result<(), SpawnError> {
    let bad =
        || SpawnError::InvalidRequest(format!("{}: not a canonical directory", cwd.display()));
    if !cwd.is_absolute() {
        return Err(bad());
    }
    match std::fs::canonicalize(cwd) {
        Ok(c) if c == cwd => {}
        _ => return Err(bad()),
    }
    match std::fs::symlink_metadata(cwd) {
        Ok(m) if m.is_dir() => Ok(()),
        _ => Err(bad()),
    }
}

fn reader<R: std::io::Read + Send + 'static>(
    r: Option<R>,
    cap: usize,
) -> std::thread::JoinHandle<(Vec<u8>, bool)> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut over = false;
        if let Some(mut r) = r {
            let mut chunk = [0u8; 8192];
            loop {
                match r.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if buf.len() + n > cap {
                            over = true;
                        } else {
                            buf.extend_from_slice(&chunk[..n]);
                        }
                    }
                }
            }
        }
        (buf, over)
    })
}

/// Spawns a request and waits for it (non-PTY children; PTY sessions arrive with M3).
///
/// # Errors
/// See [`SpawnError`].
pub fn run(req: &SpawnRequest, opts: &RunOpts, ctx: &SpawnCtx) -> Result<Output, SpawnError> {
    if let SpawnRequest::LoginShellPath { shell, consent: _ } = req {
        let exe = check_login_shell(shell, ctx)?;
        let args: Vec<String> = LOGIN_SHELL_ARGS.iter().map(|a| (*a).to_owned()).collect();
        return exec(&exe, &args, &ctx.app_data, opts, None, None);
    }
    let (program, args) = program_and_args(req)?;
    let exe = resolve(program, ctx)?;
    let cwd = match (&opts.cwd, req) {
        // Probes run only in the probe folder (SR-17).
        (_, SpawnRequest::ClaudeProbe(_)) => ctx.app_data.join("probe"),
        // In-launch auth runs in the step cwd (SR-08) and nowhere else.
        (Cwd::Dir(d), SpawnRequest::ClaudeAuthStatusInLaunch) => d.clone(),
        (Cwd::AppData, SpawnRequest::ClaudeAuthStatusInLaunch) => {
            return Err(SpawnError::InvalidRequest(
                "auth status in a launch needs the step cwd".into(),
            ));
        }
        // Non-session helpers always run in app data, whatever was asked (T-038).
        (
            _,
            SpawnRequest::ClaudeVersion
            | SpawnRequest::ClaudeAuthStatus
            | SpawnRequest::ToolVersion(_),
        )
        | (Cwd::AppData, _) => ctx.app_data.clone(),
        (Cwd::Dir(d), _) => d.clone(),
    };
    let stdin_bytes = match req {
        SpawnRequest::Git { stdin, .. } => stdin.clone(),
        _ => None,
    };
    let index = match req {
        SpawnRequest::Git {
            index_file: Some(f),
            ..
        } => Some(f.as_path()),
        _ => None,
    };
    exec(&exe, &args, &cwd, opts, stdin_bytes, index)
}

/// The login shell's checks (SR-01's path rules for a program that isn't on the allowlist by
/// name): absolute, an executable regular file, not inside a registered project, not in a
/// world-writable folder.
fn check_login_shell(shell: &Path, ctx: &SpawnCtx) -> Result<PathBuf, SpawnError> {
    let shown = shell.display().to_string();
    if !shell.is_absolute() {
        return Err(SpawnError::RelativePath(shown));
    }
    let target =
        std::fs::canonicalize(shell).map_err(|_| SpawnError::NotExecutable(shown.clone()))?;
    let roots: Vec<PathBuf> = ctx
        .projects
        .iter()
        .map(|r| std::fs::canonicalize(r).unwrap_or_else(|_| r.clone()))
        .chain(ctx.projects.iter().cloned())
        .collect();
    if inside_any(shell, &roots) || inside_any(&target, &roots) {
        return Err(SpawnError::InsideProject(shown));
    }
    let md = std::fs::metadata(&target).map_err(|_| SpawnError::NotExecutable(shown.clone()))?;
    if !md.is_file() || md.permissions().mode() & 0o111 == 0 {
        return Err(SpawnError::NotExecutable(shown));
    }
    if world_writable_dir(&target) || world_writable_dir(shell) {
        return Err(SpawnError::WorldWritable(shown));
    }
    Ok(shell.to_path_buf())
}

/// Spawns `exe` with exactly `args`, `opts.env` (after `env_clear`) and `cwd`, in its own process
/// group, and waits up to the timeout with capped output.
fn exec(
    exe: &Path,
    args: &[String],
    cwd: &Path,
    opts: &RunOpts,
    stdin_bytes: Option<Vec<u8>>,
    index_file: Option<&Path>,
) -> Result<Output, SpawnError> {
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .env_clear()
        .current_dir(cwd)
        .process_group(0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if stdin_bytes.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    for (k, v) in &opts.env {
        cmd.env(k, v);
    }
    if let Some(f) = index_file {
        cmd.env("GIT_INDEX_FILE", f);
    }
    let mut child = cmd.spawn().map_err(|e| SpawnError::Spawn(e.to_string()))?;
    let pid = child.id();
    if let (Some(bytes), Some(mut w)) = (stdin_bytes, child.stdin.take()) {
        let _ = w.write_all(&bytes);
    }
    let out = reader(child.stdout.take(), opts.output_cap);
    let err = reader(child.stderr.take(), opts.output_cap);
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                if start.elapsed() >= opts.timeout {
                    if let Some(pg) = i32::try_from(pid)
                        .ok()
                        .and_then(rustix::process::Pid::from_raw)
                    {
                        let _ =
                            rustix::process::kill_process_group(pg, rustix::process::Signal::KILL);
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(SpawnError::Timeout(opts.timeout));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(SpawnError::Spawn(e.to_string())),
        }
    };
    let (stdout, o1) = out.join().unwrap_or_default();
    let (stderr, o2) = err.join().unwrap_or_default();
    if o1 || o2 {
        return Err(SpawnError::OutputTooLarge(opts.output_cap));
    }
    Ok(Output {
        code: status.code(),
        stdout,
        stderr,
    })
}

/// What `PATH` sanitizing needs to know about a candidate folder (SR-05): it exists as a
/// directory, and neither it nor an ancestor is world-writable without the sticky bit.
#[must_use]
pub fn path_dir_facts(p: &Path) -> foreman_core::launch::env::DirFacts {
    let is_dir = std::fs::metadata(p).is_ok_and(|m| m.is_dir());
    let world = |a: &Path| {
        std::fs::metadata(a).is_ok_and(|m| {
            m.is_dir()
                && m.permissions().mode() & 0o002 != 0
                && m.permissions().mode() & 0o1000 == 0
        })
    };
    foreman_core::launch::env::DirFacts {
        is_dir,
        world_writable: is_dir && p.ancestors().any(world),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SR-25/SR-29: a URL opens as `/usr/bin/open -u <url>` and nothing else.
    #[test]
    fn open_url_argv() {
        let url = foreman_core::urls::foreman_url("https://claude.ai/chat/abc").unwrap();
        let (p, args) = program_and_args(&SpawnRequest::OpenUrl(url)).unwrap();
        assert_eq!(p, Program::Open);
        assert_eq!(args, ["-u", "https://claude.ai/chat/abc"]);
    }
}
