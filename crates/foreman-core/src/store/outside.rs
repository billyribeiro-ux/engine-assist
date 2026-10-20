//! Reads outside projects, the blueprint repo and app data (SR-31a, AC-XC-30): a fixed list,
//! expressed as an enum so no other path can be named. Each read walks from a fixed base with the
//! same no-symlink, regular-file, one-link and size rules as project reads (256 KB, Claude config).

use std::path::{Path, PathBuf};

use super::{Root, RootKind, StoreError};
use crate::pathpolicy::{self, ReadClass};

/// The `~/.claude/` folders and file Foreman may read (fingerprint sources).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserClaudeEntry {
    /// `~/.claude/CLAUDE.md`
    ClaudeMd,
    /// `~/.claude/agents/<file>`
    Agents,
    /// `~/.claude/skills/<file>`
    Skills,
    /// `~/.claude/commands/<file>`
    Commands,
    /// `~/.claude/rules/<file>`
    Rules,
    /// `~/.claude/output-styles/<file>`
    OutputStyles,
}

/// The fixed list of outside reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutsideRead {
    /// `~/.claude/settings.json` (or `$CLAUDE_CONFIG_DIR/settings.json`).
    UserSettings,
    /// `/Library/Application Support/ClaudeCode/managed-settings.json`.
    ManagedSettings,
    /// `/Library/Application Support/ClaudeCode/managed-settings.d/<name>.json`.
    ManagedDropIn(String),
    /// `~/.claude.json`, of which only `mcpServers` is kept.
    ClaudeJsonMcpServers,
    /// A file in `~/.claude/` (a single file name for the folders).
    UserClaude(UserClaudeEntry, Option<String>),
    /// `CLAUDE.md` or `CLAUDE.local.md` in an ancestor folder of a project (absolute folder).
    AncestorClaudeMd {
        /// The ancestor folder.
        dir: PathBuf,
        /// `CLAUDE.local.md` instead of `CLAUDE.md`.
        local: bool,
    },
    /// `/etc/paths`.
    EtcPaths,
    /// `/etc/paths.d/<name>`.
    EtcPathsD(String),
    /// `/etc/shells`.
    EtcShells,
    /// One of the blueprint auto-detect candidates under home (`dev/claude-blueprint/...`).
    BlueprintCandidate(usize),
}

/// The blueprint auto-detect candidates, relative to home.
pub const BLUEPRINT_CANDIDATES: [&str; 3] = [
    "dev/claude-blueprint/claude-build-blueprint.md",
    "claude-blueprint/claude-build-blueprint.md",
    "Documents/claude-blueprint/claude-build-blueprint.md",
];

const MANAGED: &str = "/Library/Application Support/ClaudeCode";

fn single(name: &str) -> Result<&str, StoreError> {
    let p = pathpolicy::check_read(name, ReadClass::ClaudeConfig)?;
    if p.as_str().contains('/') {
        return Err(StoreError::NotFound(name.to_owned()));
    }
    Ok(name)
}

/// The base folder and relative path of an outside read.
///
/// # Errors
/// When a dynamic name isn't a single safe component.
pub fn location(r: &OutsideRead, home: &Path, claude_config_dir: Option<&Path>) -> Result<(PathBuf, String), StoreError> {
    let cfg = claude_config_dir.map_or_else(|| home.join(".claude"), Path::to_path_buf);
    Ok(match r {
        OutsideRead::UserSettings => (cfg, "settings.json".into()),
        OutsideRead::ManagedSettings => (PathBuf::from(MANAGED), "managed-settings.json".into()),
        OutsideRead::ManagedDropIn(n) => (
            PathBuf::from(MANAGED),
            format!("managed-settings.d/{}", single(n)?),
        ),
        OutsideRead::ClaudeJsonMcpServers => (home.to_path_buf(), ".claude.json".into()),
        OutsideRead::UserClaude(e, f) => {
            let dir = match e {
                UserClaudeEntry::ClaudeMd => return Ok((cfg, "CLAUDE.md".into())),
                UserClaudeEntry::Agents => "agents",
                UserClaudeEntry::Skills => "skills",
                UserClaudeEntry::Commands => "commands",
                UserClaudeEntry::Rules => "rules",
                UserClaudeEntry::OutputStyles => "output-styles",
            };
            let f = f.as_deref().ok_or_else(|| StoreError::NotFound(dir.into()))?;
            (cfg, format!("{dir}/{}", single(f)?))
        }
        OutsideRead::AncestorClaudeMd { dir, local } => (
            dir.clone(),
            if *local { "CLAUDE.local.md" } else { "CLAUDE.md" }.into(),
        ),
        OutsideRead::EtcPaths => (PathBuf::from("/etc"), "paths".into()),
        OutsideRead::EtcPathsD(n) => (PathBuf::from("/etc"), format!("paths.d/{}", single(n)?)),
        OutsideRead::EtcShells => (PathBuf::from("/etc"), "shells".into()),
        OutsideRead::BlueprintCandidate(i) => (
            home.to_path_buf(),
            (*BLUEPRINT_CANDIDATES
                .get(*i)
                .ok_or_else(|| StoreError::NotFound("candidate".into()))?)
            .to_owned(),
        ),
    })
}

/// Reads one entry of the fixed list. `~/.claude.json` is reduced to `{"mcpServers": …}` plus each
/// project's `mcpServers` before it leaves this function.
///
/// # Errors
/// See [`StoreError`].
pub fn read(r: &OutsideRead, home: &Path, claude_config_dir: Option<&Path>) -> Result<Vec<u8>, StoreError> {
    let (base, rel) = location(r, home, claude_config_dir)?;
    let root = Root::open(&base, RootKind::Outside)?;
    // Lexically: the syntax rules and the secret patterns (never `.ssh`, `.env`, …).
    let p = pathpolicy::check_read(&rel, ReadClass::ClaudeConfig)?;
    let class = if matches!(r, OutsideRead::BlueprintCandidate(_)) {
        ReadClass::Template
    } else {
        ReadClass::ClaudeConfig
    };
    let (bytes, _) = root.read(&p, class)?;
    if *r == OutsideRead::ClaudeJsonMcpServers {
        let v = crate::json::parse_value(&bytes).map_err(|e| StoreError::Io {
            name: rel.clone(),
            message: e.to_string(),
            errno: 0,
        })?;
        let mut out = serde_json::Map::new();
        if let Some(m) = v.get("mcpServers") {
            out.insert("mcpServers".into(), m.clone());
        }
        if let Some(projects) = v.get("projects").and_then(|p| p.as_object()) {
            let mut po = serde_json::Map::new();
            for (k, pv) in projects {
                if let Some(m) = pv.get("mcpServers") {
                    po.insert(k.clone(), serde_json::json!({ "mcpServers": m }));
                }
            }
            out.insert("projects".into(), serde_json::Value::Object(po));
        }
        return Ok(crate::json::value_to_canonical(&serde_json::Value::Object(out)).into_bytes());
    }
    Ok(bytes)
}

/// Whether a transcript exists: `statat` only, never opened (SR-31a).
#[must_use]
pub fn transcript_exists(path: &Path) -> bool {
    rustix::fs::statat(rustix::fs::CWD, path, rustix::fs::AtFlags::SYMLINK_NOFOLLOW).is_ok()
}
