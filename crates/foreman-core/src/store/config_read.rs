//! Read-only access to the fingerprint sources (SR-06, SR-31, SR-31a; ADR-021 §2). Nothing here
//! writes.
//!
//! - **Repo content** (`Origin::Repo`): no symlink at any component below the repo root, a
//!   regular file with one link, ≤ 256 KB.
//! - **Outside sources** (`Origin::Outside`): a symlink is followed only when its target is a
//!   regular file outside every registered project; link and target are both recorded, so a
//!   retarget changes the fingerprint. A symlink into a project, to a directory, a loop or a
//!   non-regular file is refused.
//! - `~/.claude.json` is streamed under its own 64 MB cap; only `mcpServers` (user scope and the
//!   wanted `projects[<path>]`) and the names of those projects' `env` keys are kept.
//! - Plugin folders (`HashOnly`) are hashed while streaming, never kept, under a 64 MB per-file cap.

use std::collections::BTreeMap;
use std::fmt;
use std::io::Read as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, Visitor};
use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};

use crate::fingerprint::Origin;

/// Per-file cap for parsed and Markdown sources (ADR-021).
pub const FILE_CAP: u64 = 256 * 1024;
/// `~/.claude.json` cap (ADR-021).
pub const CLAUDE_JSON_CAP: u64 = 64 * 1024 * 1024;
/// Per-file cap for hash-only sources (plugin folders).
pub const HASH_ONLY_CAP: u64 = 64 * 1024 * 1024;
/// Files per folder source before the walk gives up (blocks with the folder named).
pub const MAX_FILES: usize = 20_000;
/// Folder depth.
pub const MAX_DEPTH: usize = 16;

/// The identity used by the stat cache (files.md §4.9 `stat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StatKey {
    /// Device.
    pub dev: u64,
    /// Inode.
    pub ino: u64,
    /// Size.
    pub size: u64,
    /// Modification time (ns).
    pub mtime_ns: i64,
}

impl StatKey {
    fn of(m: &std::fs::Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            size: m.len(),
            mtime_ns: m
                .mtime()
                .saturating_mul(1_000_000_000)
                .saturating_add(m.mtime_nsec()),
        }
    }
}

/// Whether the content is needed or only its hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Content {
    /// Read and keep (≤ 256 KB): settings, memory files.
    Full,
    /// Hash while streaming (≤ 64 MB); the stat cache may answer instead.
    HashOnly,
}

/// One read source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadSource {
    /// The listed path.
    pub path: PathBuf,
    /// The followed target, for an outside symlink.
    pub link_target: Option<PathBuf>,
    /// The bytes (`Content::Full`).
    pub bytes: Option<Vec<u8>>,
    /// SHA-256 of the content.
    pub sha256: String,
    /// The stat of what was read (the target for a link).
    pub stat: StatKey,
    /// The hash came from the stat cache.
    pub cached: bool,
}

/// Why a source can't be read safely. Every variant blocks the launch with the path named.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigReadError {
    /// Over its cap.
    #[error("{path}: {size} bytes is over the {cap}-byte limit")]
    TooLarge {
        /// Path.
        path: String,
        /// Size.
        size: u64,
        /// Cap.
        cap: u64,
    },
    /// A symlink or file type the rules refuse.
    #[error("{path}: {why}")]
    Unsafe {
        /// Path.
        path: String,
        /// Why.
        why: &'static str,
    },
    /// An OS error.
    #[error("{path}: {message}")]
    Io {
        /// Path.
        path: String,
        /// Error text.
        message: String,
    },
}

fn io_err(p: &Path, e: &std::io::Error) -> ConfigReadError {
    ConfigReadError::Io {
        path: p.display().to_string(),
        message: e.to_string(),
    }
}

fn unsafe_err(p: &Path, why: &'static str) -> ConfigReadError {
    ConfigReadError::Unsafe {
        path: p.display().to_string(),
        why,
    }
}

fn hex(d: &[u8]) -> String {
    d.iter().fold(String::with_capacity(64), |mut s, b| {
        s.push(char::from(b"0123456789abcdef"[usize::from(b >> 4)]));
        s.push(char::from(b"0123456789abcdef"[usize::from(b & 15)]));
        s
    })
}

/// Checks that no component of `p` below `root` is a symlink (repo content, SR-31).
fn no_symlink_below(root: &Path, p: &Path) -> Result<(), ConfigReadError> {
    let Ok(rest) = p.strip_prefix(root) else {
        return Err(unsafe_err(p, "outside the repo"));
    };
    let mut cur = root.to_path_buf();
    for c in rest.components() {
        cur.push(c);
        match std::fs::symlink_metadata(&cur) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(unsafe_err(&cur, "a symlink in the repo (not followed)"));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(io_err(&cur, &e)),
        }
    }
    Ok(())
}

fn open_nofollow(p: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
        .open(p)
}

fn read_capped(
    f: std::fs::File,
    shown: &Path,
    cap: u64,
    keep: bool,
) -> Result<(Option<Vec<u8>>, String), ConfigReadError> {
    let mut hasher = Sha256::new();
    let mut kept = Vec::new();
    let mut total = 0u64;
    let mut r = f.take(cap + 1);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = r.read(&mut buf).map_err(|e| io_err(shown, &e))?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > cap {
            return Err(ConfigReadError::TooLarge {
                path: shown.display().to_string(),
                size: total,
                cap,
            });
        }
        hasher.update(&buf[..n]);
        if keep {
            kept.extend_from_slice(&buf[..n]);
        }
    }
    Ok((keep.then_some(kept), hex(&hasher.finalize())))
}

/// Reads one source file. `Ok(None)` when it doesn't exist. `cache` answers a hash for an
/// unchanged `(path, stat)` (only used for `HashOnly`).
///
/// # Errors
/// [`ConfigReadError`] for anything the rules refuse.
pub fn read_source(
    path: &Path,
    origin: Origin,
    repo_root: &Path,
    projects: &[PathBuf],
    content: Content,
    cache: &dyn Fn(&Path, &StatKey) -> Option<String>,
) -> Result<Option<ReadSource>, ConfigReadError> {
    let lm = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) if e.raw_os_error() == Some(rustix::io::Errno::NOTDIR.raw_os_error()) => {
            return Ok(None);
        }
        Err(e) => return Err(io_err(path, &e)),
    };
    let (target, link_target) = if lm.file_type().is_symlink() {
        if origin == Origin::Repo {
            return Err(unsafe_err(path, "a symlink in the repo (not followed)"));
        }
        let t = std::fs::canonicalize(path)
            .map_err(|_| unsafe_err(path, "a broken or looping symlink"))?;
        let canon_projects: Vec<PathBuf> = projects
            .iter()
            .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()))
            .collect();
        if canon_projects.iter().any(|r| t.starts_with(r)) {
            return Err(unsafe_err(path, "a symlink into a project folder"));
        }
        (t.clone(), Some(t))
    } else {
        if origin == Origin::Repo {
            no_symlink_below(repo_root, path)?;
        }
        (path.to_path_buf(), None)
    };
    let f = open_nofollow(&target).map_err(|e| {
        if e.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error()) {
            unsafe_err(path, "a symlink where a file was expected")
        } else {
            io_err(path, &e)
        }
    })?;
    let m = f.metadata().map_err(|e| io_err(path, &e))?;
    if !m.file_type().is_file() {
        return Err(unsafe_err(path, "not a regular file"));
    }
    if origin == Origin::Repo && m.nlink() > 1 {
        return Err(unsafe_err(path, "a hard-linked file in the repo"));
    }
    let stat = StatKey::of(&m);
    let cap = match content {
        Content::Full => FILE_CAP,
        Content::HashOnly => HASH_ONLY_CAP,
    };
    if stat.size > cap {
        return Err(ConfigReadError::TooLarge {
            path: path.display().to_string(),
            size: stat.size,
            cap,
        });
    }
    if content == Content::HashOnly
        && let Some(h) = cache(path, &stat)
    {
        return Ok(Some(ReadSource {
            path: path.to_path_buf(),
            link_target,
            bytes: None,
            sha256: h,
            stat,
            cached: true,
        }));
    }
    let (bytes, sha256) = read_capped(f, path, cap, content == Content::Full)?;
    Ok(Some(ReadSource {
        path: path.to_path_buf(),
        link_target,
        bytes,
        sha256,
        stat,
        cached: false,
    }))
}

/// Every file below `base` (sorted), skipping top-level `exclude` names. A missing `base` is
/// empty. A symlinked directory (including `base` itself) is refused; symlinked files are listed
/// and judged by [`read_source`].
///
/// # Errors
/// [`ConfigReadError`] for a directory symlink, too many files or too deep a tree.
pub fn list_files(base: &Path, exclude: &[String]) -> Result<Vec<PathBuf>, ConfigReadError> {
    match std::fs::symlink_metadata(base) {
        Ok(m) if m.file_type().is_symlink() => {
            return Err(unsafe_err(base, "a symlinked folder (not followed)"));
        }
        Ok(m) if !m.is_dir() => return Ok(vec![]),
        Ok(_) => {}
        Err(_) => return Ok(vec![]),
    }
    let mut out = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> = vec![(base.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            return Err(unsafe_err(&dir, "nested too deeply"));
        }
        let rd = std::fs::read_dir(&dir).map_err(|e| io_err(&dir, &e))?;
        for e in rd {
            let e = e.map_err(|e| io_err(&dir, &e))?;
            let name = e.file_name().to_string_lossy().into_owned();
            if depth == 0 && exclude.contains(&name) {
                continue;
            }
            let p = e.path();
            let ft = e.file_type().map_err(|err| io_err(&p, &err))?;
            if ft.is_symlink() {
                let target_is_dir = std::fs::metadata(&p).is_ok_and(|m| m.is_dir());
                if target_is_dir {
                    return Err(unsafe_err(&p, "a symlinked folder (not followed)"));
                }
                out.push(p);
            } else if ft.is_dir() {
                stack.push((p, depth + 1));
            } else {
                out.push(p);
            }
            if out.len() > MAX_FILES {
                return Err(unsafe_err(base, "more files than Foreman fingerprints"));
            }
        }
    }
    out.sort();
    Ok(out)
}

struct Wanted<'a>(&'a [String]);

struct ProjectsSeed<'a>(&'a [String]);

impl<'de> DeserializeSeed<'de> for ProjectsSeed<'_> {
    type Value = Map<String, Value>;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for ProjectsSeed<'_> {
    type Value = Map<String, Value>;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the projects map")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
        let mut out = Map::new();
        while let Some(k) = a.next_key::<String>()? {
            if self.0.contains(&k) {
                let entry: ProjectEntry = a.next_value()?;
                let mut m = Map::new();
                if let Some(s) = entry.mcp_servers {
                    m.insert("mcpServers".into(), s);
                }
                if !entry.env_names.is_empty() {
                    m.insert(
                        "env".into(),
                        Value::Object(
                            entry
                                .env_names
                                .into_iter()
                                .map(|(n, h)| (n, Value::String(h)))
                                .collect(),
                        ),
                    );
                }
                out.insert(k, Value::Object(m));
            } else {
                a.next_value::<IgnoredAny>()?;
            }
        }
        Ok(out)
    }
}

struct ProjectEntry {
    mcp_servers: Option<Value>,
    env_names: BTreeMap<String, String>,
}

impl<'de> de::Deserialize<'de> for ProjectEntry {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = ProjectEntry;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a project entry")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut e = ProjectEntry {
                    mcp_servers: None,
                    env_names: BTreeMap::new(),
                };
                while let Some(k) = a.next_key::<String>()? {
                    match k.as_str() {
                        "mcpServers" => e.mcp_servers = Some(a.next_value()?),
                        "env" => {
                            let env: BTreeMap<String, Value> = a.next_value()?;
                            // Names only; each value becomes its hash (never kept).
                            e.env_names = env
                                .into_iter()
                                .map(|(n, v)| {
                                    let h = hex(&Sha256::digest(v.to_string().as_bytes()));
                                    (n, format!("sha256:{}", &h[..8]))
                                })
                                .collect();
                        }
                        _ => {
                            a.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(e)
            }
        }
        d.deserialize_map(V)
    }
}

impl<'de> DeserializeSeed<'de> for Wanted<'_> {
    type Value = Map<String, Value>;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Wanted<'_> {
    type Value = Map<String, Value>;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the ~/.claude.json object")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
        let mut out = Map::new();
        while let Some(k) = a.next_key::<String>()? {
            match k.as_str() {
                "mcpServers" => {
                    out.insert(k, a.next_value::<Value>()?);
                }
                "projects" => {
                    let p = a.next_value_seed(ProjectsSeed(self.0))?;
                    out.insert(k, Value::Object(p));
                }
                _ => {
                    a.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(out)
    }
}

/// Streams `~/.claude.json` and keeps only `mcpServers` and the wanted projects' `mcpServers`
/// and `env` names. The hash is over the reduced canonical JSON, so unrelated keys (sign-in
/// state, history) never change the fingerprint.
///
/// # Errors
/// [`ConfigReadError`] over the cap, for a refused symlink, or for invalid JSON.
pub fn read_claude_json(
    path: &Path,
    repo_keys: &[String],
    projects: &[PathBuf],
    cap: u64,
) -> Result<Option<(Value, ReadSource)>, ConfigReadError> {
    let first_look = read_source(
        path,
        Origin::Outside,
        Path::new("/"),
        projects,
        Content::HashOnly,
        // Never hash 64 MB here: the stat is all this first look needs.
        &|_, s| Some(format!("{}", s.size)),
    )
    .map_err(|e| match e {
        ConfigReadError::TooLarge { size, .. } => ConfigReadError::TooLarge {
            path: path.display().to_string(),
            size,
            cap,
        },
        other => other,
    })?;
    let Some(probe) = first_look else {
        return Ok(None);
    };
    if probe.stat.size > cap {
        return Err(ConfigReadError::TooLarge {
            path: path.display().to_string(),
            size: probe.stat.size,
            cap,
        });
    }
    let target = probe
        .link_target
        .clone()
        .unwrap_or_else(|| path.to_path_buf());
    let f = open_nofollow(&target).map_err(|e| io_err(path, &e))?;
    let reader = std::io::BufReader::new(f.take(cap + 1));
    let mut de = serde_json::Deserializer::from_reader(reader);
    let reduced = Wanted(repo_keys)
        .deserialize(&mut de)
        .map_err(|e| ConfigReadError::Io {
            path: path.display().to_string(),
            message: format!("not valid JSON: {e}"),
        })?;
    let v = Value::Object(reduced);
    let canon = crate::json::value_to_canonical(&v);
    let sha = hex(&Sha256::digest(canon.as_bytes()));
    Ok(Some((
        v,
        ReadSource {
            sha256: sha,
            bytes: None,
            cached: false,
            ..probe
        },
    )))
}
