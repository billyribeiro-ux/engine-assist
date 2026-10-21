//! Foreman host integration (ADR-003): `spawn` (the only `Command`/`CommandBuilder` site), `git::run`
//! (the single git entry point), the `claude` CLI wrappers, the launch pipeline, `PtySession`,
//! re-arm, the watcher and sensor ingest. No Tauri: `foreman-app` adapts these to IPC.
#![forbid(unsafe_code)]

pub mod claude;
pub mod decisions;
pub mod fingerprint;
pub mod git;
pub mod launch;
pub mod lock;
pub mod pty;
pub mod rearm;
pub mod sensor;
pub mod spawn;
pub mod watch;

/// The core version this crate was built against.
#[must_use]
pub const fn core_version() -> &'static str {
    foreman_core::version()
}
