//! `network::blocked_network_run` (M0-11, AC-M0-10, ADR-016 decision 6). macOS only: the
//! deny-network profile must block an outbound request, and the test double must still run under
//! it. The full harness run and the app's first-run path are re-run under it in M3-29 and M5-08.
#![cfg(target_os = "macos")]
#![allow(
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "integration-test code (tests/) that spawns sandbox-exec"
)]

mod network {
    use std::path::PathBuf;
    use std::process::Command;

    fn profile() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/sandbox/deny-network.sb")
    }

    #[test]
    fn blocked_network_run() {
        let blocked = Command::new("/usr/bin/sandbox-exec")
            .arg("-f")
            .arg(profile())
            .args([
                "/usr/bin/curl",
                "-sS",
                "-m",
                "5",
                "-o",
                "/dev/null",
                "https://example.com",
            ])
            .status()
            .unwrap();
        assert!(
            !blocked.success(),
            "deny-network profile let an outbound request through"
        );

        let local = Command::new("/usr/bin/sandbox-exec")
            .arg("-f")
            .arg(profile())
            .args(["/bin/echo", "ok"])
            .output()
            .unwrap();
        assert!(local.status.success());
        assert_eq!(String::from_utf8(local.stdout).unwrap(), "ok\n");
    }
}
