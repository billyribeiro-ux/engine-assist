//! The store (ADR-010, ADR-011, SR-30–SR-33, SR-53): the only module in the core that touches the
//! filesystem. Reads and writes resolve paths component by component through held directory fds
//! (`walk`), refuse symlinks at any component, hard links and non-regular files, apply size caps,
//! and write atomically and durably without ever overwriting a newer external edit (`atomic`).
#![allow(
    clippy::disallowed_methods,
    reason = "store is the core's only filesystem module (clippy.toml, AC-XC-30)"
)]

use std::io::Read as _;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use rustix::fs::{FileType, Mode, OFlags};
use rustix::io::Errno;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::pathpolicy::{PolicyViolation, ReadClass, RepoPath};

pub mod app_dirs;
mod atomic;
pub mod config_read;
pub mod dirs;
pub mod export;
pub mod git_exclude;
pub mod outside;
pub mod walk;

pub use atomic::{FaultPoint, WriteOptions, WriteOutcome, is_temp_name};

/// Hex SHA-256 of bytes.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in d {
        s.push(char::from(b"0123456789abcdef"[usize::from(b >> 4)]));
        s.push(char::from(b"0123456789abcdef"[usize::from(b & 15)]));
    }
    s
}

/// Which kind of root this is; decides modes and whether reported names are checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootKind {
    /// A registered project folder (repo).
    Project,
    /// The blueprint repo (read only).
    Blueprint,
    /// Foreman's app data: directories `0700`, files `0600` (SR-53).
    AppData,
    /// A git directory, for the `info/exclude` writer only (SR-32).
    GitDir,
    /// A base folder of the fixed outside-read list (SR-31a): secrets refused, nothing written.
    Outside,
    /// The folder Billy chose in a save dialog for a user export (`export::write_user_export`).
    Export,
}

/// What Foreman knew about a file when it read it (ADR-010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ReadStamp {
    /// The file didn't exist.
    Absent,
    /// The file as read.
    Present {
        /// Device.
        dev: u64,
        /// Inode.
        ino: u64,
        /// Size.
        size: u64,
        /// Modification time in ns.
        mtime_ns: i64,
        /// SHA-256 of the bytes.
        sha256: String,
    },
}

impl ReadStamp {
    /// The content hash, if present.
    #[must_use]
    pub fn sha256(&self) -> Option<&str> {
        match self {
            Self::Absent => None,
            Self::Present { sha256, .. } => Some(sha256),
        }
    }
}

/// A store failure. Every variant names the file or component.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// A path-policy refusal (lexical or during the walk).
    #[error(transparent)]
    Policy(#[from] PolicyViolation),
    /// Missing.
    #[error("{0}: not found")]
    NotFound(String),
    /// Not a regular file (FIFO, device, directory).
    #[error("{0}: not a regular file")]
    NotRegular(String),
    /// More than one hard link (SR-31).
    #[error("{0}: has {1} hard links")]
    HardLink(String, u64),
    /// Larger than its class allows.
    #[error("{name}: {size} bytes is over the {cap}-byte limit")]
    TooLarge {
        /// File.
        name: String,
        /// Size.
        size: u64,
        /// Cap.
        cap: u64,
    },
    /// The file changed since Foreman read it (SR-33): nothing was overwritten.
    #[error("{name} changed on disk since Foreman read it. Review the newer version")]
    Conflict {
        /// File.
        name: String,
        /// The stamp of what is on disk now.
        current: Box<ReadStamp>,
    },
    /// An OS error.
    #[error("{name}: {message}")]
    Io {
        /// File.
        name: String,
        /// Error text.
        message: String,
        /// errno.
        errno: i32,
    },
    /// A fault injected by a test (crash simulation).
    #[error("injected fault at {0:?}")]
    Injected(FaultPoint),
}

impl StoreError {
    pub(crate) fn io(name: &str, e: Errno) -> Self {
        Self::Io {
            name: name.to_owned(),
            message: e.to_string(),
            errno: e.raw_os_error(),
        }
    }
}

/// An open root directory. Everything below it is reached through this fd.
#[derive(Debug)]
pub struct Root {
    fd: OwnedFd,
    path: PathBuf,
    kind: RootKind,
    reporter: walk::NameReporter,
    recovery: Option<(PathBuf, String)>,
}

fn identity(s: &str) -> String {
    s.to_owned()
}

impl Root {
    /// Opens a root directory. Its own ancestors may be symlinks (Billy chose the path); the
    /// canonical path is recorded.
    ///
    /// # Errors
    /// When the path isn't an openable directory.
    pub fn open(abs: &Path, kind: RootKind) -> Result<Self, StoreError> {
        let name = abs.display().to_string();
        let fd = rustix::fs::open(
            abs,
            OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::RDONLY,
            Mode::empty(),
        )
        .map_err(|e| {
            if e == Errno::NOENT {
                StoreError::NotFound(name.clone())
            } else {
                StoreError::io(&name, e)
            }
        })?;
        let path = std::fs::canonicalize(abs).unwrap_or_else(|_| abs.to_path_buf());
        Ok(Self {
            fd,
            path,
            kind,
            reporter: identity,
            recovery: None,
        })
    }

    /// Creates (mode `0700`) and opens the app-data root.
    ///
    /// # Errors
    /// When it can't be created or opened.
    pub fn open_app_data(abs: &Path) -> Result<Self, StoreError> {
        use std::os::unix::fs::DirBuilderExt as _;
        let name = abs.display().to_string();
        if let Err(e) = std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(abs)
        {
            return Err(StoreError::Io {
                name,
                message: e.to_string(),
                errno: e.raw_os_error().unwrap_or(0),
            });
        }
        let root = Self::open(abs, RootKind::AppData)?;
        rustix::fs::fchmod(&root.fd, Mode::from_raw_mode(0o700))
            .map_err(|e| StoreError::io(&name, e))?;
        Ok(root)
    }

    /// Where leftover temp files that differ from their target are moved (ADR-010):
    /// `<app_data>/recovered/<timestamp>-<label>-<name>`.
    #[must_use]
    pub fn with_recovery(mut self, app_data: &Path, label: &str) -> Self {
        self.recovery = Some((app_data.to_path_buf(), label.to_owned()));
        self
    }

    /// Test hook: how reported names are seen (simulates a case-insensitive volume).
    #[doc(hidden)]
    #[must_use]
    pub fn with_name_reporter(mut self, r: walk::NameReporter) -> Self {
        self.reporter = r;
        self
    }

    /// The canonical root path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The root kind.
    #[must_use]
    pub fn kind(&self) -> RootKind {
        self.kind
    }

    pub(crate) fn opts<'a>(&self, create: bool, exempt: Option<&'a str>) -> walk::WalkOpts<'a> {
        walk::WalkOpts {
            create: create.then_some(if self.kind == RootKind::AppData {
                0o700
            } else {
                0o755
            }),
            exempt,
            names: match self.kind {
                RootKind::Project | RootKind::Blueprint => walk::NameCheck::Deny,
                RootKind::Outside | RootKind::Export => walk::NameCheck::SecretsOnly,
                RootKind::AppData | RootKind::GitDir => walk::NameCheck::None,
            },
            reporter: self.reporter,
        }
    }

    fn exempt_for(p: &RepoPath) -> Option<&'static str> {
        let mut c = p.components();
        match c.next() {
            Some(".foreman") => Some(".foreman"),
            Some("spec") if c.next() == Some(".claude") => Some(".claude"),
            _ => None,
        }
    }

    /// Opens the final component for reading, refusing symlinks, non-regular files and hard
    /// links; returns the open file and its stat.
    fn open_final(
        &self,
        p: &RepoPath,
        class: ReadClass,
    ) -> Result<(OwnedFd, rustix::fs::Stat), StoreError> {
        let (parents, name) = p.split_last();
        let exempt = Self::exempt_for(p);
        let dir = walk::walk(&self.fd, &parents, self.opts(false, exempt))?;
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
        let fd = rustix::fs::openat(&dir, name, flags, Mode::empty())
            .map_err(|e| walk::classify(&dir, name, e))?;
        let st = rustix::fs::fstat(&fd).map_err(|e| StoreError::io(name, e))?;
        if FileType::from_raw_mode(stat_mode(&st)) != FileType::RegularFile {
            return Err(StoreError::NotRegular(p.to_string()));
        }
        let nlink = stat_nlink(&st);
        if nlink > 1 {
            return Err(StoreError::HardLink(p.to_string(), nlink));
        }
        let size = u64::try_from(st.st_size).unwrap_or(u64::MAX);
        if size > class.cap() {
            return Err(StoreError::TooLarge {
                name: p.to_string(),
                size,
                cap: class.cap(),
            });
        }
        let names = self.opts(false, exempt).names;
        if names != walk::NameCheck::None {
            let ino = stat_ino(&st);
            if let Some(reported) = walk::reported_name(&dir, ino) {
                walk::check_name(&(self.reporter)(&reported), exempt, names)?;
            }
        }
        Ok((fd, st))
    }

    /// Reads a file (SR-31): no symlink at any component, regular file, `nlink == 1`, size cap.
    ///
    /// # Errors
    /// See [`StoreError`].
    pub fn read(&self, p: &RepoPath, class: ReadClass) -> Result<(Vec<u8>, ReadStamp), StoreError> {
        let (fd, st) = self.open_final(p, class)?;
        let mut bytes = Vec::new();
        let mut f = std::fs::File::from(fd);
        f.by_ref()
            .take(class.cap() + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| StoreError::Io {
                name: p.to_string(),
                message: e.to_string(),
                errno: e.raw_os_error().unwrap_or(0),
            })?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > class.cap() {
            return Err(StoreError::TooLarge {
                name: p.to_string(),
                size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                cap: class.cap(),
            });
        }
        let stamp = stamp_from(&st, &bytes);
        Ok((bytes, stamp))
    }

    /// Reads a file, returning `None` with [`ReadStamp::Absent`] when it doesn't exist.
    ///
    /// # Errors
    /// See [`StoreError`] (other than not-found).
    pub fn read_opt(
        &self,
        p: &RepoPath,
        class: ReadClass,
    ) -> Result<(Option<Vec<u8>>, ReadStamp), StoreError> {
        match self.read(p, class) {
            Ok((b, s)) => Ok((Some(b), s)),
            Err(StoreError::NotFound(_)) => Ok((None, ReadStamp::Absent)),
            Err(e) => Err(e),
        }
    }

    /// Whether an entry exists (`statat` without following symlinks; never opened). Used for
    /// transcript existence checks.
    #[must_use]
    pub fn exists(&self, p: &RepoPath) -> bool {
        let (parents, name) = p.split_last();
        walk::walk(&self.fd, &parents, self.opts(false, None)).is_ok_and(|dir| {
            rustix::fs::statat(&dir, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW).is_ok()
        })
    }

    /// Writes `bytes` atomically and durably if the file still matches `expect` (ADR-010).
    ///
    /// # Errors
    /// [`StoreError::Conflict`] when the file changed; others as named.
    pub fn write(
        &self,
        p: &RepoPath,
        bytes: &[u8],
        expect: &ReadStamp,
    ) -> Result<ReadStamp, StoreError> {
        self.write_with(p, bytes, expect, &WriteOptions::default())
            .map(|o| o.stamp)
    }

    /// [`Root::write`] with options (fault injection and fallbacks for tests; recovery notices).
    ///
    /// # Errors
    /// As [`Root::write`].
    pub fn write_with(
        &self,
        p: &RepoPath,
        bytes: &[u8],
        expect: &ReadStamp,
        opts: &WriteOptions,
    ) -> Result<WriteOutcome, StoreError> {
        atomic::write(self, p, bytes, expect, opts)
    }

    /// Appends one line (≤ 4 KB) with `O_APPEND`, one `write`, `sync_data` (ADR-010 §3).
    ///
    /// # Errors
    /// When the line is too long or the append fails.
    pub fn append_line(&self, p: &RepoPath, line: &[u8]) -> Result<(), StoreError> {
        atomic::append_line(self, p, line)
    }

    pub(crate) fn fd(&self) -> &OwnedFd {
        &self.fd
    }

    pub(crate) fn recovery(&self) -> Option<&(PathBuf, String)> {
        self.recovery.as_ref()
    }
}

/// The canonical form of a path (falls back to the path itself).
#[must_use]
pub fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

#[allow(
    clippy::useless_conversion,
    reason = "stat field types differ per platform"
)]
pub(crate) fn stat_nlink(st: &rustix::fs::Stat) -> u64 {
    u64::try_from(st.st_nlink).unwrap_or(u64::MAX)
}

#[allow(
    clippy::useless_conversion,
    reason = "stat field types differ per platform"
)]
pub(crate) fn stat_mode(st: &rustix::fs::Stat) -> u32 {
    u32::try_from(st.st_mode).unwrap_or(0)
}

#[allow(
    clippy::useless_conversion,
    reason = "stat field types differ per platform"
)]
pub(crate) fn stat_mtime_secs(st: &rustix::fs::Stat) -> u64 {
    u64::try_from(st.st_mtime).unwrap_or(0)
}

#[allow(
    clippy::useless_conversion,
    reason = "stat field types differ per platform"
)]
pub(crate) fn stat_ino(st: &rustix::fs::Stat) -> u64 {
    u64::try_from(st.st_ino).unwrap_or(0)
}

#[allow(
    clippy::useless_conversion,
    reason = "stat field types differ per platform"
)]
pub(crate) fn stamp_from(st: &rustix::fs::Stat, bytes: &[u8]) -> ReadStamp {
    let mtime_ns = i64::try_from(st.st_mtime)
        .unwrap_or(0)
        .saturating_mul(1_000_000_000)
        .saturating_add(i64::try_from(st.st_mtime_nsec).unwrap_or(0));
    ReadStamp::Present {
        dev: u64::try_from(st.st_dev).unwrap_or(0),
        ino: stat_ino(st),
        size: u64::try_from(st.st_size).unwrap_or(0),
        mtime_ns,
        sha256: sha256_hex(bytes),
    }
}
