//! `release::no_webdriver` (M0-05, M0-10; ADR-015): the release dependency graph (default
//! features, the shipped target) never contains the WebDriver plugin or its HTTP server.
#![allow(
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "integration-test code (tests/) that runs `cargo tree`"
)]

mod release {
    use std::process::Command;

    fn tree(features: &[&str]) -> String {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let mut cmd = Command::new(cargo);
        cmd.args([
            "tree",
            "-p",
            "foreman-app",
            "-e",
            "normal",
            "--prefix",
            "none",
            "--target",
            "aarch64-apple-darwin",
            "--offline",
            "--locked",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"));
        if !features.is_empty() {
            cmd.args(["--features", &features.join(",")]);
        }
        let out = cmd.output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    #[test]
    fn no_webdriver() {
        let release = tree(&[]);
        for banned in ["tauri-plugin-wdio-webdriver ", "axum ", "hyper "] {
            assert!(
                !release.lines().any(|l| l.starts_with(banned)),
                "{banned}in release graph"
            );
        }
        // The check would catch it: with `e2e`, the plugin is there.
        assert!(
            tree(&["e2e"])
                .lines()
                .any(|l| l.starts_with("tauri-plugin-wdio-webdriver "))
        );
    }
}
