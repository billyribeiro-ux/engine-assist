//! Startup hardening (M2-03, ADR-014, SR-53, AC-XC-31): before anything is written, Foreman's own
//! umask is 022 and the app-data folder exists with mode `0700`. Files are created `0600` and
//! sub-folders `0700` by the core store whatever the umask; this makes the process default
//! match, so nothing Foreman or a child inherits starts out group- or world-writable.

use std::path::Path;

use foreman_core::store::{Root, StoreError};
use rustix::fs::Mode;

/// The umask Foreman runs with (and gives every child, `foreman-sys::spawn`).
pub const UMASK: u32 = 0o022;

/// Sets the process umask to [`UMASK`] and creates the app-data folder `0700`.
///
/// # Errors
/// When the app-data folder can't be created or its mode can't be set.
pub fn harden(app_data: &Path) -> Result<(), StoreError> {
    rustix::process::umask(Mode::from_raw_mode(UMASK));
    Root::open_app_data(app_data).map(drop)
}
