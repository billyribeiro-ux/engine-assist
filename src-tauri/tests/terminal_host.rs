//! M3-11 (AC-F008-20) and M3-03 (AC-F008-05, SR-24a): what Settings changes in a launch.
//! Screen reader mode adds `--ax-screen-reader` to a CLI launch (and xterm's screen-reader mode,
//! `src/lib/terminal/terminal.svelte.test.ts`); allowing an auth-changing key, and taking it off
//! the allowlist, each need Billy's native confirmation and store the value's hash only.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::project::{ProjectApp, req};
use foreman_app::ipc::commands::launch;
use foreman_app::ipc::commands::settings as settings_cmd;
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
