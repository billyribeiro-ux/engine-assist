//! IPC framework tests (`docs/contracts/ipc.md` §1, §7.1; ticket M2-01): every command over the
//! real Tauri invoke path, request validation, caught panics and request fuzzing.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "integration-test code (tests/): unwrap/expect are denied only outside tests (SR-62)"
)]

mod common;

mod ipc {
    use super::common::{TestApp, invoke, request_fixture, tauri_app};
    use foreman_app::ipc::{COMMANDS, IpcError, IpcErrorCode, run_caught};
    use serde_json::{Value, json};

    /// IPC-R1: for every command, an extra field is a typed `Invalid` over the real invoke path,
    /// and the fixture request itself passes validation (whatever the handler then answers).
    #[test]
    fn rejects_unknown_fields() {
        let t = TestApp::new();
        let (_app, webview) = tauri_app(t.app.clone());
        for cmd in COMMANDS {
            let mut req = request_fixture(cmd);
            req["unexpectedField"] = json!(1);
            let err = invoke(&webview, cmd, req).expect_err(cmd);
            assert_eq!(err["code"], "invalid", "{cmd}: {err}");
            assert_eq!(
                err["detail"],
                json!({ "kind": "invalid", "field": "req", "rule": "unknownField" }),
                "{cmd}"
            );
            if let Err(e) = invoke(&webview, cmd, request_fixture(cmd)) {
                assert_ne!(e["code"], "invalid", "{cmd}: the fixture is valid: {e}");
                let typed: IpcError = serde_json::from_value(e.clone())
                    .unwrap_or_else(|_| panic!("{cmd}: untyped error {e}"));
                assert_ne!(typed.code, IpcErrorCode::Internal, "{cmd}: {e}");
            }
        }
    }

    /// IPC-R1: a nested tagged object rejects unknown fields too.
    #[test]
    fn rejects_unknown_nested_fields() {
        let t = TestApp::new();
        let (_app, webview) = tauri_app(t.app.clone());
        let mut req = request_fixture("settings_update");
        req["patch"]["editor"] = json!({ "kind": "set", "bundleId": "com.apple.TextEdit", "x": 1 });
        let err = invoke(&webview, "settings_update", req).unwrap_err();
        assert_eq!(err["code"], "invalid");
        let mut req = request_fixture("chat_link_set");
        req["source"] = json!({ "kind": "clipboard", "url": "https://claude.ai/x" });
        let err = invoke(&webview, "chat_link_set", req).unwrap_err();
        assert_eq!(err["code"], "invalid");
    }

    /// IPC-R2: no request takes an absolute path; a repo path is relative and checked by policy.
    #[test]
    fn no_absolute_paths_accepted() {
        let t = TestApp::new();
        let (_app, webview) = tauri_app(t.app.clone());
        for cmd in [
            "file_open",
            "file_reveal",
            "manifest_mark",
            "assembler_paste_preview",
        ] {
            let mut req = request_fixture(cmd);
            req["path"] = json!("/etc/passwd");
            let err = invoke(&webview, cmd, req).unwrap_err();
            assert!(
                ["invalid", "policyViolation", "notFound", "unsupported"]
                    .contains(&err["code"].as_str().unwrap()),
                "{cmd}: {err}"
            );
            assert_ne!(err["code"], Value::Null);
        }
    }

    /// IPC-R3: a handler panic is a typed `Internal`, never a crash or an untyped string.
    #[test]
    fn panic_returns_typed_error() {
        let t = TestApp::new();
        let r: Result<(), IpcError> = run_caught(&t.app, (), |_, ()| panic!("boom"));
        let e = r.unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Internal);
        assert!(
            !e.message.contains("boom"),
            "SR-57: the panic text never reaches the webview"
        );
        assert!(foreman_app::logging::caught() >= 1);
    }

    /// IPC-R1: no request struct uses `flatten` or `untagged` (source scan).
    #[test]
    fn no_flatten_or_untagged() {
        for src in [
            include_str!("../src/ipc/dto/requests.rs"),
            include_str!("../src/ipc/types.rs"),
        ] {
            for line in src
                .lines()
                .filter(|l| l.trim_start().starts_with("#[serde("))
            {
                assert!(
                    !line.contains("flatten") && !line.contains("untagged"),
                    "{line}"
                );
            }
        }
    }
}
