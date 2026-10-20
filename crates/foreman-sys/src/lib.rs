//! Foreman host integration (ADR-003): `spawn` (the only `Command` site, M2), `git::run` (M1-08),
//! the `claude` CLI wrappers, `PtySession` and `/usr/bin/open`. No Tauri.
#![forbid(unsafe_code)]

/// The core version this crate was built against (keeps the dependency edge real until M1).
#[must_use]
pub const fn core_version() -> &'static str {
    foreman_core::version()
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(super::core_version(), foreman_core::version());
    }
}
