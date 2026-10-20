//! Shared helpers for foreman-app's integration tests.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::disallowed_methods,
    reason = "shared test helpers; test code may use std::fs (design doc §3.2)"
)]

pub mod table;

use std::path::{Path, PathBuf};

/// `src/lib/ipc/fixtures/`.
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/ipc/fixtures")
}

/// Every fixture file in `requests/` or `responses/`: (command, variant file name, JSON).
pub fn fixtures(kind: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(fixtures_dir().join(kind)).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".json") else {
            continue;
        };
        let cmd = stem.split('.').next().unwrap().to_owned();
        out.push((cmd, name.clone(), std::fs::read_to_string(&p).unwrap()));
    }
    out.sort();
    out
}

use std::sync::Arc;

use foreman_app::os::ScriptedOs;
use foreman_app::state::{App, Paths};

/// A test app: scripted OS, app data and home in a temp folder.
pub struct TestApp {
    pub app: Arc<App>,
    pub os: Arc<ScriptedOs>,
    pub dir: tempfile::TempDir,
}

impl TestApp {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let os = Arc::new(ScriptedOs::default());
        let app = Arc::new(App::new(
            os.clone(),
            Paths {
                app_data: dir.path().join("app-data"),
                home,
            },
        ));
        Self { app, os, dir }
    }

    pub fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    pub fn app_data(&self) -> PathBuf {
        self.dir.path().join("app-data")
    }
}

/// A mock-runtime Tauri app with every command registered and `state` managed, plus its window.
pub fn tauri_app(
    state: Arc<App>,
) -> (
    tauri::App<tauri::test::MockRuntime>,
    tauri::WebviewWindow<tauri::test::MockRuntime>,
) {
    use tauri::Manager as _;
    let app = foreman_app::builder(tauri::test::mock_builder())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    app.manage::<foreman_app::ipc::Shared>(state);
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    (app, webview)
}

/// Invokes a command over the real Tauri IPC path with `req` (and placeholder Channels).
pub fn invoke(
    webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    cmd: &str,
    req: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    let mut body = serde_json::json!({ "req": req });
    for (i, ch) in table::channels(cmd).iter().enumerate() {
        body[*ch] = serde_json::Value::from(format!("__CHANNEL__:{}", i + 1));
    }
    tauri::test::get_ipc_response(
        webview,
        tauri::webview::InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: tauri::http::HeaderMap::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    )
    .map(|r| r.deserialize::<serde_json::Value>().unwrap())
}

/// The request fixture of a command (the plain one, not a variant).
pub fn request_fixture(cmd: &str) -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(fixtures_dir().join(format!("requests/{cmd}.json"))).unwrap(),
    )
    .unwrap()
}
