//! A real project for command tests: a git blueprint repo with the v2.2 fixture, fake-claude as
//! the configured `claude`, and a project created through the setup commands.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::disallowed_methods,
    reason = "shared test helpers; tests may spawn git and use std::fs (design doc §3.2)"
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use foreman_app::ipc::commands::{first_run, projects};
use foreman_app::ipc::dto::launch::PrepareOutcome;
use foreman_app::ipc::types::ProjectId;
use foreman_core::schema::settings::ToolSource;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::TestApp;

/// Deserializes a request from JSON (requests have no public constructors).
pub fn req<T: DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

/// A Channel that drops what it's sent.
pub fn channel<T: tauri::ipc::IpcResponse>() -> tauri::ipc::Channel<T> {
    tauri::ipc::Channel::new(|_| Ok(()))
}

/// A raw-bytes Channel that collects what it's sent.
pub fn collect() -> (
    tauri::ipc::Channel<tauri::ipc::Response>,
    std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
) {
    let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let b = buf.clone();
    let ch = tauri::ipc::Channel::new(move |body| {
        if let tauri::ipc::InvokeResponseBody::Raw(bytes) = body {
            b.lock().unwrap().extend_from_slice(&bytes);
        }
        Ok(())
    });
    (ch, buf)
}

/// The built fake-claude binary.
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
            .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../target"));
        target.join("debug/fake-claude")
    })
    .clone()
}

fn git(dir: &Path, args: &[&str]) {
    let st = Command::new("/usr/bin/git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .status()
        .unwrap();
    assert!(st.success(), "git {args:?}");
}

/// A test app with a created project.
pub struct ProjectApp {
    pub t: TestApp,
    pub root: PathBuf,
    pub project: PathBuf,
    pub id: String,
    pub record: PathBuf,
}

impl ProjectApp {
    /// Blueprint repo, fake-claude (with `script`), and a project made through `setup_*`.
    pub fn new(script: Value) -> Self {
        let t = TestApp::new();
        let root = std::fs::canonicalize(t.dir.path()).unwrap();
        // Blueprint repo.
        let bp = root.join("blueprint");
        std::fs::create_dir_all(&bp).unwrap();
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(
                "../crates/foreman-core/tests/fixtures/blueprint/claude-build-blueprint-v2.2.md",
            ),
            bp.join("claude-build-blueprint.md"),
        )
        .unwrap();
        git(&bp, &["init", "-q", "-b", "main"]);
        git(&bp, &["add", "claude-build-blueprint.md"]);
        git(&bp, &["commit", "-q", "-m", "blueprint"]);
        // fake-claude as the configured claude.
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let record = root.join("rec.json");
        let mut cfg = script;
        cfg["record"] = json!(record);
        let tmp = bin.join(".claude.tmp");
        std::fs::copy(fake_claude(), &tmp).unwrap();
        std::fs::rename(&tmp, bin.join("claude")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        std::fs::write(bin.join("fake.json"), cfg.to_string()).unwrap();
        let (mut s, stamp, _) = t.app.settings().unwrap();
        s.tools.claude.path = Some(bin.join("claude").display().to_string());
        s.tools.claude.source = ToolSource::Settings;
        t.app.save_settings(&s, &stamp).unwrap();
        t.os.pick_next(Some(bp));
        first_run::blueprint_pick_repo(&t.app, req(json!({ "mode": "choose" }))).unwrap();
        // The project.
        let project = root.join("work/tradelog");
        std::fs::create_dir_all(&project).unwrap();
        t.os.pick_next(Some(project.clone()));
        let pick = projects::setup_pick_folder(&t.app, req(json!({}))).unwrap();
        let preview = projects::setup_preview(
            &t.app,
            req(json!({
                "pickId": pick.pick_id,
                "name": "TradeLog",
                "slug": "tradelog",
                "variables": { "ROUGH_IDEA": "A trading journal.", "TARGET_USERS": "Traders" },
                "conditions": { "idea_known": "yes", "commercial": "no" },
                "chatProjectUrl": null
            })),
        )
        .unwrap();
        let summary =
            projects::setup_apply(&t.app, req(json!({ "previewId": preview.preview_id }))).unwrap();
        let id = summary.id.as_str().to_owned();
        Self {
            t,
            root,
            project,
            id,
            record,
        }
    }

    pub fn project_id(&self) -> ProjectId {
        req(json!(self.id))
    }

    /// `launch_prepare` for `step`, acknowledging a first-scan config once (as Billy would).
    pub fn prepare(&self, step: &str) -> PrepareOutcome {
        use foreman_app::ipc::commands::launch;
        let r = || {
            launch::launch_prepare(
                &self.t.app,
                req(json!({ "projectId": self.id, "step": step, "entry": "stageView", "mode": "normal" })),
                channel(),
            )
            .unwrap()
        };
        let out = r();
        if let PrepareOutcome::Blocked { blockers, .. } = &out {
            for b in blockers {
                let v = serde_json::to_value(b).unwrap();
                if v["kind"] == "configChanged" {
                    launch::config_ack(
                        &self.t.app,
                        req(json!({ "projectId": self.id, "cwd": v["cwd"], "fingerprint": v["fingerprint"] })),
                    )
                    .unwrap();
                    return r();
                }
            }
        }
        out
    }

    /// Every run record in app data.
    pub fn run_files(&self) -> Vec<(PathBuf, Value)> {
        let runs = self.t.app_data().join("runs");
        let Ok(rd) = std::fs::read_dir(&runs) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for e in rd {
            let p = e.unwrap().path().join("run.json");
            if let Ok(b) = std::fs::read_to_string(&p) {
                out.push((p, serde_json::from_str(&b).unwrap()));
            }
        }
        out
    }
}

/// A local git identity in the project repo, so Foreman's own decision commits (DR-153) work in
/// a sandbox without a global one.
pub fn git_identity(p: &ProjectApp) {
    git(&p.project, &["config", "user.name", "Billy Test"]);
    git(&p.project, &["config", "user.email", "billy@example.com"]);
}
