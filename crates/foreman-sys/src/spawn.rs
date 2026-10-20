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
        SpawnRequest::ClaudeAuthStatus => (Program::Claude, vec!["auth".into(), "status".into()]),
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
    })
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
    let (program, args) = program_and_args(req)?;
    let exe = resolve(program, ctx)?;
    let cwd = match (&opts.cwd, req) {
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
    let mut cmd = Command::new(&exe);
    cmd.args(&args)
        .env_clear()
        .current_dir(&cwd)
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
    if let SpawnRequest::Git {
        index_file: Some(f),
        ..
    } = req
    {
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
