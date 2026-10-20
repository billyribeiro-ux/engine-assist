//! Snapshot tests for the Tauri security configuration (M0-09; SR-20..SR-23; AC-M0-08, AC-M0-09).
//! Any change to these settings must change this file in the same diff, reviewed like code.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "integration-test code (tests/): unwrap/expect are denied only outside tests (SR-62)"
)]

use serde_json::{Value, json};

const TAURI_CONF: &str = include_str!("../tauri.conf.json");
const MAIN_WINDOW: &str = include_str!("../capabilities/main-window.json");
const CARGO_TOML: &str = include_str!("../Cargo.toml");
const VITE_CONFIG: &str = include_str!("../../vite.config.ts");

fn conf() -> Value {
    serde_json::from_str(TAURI_CONF).unwrap()
}

fn strict_csp() -> Value {
    json!({
        "default-src": "'self'",
        "script-src": "'self'",
        "style-src": "'self'",
        "img-src": "'self' data:",
        "font-src": "'self'",
        "connect-src": "ipc: http://ipc.localhost",
        "object-src": "'none'",
        "base-uri": "'none'",
        "form-action": "'none'",
        "frame-ancestors": "'none'"
    })
}

mod csp {
    use super::{VITE_CONFIG, Value, conf, strict_csp};

    /// SR-20, AC-M0-08: the production CSP, exactly.
    #[test]
    fn production_config_snapshot() {
        let c = conf();
        let csp = &c["app"]["security"]["csp"];
        assert_eq!(csp, &strict_csp());
        let text = csp.to_string();
        assert!(
            !text.contains("unsafe-inline"),
            "script-src/style-src never 'unsafe-inline' (S-0 not taken)"
        );
        assert!(!text.contains("unsafe-eval"));
        assert_eq!(
            c["app"]["security"]["dangerousDisableAssetCspModification"],
            Value::Bool(false)
        );
        // SvelteKit's kit.csp stays unset so Tauri alone hash-pins the bootstrap (ADR-014).
        assert!(
            !VITE_CONFIG.contains("csp"),
            "kit csp must stay unset in vite.config.ts"
        );
    }

    /// SR-20: devCsp differs from production only by the Vite dev server in connect-src.
    #[test]
    fn dev_csp_allows_only_the_vite_dev_server() {
        let c = conf();
        let mut expected = strict_csp();
        expected["connect-src"] =
            Value::from("ipc: http://ipc.localhost http://localhost:5173 ws://localhost:5173");
        assert_eq!(c["app"]["security"]["devCsp"], expected);
    }
}

mod config {
    use super::{CARGO_TOML, Value, conf, json};

    /// SR-21, AC-M0-08: Tauri security settings, exactly.
    #[test]
    fn security_settings_snapshot() {
        let c = conf();
        let sec = &c["app"]["security"];
        assert_eq!(c["app"]["withGlobalTauri"], Value::Bool(false));
        assert_eq!(
            c["app"]["windows"],
            json!([]),
            "the window is built in Rust (SR-29)"
        );
        assert_eq!(sec["freezePrototype"], Value::Bool(true));
        assert_eq!(sec["assetProtocol"], json!({ "enable": false }));
        assert_eq!(sec["pattern"], json!({ "use": "brownfield" }));
        assert_eq!(sec["capabilities"], json!(["main-window"]));
        let keys: Vec<&str> = sec
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "assetProtocol",
                "capabilities",
                "csp",
                "dangerousDisableAssetCspModification",
                "devCsp",
                "freezePrototype",
                "pattern"
            ],
            "no other security key (e.g. headers, isolation) without updating this snapshot"
        );
        // No remote URL anywhere in the config except the local dev server.
        let text = c.to_string();
        for scheme in ["https://", "http://"] {
            for (i, _) in text.match_indices(scheme) {
                let rest = &text[i..];
                assert!(
                    rest.starts_with("https://schema.tauri.app/")
                        || rest.starts_with("http://localhost:5173")
                        || rest.starts_with("http://ipc.localhost"),
                    "unexpected URL in tauri.conf.json: {}",
                    &rest[..rest.len().min(60)]
                );
            }
        }
    }

    /// SR-21: tauri's `devtools` feature is never enabled (release builds have no inspector).
    #[test]
    fn devtools_feature_off() {
        let tauri_line = CARGO_TOML
            .lines()
            .find(|l| l.starts_with("tauri = "))
            .unwrap();
        assert!(tauri_line.contains("features = []"), "{tauri_line}");
        assert!(
            CARGO_TOML
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .all(|l| !l.contains("devtools")),
            "no devtools feature anywhere in src-tauri/Cargo.toml"
        );
    }

    /// SR-21: `dragDropEnabled: false` is applied by the Rust builder
    /// (`disable_drag_drop_handler`), since `app.windows` is empty.
    #[test]
    fn drag_drop_disabled_in_rust_builder() {
        let src = include_str!("../src/window.rs");
        assert!(src.contains(".disable_drag_drop_handler()"));
        assert!(src.contains(".on_new_window(|_, _| NewWindowResponse::Deny)"));
        assert!(src.contains(".on_navigation(navigation_allowed)"));
    }
}

mod capabilities {
    use super::{MAIN_WINDOW, Value, json};
    use foreman_app::ipc::COMMANDS;

    fn cap() -> Value {
        serde_json::from_str(MAIN_WINDOW).unwrap()
    }

    fn command_grants() -> Vec<String> {
        COMMANDS
            .iter()
            .map(|c| format!("allow-{}", c.replace('_', "-")))
            .collect()
    }

    /// SR-22, AC-M0-09: exactly these permissions; fails on anything added.
    #[test]
    fn main_window_snapshot() {
        let c = cap();
        assert_eq!(c["identifier"], "main-window");
        assert_eq!(c["windows"], json!(["main"]));
        assert_eq!(c["platforms"], json!(["macOS"]));
        assert!(
            c.get("remote").is_none(),
            "no remote URLs in any capability (SR-21)"
        );
        let mut expected: Vec<String> = vec![
            "core:event:allow-listen".into(),
            "core:event:allow-unlisten".into(),
            "core:window:allow-start-dragging".into(),
            "core:window:allow-internal-toggle-maximize".into(),
        ];
        expected.extend(command_grants());
        let got: Vec<String> = c["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                p.as_str()
                    .expect("permissions are plain identifiers, no scopes")
                    .to_owned()
            })
            .collect();
        assert_eq!(got, expected);
        let keys: Vec<&str> = c.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "$schema",
                "description",
                "identifier",
                "permissions",
                "platforms",
                "windows"
            ]
        );
    }

    /// SR-23, IPC-R8: COMMANDS = AppManifest::commands (build.rs includes the same list) =
    /// generate_handler! set = the capability's allow-* grants.
    #[test]
    fn every_command_declared_and_granted() {
        // build.rs includes src/ipc/commands_list.rs verbatim.
        let build_rs = include_str!("../build.rs");
        assert!(build_rs.contains(r#"include!("src/ipc/commands_list.rs")"#));
        assert!(build_rs.contains("AppManifest::new().commands(commands_list::COMMANDS)"));

        // Capability grants: every command, and no app command that isn't in COMMANDS.
        let grants: Vec<String> = cap()["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .filter(|p| !p.contains(':'))
            .map(str::to_owned)
            .collect();
        assert_eq!(grants, command_grants());

        // Handler set: every listed command is registered (a missing handler answers
        // "command not found"), and an unlisted name is not.
        let app = foreman_app::builder(tauri::test::mock_builder())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let invoke = |cmd: &str| {
            tauri::test::get_ipc_response(
                &webview,
                tauri::webview::InvokeRequest {
                    cmd: cmd.into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: "tauri://localhost".parse().unwrap(),
                    body: tauri::ipc::InvokeBody::Json(json!({ "req": {} })),
                    headers: tauri::http::HeaderMap::default(),
                    invoke_key: tauri::test::INVOKE_KEY.to_string(),
                },
            )
        };
        for cmd in COMMANDS {
            if let Err(e) = invoke(cmd) {
                assert!(
                    !e.to_string().contains("not found"),
                    "{cmd} not registered: {e}"
                );
            }
        }
        let missing = invoke("definitely_not_a_command").unwrap_err();
        assert!(missing.to_string().contains("not found"), "{missing}");
    }
}

mod ipc {
    use serde_json::json;

    /// IPC-R1 through the real Tauri invoke path: an unknown field is a typed `Invalid`.
    #[test]
    fn rejects_unknown_fields_over_invoke() {
        let app = foreman_app::builder(tauri::test::mock_builder())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let req = |body| tauri::webview::InvokeRequest {
            cmd: "app_bootstrap".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: tauri::http::HeaderMap::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        };
        let err =
            tauri::test::get_ipc_response(&webview, req(json!({ "req": { "x": 1 } }))).unwrap_err();
        assert_eq!(err["code"], "invalid");
        assert_eq!(
            err["detail"],
            json!({ "kind": "invalid", "field": "req", "rule": "unknownField" })
        );

        let ok = tauri::test::get_ipc_response(&webview, req(json!({ "req": {} }))).unwrap();
        let body: serde_json::Value = ok.deserialize().unwrap();
        assert_eq!(body["firstRun"], true);
        assert_eq!(body["doctor"], "unknown");
    }
}
