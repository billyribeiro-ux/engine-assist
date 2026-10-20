//! The fingerprint source list (SR-06, ADR-021 §2): data, so adding a source is one entry.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ConfigFile;

/// macOS managed settings (code.claude.com managed-settings, ADR-021).
pub const MANAGED_DIR: &str = "/Library/Application Support/ClaudeCode";

/// The source kinds of files.md §4.9 (`acked.snapshot[].source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    /// `<cfg>/settings.json`
    UserSettings,
    /// `managed-settings.json`
    ManagedSettings,
    /// `managed-settings.d/*.json`
    ManagedDropIn,
    /// `~/.claude.json` `mcpServers` (user and this project)
    ClaudeJsonMcp,
    /// `<cfg>/CLAUDE.md`
    UserMemory,
    /// `<cfg>/{agents,skills,commands,rules,output-styles}/**`
    UserDir,
    /// `CLAUDE.md` in the cwd or an ancestor
    ClaudeMd,
    /// `CLAUDE.local.md` in the cwd or an ancestor
    ClaudeLocalMd,
    /// An `@` import
    Import,
    /// The project's `.claude/**` (not `worktrees/`)
    ProjectClaudeDir,
    /// `.mcp.json`
    McpJson,
    /// An enabled plugin's folder
    Plugin,
    /// A program named by a hook, `statusLine`, `fileSuggestion`, helper or MCP `command`
    Program,
}

impl SourceKind {
    /// The wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserSettings => "userSettings",
            Self::ManagedSettings => "managedSettings",
            Self::ManagedDropIn => "managedDropIn",
            Self::ClaudeJsonMcp => "claudeJsonMcp",
            Self::UserMemory => "userMemory",
            Self::UserDir => "userDir",
            Self::ClaudeMd => "claudeMd",
            Self::ClaudeLocalMd => "claudeLocalMd",
            Self::Import => "import",
            Self::ProjectClaudeDir => "projectClaudeDir",
            Self::McpJson => "mcpJson",
            Self::Plugin => "plugin",
            Self::Program => "program",
        }
    }
}

/// Which symlink rule applies (ADR-021 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// Repo content: no symlink at any component below the repo root (SR-31).
    Repo,
    /// Outside the repo: a symlink to a regular file outside every project is followed and both
    /// paths are recorded; a symlink into a project, to a directory or to a non-regular file blocks.
    Outside,
}

/// The settings scope of a JSON source (SR-07: the allowlist exception is for `User` only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingsScope {
    /// `<cfg>/settings.json`
    User,
    /// managed settings
    Managed,
    /// `.claude/settings.json`
    Project,
    /// `.claude/settings.local.json`
    Local,
    /// `.mcp.json`
    McpJson,
    /// `~/.claude.json` (reduced)
    ClaudeJson,
}

/// One source to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceSpec {
    /// One file (missing is fine).
    File {
        /// Kind.
        kind: SourceKind,
        /// Absolute path.
        path: PathBuf,
        /// Symlink rule.
        origin: Origin,
        /// JSON settings scope (parsed and scanned) or `None`.
        scope: Option<SettingsScope>,
    },
    /// Every regular file below a folder (missing is fine).
    Dir {
        /// Kind.
        kind: SourceKind,
        /// Absolute folder.
        path: PathBuf,
        /// Symlink rule.
        origin: Origin,
        /// Top-level entries to skip (`worktrees`).
        exclude: Vec<String>,
    },
    /// `~/.claude.json`, streamed under its own cap; only `mcpServers` (user and this repo's
    /// `projects[<repo>]`) and the names of project `env` keys are kept.
    ClaudeJson {
        /// Absolute path.
        path: PathBuf,
        /// The repo path keys to keep under `projects`.
        repo_keys: Vec<String>,
    },
}

impl SourceSpec {
    /// The kind.
    #[must_use]
    pub fn kind(&self) -> SourceKind {
        match self {
            Self::File { kind, .. } | Self::Dir { kind, .. } => *kind,
            Self::ClaudeJson { .. } => SourceKind::ClaudeJsonMcp,
        }
    }

    /// The path.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::File { path, .. } | Self::Dir { path, .. } | Self::ClaudeJson { path, .. } => {
                path
            }
        }
    }
}

const USER_DIRS: [&str; 5] = ["agents", "skills", "commands", "rules", "output-styles"];

fn file(
    kind: SourceKind,
    path: PathBuf,
    origin: Origin,
    scope: Option<SettingsScope>,
) -> SourceSpec {
    SourceSpec::File {
        kind,
        path,
        origin,
        scope,
    }
}

fn claude_dir_sources(dir: &Path, out: &mut Vec<SourceSpec>) {
    out.push(SourceSpec::Dir {
        kind: SourceKind::ProjectClaudeDir,
        path: dir.join(".claude"),
        origin: Origin::Repo,
        exclude: vec!["worktrees".into()],
    });
    out.push(file(
        SourceKind::McpJson,
        dir.join(".mcp.json"),
        Origin::Repo,
        Some(SettingsScope::McpJson),
    ));
}

/// The static source list for a step (SR-06): user and managed config, `~/.claude.json`,
/// `CLAUDE.md`/`CLAUDE.local.md` from the cwd up to `/`, the project's `.claude/**` and `.mcp.json`
/// at the cwd and the repo root. `cfg` is `$CLAUDE_CONFIG_DIR` when Foreman's environment sets it.
/// `managed` is [`MANAGED_DIR`] in production.
#[must_use]
pub fn sources(
    cwd: &Path,
    repo_root: &Path,
    home: &Path,
    cfg: Option<&Path>,
    managed: &Path,
) -> Vec<SourceSpec> {
    let cfg_dir = cfg.map_or_else(|| home.join(".claude"), Path::to_path_buf);
    let mut v = vec![
        file(
            SourceKind::UserSettings,
            cfg_dir.join("settings.json"),
            Origin::Outside,
            Some(SettingsScope::User),
        ),
        file(
            SourceKind::UserMemory,
            cfg_dir.join("CLAUDE.md"),
            Origin::Outside,
            None,
        ),
    ];
    for d in USER_DIRS {
        v.push(SourceSpec::Dir {
            kind: SourceKind::UserDir,
            path: cfg_dir.join(d),
            origin: Origin::Outside,
            exclude: vec![],
        });
    }
    let repo_keys = vec![repo_root.display().to_string(), cwd.display().to_string()];
    v.push(SourceSpec::ClaudeJson {
        path: home.join(".claude.json"),
        repo_keys: repo_keys.clone(),
    });
    if let Some(c) = cfg {
        // With `CLAUDE_CONFIG_DIR`, the global config may live inside it (UNVERIFIED); both are
        // fingerprinted so either location is covered.
        v.push(SourceSpec::ClaudeJson {
            path: c.join(".claude.json"),
            repo_keys,
        });
    }
    v.push(file(
        SourceKind::ManagedSettings,
        managed.join("managed-settings.json"),
        Origin::Outside,
        Some(SettingsScope::Managed),
    ));
    v.push(SourceSpec::Dir {
        kind: SourceKind::ManagedDropIn,
        path: managed.join("managed-settings.d"),
        origin: Origin::Outside,
        exclude: vec![],
    });
    for dir in cwd.ancestors() {
        let origin = if dir.starts_with(repo_root) {
            Origin::Repo
        } else {
            Origin::Outside
        };
        v.push(file(
            SourceKind::ClaudeMd,
            dir.join("CLAUDE.md"),
            origin,
            None,
        ));
        v.push(file(
            SourceKind::ClaudeLocalMd,
            dir.join("CLAUDE.local.md"),
            origin,
            None,
        ));
    }
    claude_dir_sources(cwd, &mut v);
    if cwd != repo_root {
        claude_dir_sources(repo_root, &mut v);
    }
    v
}

/// `@` imports in a memory file (Claude Code memory imports): `@path` tokens at a word start,
/// outside fenced code blocks and inline code spans. Returns the raw paths.
#[must_use]
pub fn imports(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let mut in_code = false;
        let mut prev = ' ';
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '`' {
                in_code = !in_code;
            } else if c == '@' && !in_code && prev.is_whitespace() {
                let tok: String = chars[i + 1..]
                    .iter()
                    .take_while(|c| !c.is_whitespace() && **c != '`')
                    .collect();
                let tok = tok.trim_end_matches([',', ';', ')', ']']).to_owned();
                if !tok.is_empty() && (tok.contains('/') || tok.contains('.')) {
                    i += tok.chars().count();
                    out.push(tok);
                }
            }
            prev = c;
            i += 1;
        }
    }
    out
}

fn resolve_import(raw: &str, from: &Path, home: &Path) -> Option<PathBuf> {
    let p = if let Some(rest) = raw.strip_prefix("~/") {
        home.join(rest)
    } else if raw.starts_with('/') {
        PathBuf::from(raw)
    } else {
        from.parent()?.join(raw)
    };
    // Lexical normalization of `.` and `..` (never above `/`).
    let mut out = PathBuf::from("/");
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::Normal(n) => out.push(n),
            _ => {}
        }
    }
    Some(out)
}

/// The first word of a command line, with `$CLAUDE_PROJECT_DIR` expanded to the repo root.
fn program_of(cmd: &str, repo_root: &Path) -> Option<String> {
    let t = cmd.trim();
    let first: String = if let Some(rest) = t.strip_prefix('"') {
        rest.split('"').next().unwrap_or_default().to_owned()
    } else {
        t.split_whitespace().next().unwrap_or_default().to_owned()
    };
    if first.is_empty() {
        return None;
    }
    let root = repo_root.display().to_string();
    let expanded = first
        .replace("\"${CLAUDE_PROJECT_DIR}\"", &root)
        .replace("${CLAUDE_PROJECT_DIR}", &root)
        .replace("$CLAUDE_PROJECT_DIR", &root);
    Some(expanded)
}

/// Every program a settings-like JSON names: hook `command`s, `statusLine.command`,
/// `fileSuggestion` (string or `.command`), the helper keys, and MCP server `command`s.
#[must_use]
pub fn program_commands(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |x: Option<&Value>| {
        if let Some(s) = x.and_then(Value::as_str) {
            out.push(s.to_owned());
        }
    };
    if let Some(hooks) = v.get("hooks").and_then(Value::as_object) {
        for groups in hooks.values().filter_map(Value::as_array) {
            for g in groups {
                for h in g
                    .get("hooks")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    push(h.get("command"));
                }
            }
        }
    }
    push(v.get("statusLine").and_then(|s| s.get("command")));
    match v.get("fileSuggestion") {
        Some(Value::String(_)) => push(v.get("fileSuggestion")),
        Some(o) => push(o.get("command")),
        None => {}
    }
    for k in super::auth_keys::BLOCKED_KEYS {
        push(v.get(k));
    }
    let mut mcp = |m: Option<&Value>| {
        if let Some(servers) = m.and_then(Value::as_object) {
            for s in servers.values() {
                if let Some(c) = s.get("command").and_then(Value::as_str) {
                    out.push(c.to_owned());
                }
            }
        }
    };
    mcp(v.get("mcpServers"));
    if let Some(projects) = v.get("projects").and_then(Value::as_object) {
        for p in projects.values() {
            mcp(p.get("mcpServers"));
        }
    }
    out
}

/// Sources named by the files already read: `@` imports of memory files (depth tracked by the
/// caller, ≤ 5), enabled plugins' folders, and program files. Bare program names are returned as
/// [`ConfigFile`]s with `bare_name` set (recorded by name, nothing to read).
#[must_use]
pub fn derived_sources(
    files: &[ConfigFile],
    repo_root: &Path,
    home: &Path,
    cfg: Option<&Path>,
) -> (Vec<SourceSpec>, Vec<ConfigFile>) {
    let cfg_dir = cfg.map_or_else(|| home.join(".claude"), Path::to_path_buf);
    let mut specs = Vec::new();
    let mut names = Vec::new();
    let mut plugins = false;
    for f in files {
        let memory = matches!(
            f.kind,
            SourceKind::UserMemory
                | SourceKind::ClaudeMd
                | SourceKind::ClaudeLocalMd
                | SourceKind::Import
        );
        if let Some(text) = f.text.as_ref().filter(|_| memory) {
            for raw in imports(text) {
                if let Some(p) = resolve_import(&raw, Path::new(&f.path), home) {
                    let origin = if p.starts_with(repo_root) {
                        Origin::Repo
                    } else {
                        Origin::Outside
                    };
                    specs.push(file(SourceKind::Import, p, origin, None));
                }
            }
        }
        let Some(json) = &f.json else { continue };
        if json
            .get("enabledPlugins")
            .and_then(Value::as_object)
            .is_some_and(|m| m.values().any(|x| x.as_bool() == Some(true)))
        {
            plugins = true;
        }
        for cmd in program_commands(json) {
            let Some(prog) = program_of(&cmd, repo_root) else {
                continue;
            };
            if prog.starts_with('/') {
                let p = PathBuf::from(&prog);
                let origin = if p.starts_with(repo_root) {
                    Origin::Repo
                } else {
                    Origin::Outside
                };
                specs.push(file(SourceKind::Program, p, origin, None));
            } else if prog.contains('/') {
                specs.push(file(
                    SourceKind::Program,
                    repo_root.join(prog.trim_start_matches("./")),
                    Origin::Repo,
                    None,
                ));
            } else {
                names.push(ConfigFile {
                    kind: SourceKind::Program,
                    scope: None,
                    path: prog.clone(),
                    link_target: None,
                    sha256: crate::store::sha256_hex(format!("name:{prog}").as_bytes()),
                    json: None,
                    text: None,
                    bare_name: true,
                });
            }
        }
    }
    if plugins {
        // The plugin layout on disk isn't documented (ADR-021 UNVERIFIED): the whole `plugins/`
        // folder is fingerprinted, except `data/` (each plugin's runtime state, documented as
        // `~/.claude/plugins/data/<id>/`, not configuration).
        specs.push(SourceSpec::Dir {
            kind: SourceKind::Plugin,
            path: cfg_dir.join("plugins"),
            origin: Origin::Outside,
            exclude: vec!["data".into()],
        });
    }
    specs.sort_by(|a, b| a.path().cmp(b.path()));
    specs.dedup();
    names.sort_by(|a, b| a.path.cmp(&b.path));
    names.dedup_by(|a, b| a.path == b.path);
    (specs, names)
}
