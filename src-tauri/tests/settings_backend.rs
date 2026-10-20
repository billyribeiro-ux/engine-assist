//! M2-12: the settings backend and tool paths (AC-F029-01…03, AC-F008-03, SR-24a, SR-01; T-004,
//! T-014). Security-relevant changes are applied only after a native confirmation; the webview
//! never supplies a tool path; a relative, missing, non-executable or in-project path is refused
//! with the reason; settings persist atomically and a stale write is refused.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};

use common::TestApp;
use common::project::{ProjectApp, fake_claude, req};
use foreman_app::ipc::IpcErrorCode;
use foreman_app::ipc::commands::settings as cmd;
use foreman_app::ipc::dto::launch::PrepareOutcome;
use foreman_app::ipc::dto::requests::SettingsUpdateReq;
use foreman_app::state::{App, Paths};
use foreman_core::schema::settings::{AuthKeyAllow, ToolSource};
use serde_json::{Value, json};

const NO_CHANGE: [&str; 11] = [
    "driName",
    "editor",
    "theme",
    "terminal",
    "envPassthrough",
    "globalVariables",
    "resetTool",
    "clearLoginPath",
    "removeAllowlistEntry",
    "screenReaderMode",
    "osDisplay",
];

fn patch(changes: Value) -> Value {
    let mut p = serde_json::Map::new();
    for k in NO_CHANGE {
        p.insert(k.to_owned(), Value::Null);
    }
    for (k, v) in changes.as_object().unwrap() {
        p.insert(k.clone(), v.clone());
    }
    Value::Object(p)
}

fn view(t: &TestApp) -> Value {
    serde_json::to_value(cmd::settings_get(&t.app, req(json!({}))).unwrap()).unwrap()
}

fn update(t: &TestApp, changes: Value) -> Result<Value, foreman_app::ipc::IpcError> {
    let rev = view(t)["rev"].clone();
    cmd::settings_update(
        &t.app,
        req(json!({ "expectRev": rev, "patch": patch(changes) })),
    )
    .map(|v| serde_json::to_value(v).unwrap())
}

fn executable(dir: &Path, name: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let p = dir.join(name);
    std::fs::copy(fake_claude(), &p).unwrap();
    p
}

mod settings {
    use super::*;

    /// SR-24a, AC-F029-05: the pass-through list, resetting a tool path and removing an allowlisted
    /// key change only after a native confirmation that lists the change; Cancel saves nothing;
    /// ordinary settings need no dialog. A tool path picked in Rust's file dialog is confirmed
    /// with its resolved path and version, and the webview has no field to put a path in.
    #[test]
    fn security_fields_require_native_confirm() {
        let t = TestApp::new();
        // Seed an allowlisted key (what `auth_key_allowlist_add` stores).
        let (mut s, stamp, _) = t.app.settings().unwrap();
        s.auth_key_allowlist.push(AuthKeyAllow {
            key: "env.FOO".into(),
            sha256: "ab".repeat(32),
            added_at: "2026-10-20T12:00:00Z".into(),
        });
        t.app.save_settings(&s, &stamp).unwrap();

        // Ordinary settings: no dialog.
        update(&t, json!({ "driName": "Billy", "theme": "night" })).unwrap();
        update(
            &t,
            json!({ "editor": { "kind": "set", "bundleId": "com.microsoft.VSCode" } }),
        )
        .unwrap();
        assert!(t.os.asked().is_empty(), "{:?}", t.os.asked());

        // Each security field: Cancel changes nothing; OK applies it; the dialog names it.
        let before = view(&t);
        t.os.answer_next(false);
        let e = update(&t, json!({ "envPassthrough": ["MY_TOOL_TOKEN"] })).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert_eq!(view(&t), before);
        assert!(t.os.asked()[0].message.contains("MY_TOOL_TOKEN"));
        t.os.answer_next(true);
        let v = update(&t, json!({ "envPassthrough": ["MY_TOOL_TOKEN"] })).unwrap();
        assert_eq!(v["envPassthrough"], json!(["MY_TOOL_TOKEN"]));

        let hash = "ab".repeat(32);
        let before = view(&t);
        t.os.answer_next(false);
        let e = update(&t, json!({ "removeAllowlistEntry": hash })).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert_eq!(view(&t), before);
        assert_eq!(before["authKeyAllowlist"].as_array().unwrap().len(), 1);
        t.os.answer_next(true);
        let v = update(&t, json!({ "removeAllowlistEntry": hash })).unwrap();
        assert_eq!(v["authKeyAllowlist"], json!([]));

        // Resetting a tool path to auto-detection.
        let bin = executable(&t.dir.path().join("tools"), "claude");
        t.os.pick_next(Some(bin.clone()));
        cmd::settings_pick_tool(&t.app, req(json!({ "tool": "claude" }))).unwrap();
        let (s, _, _) = t.app.settings().unwrap();
        assert_eq!(s.tools.claude.source, ToolSource::Settings);
        let before = view(&t);
        t.os.answer_next(false);
        let e = update(&t, json!({ "resetTool": "claude" })).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert_eq!(view(&t), before);
        t.os.answer_next(true);
        update(&t, json!({ "resetTool": "claude" })).unwrap();
        assert_eq!(
            t.app.settings().unwrap().0.tools.claude.source,
            ToolSource::Auto
        );

        // A picked tool: Cancel at the file dialog or at the confirmation saves nothing; the
        // confirmation shows the resolved path and the version.
        let before = view(&t);
        t.os.pick_next(None);
        let e = cmd::settings_pick_tool(&t.app, req(json!({ "tool": "claude" }))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        t.os.pick_next(Some(bin.clone()));
        t.os.answer_next(false);
        let e = cmd::settings_pick_tool(&t.app, req(json!({ "tool": "claude" }))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        let asked = t.os.asked();
        let last = asked.last().unwrap();
        assert!(
            last.message.contains(&bin.display().to_string()),
            "{last:?}"
        );
        assert_eq!(view(&t)["tools"], before["tools"]);

        // The webview can't carry a path, program or env into settings: the patch has no such
        // field and unknown fields are refused (IPC-R1, AC-XC-06).
        for bad in [
            json!({ "claudePath": "/tmp/evil" }),
            json!({ "tools": { "claude": { "path": "/tmp/evil" } } }),
            json!({ "env": { "PATH": "/tmp" } }),
        ] {
            let mut p = patch(json!({}));
            for (k, v) in bad.as_object().unwrap() {
                p[k] = v.clone();
            }
            let r: Result<SettingsUpdateReq, _> =
                serde_json::from_value(json!({ "expectRev": 1, "patch": p }));
            assert!(r.is_err(), "{bad}");
        }
        assert!(
            t.os.asked()
                .iter()
                .all(|c| !c.message.contains("/tmp/evil"))
        );
    }

    /// AC-F029-01: a relative, missing, non-executable or in-project path is refused with the
    /// reason and nothing is saved; a good one is used after the confirmation.
    #[test]
    fn tool_path_refusals_name_the_reason() {
        let p = ProjectApp::new(json!({}));
        let t = &p.t;
        let outside = t.dir.path().join("elsewhere");
        let plain = outside.join("claude");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(&plain, "not executable").unwrap();
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
        let in_project = executable(&p.project.join("bin"), "claude");
        // A symlink outside every project whose target is inside one.
        std::fs::create_dir_all(outside.join("link")).unwrap();
        symlink(&in_project, outside.join("link/claude")).unwrap();
        let wrong_name = executable(&outside, "not-claude");
        let before = view(t)["tools"].clone();
        for (path, why) in [
            (PathBuf::from("bin/claude"), "isn't absolute"),
            (outside.join("missing/claude"), ""),
            (plain.clone(), ""),
            (in_project.clone(), ""),
            (outside.join("link/claude"), "project folder"),
            (wrong_name, ""),
        ] {
            t.os.pick_next(Some(path.clone()));
            let e = cmd::settings_pick_tool(&t.app, req(json!({ "tool": "claude" }))).unwrap_err();
            assert_eq!(e.code, IpcErrorCode::ToolMissing, "{}", path.display());
            assert!(!e.message.is_empty());
            if !why.is_empty() {
                assert!(e.message.contains(why), "{}", e.message);
            }
            assert_eq!(view(t)["tools"], before, "{}", path.display());
        }
        // A good absolute executable outside every project is accepted once confirmed.
        let good = executable(&outside, "claude");
        std::fs::write(
            outside.join("fake.json"),
            json!({ "record": t.dir.path().join("rec2.json") }).to_string(),
        )
        .unwrap();
        t.os.pick_next(Some(good.clone()));
        let tools = cmd::settings_pick_tool(&t.app, req(json!({ "tool": "claude" }))).unwrap();
        let tools = serde_json::to_value(tools).unwrap();
        assert_eq!(tools["claude"]["path"], good.display().to_string());
        assert_eq!(tools["claude"]["autoDetected"], false);
        assert_eq!(tools["claude"]["problem"], Value::Null);
    }

    /// AC-F008-03: a `claude` path that is relative, missing, not executable or inside a
    /// registered project blocks Run; the blocker names the path and why.
    #[test]
    fn run_blocked_names_the_claude_path() {
        let p = ProjectApp::new(json!({}));
        let (good_path, _) = {
            let (s, stamp, _) = p.t.app.settings().unwrap();
            (s.tools.claude.path.clone().unwrap(), stamp)
        };
        let outside = p.t.dir.path().join("elsewhere");
        std::fs::create_dir_all(&outside).unwrap();
        let plain = outside.join("claude");
        std::fs::write(&plain, "x").unwrap();
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
        let in_project = executable(&p.project.join("tools"), "claude");
        let cases = [
            ("bin/claude".to_owned(), "relative"),
            // A missing file is "missing or not executable" to the spawn checks (one error).
            (
                outside.join("nope/claude").display().to_string(),
                "notExecutable",
            ),
            (plain.display().to_string(), "notExecutable"),
            (in_project.display().to_string(), "inProject"),
        ];
        for (path, why) in cases {
            let (mut s, stamp, _) = p.t.app.settings().unwrap();
            s.tools.claude.path = Some(path.clone());
            s.tools.claude.source = ToolSource::Settings;
            p.t.app.save_settings(&s, &stamp).unwrap();
            let PrepareOutcome::Blocked { blockers, .. } = p.prepare("2.2") else {
                panic!("{path}: not blocked");
            };
            let blockers = serde_json::to_value(&blockers).unwrap();
            let b = blockers
                .as_array()
                .unwrap()
                .iter()
                .find(|b| b["kind"] == "claudeMissing")
                .unwrap_or_else(|| panic!("{path}: no claudeMissing in {blockers}"));
            assert_eq!(b["why"], why, "{path}");
            assert_eq!(b["path"], path, "{path}");
        }
        // Back to the good path: Run is no longer blocked.
        let (mut s, stamp, _) = p.t.app.settings().unwrap();
        s.tools.claude.path = Some(good_path);
        p.t.app.save_settings(&s, &stamp).unwrap();
        assert!(matches!(p.prepare("2.2"), PrepareOutcome::Ready(_)));
    }

    /// AC-F029-02: every setting persists across a restart, through an atomic write (0600).
    #[test]
    fn persists_across_restart() {
        let t = TestApp::new();
        update(
            &t,
            json!({
                "driName": "Billy R",
                "theme": "night",
                "terminal": { "fontSize": 15, "scrollback": 20000, "macOptionIsMeta": false },
                "globalVariables": { "TEAM": "solo" },
                "editor": { "kind": "set", "bundleId": "com.apple.TextEdit" }
            }),
        )
        .unwrap();
        let restarted = App::new(
            t.os.clone(),
            Paths {
                app_data: t.app_data(),
                home: t.home(),
            },
        );
        let v =
            serde_json::to_value(cmd::settings_get(&restarted, req(json!({}))).unwrap()).unwrap();
        assert_eq!(v["driName"], "Billy R");
        assert_eq!(v["theme"], "night");
        assert_eq!(v["terminal"]["fontSize"], 15);
        assert_eq!(v["terminal"]["macOptionIsMeta"], false);
        assert_eq!(v["globalVariables"]["TEAM"], "solo");
        assert_eq!(v["editorBundleId"], "com.apple.TextEdit");
        let f = t.app_data().join("settings.json");
        assert_eq!(
            std::fs::metadata(&f).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // No temp file is left beside it.
        let names: Vec<String> = std::fs::read_dir(t.app_data())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|n| !n.contains(".tmp")), "{names:?}");
        // Out-of-range values are refused and don't change the file.
        let e = update(
            &t,
            json!({ "terminal": { "fontSize": 99, "scrollback": 20000, "macOptionIsMeta": true } }),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Invalid);
    }

    /// AC-F029-03: no `settings.json` means defaults and auto-detection; a file that changed on
    /// disk since it was read refuses Foreman's write, and the next read shows the edit.
    #[test]
    fn missing_file_defaults_and_stale_write_refused() {
        let t = TestApp::new();
        assert!(!t.app_data().join("settings.json").exists());
        let v = view(&t);
        assert_eq!(v["theme"], "system");
        assert_eq!(v["tools"]["claude"]["autoDetected"], true);
        assert_eq!(v["envPassthrough"], json!([]));
        assert_eq!(v["editorBundleId"], Value::Null);

        let (s, stamp, _) = t.app.settings().unwrap();
        t.app.save_settings(&s, &stamp).unwrap();
        // Read, then Billy edits the file by hand, then Foreman writes with its old stamp.
        let (mut mine, stale, _) = t.app.settings().unwrap();
        let file = t.app_data().join("settings.json");
        let mut on_disk: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        on_disk["driName"] = json!("Edited by hand");
        std::fs::write(&file, format!("{on_disk:#}\n")).unwrap();
        mine.dri_name = Some("Foreman's value".into());
        let e = t.app.save_settings(&mine, &stale).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Conflict);
        // Nothing of Foreman's value reached the file; the reload shows the edit.
        assert!(
            std::fs::read_to_string(&file)
                .unwrap()
                .contains("Edited by hand")
        );
        assert_eq!(view(&t)["driName"], "Edited by hand");
    }
}
