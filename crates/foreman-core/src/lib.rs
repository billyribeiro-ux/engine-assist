//! Foreman's pure domain core and store (ADR-003, design doc §3.2.1).
//!
//! The core never spawns a process and never depends on Tauri or tokio. It touches the filesystem
//! only through [`store`] (`clippy.toml` `disallowed-methods` enforces it, AC-XC-30). Facts that
//! need processes or the clock are gathered by `foreman-sys` / `foreman-app` and passed in.
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

pub mod chain;
pub mod compat;
pub mod decisions;
pub mod diagnostic;
pub mod fingerprint;
pub mod git;
pub mod ids;
pub mod interest;
pub mod json;
pub mod launch;
pub mod lock;
pub mod packet;
pub mod pathpolicy;
pub mod policy;
pub mod probe;
pub mod rearm;
pub mod resolver;
pub mod rules;
pub mod sanitize;
pub mod schema;
pub mod session_state;
pub mod steps;
pub mod store;
pub mod template;

/// The crate version, as built.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::version;

    #[test]
    fn version_matches_manifest() {
        assert_eq!(version(), "0.1.0");
    }
}
