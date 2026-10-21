//! M4-34: Settings (P2) — the pass-through list behind a native confirmation and the
//! always-removed names still removed when the list names them (AC-F029-04, -05; SR-05, SR-24a;
//! T-004, T-018).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::TestApp;
use common::project::req;
use foreman_app::ipc::commands::settings as settings_cmd;
use foreman_app::ipc::error::IpcErrorCode;
use foreman_core::launch::env::PathSources;
use serde_json::{Value, json};

fn patch(rev: u32, fields: &Value) -> Value {
    let mut p = json!({
        "driName": null, "editor": null, "theme": null, "terminal": null, "envPassthrough": null,
        "globalVariables": null, "resetTool": null, "clearLoginPath": null,
        "removeAllowlistEntry": null, "screenReaderMode": null, "osDisplay": null
    });
    for (k, v) in fields.as_object().unwrap() {
        p[k] = v.clone();
    }
    json!({ "expectRev": rev, "patch": p })
}

fn stored(t: &TestApp) -> Value {
    serde_json::from_str(&std::fs::read_to_string(t.app_data().join("settings.json")).unwrap())
        .unwrap()
}

fn rev(t: &TestApp) -> u32 {
    settings_cmd::settings_get(&t.app, req(json!({})))
        .unwrap()
        .rev
        .0
}

mod settings_p2 {
    use super::*;

    /// AC-F029-04: the P2 settings persist and come back in the view (theme, terminal font size
    /// and scrollback, global variable defaults); out-of-range values are refused.
    #[test]
    fn p2_settings_persist() {
        let t = TestApp::new();
        let v = settings_cmd::settings_update(
            &t.app,
            req(patch(
                rev(&t),
                &json!({
                    "theme": "night",
                    "terminal": { "fontSize": 14, "scrollback": 25000, "macOptionIsMeta": true },
                    "globalVariables": { "PREFERRED_STACK": "SvelteKit + Rust" }
                }),
            )),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(v.theme).unwrap(), "night");
        assert_eq!(v.terminal.font_size, 14);
        assert_eq!(v.terminal.scrollback, 25_000);
        assert_eq!(v.global_variables["PREFERRED_STACK"], "SvelteKit + Rust");
        let s = stored(&t);
        assert_eq!(s["theme"], "night");
        assert_eq!(s["terminal"]["fontSize"], 14);
        assert_eq!(s["terminal"]["scrollback"], 25_000);
        assert_eq!(s["globalVariables"]["PREFERRED_STACK"], "SvelteKit + Rust");
        assert!(t.os.asked().is_empty(), "no security field: no dialog");
        for bad in [
            json!({ "terminal": { "fontSize": 8, "scrollback": 10000, "macOptionIsMeta": false } }),
            json!({ "terminal": { "fontSize": 12, "scrollback": 999, "macOptionIsMeta": false } }),
            json!({ "globalVariables": { "lower": "x" } }),
        ] {
            let e = settings_cmd::settings_update(&t.app, req(patch(rev(&t), &bad))).unwrap_err();
            assert_eq!(e.code, IpcErrorCode::Invalid, "{bad}");
        }
        // A stale revision is refused with the fresh view.
        let e = settings_cmd::settings_update(&t.app, req(patch(0, &json!({ "theme": "vellum" }))))
            .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Stale);
        assert_eq!(stored(&t)["theme"], "night");
    }
}

mod settings {
    use super::*;

    /// SR-24a, T-004/T-018: the pass-through list (and a tool reset, and removing an
    /// allowlisted key) changes only after a native confirmation that lists the change; Cancel
    /// saves nothing in the whole patch. The request carries no approval.
    #[test]
    fn security_fields_require_native_confirm() {
        let t = TestApp::new();
        let names =
            json!({ "envPassthrough": ["MY_TOOL_HOME", "ANTHROPIC_API_KEY"], "theme": "vellum" });
        // An approval field is refused by the request type (IPC-R1, IPC-R11).
        assert!(
            serde_json::from_value::<foreman_app::ipc::dto::requests::SettingsUpdateReq>({
                let mut r = patch(1, &names);
                r["approved"] = json!(true);
                r
            })
            .is_err()
        );
        t.os.answer_next(false);
        let e = settings_cmd::settings_update(&t.app, req(patch(rev(&t), &names))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        let asked = t.os.asked();
        assert_eq!(asked.len(), 1, "one native dialog");
        assert!(
            asked[0]
                .message
                .contains("Pass these environment variables to Claude Code: MY_TOOL_HOME."),
            "{:?}",
            asked[0]
        );
        assert!(
            asked[0]
                .message
                .contains("Foreman always removes ANTHROPIC_API_KEY; listing it changes nothing."),
            "{:?}",
            asked[0]
        );
        let after_cancel = std::fs::read_to_string(t.app_data().join("settings.json"))
            .map(|b| serde_json::from_str::<Value>(&b).unwrap())
            .ok();
        if let Some(s) = &after_cancel {
            assert_eq!(s["envPassthrough"], json!([]), "nothing saved");
            assert_ne!(
                s["theme"], "vellum",
                "the rest of the patch isn't saved either"
            );
        }
        t.os.answer_next(true);
        let v = settings_cmd::settings_update(&t.app, req(patch(rev(&t), &names))).unwrap();
        let listed: Vec<&str> = v.env_passthrough.iter().map(|n| n.as_str()).collect();
        assert_eq!(listed, ["ANTHROPIC_API_KEY", "MY_TOOL_HOME"]);
        assert_eq!(t.os.asked().len(), 2);
        // Clearing the list asks too.
        t.os.answer_next(true);
        settings_cmd::settings_update(
            &t.app,
            req(patch(rev(&t), &json!({ "envPassthrough": [] }))),
        )
        .unwrap();
        assert_eq!(t.os.asked().len(), 3);
        assert!(
            t.os.asked()[2]
                .message
                .contains("Pass no extra environment variables")
        );
        // A tool reset asks.
        t.os.answer_next(false);
        let e = settings_cmd::settings_update(
            &t.app,
            req(patch(rev(&t), &json!({ "resetTool": "claude" }))),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert!(t.os.asked()[3].message.contains("Forget the claude path"));
    }
}

mod env {
    use super::*;

    /// AC-F029-05 through the setting: the pass-through list stored in Settings names
    /// always-removed variables; the child environment built from it still removes them and
    /// passes the ordinary name.
    #[test]
    fn denylist_beats_passthrough() {
        let t = TestApp::new();
        t.os.answer_next(true);
        settings_cmd::settings_update(
            &t.app,
            req(patch(
                rev(&t),
                &json!({ "envPassthrough": ["ANTHROPIC_API_KEY", "AWS_PROFILE", "DYLD_INSERT_LIBRARIES", "NODE_OPTIONS", "MY_TOOL_HOME"] }),
            )),
        )
        .unwrap();
        let (s, _, _) = t.app.settings().unwrap();
        assert_eq!(s.env_passthrough.len(), 5, "stored as given");
        let own: Vec<(String, String)> = [
            ("HOME", "/Users/billy"),
            ("ANTHROPIC_API_KEY", "sk-ant-secret"),
            ("AWS_PROFILE", "prod"),
            ("DYLD_INSERT_LIBRARIES", "/tmp/evil.dylib"),
            ("NODE_OPTIONS", "--require /tmp/evil.js"),
            ("MY_TOOL_HOME", "/opt/mytool"),
        ]
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
        let sources = PathSources {
            etc_paths: vec!["/usr/bin".into()],
            etc_paths_d: Vec::new(),
            home: "/Users/billy".into(),
            pnpm_home: None,
            login: Vec::new(),
        };
        let env = foreman_sys::launch::child_env(
            &own,
            &s.env_passthrough,
            &sources,
            &[],
            uuid::Uuid::new_v4(),
        );
        for n in [
            "ANTHROPIC_API_KEY",
            "AWS_PROFILE",
            "DYLD_INSERT_LIBRARIES",
            "NODE_OPTIONS",
        ] {
            assert!(env.get(n).is_none(), "{n} must stay removed");
            assert!(foreman_core::launch::env::is_removed(n));
        }
        assert_eq!(env.get("MY_TOOL_HOME"), Some("/opt/mytool"));
    }
}
