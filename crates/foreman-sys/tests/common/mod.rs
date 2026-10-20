//! Shared helpers for foreman-sys integration tests: fixture repos with the real git, and copies
//! of the fake-claude test double installed under allowlisted names.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "shared test helpers; tests may spawn and use std::fs (design doc §3.2)"
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use foreman_sys::spawn::{Program, SpawnCtx};

/// The built fake-claude binary (built once per test run).
pub fn fake_claude() -> PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let st = Command::new(cargo)
            .args(["build", "-q", "-p", "fake-claude"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .unwrap();
        assert!(st.success());
        let target = std::env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
        target.join("debug/fake-claude")
    })
    .clone()
}

/// Copies fake-claude to `<dir>/<name>` with a `fake.json` next to it; returns the path.
pub fn install_fake(dir: &Path, name: &str, config: &serde_json::Value) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let dst = dir.join(name);
    std::fs::copy(fake_claude(), &dst).unwrap();
    std::fs::write(dir.join("fake.json"), config.to_string()).unwrap();
    dst
}

/// The real git on this machine.
pub fn real_git() -> PathBuf {
    for c in [
        "/usr/bin/git",
        "/opt/homebrew/bin/git",
        "/usr/local/bin/git",
    ] {
        if Path::new(c).exists() {
            return PathBuf::from(c);
        }
    }
    panic!("git not found")
}

/// A spawn context using the real git.
pub fn ctx(app_data: &Path) -> SpawnCtx {
    SpawnCtx {
        home: std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| "/root".into()),
        app_data: app_data.to_path_buf(),
        projects: vec![],
        settings: vec![(Program::Git, real_git())],
    }
}

/// Runs the real git in a test fixture (test code only).
pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(real_git())
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim_end().to_owned()
}

/// `git init` with an identity.
pub fn init_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "test@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
}

/// Writes a file and commits everything (fixture setup).
pub fn commit_file(dir: &Path, rel: &str, body: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
    git(dir, &["add", "--", rel]);
    git(dir, &["commit", "-qm", &format!("fixture {rel}")]);
}

/// A hostile repo: hooks everywhere, a filter driver on every file, fsmonitor and gpg programs,
/// each of which would create a marker file in `markers` if it ran.
pub fn hostile_repo(dir: &Path, markers: &Path) {
    init_repo(dir);
    std::fs::create_dir_all(markers).unwrap();
    let hooks = dir.join(".githooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let m = markers.display();
    for h in [
        "pre-commit",
        "commit-msg",
        "post-commit",
        "reference-transaction",
        "post-checkout",
        "pre-auto-gc",
        "post-index-change",
        "prepare-commit-msg",
        "post-rewrite",
    ] {
        let p = hooks.join(h);
        std::fs::write(&p, format!("#!/bin/sh\ntouch {m}/hook-{h}\n")).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // Also the default hooks directory.
    for h in ["pre-commit", "reference-transaction", "post-commit"] {
        let p = dir.join(".git/hooks").join(h);
        std::fs::write(&p, format!("#!/bin/sh\ntouch {m}/default-hook-{h}\n")).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let script = |name: &str| {
        let p = markers.join(format!("{name}.sh"));
        std::fs::write(&p, format!("#!/bin/sh\ntouch {m}/{name}\ncat\n")).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.display().to_string()
    };
    git(dir, &["config", "core.hooksPath", ".githooks"]);
    git(
        dir,
        &["config", "filter.evil.clean", &script("filter-clean")],
    );
    git(
        dir,
        &["config", "filter.evil.smudge", &script("filter-smudge")],
    );
    git(dir, &["config", "filter.evil.required", "true"]);
    git(dir, &["config", "core.fsmonitor", &script("fsmonitor")]);
    git(dir, &["config", "gpg.program", &script("gpg")]);
    git(dir, &["config", "commit.gpgSign", "true"]);
    git(dir, &["config", "core.pager", &script("pager")]);
    git(dir, &["config", "core.editor", &script("editor")]);
    std::fs::write(dir.join(".gitattributes"), "* filter=evil diff=evil\n").unwrap();
    std::fs::write(dir.join(".git/info/attributes"), "").unwrap();
    git(dir, &["config", "diff.evil.textconv", &script("textconv")]);
    // A first commit made by the test with every program disabled, so HEAD exists.
    std::fs::write(dir.join("README.md"), "hi\n").unwrap();
    let out = Command::new(real_git())
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "filter.evil.clean=cat",
            "-c",
            "filter.evil.smudge=cat",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(["add", "README.md", ".gitattributes"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new(real_git())
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "filter.evil.clean=cat",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(["commit", "-qm", "init"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read_dir(markers)
            .unwrap()
            .filter(|e| !e
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".sh"))
            .count(),
        0
    );
}

/// Marker files created by hostile programs.
pub fn markers_fired(markers: &Path) -> Vec<String> {
    std::fs::read_dir(markers)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| !n.ends_with(".sh"))
        .collect()
}
