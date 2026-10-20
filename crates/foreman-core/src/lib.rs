//! Foreman's pure domain core and store (ADR-003).
//!
//! The core never spawns a process, never depends on Tauri or tokio, and touches the filesystem
//! only through `store` (arrives in M1-07; `clippy.toml` `disallowed-methods` enforces it).
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

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
