//! Atomic, durable, compare-and-swap writes (ADR-010 §2, SR-32, SR-33):
//! temp `.<name>.<16 hex>.foreman-tmp` (`O_CREAT|O_EXCL|O_NOFOLLOW`, `0600`) through the parent fd,
//! `sync_all` (= `F_FULLFSYNC` on Apple), `renameat_with(EXCHANGE)` (or `NOREPLACE` for a new file),
//! compare the swapped-out bytes with the read stamp (swap back on mismatch), directory sync.

use std::io::{Read as _, Write as _};
use std::os::fd::OwnedFd;
use std::time::{Duration, SystemTime};

use rustix::fs::{AtFlags, FileType, Mode, OFlags, RenameFlags};
use rustix::io::Errno;

use super::{ReadStamp, Root, RootKind, StoreError, sha256_hex, stamp_from, walk};
use crate::pathpolicy::{self, RepoPath};

/// Where a test may stop a write, to simulate a crash at that moment (`atomic::crash_injection`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FaultPoint {
    /// The temp exists, empty.
    AfterTempCreate,
    /// The bytes are written, not synced.
    AfterWrite,
    /// Before `sync_all`.
    BeforeSync,
    /// After `sync_all`, before the rename.
    AfterSync,
    /// After the swap or rename, before the compare.
    AfterSwap,
    /// Before the old content (now at the temp name) is unlinked.
    BeforeUnlink,
    /// Before the directory sync.
    BeforeDirSync,
}

impl FaultPoint {
    /// Every point, in order.
    pub const ALL: [Self; 7] = [
        Self::AfterTempCreate,
        Self::AfterWrite,
        Self::BeforeSync,
        Self::AfterSync,
        Self::AfterSwap,
        Self::BeforeUnlink,
        Self::BeforeDirSync,
    ];
}

/// Write options. `Default` is production behavior.
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Stop at this point (tests).
    pub fault: Option<FaultPoint>,
    /// Behave as if `RENAME_SWAP`/`EXCHANGE` were unsupported (tests).
    pub force_no_swap: bool,
    /// Behave as if the directory full-sync were unsupported (tests).
    pub force_no_dir_sync: bool,
    /// Temps older than this are stale (1 hour).
    pub stale_after: Duration,
    /// Fixed temp suffix instead of random (tests planting a symlink at the temp name).
    pub temp_suffix: Option<String>,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            fault: None,
            force_no_swap: false,
            force_no_dir_sync: false,
            stale_after: Duration::from_secs(3600),
            temp_suffix: None,
        }
    }
}

/// What a write did besides writing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOutcome {
    /// The new stamp.
    pub stamp: ReadStamp,
    /// The swap wasn't available; a re-check plus plain rename was used (`degraded-cas`).
    pub degraded_cas: bool,
    /// The directory full-sync failed and plain `fsync` was used.
    pub dir_sync_fallback: bool,
    /// Leftover temps that differed from their target, moved to `recovered/` (shown, never deleted).
    pub recovered: Vec<String>,
    /// Leftover temps equal to their target, removed.
    pub removed_stale: Vec<String>,
}

fn fault(opts: &WriteOptions, at: FaultPoint) -> Result<(), StoreError> {
    if opts.fault == Some(at) {
        Err(StoreError::Injected(at))
    } else {
        Ok(())
    }
}

fn unsupported(e: Errno) -> bool {
    e == Errno::INVAL || e == Errno::NOTSUP || e == Errno::NOSYS || e == Errno::OPNOTSUPP
}

fn read_at(dir: &OwnedFd, name: &str) -> Result<(Vec<u8>, rustix::fs::Stat), Errno> {
    let fd = rustix::fs::openat(dir, name, OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK, Mode::empty())?;
    let st = rustix::fs::fstat(&fd)?;
    if FileType::from_raw_mode(super::stat_mode(&st)) != FileType::RegularFile {
        return Err(Errno::INVAL);
    }
    let mut v = Vec::new();
    std::fs::File::from(fd)
        .read_to_end(&mut v)
        .map_err(|e| Errno::from_raw_os_error(e.raw_os_error().unwrap_or(5)))?;
    Ok((v, st))
}

fn current_stamp(dir: &OwnedFd, name: &str) -> ReadStamp {
    match read_at(dir, name) {
        Ok((b, st)) => stamp_from(&st, &b),
        Err(_) => ReadStamp::Absent,
    }
}

fn conflict(dir: &OwnedFd, p: &RepoPath, name: &str) -> StoreError {
    StoreError::Conflict {
        name: p.to_string(),
        current: Box::new(current_stamp(dir, name)),
    }
}

fn temp_suffix() -> String {
    let id = uuid::Uuid::new_v4();
    let b = id.as_bytes();
    let mut s = String::with_capacity(16);
    for x in &b[..8] {
        s.push(char::from(b"0123456789abcdef"[usize::from(x >> 4)]));
        s.push(char::from(b"0123456789abcdef"[usize::from(x & 15)]));
    }
    s
}

/// `.<name>.<16 hex>.foreman-tmp` → `<name>`, for leftover temps of any target.
fn temp_target(file: &str) -> Option<&str> {
    let inner = file.strip_prefix('.')?.strip_suffix(".foreman-tmp")?;
    let (target, hex) = inner.rsplit_once('.')?;
    (hex.len() == 16 && hex.bytes().all(|b| b.is_ascii_hexdigit()) && !target.is_empty()).then_some(target)
}

/// True for Foreman's temp-file names (the watcher and listings ignore them).
#[must_use]
pub fn is_temp_name(file: &str) -> bool {
    temp_target(file).is_some()
}

fn handle_stale(root: &Root, dir: &OwnedFd, opts: &WriteOptions, out: &mut WriteOutcome) {
    let Ok(mut d) = rustix::fs::Dir::read_from(dir) else {
        return;
    };
    let mut temps = Vec::new();
    while let Some(Ok(e)) = d.read() {
        let n = e.file_name().to_string_lossy().into_owned();
        if temp_target(&n).is_some() {
            temps.push(n);
        }
    }
    let now = SystemTime::now();
    for t in temps {
        let Ok(st) = rustix::fs::statat(dir, t.as_str(), AtFlags::SYMLINK_NOFOLLOW) else {
            continue;
        };
        if FileType::from_raw_mode(super::stat_mode(&st)) != FileType::RegularFile {
            continue;
        }
        let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(super::stat_mtime_secs(&st));
        if now.duration_since(mtime).unwrap_or_default() < opts.stale_after {
            continue;
        }
        let Some(target) = temp_target(&t) else { continue };
        let Ok((tbytes, _)) = read_at(dir, &t) else { continue };
        let same = read_at(dir, target).is_ok_and(|(b, _)| b == tbytes);
        if same {
            if rustix::fs::unlinkat(dir, t.as_str(), AtFlags::empty()).is_ok() {
                out.removed_stale.push(t);
            }
            continue;
        }
        // Never discard: it may hold a newer external edit swapped out just before a crash.
        let Some((app_data, label)) = root.recovery() else { continue };
        let Ok(app) = Root::open_app_data(app_data) else { continue };
        let stamp = jiff::Timestamp::now().strftime("%Y%m%dT%H%M%S%.fZ").to_string();
        let rel = format!("recovered/{stamp}-{label}-{target}");
        let Ok(rp) = pathpolicy::check_app_data(&rel) else { continue };
        if app.write(&rp, &tbytes, &ReadStamp::Absent).is_ok()
            && rustix::fs::unlinkat(dir, t.as_str(), AtFlags::empty()).is_ok()
        {
            out.recovered.push(rel);
        }
    }
}

fn exempt(p: &RepoPath) -> Option<&'static str> {
    let mut c = p.components();
    match c.next() {
        Some(".foreman") => Some(".foreman"),
        Some("spec") if c.next() == Some(".claude") => Some(".claude"),
        _ => None,
    }
}

fn dir_sync(dir: &OwnedFd, opts: &WriteOptions, out: &mut WriteOutcome, name: &str) -> Result<(), StoreError> {
    let full = if opts.force_no_dir_sync {
        Err(Errno::NOTSUP)
    } else {
        let dup = rustix::io::dup(dir).map_err(|e| StoreError::io(name, e))?;
        std::fs::File::from(dup)
            .sync_all()
            .map_err(|e| Errno::from_raw_os_error(e.raw_os_error().unwrap_or(5)))
    };
    match full {
        Ok(()) => Ok(()),
        Err(e) if unsupported(e) => {
            out.dir_sync_fallback = true;
            rustix::fs::fsync(dir).map_err(|e| StoreError::io(name, e))
        }
        Err(e) => Err(StoreError::io(name, e)),
    }
}

#[allow(clippy::too_many_lines, reason = "one linear sequence of ADR-010 steps, kept together for review")]
pub(super) fn write(
    root: &Root,
    p: &RepoPath,
    bytes: &[u8],
    expect: &ReadStamp,
    opts: &WriteOptions,
) -> Result<WriteOutcome, StoreError> {
    let (parents, name) = p.split_last();
    let ex = exempt(p);
    let dir = walk::walk(root.fd(), &parents, root.opts(true, ex))?;
    let mut out = WriteOutcome {
        stamp: ReadStamp::Absent,
        degraded_cas: false,
        dir_sync_fallback: false,
        recovered: Vec::new(),
        removed_stale: Vec::new(),
    };
    handle_stale(root, &dir, opts, &mut out);

    // The existing target must be a plain regular file with one link.
    let existing_mode = match rustix::fs::statat(&dir, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(st) => {
            match FileType::from_raw_mode(super::stat_mode(&st)) {
                FileType::RegularFile => {}
                FileType::Symlink => return Err(pathpolicy::PolicyViolation::Symlink(name.to_owned()).into()),
                _ => return Err(StoreError::NotRegular(p.to_string())),
            }
            let nlink = super::stat_nlink(&st);
            if nlink > 1 {
                return Err(StoreError::HardLink(p.to_string(), nlink));
            }
            let names = root.opts(false, ex).names;
            if names != walk::NameCheck::None
                && let Some(rep) = walk::reported_name(&dir, super::stat_ino(&st))
            {
                walk::check_name(&rep, ex, names)?;
            }
            Some(super::stat_mode(&st) & 0o7777)
        }
        Err(e) if e == Errno::NOENT => None,
        Err(e) => return Err(StoreError::io(&p.to_string(), e)),
    };
    if existing_mode.is_none() && *expect != ReadStamp::Absent {
        return Err(conflict(&dir, p, name));
    }

    let suffix = opts.temp_suffix.clone().unwrap_or_else(temp_suffix);
    let tmp = format!(".{name}.{suffix}.foreman-tmp");
    let fd = rustix::fs::openat(
        &dir,
        tmp.as_str(),
        OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|e| StoreError::io(&p.to_string(), e))?;
    fault(opts, FaultPoint::AfterTempCreate)?;
    let mut file = std::fs::File::from(fd);
    let io_err = |e: std::io::Error| StoreError::Io {
        name: p.to_string(),
        message: e.to_string(),
        errno: e.raw_os_error().unwrap_or(0),
    };
    let cleanup = |dir: &OwnedFd| {
        let _ = rustix::fs::unlinkat(dir, tmp.as_str(), AtFlags::empty());
    };
    if let Err(e) = file.write_all(bytes) {
        cleanup(&dir);
        return Err(io_err(e));
    }
    fault(opts, FaultPoint::AfterWrite)?;
    if root.kind() != RootKind::AppData {
        let mode = existing_mode.map_or(0o644, |m| m & 0o777);
        if let Err(e) = rustix::fs::fchmod(&file, Mode::from_raw_mode(mode)) {
            cleanup(&dir);
            return Err(StoreError::io(&p.to_string(), e));
        }
    }
    fault(opts, FaultPoint::BeforeSync)?;
    if let Err(e) = file.sync_all() {
        cleanup(&dir);
        return Err(io_err(e));
    }
    fault(opts, FaultPoint::AfterSync)?;

    match expect {
        ReadStamp::Absent => {
            match rustix::fs::renameat_with(&dir, tmp.as_str(), &dir, name, RenameFlags::NOREPLACE) {
                Ok(()) => {}
                Err(e) if e == Errno::EXIST => {
                    cleanup(&dir);
                    return Err(conflict(&dir, p, name));
                }
                Err(e) if unsupported(e) => {
                    out.degraded_cas = true;
                    if rustix::fs::statat(&dir, name, AtFlags::SYMLINK_NOFOLLOW).is_ok() {
                        cleanup(&dir);
                        return Err(conflict(&dir, p, name));
                    }
                    rustix::fs::renameat(&dir, tmp.as_str(), &dir, name).map_err(|e| StoreError::io(&p.to_string(), e))?;
                }
                Err(e) => {
                    cleanup(&dir);
                    return Err(StoreError::io(&p.to_string(), e));
                }
            }
            fault(opts, FaultPoint::AfterSwap)?;
            fault(opts, FaultPoint::BeforeUnlink)?;
        }
        ReadStamp::Present { sha256, .. } => {
            let swapped = if opts.force_no_swap {
                Err(Errno::NOTSUP)
            } else {
                rustix::fs::renameat_with(&dir, tmp.as_str(), &dir, name, RenameFlags::EXCHANGE)
            };
            match swapped {
                Ok(()) => {
                    fault(opts, FaultPoint::AfterSwap)?;
                    let old = read_at(&dir, &tmp).map(|(b, _)| sha256_hex(&b));
                    if old.as_deref().ok() != Some(sha256.as_str()) {
                        // Someone wrote in between: put their file back and report.
                        let _ = rustix::fs::renameat_with(&dir, tmp.as_str(), &dir, name, RenameFlags::EXCHANGE);
                        cleanup(&dir);
                        return Err(conflict(&dir, p, name));
                    }
                    fault(opts, FaultPoint::BeforeUnlink)?;
                    cleanup(&dir);
                }
                Err(e) if unsupported(e) => {
                    out.degraded_cas = true;
                    let now = read_at(&dir, name).map(|(b, _)| sha256_hex(&b));
                    if now.as_deref().ok() != Some(sha256.as_str()) {
                        cleanup(&dir);
                        return Err(conflict(&dir, p, name));
                    }
                    rustix::fs::renameat(&dir, tmp.as_str(), &dir, name).map_err(|e| StoreError::io(&p.to_string(), e))?;
                    fault(opts, FaultPoint::AfterSwap)?;
                    fault(opts, FaultPoint::BeforeUnlink)?;
                }
                Err(e) if e == Errno::NOENT => {
                    cleanup(&dir);
                    return Err(conflict(&dir, p, name));
                }
                Err(e) => {
                    cleanup(&dir);
                    return Err(StoreError::io(&p.to_string(), e));
                }
            }
        }
    }
    fault(opts, FaultPoint::BeforeDirSync)?;
    dir_sync(&dir, opts, &mut out, &p.to_string())?;
    out.stamp = current_stamp(&dir, name);
    Ok(out)
}

pub(super) fn append_line(root: &Root, p: &RepoPath, line: &[u8]) -> Result<(), StoreError> {
    if line.len() > 4096 {
        return Err(StoreError::TooLarge {
            name: p.to_string(),
            size: u64::try_from(line.len()).unwrap_or(u64::MAX),
            cap: 4096,
        });
    }
    let (parents, name) = p.split_last();
    let dir = walk::walk(root.fd(), &parents, root.opts(true, exempt(p)))?;
    let fd = rustix::fs::openat(
        &dir,
        name,
        OFlags::WRONLY | OFlags::APPEND | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|e| walk::classify(&dir, name, e))?;
    let st = rustix::fs::fstat(&fd).map_err(|e| StoreError::io(name, e))?;
    if FileType::from_raw_mode(super::stat_mode(&st)) != FileType::RegularFile {
        return Err(StoreError::NotRegular(p.to_string()));
    }
    if super::stat_nlink(&st) > 1 {
        return Err(StoreError::HardLink(p.to_string(), super::stat_nlink(&st)));
    }
    let n = rustix::io::write(&fd, line).map_err(|e| StoreError::io(name, e))?;
    if n != line.len() {
        return Err(StoreError::io(name, Errno::IO));
    }
    rustix::fs::fdatasync(&fd).map_err(|e| StoreError::io(name, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_names() {
        assert_eq!(temp_target(".stage-02.md.0123456789abcdef.foreman-tmp"), Some("stage-02.md"));
        assert_eq!(temp_target(".x.0123.foreman-tmp"), None);
        assert!(!is_temp_name("stage-02.md"));
    }
}
