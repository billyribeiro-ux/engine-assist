//! `git::run`: the single git entry point (ADR-012, SR-44, SR-45, DR-140, DR-141). Every call is
//! `<abs git> -C <root> -c <SR-45 overrides> <closed-enum op>`, with `env_clear()` plus `HOME`, a
//! fixed `PATH`, `LANG=C`, `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0` (and `GIT_INDEX_FILE`
//! only for Foreman's temporary index), a 60 s timeout and an 8 MB output cap. Before any
//! operation on an existing repo, `rev-parse --show-toplevel` must equal the root.

use std::path::{Path, PathBuf};
use std::time::Duration;

use foreman_core::git::{self as core_git, GitArgError, GitOp, RevParse};

use crate::spawn::{self, Cwd, RunOpts, SpawnCtx, SpawnError, SpawnRequest};

pub mod commit;
pub mod show;

/// The default timeout (SR-45).
pub const TIMEOUT: Duration = Duration::from_secs(60);
/// The output cap (ADR-012).
pub const OUTPUT_CAP: usize = 8 * 1024 * 1024;

/// Options for one call.
#[derive(Debug, Clone)]
pub struct GitOpts {
    /// Timeout (60 s by default).
    pub timeout: Duration,
    /// stdin bytes for `hash-object --stdin`.
    pub stdin: Option<Vec<u8>>,
    /// Foreman's temporary index (`<app-data>/tmp/git-index-<uuid>`).
    pub index_file: Option<PathBuf>,
    /// Neutralize attributes (T-040): `-c core.attributesFile=/dev/null` and
    /// `GIT_ATTR_SOURCE=<empty tree>` (git ≥ 2.40), so a racily-clean entry hashed while the real
    /// index is written can't run a worktree `.gitattributes` filter driver.
    pub neutralize_attributes: bool,
}

impl Default for GitOpts {
    fn default() -> Self {
        Self {
            timeout: TIMEOUT,
            stdin: None,
            index_file: None,
            neutralize_attributes: false,
        }
    }
}

/// A git result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOutput {
    /// Exit code.
    pub code: Option<i32>,
    /// stdout.
    pub stdout: Vec<u8>,
    /// stderr.
    pub stderr: Vec<u8>,
}

impl GitOutput {
    /// True on exit 0.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }

    /// stdout as trimmed text.
    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout)
            .trim_end_matches(['\n', '\r'])
            .to_owned()
    }
}

/// A git failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GitError {
    /// Arguments failed validation.
    #[error(transparent)]
    Args(#[from] GitArgError),
    /// git couldn't be resolved or spawned, timed out, or printed too much.
    #[error(transparent)]
    Spawn(#[from] SpawnError),
    /// The root isn't its own repository (an enclosing repo, or none).
    #[error(
        "{root} is not its own git repository (git found {found:?}). Initialize a repository here"
    )]
    NotOwnRepo {
        /// The root.
        root: String,
        /// What `--show-toplevel` said.
        found: Option<String>,
    },
    /// `read-tree` without Foreman's temporary index.
    #[error("read-tree runs only on Foreman's temporary index")]
    NeedsTempIndex,
    /// git exited non-zero where success was required.
    #[error("git {op} failed: {stderr}")]
    Failed {
        /// The subcommand.
        op: String,
        /// stderr.
        stderr: String,
    },
}

/// The empty tree's OID (SHA-1), always known to git.
pub const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// The fixed environment git runs with (SR-45).
#[must_use]
pub fn env(home: &Path, git: &Path) -> Vec<(String, String)> {
    let mut path = String::from("/usr/bin:/bin:/usr/sbin:/sbin");
    if let Some(dir) = git.parent() {
        let d = dir.display().to_string();
        if !path.split(':').any(|p| p == d) {
            path.push(':');
            path.push_str(&d);
        }
    }
    vec![
        ("HOME".into(), home.display().to_string()),
        ("PATH".into(), path),
        ("LANG".into(), "C".into()),
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GIT_OPTIONAL_LOCKS".into(), "0".into()),
    ]
}

/// The full argv after the program for an operation on `root`.
///
/// # Errors
/// When the operation's arguments are invalid.
pub fn argv(root: &Path, op: &GitOp) -> Result<Vec<String>, GitError> {
    let a = core_git::args(op)?;
    let mut v = core_git::prefix(&root.display().to_string());
    v.extend(a.args);
    Ok(v)
}

fn raw(root: &Path, op: &GitOp, opts: &GitOpts, ctx: &SpawnCtx) -> Result<GitOutput, GitError> {
    let a = core_git::args(op)?;
    if a.needs_temp_index && opts.index_file.is_none() {
        return Err(GitError::NeedsTempIndex);
    }
    let git = spawn::resolve(spawn::Program::Git, ctx)?;
    let mut args = core_git::prefix(&root.display().to_string());
    let mut e = env(&ctx.home, &git);
    if opts.neutralize_attributes {
        args.push("-c".into());
        args.push("core.attributesFile=/dev/null".into());
        e.push(("GIT_ATTR_SOURCE".into(), EMPTY_TREE.into()));
    }
    args.extend(a.args);
    let req = SpawnRequest::Git {
        args,
        stdin: if a.stdin {
            Some(opts.stdin.clone().unwrap_or_default())
        } else {
            None
        },
        index_file: opts.index_file.clone(),
    };
    let ro = RunOpts {
        env: e,
        timeout: opts.timeout,
        output_cap: OUTPUT_CAP,
        cwd: Cwd::Dir(root.to_path_buf()),
    };
    let out = spawn::run(&req, &ro, ctx)?;
    Ok(GitOutput {
        code: out.code,
        stdout: out.stdout,
        stderr: out.stderr,
    })
}

/// Runs one operation on `root` (the project root or the blueprint repo root). Every operation
/// except `init` first proves `root` is its own repository (DR-141).
///
/// # Errors
/// See [`GitError`]. A non-zero exit is returned as output, not an error.
pub fn run(root: &Path, op: &GitOp, opts: &GitOpts, ctx: &SpawnCtx) -> Result<GitOutput, GitError> {
    let a = core_git::args(op)?;
    if !a.before_repo {
        let top = raw(
            root,
            &GitOp::RevParse(RevParse::ShowToplevel),
            &GitOpts::default(),
            ctx,
        )?;
        let found = top.ok().then(|| top.text());
        let same = found
            .as_deref()
            .is_some_and(|f| spawn::canonical(Path::new(f)) == spawn::canonical(root));
        if !same {
            return Err(GitError::NotOwnRepo {
                root: root.display().to_string(),
                found,
            });
        }
    }
    raw(root, op, opts, ctx)
}

/// Runs an operation and requires exit 0, returning trimmed stdout.
///
/// # Errors
/// [`GitError::Failed`] on a non-zero exit, or as [`run`].
pub fn run_ok(root: &Path, op: &GitOp, opts: &GitOpts, ctx: &SpawnCtx) -> Result<String, GitError> {
    let out = run(root, op, opts, ctx)?;
    if out.ok() {
        Ok(out.text())
    } else {
        let name = core_git::args(op)
            .map(|a| a.args[0].clone())
            .unwrap_or_default();
        Err(GitError::Failed {
            op: name,
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        })
    }
}

/// Why the exclude writer couldn't run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExcludeSetupError {
    /// git failed or the root isn't its own repo.
    #[error(transparent)]
    Git(#[from] GitError),
    /// The writer refused.
    #[error(transparent)]
    Writer(#[from] foreman_core::store::git_exclude::ExcludeError),
}

/// SR-32 at setup and open: asks git for `--git-path info/exclude` and `--git-common-dir`
/// (after `git::run` confirmed the root is its own repository) and appends the temp pattern once.
///
/// # Errors
/// See [`ExcludeSetupError`].
pub fn ensure_exclude(
    project: &foreman_core::store::Root,
    ctx: &SpawnCtx,
) -> Result<foreman_core::store::git_exclude::Ensured, ExcludeSetupError> {
    let root = project.path();
    let g = GitOpts::default();
    let ex = run_ok(
        root,
        &GitOp::RevParse(RevParse::GitPath("info/exclude".into())),
        &g,
        ctx,
    )?;
    let common = run_ok(root, &GitOp::RevParse(RevParse::GitCommonDir), &g, ctx)?;
    Ok(foreman_core::store::git_exclude::ensure(
        project,
        Path::new(&ex),
        Path::new(&common),
    )?)
}
