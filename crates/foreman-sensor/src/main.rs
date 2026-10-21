//! Foreman's hook helper (F-011, P2, ADR-009).
//!
//! Claude Code runs it in exec form from the per-run settings file:
//! `<app-data>/bin/foreman-sensor-<sha256-12> --run <runId> --token <64 hex> --event <EventName> --v 1`
//! (`docs/contracts/files.md` §4.4). It reads at most 256 KB of hook JSON from stdin and writes one
//! `events/<runId>/<seq>-<event>.json` file (§4.8) next to its own `bin/` folder.
//!
//! Rules (SR-15, SR-53, T-045, T-048, AC-F011-03, AC-F011-04):
//! - it prints nothing and always exits 0, so it never blocks or steers Claude Code: bad arguments,
//!   bad input, a missing or symlinked folder and I/O errors all end in "write nothing, exit 0";
//! - arguments are checked exactly: a run ID that isn't a lowercase v4 UUID, a token that isn't 64
//!   lowercase hex digits, an unknown event, or anything extra is refused. A well-formed but wrong
//!   token can't be detected here (the sensor has nothing to compare it with); ingest discards it
//!   (ADR-009, `app::sensor_ingest`);
//! - `events/` and `events/<runId>/` are opened with `O_DIRECTORY|O_NOFOLLOW` (a symlink is refused,
//!   AB-12) and must be owned by this user and not group- or world-writable; neither is ever created;
//! - the file is written to `.<name>.<16 hex>.foreman-tmp`, created `O_CREAT|O_EXCL|O_NOFOLLOW`
//!   mode `0600`, then renamed into place without replacing anything (`RENAME_EXCL` /
//!   `RENAME_NOREPLACE`), so a reader never sees a partial file;
//! - the file holds only the schema's fields (`schemaVersion`, `token`, `event`, `receivedAt`,
//!   `truncated`, `payload`) in canonical form (FILE-R1: sorted keys, two-space indent, LF);
//! - after writing, only the newest 20 event files of the run are kept (SR-53, AC-F011-07).
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_methods,
    reason = "the foreman-sensor crate is its own event-file writer (clippy.toml, design doc §3.2)"
)]

use std::ffi::OsString;
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, RenameFlags};
use rustix::io::Errno;
use serde_json::{Value, json};

/// Stdin cap (SR-15): anything beyond is dropped and the event is marked `truncated`.
const STDIN_CAP: usize = 256 * 1024;
/// Events kept per run (SR-53).
const EVENTS_PER_RUN: usize = 20;
/// The longest the sensor waits for Claude Code to finish writing stdin. A complete JSON object
/// ends the read early, so a writer that never closes the pipe can't hold the hook open.
const STDIN_DEADLINE: Duration = Duration::from_secs(1);
/// The hook events the run settings register (files.md §4.8 `event` enum).
const EVENTS: [&str; 11] = [
    "SessionStart",
    "Stop",
    "StopFailure",
    "Notification",
    "PermissionRequest",
    "SessionEnd",
    "SubagentStart",
    "SubagentStop",
    "TaskCreated",
    "TaskCompleted",
    "PostToolUse",
];
/// `bin/foreman-sensor-<sha256-12>` (files.md §2 row 24).
const BIN_PREFIX: &str = "foreman-sensor-";
const TMP_SUFFIX: &str = ".foreman-tmp";

fn main() {
    // A panic must neither print (stderr reaches Claude Code's hook output) nor change the exit
    // status. Every failure path below is already a silent `None`; this is the backstop.
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::panic::catch_unwind(run);
}

/// Everything the sensor does. `None` means "write nothing"; the exit status is 0 either way.
fn run() -> Option<()> {
    let args = Args::parse(std::env::args_os().skip(1))?;
    let app_data = app_data_dir(&std::env::current_exe().ok()?)?;
    // Open the folder before reading stdin: a missing or refused folder costs no read.
    let run_dir = open_run_dir(&app_data, &args.run_id)?;
    let input = read_stdin(STDIN_CAP, STDIN_DEADLINE)?;
    let received = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    let body = event_json(&args, &input, received)?;
    let name = format!(
        "{:020}-{:010}-{}.json",
        received.as_nanos(),
        std::process::id(),
        args.event
    );
    write_new(&run_dir, &name, body.as_bytes(), received)?;
    prune(&run_dir, EVENTS_PER_RUN);
    Some(())
}

/// The validated argument vector (files.md §4.4: exactly 8 strings, in this order).
#[derive(Debug, PartialEq, Eq)]
struct Args {
    run_id: String,
    token: String,
    event: &'static str,
}

impl Args {
    fn parse(args: impl Iterator<Item = OsString>) -> Option<Self> {
        let args: Vec<String> = args
            .take(9)
            .map(OsString::into_string)
            .collect::<Result<_, _>>()
            .ok()?;
        let [
            flag_run,
            run_id,
            flag_token,
            token,
            flag_event,
            event,
            flag_v,
            v,
        ] = <[String; 8]>::try_from(args).ok()?;
        if flag_run != "--run"
            || flag_token != "--token"
            || flag_event != "--event"
            || flag_v != "--v"
            || v != "1"
        {
            return None;
        }
        if !is_uuid_v4(&run_id) || !is_token(&token) {
            return None;
        }
        let event = EVENTS.into_iter().find(|e| *e == event)?;
        Some(Self {
            run_id,
            token,
            event,
        })
    }
}

fn is_lower_hex(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}

/// The per-run token: 64 lowercase hex digits (files.md §4.6 `sensorToken`).
fn is_token(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(is_lower_hex)
}

/// `urn:foreman:common#/$defs/uuid`: a lowercase v4 UUID. It's also a safe single path component.
fn is_uuid_v4(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, &c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            14 => c == b'4',
            19 => matches!(c, b'8' | b'9' | b'a' | b'b'),
            _ => is_lower_hex(c),
        })
}

/// `<app-data>` from the sensor's own path, `<app-data>/bin/foreman-sensor-<12 hex>`. Anything else
/// (a build-tree binary, a renamed copy) writes nothing.
fn app_data_dir(exe: &Path) -> Option<PathBuf> {
    let name = exe.file_name()?.to_str()?;
    let hash = name.strip_prefix(BIN_PREFIX)?;
    if hash.len() != 12 || !hash.bytes().all(is_lower_hex) {
        return None;
    }
    let bin = exe.parent()?;
    if bin.file_name()? != "bin" {
        return None;
    }
    Some(bin.parent()?.to_path_buf())
}

/// Opens `<app-data>/events/<runId>/` without following a symlink at either of the last two
/// components and checks both are this user's, not group- or world-writable, directories.
fn open_run_dir(app_data: &Path, run_id: &str) -> Option<OwnedFd> {
    let dir_flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    let app = rustix::fs::open(app_data, dir_flags, Mode::empty()).ok()?;
    let events =
        rustix::fs::openat(&app, "events", dir_flags | OFlags::NOFOLLOW, Mode::empty()).ok()?;
    owned_private_dir(&events)?;
    let run =
        rustix::fs::openat(&events, run_id, dir_flags | OFlags::NOFOLLOW, Mode::empty()).ok()?;
    owned_private_dir(&run)?;
    Some(run)
}

fn owned_private_dir(fd: &OwnedFd) -> Option<()> {
    let st = rustix::fs::fstat(fd).ok()?;
    let is_dir = FileType::from_raw_mode(st.st_mode) == FileType::Directory;
    let mine = st.st_uid == rustix::process::geteuid().as_raw();
    let shared = Mode::from_raw_mode(st.st_mode).intersects(Mode::WGRP | Mode::WOTH);
    (is_dir && mine && !shared).then_some(())
}

/// What came in on stdin.
#[derive(Debug, PartialEq, Eq)]
struct Input {
    bytes: Vec<u8>,
    /// More than `cap` bytes were offered; only the first `cap` were read.
    truncated: bool,
}

/// Reads stdin up to `cap` bytes, stopping at EOF, at the cap, once the bytes form a complete JSON
/// object, or at the deadline. `None` on a read error.
fn read_stdin(cap: usize, deadline: Duration) -> Option<Input> {
    let stdin = std::io::stdin();
    read_capped(stdin.as_fd(), cap, deadline)
}

fn read_capped(fd: impl AsFd, cap: usize, deadline: Duration) -> Option<Input> {
    let fd = fd.as_fd();
    let start = Instant::now();
    // One extra byte tells "exactly at the cap" from "over the cap".
    let mut bytes = vec![0_u8; cap + 1];
    let mut len = 0;
    loop {
        let left = deadline.checked_sub(start.elapsed())?;
        let timeout = Timespec {
            tv_sec: i64::try_from(left.as_secs()).ok()?,
            tv_nsec: left.subsec_nanos().into(),
        };
        let mut fds = [PollFd::new(&fd, PollFlags::IN)];
        match rustix::event::poll(&mut fds, Some(&timeout)) {
            Ok(0) => break, // deadline: keep what arrived
            Ok(_) => {}
            Err(Errno::INTR) => continue,
            Err(_) => return None,
        }
        let Some(dst) = bytes.get_mut(len..) else {
            break;
        };
        match rustix::io::read(fd, dst) {
            Ok(0) => break,
            Ok(n) => {
                len += n;
                if len > cap || ends_complete_object(bytes.get(..len)?) {
                    break;
                }
            }
            Err(Errno::INTR | Errno::AGAIN) => {}
            Err(_) => return None,
        }
    }
    let truncated = len > cap;
    bytes.truncate(len.min(cap));
    Some(Input { bytes, truncated })
}

/// True when `buf` (trimmed) is one complete JSON object. Only tried when the last non-whitespace
/// byte is `}`, so a large payload arriving in chunks is parsed at most a few times.
fn ends_complete_object(buf: &[u8]) -> bool {
    buf.iter().rev().find(|b| !b.is_ascii_whitespace()) == Some(&b'}')
        && serde_json::from_slice::<Value>(buf).is_ok_and(|v| v.is_object())
}

/// The event file (files.md §4.8), canonical: sorted keys, two-space indent, LF, trailing newline.
///
/// `payload` is the hook input when it parses as a JSON object. Input that was cut at the cap and
/// no longer parses becomes `payload: null, truncated: true`. Input that wasn't cut and isn't a
/// JSON object (garbage, an array, empty stdin) writes nothing.
fn event_json(args: &Args, input: &Input, received: Duration) -> Option<String> {
    let payload = match serde_json::from_slice::<Value>(&input.bytes) {
        Ok(mut v @ Value::Object(_)) => {
            v.sort_all_objects();
            v
        }
        _ if input.truncated => Value::Null,
        _ => return None,
    };
    let mut doc = json!({
        "schemaVersion": 1,
        "token": args.token,
        "event": args.event,
        "receivedAt": rfc3339_millis(received),
        "truncated": input.truncated,
        "payload": payload,
    });
    doc.sort_all_objects();
    let mut out = serde_json::to_string_pretty(&doc).ok()?;
    out.push('\n');
    Some(out)
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` (`urn:foreman:common#/$defs/timestamp`), from the Unix epoch.
fn rfc3339_millis(since_epoch: Duration) -> String {
    let secs = since_epoch.as_secs();
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let tod = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        tod / 3600,
        tod % 3600 / 60,
        tod % 60,
        since_epoch.subsec_millis()
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (H. Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        y,
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

/// Writes `body` as the new file `name` in `dir`: a `0600` temp file created with
/// `O_CREAT|O_EXCL|O_NOFOLLOW`, then renamed into place without replacing anything. On any failure
/// the temp file is removed and nothing is left behind.
fn write_new(dir: &OwnedFd, name: &str, body: &[u8], salt: Duration) -> Option<()> {
    let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut seed = salt.as_nanos() as u64 ^ (u64::from(std::process::id()) << 32);
    for _ in 0..3 {
        seed = splitmix64(seed);
        let tmp = format!(".{name}.{seed:016x}{TMP_SUFFIX}");
        let fd = match rustix::fs::openat(dir, tmp.as_str(), flags, Mode::RUSR | Mode::WUSR) {
            Ok(fd) => fd,
            Err(Errno::EXIST) => continue,
            Err(_) => return None,
        };
        let written = rustix::fs::fchmod(&fd, Mode::RUSR | Mode::WUSR).is_ok()
            && write_all(&fd, body)
            && rustix::fs::renameat_with(dir, tmp.as_str(), dir, name, RenameFlags::NOREPLACE)
                .is_ok();
        drop(fd);
        if !written {
            let _ = rustix::fs::unlinkat(dir, tmp.as_str(), AtFlags::empty());
            return None;
        }
        return Some(());
    }
    None
}

fn write_all(fd: &OwnedFd, mut body: &[u8]) -> bool {
    while !body.is_empty() {
        match rustix::io::write(fd, body) {
            Ok(0) => return false,
            Ok(n) => body = body.get(n..).unwrap_or_default(),
            Err(Errno::INTR) => {}
            Err(_) => return false,
        }
    }
    true
}

fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// `<20-digit ns>-<10-digit pid>-<Event>.json`: the names this sensor writes.
fn is_event_file(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".json") else {
        return false;
    };
    let b = stem.as_bytes();
    b.len() > 32
        && b.get(..20)
            .is_some_and(|d| d.iter().all(u8::is_ascii_digit))
        && b.get(20) == Some(&b'-')
        && b.get(21..31)
            .is_some_and(|d| d.iter().all(u8::is_ascii_digit))
        && b.get(31) == Some(&b'-')
        && stem.get(32..).is_some_and(|e| EVENTS.contains(&e))
}

/// Keeps the newest `keep` event files of the run (names sort by `seq`) and unlinks the rest.
/// Other names are left alone. Races with a concurrent sensor are harmless (`ENOENT` is ignored).
fn prune(dir: &OwnedFd, keep: usize) {
    let Ok(entries) = Dir::read_from(dir) else {
        return;
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().ok().map(str::to_owned))
        .filter(|n| is_event_file(n))
        .collect();
    if names.len() <= keep {
        return;
    }
    names.sort_unstable();
    let excess = names.len() - keep;
    for old in names.iter().take(excess) {
        let _ = rustix::fs::unlinkat(dir, old.as_str(), AtFlags::empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const RUN: &str = "0b5c9d8e-1f2a-4b3c-9d4e-5f6a7b8c9d0e";
    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn os(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }

    fn good() -> Vec<&'static str> {
        vec![
            "--run", RUN, "--token", TOKEN, "--event", "Stop", "--v", "1",
        ]
    }

    fn private_dir() -> (tempfile::TempDir, OwnedFd) {
        let tmp = tempfile::tempdir().unwrap();
        let fd = rustix::fs::open(
            tmp.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();
        (tmp, fd)
    }

    #[test]
    fn args_exact_form_only() {
        let ok = Args::parse(os(&good()).into_iter()).unwrap();
        assert_eq!(ok.run_id, RUN);
        assert_eq!(ok.event, "Stop");
        // Any change to the exact 8-element vector is refused.
        let mut extra = good();
        extra.push("--x");
        assert!(Args::parse(os(&extra).into_iter()).is_none());
        assert!(Args::parse(os(&good()[..6]).into_iter()).is_none());
        for (i, bad) in [
            (0, "--runs"),
            (1, "../../etc"),
            (1, "0B5C9D8E-1F2A-4B3C-9D4E-5F6A7B8C9D0E"),
            (1, "0b5c9d8e-1f2a-1b3c-9d4e-5f6a7b8c9d0e"),
            (3, &TOKEN[..63]),
            (3, &TOKEN.to_uppercase()),
            (5, "PreToolUse"),
            (5, "stop"),
            (7, "2"),
        ] {
            let mut v: Vec<String> = good().into_iter().map(str::to_owned).collect();
            v[i] = bad.to_owned();
            assert!(
                Args::parse(v.into_iter().map(OsString::from)).is_none(),
                "{i} {bad}"
            );
        }
    }

    #[test]
    fn app_data_only_from_installed_layout() {
        let ok = Path::new("/a/App Support/bin/foreman-sensor-0123456789ab");
        assert_eq!(app_data_dir(ok).unwrap(), Path::new("/a/App Support"));
        for bad in [
            "/a/bin/foreman-sensor",
            "/a/bin/foreman-sensor-0123456789AB",
            "/a/bin/foreman-sensor-0123456789abc",
            "/a/target/foreman-sensor-0123456789ab",
        ] {
            assert!(app_data_dir(Path::new(bad)).is_none(), "{bad}");
        }
    }

    #[test]
    fn timestamp_is_rfc3339_utc() {
        assert_eq!(rfc3339_millis(Duration::ZERO), "1970-01-01T00:00:00.000Z");
        // 2026-10-20T13:45:07.123Z
        let t = Duration::from_millis(1_792_503_907_123);
        assert_eq!(rfc3339_millis(t), "2026-10-20T13:45:07.123Z");
        // Leap day.
        assert_eq!(
            rfc3339_millis(Duration::from_secs(951_782_400)),
            "2000-02-29T00:00:00.000Z"
        );
    }

    #[test]
    fn event_file_names() {
        assert!(is_event_file("00000000000000000001-0000000042-Stop.json"));
        assert!(!is_event_file("00000000000000000001-0000000042-Bogus.json"));
        assert!(!is_event_file(
            ".00000000000000000001-0000000042-Stop.json.0011223344556677.foreman-tmp"
        ));
        assert!(!is_event_file("1-42-Stop.json"));
    }

    #[test]
    fn write_new_is_0600_and_leaves_no_temp() {
        let (tmp, dir) = private_dir();
        write_new(&dir, "a.json", b"{}\n", Duration::from_nanos(7)).unwrap();
        let path = tmp.path().join("a.json");
        assert_eq!(std::fs::read(&path).unwrap(), b"{}\n");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 1);
    }

    /// A symlink planted at the final name is never followed or replaced, and the temp is removed.
    #[test]
    fn write_new_refuses_planted_symlink_at_target() {
        let (tmp, dir) = private_dir();
        let outside = tempfile::tempdir().unwrap();
        let victim = outside.path().join("victim");
        std::fs::write(&victim, b"keep").unwrap();
        std::os::unix::fs::symlink(&victim, tmp.path().join("a.json")).unwrap();
        assert!(write_new(&dir, "a.json", b"evil", Duration::from_nanos(7)).is_none());
        assert_eq!(std::fs::read(&victim).unwrap(), b"keep");
        assert!(
            std::fs::symlink_metadata(tmp.path().join("a.json"))
                .unwrap()
                .is_symlink()
        );
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 1);
    }

    /// A symlink planted at the temp name is refused by `O_EXCL|O_NOFOLLOW`; a fresh name is tried.
    #[test]
    fn write_new_refuses_planted_symlink_at_temp() {
        let (tmp, dir) = private_dir();
        let outside = tempfile::tempdir().unwrap();
        let victim = outside.path().join("victim");
        std::fs::write(&victim, b"keep").unwrap();
        let salt = Duration::from_nanos(7);
        let seed = splitmix64(salt.as_nanos() as u64 ^ (u64::from(std::process::id()) << 32));
        let planted = tmp.path().join(format!(".a.json.{seed:016x}{TMP_SUFFIX}"));
        std::os::unix::fs::symlink(&victim, &planted).unwrap();
        write_new(&dir, "a.json", b"ok", salt).unwrap();
        assert_eq!(std::fs::read(&victim).unwrap(), b"keep");
        assert_eq!(std::fs::read(tmp.path().join("a.json")).unwrap(), b"ok");
    }

    #[test]
    fn prune_keeps_newest_event_files_only() {
        let (tmp, dir) = private_dir();
        for i in 0..25_u64 {
            std::fs::write(
                tmp.path().join(format!("{i:020}-0000000001-Stop.json")),
                b"{}",
            )
            .unwrap();
        }
        std::fs::write(tmp.path().join("other.json"), b"{}").unwrap();
        prune(&dir, 20);
        let mut left: Vec<String> = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left.len(), 21);
        assert_eq!(left[0], format!("{:020}-0000000001-Stop.json", 5));
        assert!(left.contains(&"other.json".to_owned()));
    }

    #[test]
    fn read_capped_stops_at_cap_and_at_complete_object() {
        let (mut r, mut w) = std::io::pipe().unwrap();
        // Stdin stays open: a complete object ends the read before the deadline.
        std::io::Write::write_all(&mut w, b"{\"a\":1}\n").unwrap();
        let start = Instant::now();
        let got = read_capped(&r, 16, Duration::from_secs(5)).unwrap();
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(
            got,
            Input {
                bytes: b"{\"a\":1}\n".to_vec(),
                truncated: false
            }
        );
        std::io::Write::write_all(&mut w, &[b'x'; 40]).unwrap();
        drop(w);
        let got = read_capped(&r, 16, Duration::from_secs(5)).unwrap();
        assert_eq!(
            got,
            Input {
                bytes: vec![b'x'; 16],
                truncated: true
            }
        );
        let _ = std::io::Read::read(&mut r, &mut [0; 64]);
    }

    #[test]
    fn read_capped_gives_up_at_deadline() {
        let (r, mut w) = std::io::pipe().unwrap();
        std::io::Write::write_all(&mut w, b"{\"a\":").unwrap();
        let got = read_capped(&r, 16, Duration::from_millis(50)).unwrap();
        assert_eq!(
            got,
            Input {
                bytes: b"{\"a\":".to_vec(),
                truncated: false
            }
        );
    }
}
