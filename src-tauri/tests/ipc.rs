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

/// ipc.md §6 (code review W7): every event's payload, as the webview receives it. Events carry
/// identifiers, numbers and enums only.
mod events {
    use foreman_app::ipc::dto::events::{
        ConfigChanged, DecisionPending, DoctorStatus, PacketChanged, ProjectChanged,
        ProjectsChanged, RunStatusChanged, SettingsChanged, TamperDetected,
    };
    use foreman_app::ipc::types::{CwdKind, DoctorLevel, ProjectId, Rev, RunId, RunStatus, Stage};

    #[test]
    fn payload_snapshot() {
        let p = || ProjectId::from_trusted("7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11");
        let j = |v: serde_json::Value| v.to_string();
        let got = [
            j(serde_json::to_value(ProjectChanged {
                project_id: p(),
                rev: Rev(4),
            })
            .unwrap()),
            j(serde_json::to_value(ProjectsChanged { rev: Rev(2) }).unwrap()),
            j(serde_json::to_value(PacketChanged {
                project_id: p(),
                stage: Stage(3),
                rev: Rev(9),
            })
            .unwrap()),
            j(serde_json::to_value(DecisionPending {
                project_id: p(),
                count: 1,
            })
            .unwrap()),
            j(serde_json::to_value(TamperDetected {
                project_id: p(),
                stage: Stage(1),
                rev: Rev(4),
            })
            .unwrap()),
            j(serde_json::to_value(RunStatusChanged {
                run_id: RunId::from_trusted("a0000000-0000-4000-8000-000000000023"),
                status: RunStatus::Idle,
                rev: Rev(7),
            })
            .unwrap()),
            j(serde_json::to_value(ConfigChanged {
                project_id: p(),
                cwd: CwdKind::Spec,
            })
            .unwrap()),
            j(serde_json::to_value(DoctorStatus {
                level: DoctorLevel::Warning,
            })
            .unwrap()),
            j(serde_json::to_value(SettingsChanged { rev: Rev(5) }).unwrap()),
        ]
        .join("\n");
        let expected = r#"{"projectId":"7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11","rev":4}
{"rev":2}
{"projectId":"7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11","rev":9,"stage":3}
{"count":1,"projectId":"7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11"}
{"projectId":"7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11","rev":4,"stage":1}
{"rev":7,"runId":"a0000000-0000-4000-8000-000000000023","status":"idle"}
{"cwd":"spec","projectId":"7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11"}
{"level":"warning"}
{"rev":5}"#;
        assert_eq!(got, expected);
    }
}

/// ipc.md §5 (code review W7): PTY output reaches the webview as raw bytes, never JSON.
mod pty {
    use std::sync::{Arc, Mutex};

    use foreman_app::runs::Outputs;
    use tauri::ipc::{Channel, InvokeResponseBody};

    /// (sent raw, the bytes) per message.
    type Sent = Arc<Mutex<Vec<(bool, Vec<u8>)>>>;

    #[test]
    fn channel_sends_raw_bytes() {
        let got: Sent = Arc::new(Mutex::new(Vec::new()));
        let g = Arc::clone(&got);
        let ch = Channel::new(move |body| {
            let entry = match body {
                InvokeResponseBody::Raw(b) => (true, b),
                InvokeResponseBody::Json(s) => (false, s.into_bytes()),
            };
            g.lock().unwrap().push(entry);
            Ok(())
        });
        let out = Outputs::default();
        out.attach(ch);
        let bytes = b"\x1b[1mbold\x1b[0m \xff\xfe not UTF-8\r\n".to_vec();
        out.send(&bytes);
        let got = got.lock().unwrap();
        assert_eq!(got.len(), 1);
        assert!(got[0].0, "sent as JSON");
        assert_eq!(got[0].1, bytes, "the bytes changed on the way");
    }
}
