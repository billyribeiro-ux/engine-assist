//! The child environment and `PATH` (SR-05, ADR-007 §4–5, DR-123).
//!
//! The child starts from `env_clear()` and gets exactly [`build_env`]'s list: the base allowlist,
//! the two Claude variables taken only from Foreman's own environment, and Billy's pass-through
//! names, minus the always-removed families, which beat the pass-through.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Copied from Foreman's own environment when present (SR-05).
pub const BASE: [&str; 6] = ["HOME", "USER", "LOGNAME", "SHELL", "TMPDIR", "LANG"];

/// Taken only from Foreman's own environment, never from the pass-through list or a settings file.
pub const OWN_ONLY: [&str; 2] = ["CLAUDE_CODE_OAUTH_TOKEN", "CLAUDE_CONFIG_DIR"];

/// Fixed values (SR-05).
pub const FIXED: [(&str, &str); 2] = [("TERM", "xterm-256color"), ("COLORTERM", "truecolor")];

/// The always-removed families as `(prefix, suffix)` patterns: `("ANTHROPIC_", "")` is
/// `ANTHROPIC_*`, `("", "_PROXY")` is `*_PROXY`, `("ENV", "")` with exact match is `ENV`.
pub const REMOVED: [Pattern; 21] = [
    Pattern::Prefix("ANTHROPIC_"),
    Pattern::Prefix("CLAUDE_CODE_"),
    Pattern::Prefix("AWS_"),
    Pattern::Prefix("GOOGLE_"),
    Pattern::Prefix("GCLOUD_"),
    Pattern::Prefix("CLOUDSDK_"),
    Pattern::Prefix("AZURE_"),
    Pattern::Suffix("_PROXY"),
    Pattern::Suffix("_proxy"),
    Pattern::Exact("NODE_EXTRA_CA_CERTS"),
    Pattern::Exact("NODE_TLS_REJECT_UNAUTHORIZED"),
    Pattern::Exact("SSL_CERT_FILE"),
    Pattern::Exact("SSL_CERT_DIR"),
    Pattern::Prefix("DYLD_"),
    Pattern::Exact("NODE_OPTIONS"),
    Pattern::Exact("BASH_ENV"),
    Pattern::Exact("ENV"),
    Pattern::Prefix("GIT_"),
    // Not in SR-05's list but outside the allowlist anyway; kept here so a pass-through entry can't
    // re-add what the fixed and own-only rules decide.
    Pattern::Exact("FOREMAN_RUN_ID"),
    Pattern::Exact("TERM"),
    Pattern::Exact("COLORTERM"),
];

/// A name pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    /// `PREFIX*`
    Prefix(&'static str),
    /// `*SUFFIX`
    Suffix(&'static str),
    /// Exactly this name.
    Exact(&'static str),
}

impl Pattern {
    /// Whether `name` matches.
    #[must_use]
    pub fn matches(self, name: &str) -> bool {
        match self {
            Self::Prefix(p) => name.starts_with(p),
            Self::Suffix(s) => name.ends_with(s),
            Self::Exact(e) => name == e,
        }
    }
}

/// `^[A-Za-z_][A-Za-z0-9_]{0,127}$` (`EnvName`).
#[must_use]
pub fn is_env_name(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 128
        && (b[0].is_ascii_alphabetic() || b[0] == b'_')
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

/// Whether a name is on the always-removed list (SR-05).
#[must_use]
pub fn is_removed(name: &str) -> bool {
    REMOVED.iter().any(|p| p.matches(name))
}

/// The child environment: exactly these variables, sorted by name.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ChildEnv(pub Vec<(String, String)>);

impl ChildEnv {
    /// The names (for the preview and the run record; never the values, SR-19).
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.0.iter().map(|(k, _)| k.clone()).collect()
    }

    /// One value.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// `envKey` for the auth cache (files.md §4.9): SHA-256 over the sorted names and the
    /// SHA-256 of each value; never the values themselves. `FOREMAN_RUN_ID` is left out: it is
    /// new for every run and can't change how Claude Code authenticates.
    #[must_use]
    pub fn key(&self) -> String {
        let mut text = String::new();
        for (k, v) in self.0.iter().filter(|(k, _)| k != "FOREMAN_RUN_ID") {
            text.push_str(k);
            text.push('=');
            text.push_str(&crate::store::sha256_hex(v.as_bytes()));
            text.push('\n');
        }
        crate::store::sha256_hex(text.as_bytes())
    }
}

/// SR-05 / DR-123: builds the child environment from Foreman's own environment, the pass-through
/// names, the built `PATH` and the run ID.
#[must_use]
pub fn build_env(
    own: &[(String, String)],
    passthrough: &[String],
    path: &BuiltPath,
    run_id: &str,
) -> ChildEnv {
    let own_map: BTreeMap<&str, &str> = own
        .iter()
        .filter(|(k, _)| is_env_name(k))
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in &own_map {
        if BASE.contains(k) || k.starts_with("LC_") || OWN_ONLY.contains(k) {
            out.insert((*k).to_owned(), (*v).to_owned());
        }
    }
    for name in passthrough {
        if !is_env_name(name) || is_removed(name) || name == "PATH" {
            continue;
        }
        if let Some(v) = own_map.get(name.as_str()) {
            out.insert(name.clone(), (*v).to_owned());
        }
    }
    for (k, v) in FIXED {
        out.insert(k.to_owned(), v.to_owned());
    }
    out.insert("PATH".into(), path.joined());
    out.insert("FOREMAN_RUN_ID".into(), run_id.to_owned());
    ChildEnv(out.into_iter().collect())
}

/// What is known about a `PATH` candidate (gathered by `foreman-sys` with `stat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DirFacts {
    /// It exists and is a directory.
    pub is_dir: bool,
    /// It or an ancestor is world-writable without the sticky bit.
    pub world_writable: bool,
}

/// Why a `PATH` entry was dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PathDrop {
    /// Empty, `.` or relative.
    NotAbsolute,
    /// Missing or not a directory.
    Missing,
    /// Inside a registered project.
    InProject,
    /// World-writable.
    WorldWritable,
    /// Already listed.
    Duplicate,
    /// Contains `:` or a control character.
    BadCharacter,
}

/// The sanitized `PATH` and what was dropped.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuiltPath {
    /// Kept entries, in order.
    pub entries: Vec<PathBuf>,
    /// Dropped entries with the reason.
    pub dropped: Vec<(String, PathDrop)>,
}

impl BuiltPath {
    /// `a:b:c`.
    #[must_use]
    pub fn joined(&self) -> String {
        self.entries
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(":")
    }
}

/// The `PATH` sources in order (ADR-007 §5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PathSources {
    /// `/etc/paths` lines.
    pub etc_paths: Vec<String>,
    /// `/etc/paths.d/*` lines (files in name order).
    pub etc_paths_d: Vec<String>,
    /// `$HOME`.
    pub home: PathBuf,
    /// The pnpm home, when known.
    pub pnpm_home: Option<PathBuf>,
    /// The consented login-shell PATH (Settings `loginPath.entries`).
    pub login: Vec<String>,
}

impl PathSources {
    /// Every candidate in order: `/etc/paths`, `/etc/paths.d/*`, `/opt/homebrew/bin`,
    /// `/opt/homebrew/sbin`, `~/.cargo/bin`, the pnpm home, then the login PATH.
    #[must_use]
    pub fn candidates(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        v.extend(self.etc_paths.iter().cloned());
        v.extend(self.etc_paths_d.iter().cloned());
        v.push("/opt/homebrew/bin".into());
        v.push("/opt/homebrew/sbin".into());
        v.push(self.home.join(".cargo/bin").display().to_string());
        if let Some(p) = &self.pnpm_home {
            v.push(p.display().to_string());
        }
        v.extend(self.login.iter().cloned());
        v
    }
}

/// SR-05: keeps absolute, existing, non-project, non-world-writable directories, in order,
/// without duplicates. `facts` answers for each absolute candidate.
pub fn build_path(
    candidates: &[String],
    projects: &[PathBuf],
    facts: &mut dyn FnMut(&Path) -> DirFacts,
) -> BuiltPath {
    let mut out = BuiltPath::default();
    for raw in candidates {
        let t = raw.trim();
        let drop = if t.is_empty() || !t.starts_with('/') {
            Some(PathDrop::NotAbsolute)
        } else if t.contains(':') || t.chars().any(char::is_control) {
            Some(PathDrop::BadCharacter)
        } else {
            let p = Path::new(t);
            if out.entries.iter().any(|e| e == p) {
                Some(PathDrop::Duplicate)
            } else if projects.iter().any(|r| p.starts_with(r)) {
                Some(PathDrop::InProject)
            } else {
                let f = facts(p);
                if !f.is_dir {
                    Some(PathDrop::Missing)
                } else if f.world_writable {
                    Some(PathDrop::WorldWritable)
                } else {
                    None
                }
            }
        };
        match drop {
            None => out.entries.push(PathBuf::from(t)),
            Some(d) => out.dropped.push((raw.clone(), d)),
        }
    }
    out
}
