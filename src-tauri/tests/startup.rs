//! M2-03: startup hardening (AC-XC-27 companion, AC-XC-31, SR-53, ADR-014): Foreman's umask is 022
//! and its app-data folders are `0700` from the first moment, whatever the umask it was launched
//! with.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use common::TestApp;
use foreman_app::startup;
use rustix::fs::Mode;

fn mode(p: &Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

fn walk(dir: &Path, out: &mut Vec<(String, u32, bool)>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let md = std::fs::symlink_metadata(&p).unwrap();
        out.push((
            p.display().to_string(),
            md.permissions().mode() & 0o777,
            md.is_dir(),
        ));
        if md.is_dir() {
            walk(&p, out);
        }
    }
}

mod fs {
    use super::*;

    /// `fs::app_data_permissions` (startup case; AC-XC-14, AC-XC-31): launched with a umask of 0
    /// (or an over-tight 077), startup sets umask 022 and creates the app-data folder `0700`;
    /// an existing wider folder is tightened; what the app then writes is `0600` / `0700`.
    #[test]
    fn app_data_permissions() {
        // One test owns the process umask in this binary.
        for launched_with in [0o000, 0o077] {
            rustix::process::umask(Mode::from_raw_mode(launched_with));
            let base = tempfile::tempdir().unwrap();
            let app = base.path().join("Library/Application Support/dev.foreman");
            startup::harden(&app).unwrap();
            // The process default is now 022 (umask returns the previous value).
            let prev = rustix::process::umask(Mode::from_raw_mode(0o022));
            assert_eq!(
                prev.as_raw_mode(),
                startup::UMASK,
                "launched with {launched_with:o}"
            );
            assert_eq!(mode(&app), 0o700);
            // The parents Foreman had to make are not left world-writable either.
            assert_eq!(mode(app.parent().unwrap()) & 0o022, 0);
        }

        // A folder left wider by an older build (or a hand `mkdir`) is tightened at startup.
        let base = tempfile::tempdir().unwrap();
        let app = base.path().join("dev.foreman");
        std::fs::create_dir(&app).unwrap();
        std::fs::set_permissions(&app, std::fs::Permissions::from_mode(0o755)).unwrap();
        startup::harden(&app).unwrap();
        assert_eq!(mode(&app), 0o700);

        // And the app's own writes land 0600 in 0700 folders.
        rustix::process::umask(Mode::from_raw_mode(0o000));
        let t = TestApp::new();
        startup::harden(&t.app_data()).unwrap();
        let (s, stamp, _) = t.app.settings().unwrap();
        t.app.save_settings(&s, &stamp).unwrap();
        let mut all = Vec::new();
        walk(&t.app_data(), &mut all);
        assert!(!all.is_empty());
        for (p, m, is_dir) in all {
            assert_eq!(m, if is_dir { 0o700 } else { 0o600 }, "{p}");
        }
        assert_eq!(mode(&t.app_data()), 0o700);
    }
}
