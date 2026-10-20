//! Pure git argument builders over a closed operation enum (ADR-012 decision 1, DR-140, SR-45).
//! There is no `status`, `diff`, `add`, `commit` or `ls-files` operation and no free-form argv.
//! `foreman-sys::git::run` is the only function that spawns git.

use crate::ids::is_git_oid;

/// The fixed `-c` overrides on every git invocation (SR-45), in order.
pub const OVERRIDES: [&str; 10] = [
    "core.hooksPath=/dev/null",
    "core.fsmonitor=false",
    "core.pager=cat",
    "core.editor=true",
    "sequence.editor=true",
    "commit.gpgSign=false",
    "tag.gpgSign=false",
    "core.sshCommand=false",
    "credential.helper=",
    "protocol.allow=never",
];

/// The git subcommands Foreman runs: exactly DR-140's list.
pub const SUBCOMMANDS: [&str; 13] = [
    "init",
    "rev-parse",
    "symbolic-ref",
    "config",
    "cat-file",
    "ls-tree",
    "hash-object",
    "update-index",
    "read-tree",
    "write-tree",
    "commit-tree",
    "update-ref",
    "show",
];

/// The zero OID (an unborn branch's "old" value for `update-ref`).
pub const ZERO_OID: &str = "0000000000000000000000000000000000000000";

/// `rev-parse` forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevParse {
    /// `--show-toplevel`
    ShowToplevel,
    /// `--git-path <path>`
    GitPath(String),
    /// `--git-common-dir`
    GitCommonDir,
    /// `--verify -q HEAD`
    VerifyHead,
    /// `<rev>:<path>` (the blob OID at a revision)
    BlobAt {
        /// `HEAD` or an OID.
        rev: String,
        /// Repo path.
        path: String,
    },
}

/// `read-tree` sources (temporary index only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadTreeFrom {
    /// `read-tree <tip>`
    Tip(String),
    /// `read-tree --empty`
    Empty,
}

/// The closed operation enum (DR-140).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitOp {
    /// `init`
    Init,
    /// `rev-parse …`
    RevParse(RevParse),
    /// `symbolic-ref -q HEAD`
    SymbolicRefHead,
    /// `config --get <key>` (optionally `--show-origin --show-scope`)
    ConfigGet {
        /// Key.
        key: String,
        /// Also print origin and scope.
        show_origin_and_scope: bool,
    },
    /// `cat-file blob <rev>:<path>`
    CatFileBlob {
        /// Revision.
        rev: String,
        /// Path.
        path: String,
    },
    /// `ls-tree <rev> -- <path>`
    LsTree {
        /// Revision.
        rev: String,
        /// Path.
        path: String,
    },
    /// `show <rev>:<path>` (raw blob)
    ShowBlob {
        /// Revision.
        rev: String,
        /// Path.
        path: String,
    },
    /// `hash-object [-w] --no-filters --stdin`
    HashObjectNoFilters {
        /// Write the object.
        write: bool,
    },
    /// `update-index --add --cacheinfo 100644,<oid>,<path>` per entry
    UpdateIndexCacheInfo {
        /// `(oid, path)` entries.
        entries: Vec<(String, String)>,
    },
    /// `read-tree <tip>` / `read-tree --empty` (temporary index only)
    ReadTree(ReadTreeFrom),
    /// `write-tree`
    WriteTree,
    /// `commit-tree <tree> [-p <parent>] --no-gpg-sign -m <msg>`
    CommitTree {
        /// Tree OID.
        tree: String,
        /// Parent OID (none for the first commit).
        parent: Option<String>,
        /// Message.
        msg: String,
    },
    /// `update-ref <ref> <new> <old>`
    UpdateRef {
        /// `HEAD` or `refs/heads/<name>`.
        refname: String,
        /// New OID.
        new: String,
        /// Old OID (zero OID for an unborn branch).
        old: String,
    },
}

/// Arguments for one git call (after `git -C <root> -c …`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitArgs {
    /// The operation's arguments, starting with the subcommand.
    pub args: Vec<String>,
    /// Must run against Foreman's temporary index (`GIT_INDEX_FILE`).
    pub needs_temp_index: bool,
    /// Reads stdin (`hash-object --stdin`).
    pub stdin: bool,
    /// Runs before the repository exists (skips the own-repo check).
    pub before_repo: bool,
}

/// An argument that failed validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid git argument {what}: {value:?}")]
pub struct GitArgError {
    /// Which argument.
    pub what: &'static str,
    /// Its value.
    pub value: String,
}

fn bad(what: &'static str, value: &str) -> GitArgError {
    GitArgError {
        what,
        value: value.to_owned(),
    }
}

fn rev(r: &str) -> Result<&str, GitArgError> {
    if r == "HEAD" || is_git_oid(r) {
        Ok(r)
    } else {
        Err(bad("rev", r))
    }
}

fn path(p: &str) -> Result<&str, GitArgError> {
    let ok = !p.is_empty()
        && !p.starts_with('-')
        && !p.starts_with('/')
        && !p.contains(':')
        && !p.chars().any(char::is_control)
        && p.split('/').all(|c| !c.is_empty() && c != "." && c != "..");
    if ok { Ok(p) } else { Err(bad("path", p)) }
}

fn oid(o: &str) -> Result<&str, GitArgError> {
    if is_git_oid(o) {
        Ok(o)
    } else {
        Err(bad("oid", o))
    }
}

fn refname(r: &str) -> Result<&str, GitArgError> {
    let ok = r == "HEAD"
        || r.strip_prefix("refs/heads/").is_some_and(|b| {
            !b.is_empty()
                && !b.starts_with('-')
                && !b.contains("..")
                && b.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"/._-".contains(&c))
        });
    if ok { Ok(r) } else { Err(bad("ref", r)) }
}

fn config_key(k: &str) -> Result<&str, GitArgError> {
    let ok = k.contains('.')
        && k.starts_with(|c: char| c.is_ascii_alphabetic())
        && k.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-');
    if ok { Ok(k) } else { Err(bad("config key", k)) }
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

#[allow(clippy::too_many_lines, reason = "one match arm per DR-140 operation, kept together for review")]
/// Builds the validated arguments of an operation.
///
/// # Errors
/// When a revision, OID, path, ref or key isn't well-formed (nothing option-like gets through).
pub fn args(op: &GitOp) -> Result<GitArgs, GitArgError> {
    let mut g = GitArgs {
        args: Vec::new(),
        needs_temp_index: false,
        stdin: false,
        before_repo: false,
    };
    g.args = match op {
        GitOp::Init => {
            g.before_repo = true;
            s(&["init", "-q"])
        }
        GitOp::RevParse(r) => match r {
            RevParse::ShowToplevel => s(&["rev-parse", "--show-toplevel"]),
            RevParse::GitPath(p) => vec!["rev-parse".into(), "--git-path".into(), path(p)?.into()],
            RevParse::GitCommonDir => s(&["rev-parse", "--git-common-dir"]),
            RevParse::VerifyHead => s(&["rev-parse", "--verify", "-q", "HEAD"]),
            RevParse::BlobAt { rev: r, path: p } => {
                vec!["rev-parse".into(), format!("{}:{}", rev(r)?, path(p)?)]
            }
        },
        GitOp::SymbolicRefHead => s(&["symbolic-ref", "-q", "HEAD"]),
        GitOp::ConfigGet {
            key,
            show_origin_and_scope,
        } => {
            let mut a = s(&["config"]);
            if *show_origin_and_scope {
                a.extend(s(&["--show-origin", "--show-scope"]));
            }
            a.push("--get".into());
            a.push(config_key(key)?.into());
            a
        }
        GitOp::CatFileBlob { rev: r, path: p } => {
            vec![
                "cat-file".into(),
                "blob".into(),
                format!("{}:{}", rev(r)?, path(p)?),
            ]
        }
        GitOp::LsTree { rev: r, path: p } => {
            vec![
                "ls-tree".into(),
                rev(r)?.into(),
                "--".into(),
                path(p)?.into(),
            ]
        }
        GitOp::ShowBlob { rev: r, path: p } => {
            vec!["show".into(), format!("{}:{}", rev(r)?, path(p)?)]
        }
        GitOp::HashObjectNoFilters { write } => {
            g.stdin = true;
            let mut a = s(&["hash-object"]);
            if *write {
                a.push("-w".into());
            }
            a.extend(s(&["--no-filters", "--stdin"]));
            a
        }
        GitOp::UpdateIndexCacheInfo { entries } => {
            if entries.is_empty() {
                return Err(bad("entries", ""));
            }
            let mut a = s(&["update-index", "--add"]);
            for (o, p) in entries {
                a.push("--cacheinfo".into());
                a.push(format!("100644,{},{}", oid(o)?, path(p)?));
            }
            a
        }
        GitOp::ReadTree(from) => {
            g.needs_temp_index = true;
            match from {
                ReadTreeFrom::Tip(t) => vec!["read-tree".into(), oid(t)?.into()],
                ReadTreeFrom::Empty => s(&["read-tree", "--empty"]),
            }
        }
        GitOp::WriteTree => s(&["write-tree"]),
        GitOp::CommitTree { tree, parent, msg } => {
            let mut a = vec!["commit-tree".into(), oid(tree)?.into()];
            if let Some(p) = parent {
                a.push("-p".into());
                a.push(oid(p)?.into());
            }
            if msg.is_empty() || msg.chars().any(|c| c.is_control() && c != '\n') {
                return Err(bad("message", msg));
            }
            a.push("--no-gpg-sign".into());
            a.push("-m".into());
            a.push(msg.clone());
            a
        }
        GitOp::UpdateRef {
            refname: r,
            new,
            old,
        } => vec![
            "update-ref".into(),
            refname(r)?.into(),
            oid(new)?.into(),
            oid(old)?.into(),
        ],
    };
    Ok(g)
}

/// The global options before the subcommand: `-C <root>` and every `-c` override.
#[must_use]
pub fn prefix(root: &str) -> Vec<String> {
    let mut v = vec!["-C".to_owned(), root.to_owned()];
    for o in OVERRIDES {
        v.push("-c".into());
        v.push(o.into());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_like_values_refused() {
        assert!(
            args(&GitOp::CatFileBlob {
                rev: "--output=x".into(),
                path: "a".into()
            })
            .is_err()
        );
        assert!(
            args(&GitOp::LsTree {
                rev: "HEAD".into(),
                path: "-x".into()
            })
            .is_err()
        );
        assert!(
            args(&GitOp::ConfigGet {
                key: "--global".into(),
                show_origin_and_scope: false
            })
            .is_err()
        );
        assert!(
            args(&GitOp::UpdateRef {
                refname: "refs/heads/-x".into(),
                new: ZERO_OID.into(),
                old: ZERO_OID.into()
            })
            .is_err()
        );
        assert!(
            args(&GitOp::RevParse(RevParse::BlobAt {
                rev: "HEAD".into(),
                path: "../x".into()
            }))
            .is_err()
        );
    }
}
