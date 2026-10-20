//! M4-12 (sys part), M4-08/M4-09 (ingest side): contract probes in the probe folder with the
//! redesigned probe B, and sensor events written by the real `foreman-sensor` through hooks that
//! fake-claude fires from the run settings (ADR-009, ADR-017, SR-15, SR-17).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use common::Collect;
use foreman_core::ids::{Effort, Mode, Model, Start};
use foreman_core::launch::argv;
use foreman_core::launch::run_settings::{self, RunSettingsOpts, SensorHooks};
use foreman_core::probe::{self as fprobe, ProbeResult};
use foreman_core::session_state::{Activity, SessionState};
use foreman_core::store::Root;
use foreman_sys::claude;
use foreman_sys::launch as fl;
use foreman_sys::pty::{self as fpty, PtySession, Sink};
use foreman_sys::spawn::{Program, SpawnCtx};
use uuid::Uuid;

struct Rig {
    _dir: tempfile::TempDir,
    root: PathBuf,
    app: PathBuf,
    record: PathBuf,
    ctx: SpawnCtx,
    sensor: PathBuf,
}

fn rig(script: serde_json::Value) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let root = common::canon(dir.path());
    let app = root.join("app");
    std::fs::create_dir_all(&app).unwrap();
    std::fs::set_permissions(&app, std::fs::Permissions::from_mode(0o700)).unwrap();
    let record = root.join("rec.json");
    let mut cfg = script;
    cfg["record"] = serde_json::json!(record);
    let claude = common::install_fake(&root.join("bin"), "claude", &cfg);
    let sensor = common::install_sensor(&app);
    std::fs::create_dir_all(root.join("home")).unwrap();
    std::fs::create_dir_all(root.join("repo")).unwrap();
    Rig {
        ctx: SpawnCtx {
            home: root.join("home"),
            app_data: app.clone(),
            projects: vec![root.join("repo")],
            settings: vec![(Program::Claude, claude)],
        },
        _dir: dir,
        root,
        app,
        record,
        sensor,
    }
}

fn token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn calls(r: &Rig) -> Vec<serde_json::Value> {
    let mut p = r.record.as_os_str().to_owned();
    p.push(".calls.jsonl");
    std::fs::read_to_string(PathBuf::from(p))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Resets the probe folder and runs both probes; returns the report.
fn probes(r: &Rig) -> claude::ProbeReport {
    let app = Root::open_app_data(&r.app).unwrap();
    let probe_run = Uuid::new_v4();
    let tok = token();
    let hooks = SensorHooks {
        command: r.sensor.display().to_string(),
        run_id: probe_run.hyphenated().to_string(),
        token: tok.clone(),
    };
    let (bytes, _) = fl::probe_settings(&r.app, &hooks).unwrap();
    let settings = fl::reset_probe_dir(&app, &bytes, probe_run).unwrap();
    let st = Start {
        model: Model::Opus,
        effort: Effort::High,
        mode: Some(Mode::Plan),
    };
    let a = argv::probe_a(&settings, Uuid::new_v4(), st).unwrap();
    let (b1, b2) = argv::probe_b(Uuid::new_v4(), Uuid::new_v4()).unwrap();
    let env = common::child_env(&r.root.join("home"), probe_run);
    claude::run_probes(&r.ctx, &env.0, a.0, (b1.0, b2), &|| {
        fl::probe_marker(&app, probe_run, &tok)
    })
}

mod probe {
    use super::*;

    #[test]
    fn runs_only_in_probe_dir() {
        let r = rig(serde_json::json!({}));
        let rep = probes(&r);
        assert_eq!(rep.a.result, ProbeResult::Pass, "{rep:?}");
        assert_eq!(rep.b.result, ProbeResult::Pass, "{rep:?}");
        let c = calls(&r);
        assert_eq!(c.len(), 3);
        for call in &c {
            assert_eq!(call["cwd"], r.app.join("probe").display().to_string());
            assert_eq!(call["argv"][1], "--init-only");
        }
        // B1 is the fork form; B2 the refused control.
        let b1: Vec<&str> = c[1]["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(&b1[1..4], ["--init-only", "--resume", b1[3]]);
        assert!(b1.contains(&"--fork-session"));
        let b2: Vec<&str> = c[2]["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(!b2.contains(&"--fork-session") && b2.contains(&"--session-id"));
        // A probe request outside the fixed flags never spawns.
        let e = foreman_sys::spawn::run(
            &foreman_sys::spawn::SpawnRequest::ClaudeProbe(vec![
                "--init-only".into(),
                "--print".into(),
            ]),
            &foreman_sys::spawn::RunOpts {
                env: vec![],
                timeout: Duration::from_secs(5),
                output_cap: 1024,
                cwd: foreman_sys::spawn::Cwd::AppData,
            },
            &r.ctx,
        );
        assert!(e.is_err());
        assert_eq!(calls(&r).len(), 3);
    }

    #[test]
    fn dir_reset_before_run() {
        let r = rig(serde_json::json!({}));
        let probe_dir = r.app.join("probe");
        std::fs::create_dir_all(probe_dir.join(".claude")).unwrap();
        std::fs::write(
            probe_dir.join(".claude/settings.json"),
            r#"{"apiKeyHelper":"x"}"#,
        )
        .unwrap();
        std::fs::write(probe_dir.join("CLAUDE.md"), "planted").unwrap();
        std::fs::write(r.root.join("outside.txt"), "keep me").unwrap();
        symlink(r.root.join("outside.txt"), probe_dir.join("link")).unwrap();
        std::fs::set_permissions(&probe_dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        let app = Root::open_app_data(&r.app).unwrap();
        let run = Uuid::new_v4();
        fl::reset_probe_dir(&app, b"{}", run).unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&probe_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["settings.json"]);
        assert_eq!(
            std::fs::metadata(&probe_dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::read_to_string(r.root.join("outside.txt")).unwrap(),
            "keep me"
        );
        // The probe settings carry exactly one hook (files.md §4.11).
        let hooks = SensorHooks {
            command: r.sensor.display().to_string(),
            run_id: run.hyphenated().to_string(),
            token: token(),
        };
        let (bytes, _) = fl::probe_settings(&r.app, &hooks).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v["hooks"].as_object().unwrap().keys().collect::<Vec<_>>(),
            ["SessionStart"]
        );
    }

    #[test]
    fn failure_disables_feature() {
        let r = rig(serde_json::json!({
            "initOnlyStderr": "Warning: Unknown --effort value 'high' — ignoring it and using the default effort. Valid values: low, medium, high, xhigh, max."
        }));
        let rep = probes(&r);
        assert_eq!(rep.a.result, ProbeResult::FlagFailure);
        assert_eq!(rep.a.flag.as_deref(), Some("--effort"));
        assert_eq!(
            fprobe::disabled_features(&[rep.a.clone(), rep.b.clone()]),
            ["effort"]
        );
        // No hooks firing (a trust prompt or a changed hook contract): inconclusive, not a pass.
        let r = rig(serde_json::json!({ "initOnlyHooks": false }));
        assert_eq!(probes(&r).a.result, ProbeResult::Inconclusive);
    }
}

mod sensor {
    use super::*;

    /// End to end: run settings with the sensor hooks, a PTY session that fires `SessionStart`,
    /// `Stop` and `SessionEnd` through them, the real sensor writing event files, and ingest
    /// folding them; a forged event with another token is dropped.
    #[test]
    fn hooks_fire_through_run_settings() {
        let r = rig(serde_json::json!({}));
        let app = Root::open_app_data(&r.app).unwrap();
        let run = Uuid::new_v4();
        let tok = token();
        foreman_core::store::app_dirs::reset_dir(&app, &format!("events/{run}")).unwrap();
        let f = run_settings::build(
            &r.root.join("repo"),
            &r.app,
            &RunSettingsOpts {
                sensor: Some(SensorHooks {
                    command: r.sensor.display().to_string(),
                    run_id: run.hyphenated().to_string(),
                    token: tok.clone(),
                }),
                ..RunSettingsOpts::default()
            },
        )
        .unwrap();
        let (bytes, _) = run_settings::to_bytes(&f);
        let settings = r.app.join(format!("runs/{run}/settings.json"));
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::write(&settings, bytes).unwrap();
        let env = common::child_env(&r.root.join("home"), run);
        let sink = Arc::new(Collect::default());
        let args: Vec<String> = [
            "--session-id",
            "6f1c2a3b-1d2e-4f50-8a9b-0c1d2e3f4a5b",
            "--model",
            "opus",
            "--permission-mode",
            "plan",
            "--settings",
            settings.to_str().unwrap(),
            "hello",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let s = PtySession::spawn(
            &r.ctx,
            &args,
            &env.0,
            &r.root.join("repo"),
            fpty::size(80, 24).unwrap(),
            Arc::clone(&sink) as Arc<dyn Sink>,
        )
        .unwrap();
        sink.wait_for("ECHO: hello", 10);
        let st = foreman_sys::sensor::ingest(
            &app,
            &run.hyphenated().to_string(),
            &tok,
            SessionState::default(),
        )
        .unwrap();
        assert_eq!(st.activity, Activity::Idle, "{st:?}");
        assert_eq!(st.last_assistant_message.as_deref(), Some("ECHO: hello"));
        assert_eq!(st.permission_mode.as_deref(), Some("plan"));
        s.write(b"/exit\r").unwrap();
        s.wait_exit(Duration::from_secs(10)).unwrap();
        let st =
            foreman_sys::sensor::ingest(&app, &run.hyphenated().to_string(), &tok, st).unwrap();
        assert_eq!(
            st.activity,
            Activity::Ended {
                reason: Some("prompt_input_exit".into())
            }
        );
        // A forged event (well-formed, wrong token) is dropped by ingest.
        let forged = r.app.join(format!(
            "events/{run}/99999999999999999999-0000000001-Stop.json"
        ));
        std::fs::write(&forged, format!(
            "{{\"event\":\"Stop\",\"payload\":{{}},\"receivedAt\":\"2026-10-20T00:00:00.000Z\",\"schemaVersion\":1,\"token\":\"{}\",\"truncated\":false}}",
            "f".repeat(64)
        )).unwrap();
        std::fs::set_permissions(&forged, std::fs::Permissions::from_mode(0o600)).unwrap();
        let st =
            foreman_sys::sensor::ingest(&app, &run.hyphenated().to_string(), &tok, st).unwrap();
        assert_eq!(st.discarded, 1);
        assert!(matches!(st.activity, Activity::Ended { .. }));
    }
}
