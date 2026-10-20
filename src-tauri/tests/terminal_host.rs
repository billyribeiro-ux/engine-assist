//! M3-11 (AC-F008-20), M3-03 (AC-F008-05, SR-24a) and M3-13 (AC-F009-09): what Settings changes
//! in a launch, and the quit guard. Screen reader mode adds `--ax-screen-reader` to a CLI launch
//! (and xterm's screen-reader mode, `src/lib/terminal/terminal.svelte.test.ts`); allowing an
//! auth-changing key, and taking it off the allowlist, each need Billy's native confirmation and
//! store the value's hash only; quitting with live sessions asks first and hangs them up on OK.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::project::{ProjectApp, collect, req};
use foreman_app::ipc::commands::launch;
use foreman_app::ipc::commands::settings as settings_cmd;
use foreman_app::ipc::commands::terminal as term_cmd;
use foreman_app::ipc::dto::launch::{LaunchPreview, PrepareOutcome};
use foreman_app::ipc::error::IpcErrorCode;
use serde_json::{Value, json};

fn patch(fields: Value) -> Value {
    let mut p = json!({
        "driName": null, "editor": null, "theme": null, "terminal": null,
        "envPassthrough": null, "globalVariables": null, "resetTool": null,
        "clearLoginPath": null, "removeAllowlistEntry": null, "screenReaderMode": null,
        "osDisplay": null
    });
    for (k, v) in fields.as_object().unwrap() {
        p[k] = v.clone();
    }
    p
}

fn view(p: &ProjectApp) -> Value {
    serde_json::to_value(settings_cmd::settings_get(&p.t.app, req(json!({}))).unwrap()).unwrap()
}

fn update(p: &ProjectApp, fields: Value) -> Result<Value, foreman_app::ipc::IpcError> {
    let rev = view(p)["rev"].clone();
    settings_cmd::settings_update(
        &p.t.app,
        req(json!({ "expectRev": rev, "patch": patch(fields) })),
    )
    .map(|v| serde_json::to_value(v).unwrap())
}

fn ready(p: &ProjectApp, step: &str) -> LaunchPreview {
    match p.prepare(step) {
        PrepareOutcome::Ready(pv) => pv,
        PrepareOutcome::Blocked { blockers, .. } => panic!("blocked: {blockers:?}"),
    }
}

mod terminal {
    use super::*;

    /// AC-F008-20: Settings → Screen reader mode on adds `--ax-screen-reader` to the argv Billy
    /// sees and the argv spawned; off, it isn't there.
    #[test]
    fn screen_reader_mode_reaches_argv() {
        let p = ProjectApp::new(json!({}));
        let off = ready(&p, "2.2");
        assert!(
            !off.argv.iter().any(|a| a == "--ax-screen-reader"),
            "{:?}",
            off.argv
        );
        assert_eq!(
            update(&p, json!({ "screenReaderMode": true })).unwrap()["screenReaderMode"],
            true
        );
        let on = ready(&p, "2.2");
        let at = on.argv.iter().position(|a| a == "--ax-screen-reader");
        assert!(at.is_some(), "{:?}", on.argv);
        // A flag, before the prompt (the prompt stays last, SR-04).
        assert!(at.unwrap() < on.argv.len() - 1);
        assert_eq!(
            update(&p, json!({ "screenReaderMode": false })).unwrap()["screenReaderMode"],
            false
        );
        let off = ready(&p, "2.2");
        assert!(!off.argv.iter().any(|a| a == "--ax-screen-reader"));
    }
}

mod settings {
    use super::*;

    /// SR-24a, AC-F008-05 (allowlisted hashes): allowing an auth-changing key from
    /// `~/.claude/settings.json`, and removing it from the allowlist, each ask in a native dialog
    /// first; Cancel changes nothing, and only the value's hash is stored.
    #[test]
    fn security_fields_require_native_confirm() {
        let p = ProjectApp::new(json!({}));
        let claude_dir = p.t.home().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{ "env": { "AWS_PROFILE": "allow-me-profile-3f" } }"#,
        )
        .unwrap();
        let PrepareOutcome::Blocked { blockers, .. } = p.prepare("2.2") else {
            panic!("an auth-changing key blocks the launch");
        };
        let hit = blockers
            .iter()
            .map(|b| serde_json::to_value(b).unwrap())
            .find(|b| b["kind"] == "authKeyBlocked")
            .unwrap_or_else(|| panic!("{blockers:?}"));
        assert_eq!(hit["allowlistable"], true, "{hit}");
        let allow = || launch::auth_key_allowlist_add(&p.t.app, req(json!({ "hit": hit["hit"] })));

        // Cancel in the native dialog: nothing stored.
        let asked = p.t.os.asked().len();
        p.t.os.answer_next(false);
        let e = allow().unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert_eq!(p.t.os.asked().len(), asked + 1);
        let dialog = p.t.os.asked().last().unwrap().clone();
        assert_eq!(dialog.ok, "Allow key");
        assert!(dialog.message.contains("AWS_PROFILE"), "{}", dialog.message);
        assert!(
            !dialog.message.contains("allow-me-profile-3f"),
            "the value is never shown"
        );
        assert_eq!(view(&p)["authKeyAllowlist"], json!([]));

        // OK: the hash is stored, never the value, and the launch no longer blocks on the key.
        p.t.os.answer_next(true);
        let s = serde_json::to_value(allow().unwrap()).unwrap();
        let list = s["authKeyAllowlist"].as_array().unwrap();
        assert_eq!(list.len(), 1, "{s}");
        let hash = list[0]["sha256"].as_str().unwrap().to_owned();
        assert_eq!(hash.len(), 64);
        let stored = std::fs::read_to_string(p.t.app_data().join("settings.json")).unwrap();
        assert!(stored.contains(&hash) && !stored.contains("allow-me-profile-3f"));
        match p.prepare("2.2") {
            PrepareOutcome::Ready(_) => {}
            PrepareOutcome::Blocked { blockers, .. } => assert!(
                !blockers
                    .iter()
                    .any(|b| serde_json::to_value(b).unwrap()["kind"] == "authKeyBlocked"),
                "{blockers:?}"
            ),
        }

        // Removing it from the allowlist asks too; Cancel keeps it.
        p.t.os.answer_next(false);
        let e = update(&p, json!({ "removeAllowlistEntry": hash })).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert_eq!(p.t.os.asked().last().unwrap().ok, "Change");
        assert_eq!(view(&p)["authKeyAllowlist"].as_array().unwrap().len(), 1);
        p.t.os.answer_next(true);
        let s = update(&p, json!({ "removeAllowlistEntry": hash })).unwrap();
        assert_eq!(s["authKeyAllowlist"], json!([]));
        // And the key blocks again.
        let PrepareOutcome::Blocked { blockers, .. } = p.prepare("2.2") else {
            panic!("the key blocks again once it is off the allowlist");
        };
        assert!(
            blockers
                .iter()
                .any(|b| serde_json::to_value(b).unwrap()["kind"] == "authKeyBlocked")
        );
    }
}

fn wait_for(buf: &Arc<Mutex<Vec<u8>>>, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if String::from_utf8_lossy(&buf.lock().unwrap()).contains(needle) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for {needle:?}");
}

mod ipc {
    use super::*;

    /// SR-24a, AC-F009-09 (the quit guard): with live sessions, quitting asks in a native dialog;
    /// a scripted Cancel sends no signal (the session keeps answering), OK hangs every session up.
    /// With no live session, nothing is asked.
    #[test]
    fn security_confirmations_are_native() {
        let p = ProjectApp::new(json!({}));
        let app = &p.t.app;
        let asked = p.t.os.asked().len();
        assert!(foreman_app::quit::guard(app), "nothing live: quit at once");
        assert_eq!(p.t.os.asked().len(), asked, "and without a dialog");

        let pv = ready(&p, "2.2");
        let (ch, buf) = collect();
        let run = launch::launch_start(app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
        wait_for(&buf, "FAKE-CLAUDE READY");

        p.t.os.answer_next(false);
        assert!(!foreman_app::quit::guard(app));
        let d = p.t.os.asked().last().unwrap().clone();
        assert_eq!(d.title, "Quit Foreman?");
        assert_eq!(d.ok, "Quit");
        assert_eq!(
            d.message,
            "1 session is running; it ends when Foreman quits; you can resume it later."
        );
        term_cmd::pty_write(app, req(json!({ "runId": run.run_id, "data": "alive\r" }))).unwrap();
        wait_for(&buf, "ECHO: alive");
        assert_eq!(app.runs().live().len(), 1, "Cancel sent nothing");

        p.t.os.answer_next(true);
        assert!(foreman_app::quit::guard(app));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !app.runs().live().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            app.runs().live().is_empty(),
            "OK hung the session up (SIGHUP)"
        );
    }
}

mod quit {
    /// AC-F009-09: the guard's words, singular and plural.
    #[test]
    fn guard_wording() {
        assert_eq!(
            foreman_app::quit::message(3),
            "3 sessions are running; they end when Foreman quits; you can resume each later."
        );
        assert_eq!(
            foreman_app::quit::message(1),
            "1 session is running; it ends when Foreman quits; you can resume it later."
        );
    }
}

mod output_tail {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    /// AC-F009-07, SR-53 (M3-09): a run's output tail is in `runs/<runId>/output.log` (`0600`) after
    /// it exits; the tail is the PTY ring's last 2,000 lines (`foreman_sys::pty::RING_LINES`).
    #[test]
    fn output_log_has_the_last_lines() {
        assert_eq!(foreman_sys::pty::RING_LINES, 2_000);
        let p = ProjectApp::new(json!({}));
        let app = &p.t.app;
        let pv = ready(&p, "2.2");
        let (ch, buf) = collect();
        let run = launch::launch_start(app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
        wait_for(&buf, "FAKE-CLAUDE READY");
        term_cmd::pty_write(
            app,
            req(json!({ "runId": run.run_id, "data": "tail-marker-61\r" })),
        )
        .unwrap();
        wait_for(&buf, "ECHO: tail-marker-61");
        term_cmd::pty_terminate(app, req(json!({ "runId": run.run_id }))).unwrap();
        let run_id = serde_json::to_value(&run.run_id)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let log = p.t.app_data().join("runs").join(&run_id).join("output.log");
        let deadline = Instant::now() + Duration::from_secs(15);
        while !log.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        let text = String::from_utf8_lossy(&std::fs::read(&log).unwrap()).into_owned();
        assert!(text.contains("ECHO: tail-marker-61"), "{text}");
        assert!(text.lines().count() <= 2_000);
        assert_eq!(
            std::fs::metadata(&log).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
