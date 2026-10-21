//! Gathers the config fingerprint for a cwd (SR-06, ADR-021 §2): reads every source through
//! `foreman_core::store::config_read` with the stat cache, follows `@` imports (depth ≤ 5), adds
//! enabled plugins and named programs, and returns the files plus the new stat cache.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use foreman_core::fingerprint::{self, ConfigFile, Origin, SettingsScope, SourceKind, SourceSpec};
use foreman_core::launch::Blocker;
use foreman_core::schema::fingerprint::StatEntry;
use foreman_core::store::config_read::{
    self, CLAUDE_JSON_CAP, ConfigReadError, Content, ReadSource, StatKey,
};

/// Import depth limit (ADR-021).
pub const IMPORT_DEPTH: usize = 5;

/// Where to look.
#[derive(Debug, Clone)]
pub struct Where<'a> {
    /// The step cwd (canonical).
    pub cwd: &'a Path,
    /// The repo root (canonical).
    pub repo_root: &'a Path,
    /// `$HOME`.
    pub home: &'a Path,
    /// `$CLAUDE_CONFIG_DIR` from Foreman's own environment.
    pub config_dir: Option<&'a Path>,
    /// Managed settings folder ([`fingerprint::MANAGED_DIR`] in production).
    pub managed: &'a Path,
    /// Registered projects (an outside symlink into one blocks).
    pub projects: &'a [PathBuf],
}

/// The gathered set.
#[derive(Debug, Clone, PartialEq)]
pub struct Gathered {
    /// Every read source.
    pub files: Vec<ConfigFile>,
    /// The new stat cache.
    pub stat: Vec<StatEntry>,
    /// No stat cache existed (the first scan in this cwd).
    pub first_scan: bool,
    /// The fingerprint.
    pub fingerprint: String,
}

fn blocker_for(e: &ConfigReadError) -> Blocker {
    match e {
        ConfigReadError::TooLarge { path, size, .. } => Blocker::ConfigTooLarge {
            file: path.clone(),
            bytes: u32::try_from(*size).unwrap_or(u32::MAX),
        },
        ConfigReadError::Unsafe { path, .. } | ConfigReadError::Io { path, .. } => {
            Blocker::ConfigTooLarge {
                file: path.clone(),
                bytes: 0,
            }
        }
    }
}

fn content_for(kind: SourceKind, scope: Option<SettingsScope>) -> Content {
    match kind {
        SourceKind::UserSettings
        | SourceKind::ManagedSettings
        | SourceKind::ManagedDropIn
        | SourceKind::UserMemory
        | SourceKind::ClaudeMd
        | SourceKind::ClaudeLocalMd
        | SourceKind::Import
        | SourceKind::McpJson => Content::Full,
        SourceKind::ProjectClaudeDir if scope.is_some() => Content::Full,
        _ => Content::HashOnly,
    }
}

fn project_scope(path: &Path) -> Option<SettingsScope> {
    let name = path.file_name()?.to_str()?;
    let parent = path.parent()?.file_name()?.to_str()?;
    (parent == ".claude").then_some(())?;
    match name {
        "settings.json" => Some(SettingsScope::Project),
        "settings.local.json" => Some(SettingsScope::Local),
        _ => None,
    }
}

fn to_file(
    kind: SourceKind,
    scope: Option<SettingsScope>,
    r: ReadSource,
) -> Result<ConfigFile, Blocker> {
    let path = r.path.display().to_string();
    let (json, text) = match (&r.bytes, scope) {
        (Some(b), Some(_)) => {
            let v = foreman_core::json::parse_value(b).map_err(|_| Blocker::ConfigTooLarge {
                file: path.clone(),
                bytes: 0,
            })?;
            (Some(v), None)
        }
        (Some(b), None) => {
            let is_md = Path::new(&path)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md"));
            (None, is_md.then(|| String::from_utf8_lossy(b).into_owned()))
        }
        (None, _) => (None, None),
    };
    Ok(ConfigFile {
        kind,
        scope,
        path,
        link_target: r.link_target.map(|t| t.display().to_string()),
        sha256: r.sha256,
        json,
        text,
        bare_name: false,
    })
}

/// Reads every source; the first source that can't be read safely blocks (`ConfigTooLarge` with
/// the path). `cache` is the previous stat cache for this cwd.
///
/// # Errors
/// The blockers.
pub fn gather(w: &Where<'_>, cache: &[StatEntry]) -> Result<Gathered, Vec<Blocker>> {
    let by_path: BTreeMap<&str, &StatEntry> = cache.iter().map(|e| (e.path.as_str(), e)).collect();
    let lookup = |p: &Path, s: &StatKey| -> Option<String> {
        let e = by_path.get(p.display().to_string().as_str())?;
        (e.dev == s.dev && e.ino == s.ino && e.size == s.size && e.mtime_ns == s.mtime_ns)
            .then(|| e.sha256.clone())
    };
    let mut files: Vec<ConfigFile> = Vec::new();
    let mut stat: Vec<StatEntry> = Vec::new();
    let mut blockers: Vec<Blocker> = Vec::new();
    let mut seen: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
    let mut specs = fingerprint::sources(w.cwd, w.repo_root, w.home, w.config_dir, w.managed);
    let mut depth = 0usize;
    loop {
        for spec in std::mem::take(&mut specs) {
            if !seen.insert(spec.path().to_path_buf()) {
                continue;
            }
            match spec {
                SourceSpec::File {
                    kind,
                    path,
                    origin,
                    scope,
                } => read_one(
                    w,
                    &lookup,
                    kind,
                    scope,
                    &path,
                    origin,
                    &mut files,
                    &mut stat,
                    &mut blockers,
                ),
                SourceSpec::Dir {
                    kind,
                    path,
                    origin,
                    exclude,
                } => match config_read::list_files(&path, &exclude) {
                    Ok(list) => {
                        for p in list {
                            let scope = if kind == SourceKind::ProjectClaudeDir {
                                project_scope(&p)
                            } else if kind == SourceKind::ManagedDropIn {
                                Some(SettingsScope::Managed)
                            } else {
                                None
                            };
                            read_one(
                                w,
                                &lookup,
                                kind,
                                scope,
                                &p,
                                origin,
                                &mut files,
                                &mut stat,
                                &mut blockers,
                            );
                        }
                    }
                    Err(e) => blockers.push(blocker_for(&e)),
                },
                SourceSpec::ClaudeJson { path, repo_keys } => {
                    match config_read::read_claude_json(
                        &path,
                        &repo_keys,
                        w.projects,
                        CLAUDE_JSON_CAP,
                    ) {
                        Ok(Some((v, r))) => files.push(ConfigFile {
                            kind: SourceKind::ClaudeJsonMcp,
                            scope: Some(SettingsScope::ClaudeJson),
                            path: r.path.display().to_string(),
                            link_target: r.link_target.map(|t| t.display().to_string()),
                            sha256: r.sha256,
                            json: Some(v),
                            text: None,
                            bare_name: false,
                        }),
                        Ok(None) => {}
                        Err(e) => blockers.push(blocker_for(&e)),
                    }
                }
            }
        }
        if depth >= IMPORT_DEPTH {
            break;
        }
        let (more, names) = fingerprint::derived_sources(&files, w.repo_root, w.home, w.config_dir);
        for n in names {
            if !files.iter().any(|f| f.bare_name && f.path == n.path) {
                files.push(n);
            }
        }
        specs = more
            .into_iter()
            .filter(|s| !seen.contains(s.path()))
            .collect();
        if specs.is_empty() {
            break;
        }
        depth += 1;
    }
    if !blockers.is_empty() {
        return Err(blockers);
    }
    let first_scan = cache.is_empty();
    stat.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Gathered {
        fingerprint: fingerprint::fingerprint(&files),
        files,
        stat,
        first_scan,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "one internal helper with the gather state"
)]
fn read_one(
    w: &Where<'_>,
    lookup: &dyn Fn(&Path, &StatKey) -> Option<String>,
    kind: SourceKind,
    scope: Option<SettingsScope>,
    path: &Path,
    origin: Origin,
    files: &mut Vec<ConfigFile>,
    stat: &mut Vec<StatEntry>,
    blockers: &mut Vec<Blocker>,
) {
    let content = content_for(kind, scope);
    match config_read::read_source(path, origin, w.repo_root, w.projects, content, lookup) {
        Ok(Some(r)) => {
            if content == Content::HashOnly {
                stat.push(StatEntry {
                    path: r.path.display().to_string(),
                    dev: r.stat.dev,
                    ino: r.stat.ino,
                    size: r.stat.size,
                    mtime_ns: r.stat.mtime_ns,
                    sha256: r.sha256.clone(),
                });
            }
            match to_file(kind, scope, r) {
                Ok(f) => files.push(f),
                Err(b) => blockers.push(b),
            }
        }
        Ok(None) => {}
        Err(e) => blockers.push(blocker_for(&e)),
    }
}
