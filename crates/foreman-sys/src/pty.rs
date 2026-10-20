//! `PtySession` over portable-pty 0.9.0 (ADR-006, SR-24, SR-28, SR-57, NFR-08).
//!
//! - The command comes only from [`crate::spawn::claude_pty_command`] (absolute re-checked
//!   program, core argv, `env_clear` + the built env, canonical cwd, umask 022). portable-pty's
//!   Unix spawn adds `setsid`, the controlling TTY and closes fds above 2.
//! - Output: a reader thread reads ≤ 64 KB chunks, keeps a ring of the last 2,000 lines (for
//!   `pty_attach` and `output.log`) and hands each chunk to the sink as raw bytes (`foreman-app`
//!   sends them as `tauri::ipc::Response`). `set_flow(true)` parks the reader, so the kernel PTY
//!   buffer back-pressures `claude` (NFR-08).
//! - Input: [`PtySession::write`] (keystrokes ≤ 64 KB) only while the process is live (SR-24);
//!   pastes are sanitized in Rust first ([`sanitize_paste`], SR-28).
//! - Signals: [`PtySession::terminate`] (SIGTERM to the process group, SIGKILL after the grace
//!   period) is for Terminate after Billy's native confirmation; [`PtySession::hangup`] is the quit
//!   path (SIGHUP). Re-arm never signals (`crate::rearm`).

use std::collections::VecDeque;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use foreman_core::sanitize::{self, SanitizeReport};
use portable_pty::{Child, ChildKiller, MasterPty, PtySize};

use crate::spawn::{self, SpawnCtx, SpawnError};

/// Max bytes per write (ipc.md `PtyData`).
pub const MAX_WRITE: usize = 64 * 1024;
/// Max chunk handed to the sink.
pub const CHUNK: usize = 64 * 1024;
/// Lines kept for replay.
pub const RING_LINES: usize = 2_000;
/// Bytes kept for replay at most (a line can be long).
pub const RING_BYTES: usize = 4 * 1024 * 1024;
/// SIGTERM → SIGKILL grace (ADR-006).
pub const TERMINATE_GRACE: Duration = Duration::from_secs(5);

/// A PTY failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PtyError {
    /// The spawn checks refused.
    #[error(transparent)]
    Spawn(#[from] SpawnError),
    /// The PTY system failed.
    #[error("pty: {0}")]
    Pty(String),
    /// The process isn't live (SR-24: `pty-not-live`).
    #[error("the session isn't running")]
    NotLive,
    /// More than 64 KB in one write.
    #[error("write over {MAX_WRITE} bytes")]
    TooLarge,
    /// cols 2..=1000, rows 2..=500.
    #[error("size out of range")]
    BadSize,
}

/// How the process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitInfo {
    /// Exit code (`None` when a signal ended it).
    pub code: Option<i32>,
    /// The signal's name was reported.
    pub signaled: bool,
}

/// Run state as the PTY sees it (the P2 sensor adds idle/blocked in `foreman-app`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtyStatus {
    /// Running.
    Running,
    /// Exited.
    Exited(ExitInfo),
}

/// Receives output chunks (raw bytes, in order) and the exit.
pub trait Sink: Send + Sync + 'static {
    /// One chunk.
    fn chunk(&self, bytes: &[u8]);
    /// The process exited (after the last chunk the reader saw).
    fn exited(&self, info: ExitInfo);
}

struct Ring {
    buf: VecDeque<u8>,
    lines: usize,
}

impl Ring {
    fn push(&mut self, b: &[u8]) {
        self.lines += b.iter().filter(|c| **c == b'\n').count();
        self.buf.extend(b);
        while self.lines > RING_LINES || self.buf.len() > RING_BYTES {
            match self.buf.iter().position(|c| *c == b'\n') {
                Some(i) if self.lines > 0 => {
                    self.buf.drain(..=i);
                    self.lines -= 1;
                }
                _ => {
                    let extra = self.buf.len().saturating_sub(RING_BYTES);
                    self.buf.drain(..extra.max(1).min(self.buf.len()));
                    if self.buf.len() <= RING_BYTES {
                        break;
                    }
                }
            }
        }
    }
}

struct Inner {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn std::io::Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    pid: Option<u32>,
    ring: Mutex<Ring>,
    paused: Mutex<bool>,
    flow: Condvar,
    status: Mutex<PtyStatus>,
    /// Set after the sink was told of the exit; what `wait_exit` waits on, so a caller that saw
    /// the exit also sees everything the sink did with it.
    reported: Mutex<Option<ExitInfo>>,
    exited: Condvar,
    live: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One `claude` session in a PTY.
#[derive(Clone)]
pub struct PtySession {
    inner: Arc<Inner>,
    program: PathBuf,
}

impl std::fmt::Debug for PtySession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtySession")
            .field("pid", &self.inner.pid)
            .field("program", &self.program)
            .finish_non_exhaustive()
    }
}

/// `cols` 2..=1000, `rows` 2..=500 (ipc.md `pty_resize`).
///
/// # Errors
/// [`PtyError::BadSize`].
pub fn size(cols: u16, rows: u16) -> Result<PtySize, PtyError> {
    if !(2..=1000).contains(&cols) || !(2..=500).contains(&rows) {
        return Err(PtyError::BadSize);
    }
    Ok(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })
}

impl PtySession {
    /// Spawns `claude` with `args` in `cwd` with exactly `env`, streaming output to `sink`.
    ///
    /// # Errors
    /// [`PtyError`] when a check refuses or the PTY can't be opened.
    pub fn spawn(
        ctx: &SpawnCtx,
        args: &[String],
        env: &[(String, String)],
        cwd: &Path,
        pty_size: PtySize,
        sink: Arc<dyn Sink>,
    ) -> Result<Self, PtyError> {
        let (cmd, program) = spawn::claude_pty_command(ctx, args, env, cwd)?;
        let pty = portable_pty::native_pty_system();
        let pair = pty
            .openpty(pty_size)
            .map_err(|e| PtyError::Pty(e.to_string()))?;
        // Re-check right before spawn: portable-pty falls back to $HOME for a missing cwd.
        spawn::check_session_cwd(cwd)?;
        let child: Box<dyn Child + Send + Sync> = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::Pty(e.to_string()))?;
        drop(pair.slave);
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::Pty(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::Pty(e.to_string()))?;
        let inner = Arc::new(Inner {
            killer: Mutex::new(child.clone_killer()),
            pid: child.process_id(),
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            ring: Mutex::new(Ring {
                buf: VecDeque::new(),
                lines: 0,
            }),
            paused: Mutex::new(false),
            flow: Condvar::new(),
            status: Mutex::new(PtyStatus::Running),
            reported: Mutex::new(None),
            exited: Condvar::new(),
            live: AtomicBool::new(true),
        });
        let reader_done = Arc::new((Mutex::new(false), Condvar::new()));
        {
            let inner = Arc::clone(&inner);
            let sink = Arc::clone(&sink);
            let done = Arc::clone(&reader_done);
            std::thread::Builder::new()
                .name("pty-reader".into())
                .spawn(move || read_loop(&inner, reader, sink.as_ref(), &done))
                .map_err(|e| PtyError::Pty(e.to_string()))?;
        }
        {
            let inner = Arc::clone(&inner);
            let done = Arc::clone(&reader_done);
            std::thread::Builder::new()
                .name("pty-wait".into())
                .spawn(move || wait_loop(&inner, child, sink.as_ref(), &done))
                .map_err(|e| PtyError::Pty(e.to_string()))?;
        }
        Ok(Self { inner, program })
    }

    /// The resolved program that was spawned.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// The child's PID (also its process group and session ID).
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.inner.pid
    }

    /// Whether the process is running.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.inner.live.load(Ordering::SeqCst)
    }

    /// The status.
    #[must_use]
    pub fn status(&self) -> PtyStatus {
        *lock(&self.inner.status)
    }

    /// Writes keystrokes (≤ 64 KB) — only while live (SR-24).
    ///
    /// # Errors
    /// [`PtyError::NotLive`], [`PtyError::TooLarge`], or a write failure.
    pub fn write(&self, bytes: &[u8]) -> Result<(), PtyError> {
        if bytes.len() > MAX_WRITE {
            return Err(PtyError::TooLarge);
        }
        if !self.is_live() {
            return Err(PtyError::NotLive);
        }
        let mut w = lock(&self.inner.writer);
        w.write_all(bytes)
            .and_then(|()| w.flush())
            .map_err(|e| PtyError::Pty(e.to_string()))
    }

    /// Ctrl-C (`pty_interrupt`).
    ///
    /// # Errors
    /// As [`PtySession::write`].
    pub fn interrupt(&self) -> Result<(), PtyError> {
        self.write(b"\x03")
    }

    /// `MasterPty::resize`.
    ///
    /// # Errors
    /// [`PtyError::NotLive`] or a resize failure.
    pub fn resize(&self, s: PtySize) -> Result<(), PtyError> {
        if !self.is_live() {
            return Err(PtyError::NotLive);
        }
        lock(&self.inner.master)
            .resize(s)
            .map_err(|e| PtyError::Pty(e.to_string()))
    }

    /// Pauses or resumes the reader (`pty_flow`, NFR-08).
    pub fn set_flow(&self, paused: bool) {
        *lock(&self.inner.paused) = paused;
        self.inner.flow.notify_all();
    }

    /// The replay tail (≤ 2,000 lines) and its line count (`pty_attach`, `output.log`).
    #[must_use]
    pub fn tail(&self) -> (Vec<u8>, u32) {
        let r = lock(&self.inner.ring);
        (
            r.buf.iter().copied().collect(),
            u32::try_from(r.lines).unwrap_or(u32::MAX),
        )
    }

    /// Waits up to `timeout` for the exit; returns only after the sink was told of it (so the
    /// sink's `exited` must not call this).
    #[must_use]
    pub fn wait_exit(&self, timeout: Duration) -> Option<ExitInfo> {
        let deadline = Instant::now() + timeout;
        let mut st = lock(&self.inner.reported);
        loop {
            if let Some(i) = *st {
                return Some(i);
            }
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            st = self
                .inner
                .exited
                .wait_timeout(st, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    fn signal_group(&self, sig: rustix::process::Signal) {
        if let Some(pg) = self
            .inner
            .pid
            .and_then(|p| i32::try_from(p).ok())
            .and_then(rustix::process::Pid::from_raw)
        {
            let _ = rustix::process::kill_process_group(pg, sig);
        }
    }

    /// Terminate (ADR-006): SIGTERM to the process group, SIGKILL after `grace`. Only after
    /// Billy's native confirmation (SR-13, SR-24a); re-arm never calls it.
    pub fn terminate(&self, grace: Duration) -> Option<ExitInfo> {
        if !self.is_live() {
            return self.wait_exit(Duration::ZERO);
        }
        self.signal_group(rustix::process::Signal::TERM);
        if let Some(i) = self.wait_exit(grace) {
            return Some(i);
        }
        self.signal_group(rustix::process::Signal::KILL);
        self.wait_exit(Duration::from_secs(2))
    }

    /// The quit path after the quit guard (SR-57): portable-pty's killer (SIGHUP, then SIGKILL
    /// after about 250 ms).
    pub fn hangup(&self) {
        if self.is_live() {
            let _ = lock(&self.inner.killer).kill();
        }
    }
}

fn read_loop(
    inner: &Inner,
    mut reader: Box<dyn std::io::Read + Send>,
    sink: &dyn Sink,
    done: &(Mutex<bool>, Condvar),
) {
    let mut buf = vec![0u8; CHUNK];
    loop {
        {
            let mut p = lock(&inner.paused);
            while *p {
                p = inner
                    .flow
                    .wait_timeout(p, Duration::from_millis(200))
                    .unwrap_or_else(PoisonError::into_inner)
                    .0;
            }
        }
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                lock(&inner.ring).push(&buf[..n]);
                sink.chunk(&buf[..n]);
            }
        }
    }
    *lock(&done.0) = true;
    done.1.notify_all();
}

fn wait_loop(
    inner: &Inner,
    mut child: Box<dyn Child + Send + Sync>,
    sink: &dyn Sink,
    reader_done: &(Mutex<bool>, Condvar),
) {
    let st = child.wait();
    inner.live.store(false, Ordering::SeqCst);
    let info = match st {
        Ok(s) => ExitInfo {
            code: if s.signal().is_some() {
                None
            } else {
                i32::try_from(s.exit_code()).ok()
            },
            signaled: s.signal().is_some(),
        },
        Err(_) => ExitInfo {
            code: None,
            signaled: false,
        },
    };
    // Let the reader drain what the child wrote before it exited (bounded wait).
    {
        let mut d = lock(&reader_done.0);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !*d && Instant::now() < deadline && !*lock(&inner.paused) {
            d = reader_done
                .1
                .wait_timeout(d, Duration::from_millis(50))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
    *lock(&inner.status) = PtyStatus::Exited(info);
    sink.exited(info);
    *lock(&inner.reported) = Some(info);
    inner.exited.notify_all();
}

/// A sanitized paste (`pty_paste` → `PasteText`): ESC, C0 (except TAB, LF, CR), DEL and C1 are
/// removed in Rust before xterm adds the bracketed-paste markers (SR-28), so an embedded
/// `ESC[201~` can't end the paste early.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteText {
    /// The text for `term.paste`.
    pub text: String,
    /// How many characters were removed (shown as a notice).
    pub removed: usize,
    /// "removed N control characters", when any were.
    pub notice: Option<String>,
}

/// SR-28.
#[must_use]
pub fn sanitize_paste(text: &str) -> PasteText {
    let (t, r): (String, SanitizeReport) = sanitize::for_paste(text);
    PasteText {
        notice: r.message(),
        removed: r.removed,
        text: t,
    }
}

/// What xterm sends for `term.paste(text)` when the TUI enabled bracketed paste (DECSET 2004):
/// `ESC[200~` + text (CR line endings) + `ESC[201~`. Used by tests and by any Rust-side paste.
#[must_use]
pub fn bracketed(text: &str) -> Vec<u8> {
    let body = text.replace("\r\n", "\r").replace('\n', "\r");
    let mut v = b"\x1b[200~".to_vec();
    v.extend_from_slice(body.as_bytes());
    v.extend_from_slice(b"\x1b[201~");
    v
}
