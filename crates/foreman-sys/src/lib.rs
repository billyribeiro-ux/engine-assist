//! Foreman host integration (ADR-003): `spawn` (the only `Command` site), `git::run` (the single git
//! entry point), and later the `claude` CLI wrappers, `PtySession` and `/usr/bin/open`. No Tauri.
#![forbid(unsafe_code)]

pub mod decisions;
pub mod git;
pub mod lock;
pub mod spawn;

/// The core version this crate was built against.
#[must_use]
pub const fn core_version() -> &'static str {
    foreman_core::version()
}
