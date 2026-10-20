//! fake-claude behaviour (M0-05): argv/env/cwd recording, `-v`, `auth status`, `--init-only`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "integration-test code (tests/): unwrap/expect are denied only outside tests (SR-62)"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "integration tests spawn the test double and read its record (test code, ADR-003)"
)]

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn fake(dir: &Path, args: &[&str], script: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fake-claude"));
    cmd.args(args)
        .current_dir(dir)
        .env_clear()
        .env("FAKE_CLAUDE_RECORD_DIR", dir)
        .env("FOREMAN_RUN_ID", "run-1")
        .env("FOREMAN_MARKER", "m0-05");
    if let Some(body) = script {
        let path = dir.join("script.json");
        std::fs::write(&path, body).unwrap();
        cmd.env("FAKE_CLAUDE_SCRIPT", path);
    }
    cmd.output().unwrap()
}

fn recorded(dir: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("run-1.json")).unwrap()).unwrap()
}

#[test]
fn records_argv_env_and_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let out = fake(
        tmp.path(),
        &["--session-id", "abc", "--model", "opus", "hello world"],
        None,
    );
    assert!(out.status.success());
    let rec = recorded(tmp.path());
    let argv: Vec<&str> = rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .skip(1)
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        argv,
        ["--session-id", "abc", "--model", "opus", "hello world"]
    );
    assert_eq!(rec["env"]["FOREMAN_MARKER"], "m0-05");
    assert!(
        rec["env"].get("HOME").is_none(),
        "env_clear must be visible in the record"
    );
    let cwd = std::fs::canonicalize(tmp.path()).unwrap();
    assert_eq!(
        std::fs::canonicalize(rec["cwd"].as_str().unwrap()).unwrap(),
        cwd
    );
}

#[test]
fn answers_version_from_script() {
    let tmp = tempfile::tempdir().unwrap();
    let out = fake(
        tmp.path(),
        &["-v"],
        Some(r#"{"version":"9.9.9 (Claude Code)"}"#),
    );
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "9.9.9 (Claude Code)\n"
    );
}

#[test]
fn answers_auth_status_with_scripted_method_and_exit() {
    let tmp = tempfile::tempdir().unwrap();
    let out = fake(
        tmp.path(),
        &["auth", "status"],
        Some(r#"{"auth":{"loggedIn":true,"authMethod":"api_key"},"authExit":1}"#),
    );
    assert_eq!(out.status.code(), Some(1));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["authMethod"], "api_key");
}

#[test]
fn init_only_exit_code_is_scripted() {
    let tmp = tempfile::tempdir().unwrap();
    let out = fake(
        tmp.path(),
        &["--init-only", "--settings", "x.json"],
        Some(r#"{"initOnlyExit":3,"initOnlyOutput":"hooks ran"}"#),
    );
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "hooks ran\n");
}

#[test]
fn plays_scripted_output_and_exit_code() {
    let tmp = tempfile::tempdir().unwrap();
    let out = fake(
        tmp.path(),
        &["prompt"],
        Some(r#"{"output":"done","exitCode":2}"#),
    );
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "done\n");
}
