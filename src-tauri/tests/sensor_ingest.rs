//! M4-07…M4-10 in the app (ADR-009, ADR-008; SR-15, SR-18, SR-53; T-024, T-029, T-031, T-045…
//! T-050): the sensor installed from the bundle, its hash checked before every run settings file,
//! the hooks in exec form with a per-run token, ingest into display state and `run:status`, the
//! re-arm's sensor idle and busy paths, retention, and that nothing ever writes Claude Code's
//! settings. Runs fake-claude, which fires the hooks from `--settings` through the real sensor.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::project::{ProjectApp, channel, collect, req};
use common::sensor as rig;
use foreman_app::ipc::commands::{launch, steps, terminal};
use foreman_app::ipc::dto::events::Event;
use foreman_app::ipc::dto::launch::{LaunchPreview, PrepareOutcome, RearmOutcome, RunStarted};
use foreman_app::ipc::error::IpcErrorCode;
use foreman_app::ipc::types::RunStatus;
use serde_json::{Value, json};

fn ready(p: &ProjectApp, step: &str) -> LaunchPreview {
    match p.prepare(step) {
        PrepareOutcome::Ready(pv) => pv,
        PrepareOutcome::Blocked { blockers, .. } => panic!("blocked: {blockers:?}"),
    }
}

fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("timed out waiting for {what}");
}

fn wait_for(buf: &Arc<Mutex<Vec<u8>>>, needle: &str) {
    wait_until(needle, || {
        String::from_utf8_lossy(&buf.lock().unwrap()).contains(needle)
    });
}

fn start(p: &ProjectApp, step: &str) -> (RunStarted, Arc<Mutex<Vec<u8>>>) {
    let pv = ready(p, step);
    let (ch, buf) = collect();
    let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
    wait_for(&buf, "FAKE-CLAUDE READY");
    (r, buf)
}

fn run_json(p: &ProjectApp, run_id: &str) -> Value {
    let f = p.t.app_data().join("runs").join(run_id).join("run.json");
    serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap()
}

fn settings_json(p: &ProjectApp, run_id: &str) -> Value {
    let f =
        p.t.app_data()
            .join("runs")
            .join(run_id)
            .join("settings.json");
    serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap()
}

fn statuses(p: &ProjectApp, run_id: &str) -> Vec<RunStatus> {
    p.t.os
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::RunStatus(s) if s.run_id.as_str() == run_id => Some(s.status),
            _ => None,
        })
        .collect()
}

fn events_in(p: &ProjectApp, run_id: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(p.t.app_data().join("events").join(run_id))
        .map(|r| {
            r.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Sequence numbers after anything the real sensor writes (its `seq` is nanoseconds since 1970).
const BASE: u64 = 9_000_000_000_000_000_000;

fn progress_json(p: &ProjectApp) -> Option<String> {
    std::fs::read_to_string(
        p.t.app_data()
            .join("projects")
            .join(&p.id)
            .join("progress.json"),
    )
    .ok()
}

/// A project app with the real sensor bundled.
fn with_sensor(script: Value) -> ProjectApp {
    let p = ProjectApp::new(script);
    rig::enable(&p.t.app);
    p
}

/// Types into the live session (fake-claude echoes and fires `Stop`).
fn say(p: &ProjectApp, run: &str, text: &str) {
    terminal::pty_write(
        &p.t.app,
        req(json!({ "runId": run, "data": format!("{text}\r") })),
    )
    .unwrap();
}

mod sensor {
    use super::*;

    /// M4-07 (ADR-009, files.md §2 row 24): the bundled bytes are installed into
    /// `<app-data>/bin/foreman-sensor-<sha256-12>`, `0700`, and nothing else is kept there.
    #[test]
    fn installed_from_bundle() {
        let p = with_sensor(json!({}));
        let bin = p.t.app_data().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("foreman-sensor-000000000000"), b"old").unwrap();
        let foreman_app::sensor::Check::Ok(path) = foreman_app::sensor::install(&p.t.app) else {
            panic!("not installed")
        };
        assert_eq!(
            path,
            rig::installed(&std::fs::canonicalize(p.t.app_data()).unwrap())
        );
        assert_eq!(std::fs::read(&path).unwrap(), &*rig::sensor_bytes());
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let names: Vec<_> = std::fs::read_dir(&bin).unwrap().collect();
        assert_eq!(names.len(), 1, "the older sensor was replaced");
        // A build without the sensor reports it and installs nothing.
        p.t.app.sensor().set_bundle(None);
        assert_eq!(
            foreman_app::sensor::install(&p.t.app),
            foreman_app::sensor::Check::NotBundled
        );
    }

    /// AC-F011-01, SR-09, SR-15 (T-024, T-046): every hook in exec form (`command` + `args`, no
    /// shell string) at the installed sensor with the run's token; `PostToolUse` matches `Bash`;
    /// `disableAllHooks: false`; the token is in `run.json` and nowhere in the argv.
    #[test]
    fn exec_form_only() {
        let p = with_sensor(json!({}));
        let (r, _) = start(&p, "2.2");
        let id = r.run_id.as_str();
        let s = settings_json(&p, id);
        let rec = run_json(&p, id);
        let token = rec["sensorToken"].as_str().unwrap().to_owned();
        assert_eq!(token.len(), 64);
        assert!(
            token
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        assert_eq!(s["disableAllHooks"], false);
        let hooks = s["hooks"].as_object().unwrap();
        let mut names: Vec<&str> = hooks.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "Notification",
                "PermissionRequest",
                "PostToolUse",
                "SessionEnd",
                "SessionStart",
                "Stop",
                "StopFailure",
                "SubagentStart",
                "SubagentStop",
                "TaskCompleted",
                "TaskCreated"
            ]
        );
        let bin = rig::installed(&std::fs::canonicalize(p.t.app_data()).unwrap());
        for (ev, groups) in hooks {
            let g = &groups[0];
            assert_eq!(
                g["matcher"].as_str(),
                (ev == "PostToolUse").then_some("Bash"),
                "{ev}"
            );
            let h = &g["hooks"][0];
            assert_eq!(h["type"], "command");
            assert_eq!(h["command"].as_str().unwrap(), bin.to_str().unwrap());
            let args: Vec<&str> = h["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_str().unwrap())
                .collect();
            assert_eq!(
                args,
                ["--run", id, "--token", &token, "--event", ev, "--v", "1"]
            );
        }
        let argv = serde_json::to_string(&rec["argv"]).unwrap();
        assert!(
            !argv.contains(&token),
            "the token never appears in the argv"
        );
        p.t.app.runs().hangup_all();
        // Another run, another token.
        let q = with_sensor(json!({}));
        let (r2, _) = start(&q, "2.2");
        assert_ne!(
            run_json(&q, r2.run_id.as_str())["sensorToken"],
            rec["sensorToken"]
        );
        q.t.app.runs().hangup_all();
    }

    /// AC-F011-02, SR-15 (T-047, T-050): the installed binary's hash is checked before every run
    /// settings file; a changed binary is re-installed once; if that fails the run gets no hooks,
    /// no token and a warning, and the P1 manual behaviors apply.
    #[test]
    fn hash_checked_per_run() {
        let p = with_sensor(json!({}));
        let foreman_app::sensor::Check::Ok(bin) = foreman_app::sensor::install(&p.t.app) else {
            panic!()
        };
        // Swapped for something else between runs.
        std::fs::write(&bin, b"#!/bin/sh\nexit 0\n").unwrap();
        let pv = ready(&p, "2.2");
        assert!(
            pv.warnings.iter().all(|w| !w.contains("hash")),
            "{:?}",
            pv.warnings
        );
        assert_eq!(
            std::fs::read(&bin).unwrap(),
            &*rig::sensor_bytes(),
            "re-installed"
        );
        drop(pv);
        // Now it can't be replaced: a folder sits where the binary goes.
        std::fs::remove_file(&bin).unwrap();
        std::fs::create_dir_all(bin.join("x")).unwrap();
        let pv = ready(&p, "2.2");
        assert!(
            pv.warnings.iter().any(|w| w.contains("expected hash")),
            "{:?}",
            pv.warnings
        );
        let (ch, buf) = collect();
        let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
        wait_for(&buf, "FAKE-CLAUDE READY");
        let id = r.run_id.as_str();
        assert!(
            settings_json(&p, id)
                .get("hooks")
                .is_none_or(Value::is_null)
        );
        assert!(run_json(&p, id)["sensorToken"].is_null());
        assert!(!p.t.app_data().join("events").join(id).exists());
        p.t.app.runs().hangup_all();
    }

    /// AC-F011-05, AC-F009-14, AC-F011-08 end to end: fake-claude fires `SessionStart`, `Stop` and
    /// `SessionEnd` through the run settings; the real sensor writes the files; ingest derives the
    /// state and emits `run:status` idle — and nothing else happens: no step changes, no run
    /// starts, no file outside `events/` is written.
    #[test]
    fn events_only_update_state() {
        let p = with_sensor(json!({}));
        let (r, _) = start(&p, "2.2");
        let id = r.run_id.as_str().to_owned();
        say(&p, &id, "hello");
        wait_until("idle", || statuses(&p, &id).contains(&RunStatus::Idle));
        let st = foreman_app::sensor_ingest::live_state(&p.t.app, &id).unwrap();
        assert_eq!(st.last_assistant_message.as_deref(), Some("ECHO: hello"));
        assert_eq!(st.observed_model.as_deref(), Some("fable"));
        assert_eq!(
            foreman_app::sensor_ingest::live_status(&p.t.app, &id),
            RunStatus::Idle
        );
        // Display state only: the progress file, the run table and the step are as they were.
        let before = progress_json(&p);
        let calls_before = std::fs::read_to_string(format!("{}.calls.jsonl", p.record.display()))
            .unwrap_or_default()
            .lines()
            .count();
        say(&p, &id, "again");
        // W8: wait until the app has ingested the new events (a condition, not a sleep), then
        // check that ingesting them did nothing else.
        wait_until("ingested", || {
            foreman_app::sensor_ingest::live_state(&p.t.app, &id)
                .and_then(|s| s.last_assistant_message)
                .as_deref()
                == Some("ECHO: again")
        });
        assert_eq!(p.t.app.runs().live().len(), 1, "no run started");
        let calls_after = std::fs::read_to_string(format!("{}.calls.jsonl", p.record.display()))
            .unwrap_or_default()
            .lines()
            .count();
        assert_eq!(calls_after, calls_before, "nothing was spawned");
        let after = progress_json(&p);
        assert_eq!(before, after);
        // The exit: SessionEnd is ingested, and run.json records what the session observed.
        say(&p, &id, "/exit");
        wait_until("exit", || p.t.app.runs().live().is_empty());
        wait_until("observed", || !run_json(&p, &id)["observed"].is_null());
        assert_eq!(run_json(&p, &id)["observed"]["model"], "fable");
        // N2: once the run is over its token and derived state are forgotten in memory (run.json
        // keeps the token for later reads).
        wait_until("forgotten", || {
            p.t.app.sensor().token(&id).is_none()
                && foreman_app::sensor_ingest::live_state(&p.t.app, &id).is_none()
        });
        assert!(run_json(&p, &id)["sensorToken"].is_string());
        let st = foreman_app::sensor_ingest::session_state(
            &foreman_core::store::Root::open_app_data(&p.t.app_data()).unwrap(),
            &id,
        )
        .unwrap();
        assert!(matches!(
            st.activity,
            foreman_core::session_state::Activity::Ended { .. }
        ));
        // The module can't act: it calls nothing that launches, re-arms, ends, signs or writes.
        let src = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sensor_ingest.rs"),
        )
        .unwrap();
        let code: String = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in [
            "launch::",
            "rearm_start",
            "rearm_continue",
            "foreman_sys::rearm",
            "terminate",
            "interrupt",
            "hangup",
            "signing",
            "sign_",
            "save_progress",
            "save_settings",
            "write_run",
            ".write(",
            "spawn::",
            "commands::",
            "set_bundle",
            "install",
        ] {
            assert!(!code.contains(forbidden), "sensor_ingest uses {forbidden}");
        }
    }

    /// AC-F011-03, T-045: an event with another run's token is dropped by ingest and changes
    /// nothing shown.
    #[test]
    fn rejects_wrong_token() {
        let p = with_sensor(json!({}));
        let (r, _) = start(&p, "2.2");
        let id = r.run_id.as_str().to_owned();
        wait_until("SessionStart", || {
            foreman_app::sensor_ingest::ingest_once(&p.t.app, &id)
                .is_some_and(|s| s.last_seq.is_some())
        });
        let before = foreman_app::sensor_ingest::ingest_once(&p.t.app, &id).unwrap();
        rig::put_event(
            &p.t.app_data(),
            &id,
            BASE + 999,
            &"f".repeat(64),
            "PermissionRequest",
            json!({ "tool_name": "Bash", "permission_mode": "bypassPermissions" }),
        );
        let after = foreman_app::sensor_ingest::ingest_once(&p.t.app, &id).unwrap();
        assert_eq!(after.discarded, before.discarded + 1);
        assert_eq!(after.activity, before.activity);
        assert_eq!(after.permission_mode, before.permission_mode);
        p.t.app.runs().hangup_all();
    }

    /// AC-F011-06: the live mode differs from the step's → a visible warning on the step; the
    /// session is left as it is.
    #[test]
    fn mode_guard_warns() {
        let p = with_sensor(json!({}));
        let (r, _) = start(&p, "2.2");
        let id = r.run_id.as_str().to_owned();
        let token = run_json(&p, &id)["sensorToken"]
            .as_str()
            .unwrap()
            .to_owned();
        rig::put_event(
            &p.t.app_data(),
            &id,
            BASE + 990,
            &token,
            "PostToolUse",
            json!({ "permission_mode": "acceptEdits", "tool_input": {} }),
        );
        foreman_app::sensor_ingest::ingest_once(&p.t.app, &id).unwrap();
        let v =
            steps::step_view(&p.t.app, req(json!({ "projectId": p.id, "step": "2.2" }))).unwrap();
        assert!(
            v.warnings
                .iter()
                .any(|w| w.contains("acceptEdits") && w.contains("doesn't change it")),
            "{:?}",
            v.warnings
        );
        p.t.app.runs().hangup_all();
    }

    /// AC-F011-07, SR-53: at most 20 event files per run are kept.
    #[test]
    fn keeps_last_20_events() {
        let p = with_sensor(json!({}));
        let (r, _) = start(&p, "2.2");
        let id = r.run_id.as_str().to_owned();
        let token = run_json(&p, &id)["sensorToken"]
            .as_str()
            .unwrap()
            .to_owned();
        for i in 0..30 {
            rig::put_event(
                &p.t.app_data(),
                &id,
                BASE + i,
                &token,
                "PostToolUse",
                json!({ "tool_input": {} }),
            );
        }
        foreman_app::sensor_ingest::ingest_once(&p.t.app, &id).unwrap();
        assert_eq!(events_in(&p, &id).len(), 20);
        p.t.app.runs().hangup_all();
    }
}

mod rearm {
    use super::*;

    /// M4-10 (AC-F010-13, AC-F010-14, AC-F011-08, AC-F011-09; T-029): sensor events never start a
    /// re-arm; Run does. With the sensor idle, Run re-arms without the manual dialog. With
    /// background tasks, Run asks "N background tasks still running" (Wait refuses, End anyway
    /// re-arms). Without events, Billy's native idle confirmation stays.
    #[test]
    fn only_from_user_action() {
        let p = with_sensor(json!({}));
        let (old, _) = start(&p, "2.2");
        let id = old.run_id.as_str().to_owned();
        let token = run_json(&p, &id)["sensorToken"]
            .as_str()
            .unwrap()
            .to_owned();
        say(&p, &id, "hello");
        wait_until("idle", || statuses(&p, &id).contains(&RunStatus::Idle));
        // Idle is only shown: no re-arm, no run, nothing typed into the session.
        std::thread::sleep(Duration::from_millis(1200));
        assert_eq!(p.t.app.runs().live().len(), 1);
        // Background work: a task is outstanding.
        rig::put_event(
            &p.t.app_data(),
            &id,
            BASE,
            &token,
            "TaskCreated",
            json!({ "task_id": "t1" }),
        );
        rig::put_event(
            &p.t.app_data(),
            &id,
            BASE + 1,
            &token,
            "TaskCreated",
            json!({ "task_id": "t2" }),
        );
        let pv = ready(&p, "2.3");
        let asked = p.t.os.asked().len();
        p.t.os.answer_next(false); // Wait
        let e = terminal::rearm_start(
            &p.t.app,
            req(json!({ "planId": pv.plan_id })),
            channel(),
            channel(),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed, "{e:?}");
        let dialogs = p.t.os.asked();
        assert_eq!(dialogs.len(), asked + 1);
        assert!(
            dialogs
                .last()
                .unwrap()
                .title
                .contains("2 background tasks still running"),
            "{:?}",
            dialogs.last()
        );
        assert!(
            p.t.app.runs().get(&id).unwrap().session.is_live(),
            "Wait sent nothing"
        );
        // The tasks finish: idle again, and Run re-arms with no dialog at all.
        for (i, t) in ["t1", "t2"].iter().enumerate() {
            rig::put_event(
                &p.t.app_data(),
                &id,
                BASE + 10 + i as u64,
                &token,
                "TaskCompleted",
                json!({ "task_id": t }),
            );
        }
        rig::put_event(
            &p.t.app_data(),
            &id,
            BASE + 20,
            &token,
            "Stop",
            json!({ "last_assistant_message": "done" }),
        );
        let pv = ready(&p, "2.3");
        let asked = p.t.os.asked().len();
        let (out, buf) = collect();
        let r = terminal::rearm_start(
            &p.t.app,
            req(json!({ "planId": pv.plan_id })),
            out,
            channel(),
        )
        .unwrap();
        assert_eq!(p.t.os.asked().len(), asked, "no manual idle dialog");
        let RearmOutcome::Relaunched(new) = r else {
            panic!("{r:?}")
        };
        wait_for(&buf, "FAKE-CLAUDE READY");
        assert!(!p.t.app.runs().get(&id).unwrap().session.is_live());
        // The relaunched run has its own token.
        assert_ne!(
            run_json(&p, new.run_id.as_str())["sensorToken"],
            json!(token)
        );
        p.t.app.runs().hangup_all();
    }
}

mod rearm_manual {
    use super::*;

    /// AC-F011-09: a run with the sensor whose hooks never fire still runs, and Run asks Billy
    /// the native idle question.
    #[test]
    fn manual_idle_without_events() {
        let p = with_sensor(json!({ "tui": { "noHooks": true } }));
        let (old, _) = start(&p, "2.2");
        let id = old.run_id.as_str().to_owned();
        assert!(run_json(&p, &id)["sensorToken"].is_string());
        say(&p, &id, "hello");
        std::thread::sleep(Duration::from_millis(800));
        assert!(events_in(&p, &id).is_empty());
        assert_eq!(
            foreman_app::sensor_ingest::live_status(&p.t.app, &id),
            RunStatus::Running
        );
        let pv = ready(&p, "2.3");
        let asked = p.t.os.asked().len();
        let (out, buf) = collect();
        let r = terminal::rearm_start(
            &p.t.app,
            req(json!({ "planId": pv.plan_id })),
            out,
            channel(),
        )
        .unwrap();
        assert!(matches!(r, RearmOutcome::Relaunched(_)), "{r:?}");
        wait_for(&buf, "FAKE-CLAUDE READY");
        let dialogs = p.t.os.asked();
        assert_eq!(dialogs.len(), asked + 1);
        assert_eq!(dialogs.last().unwrap().title, "Is Claude idle?");
        p.t.app.runs().hangup_all();
    }
}

mod retention {
    use super::*;

    fn record(p: &ProjectApp, run: &str, wall_start: &str, exited_ms: Option<u64>, status: &str) {
        let dir = p.t.app_data().join("runs").join(run);
        std::fs::create_dir_all(&dir).unwrap();
        let rec = json!({
            "argv": ["/x/claude"], "chainId": null, "claudeVersion": null, "copyPromptOnCli": false,
            "cwd": "project", "entryPoint": "stageView", "envNames": [], "exitCode": 0,
            "intended": null, "kind": "launch", "name": null, "observed": null, "override": false,
            "pid": null, "projectId": p.id, "promptSha256": null, "runId": run,
            "runInputInterventions": 0, "runSettingsSha256": null, "schemaVersion": 1,
            "sensorToken": "a".repeat(64), "stage": 2, "status": status, "stepId": "2.2",
            "surface": "cli",
            "t": { "confirmed": null, "exited": exited_ms, "lookStart": null, "prepared": null,
                   "spawned": null, "wallStart": wall_start },
            "timeToStartMs": null
        });
        std::fs::write(
            dir.join("run.json"),
            serde_json::to_string_pretty(&rec).unwrap(),
        )
        .unwrap();
        let ev = p.t.app_data().join("events").join(run);
        std::fs::create_dir_all(&ev).unwrap();
        std::fs::write(ev.join("00000000000000000001-0000000001-Stop.json"), "{}").unwrap();
    }

    /// AC-F011-07, SR-53, PR-04: a run's events go 30 days after it ended (the startup scan);
    /// younger runs keep theirs; a folder that belongs to no run goes.
    #[test]
    fn events_max_age() {
        let p = ProjectApp::new(json!({}));
        let day = 86_400_000_u64;
        p.t.os
            .set_now(jiff::Timestamp::from_second(1_792_400_400).unwrap());
        let ids = [
            "0b000000-0000-4000-8000-000000000001",
            "0b000000-0000-4000-8000-000000000002",
            "0b000000-0000-4000-8000-000000000003",
            "0b000000-0000-4000-8000-000000000004",
        ];
        // Started 40 days ago, ran 2 days: ended 38 days ago.
        record(&p, ids[0], "2026-09-09T09:00:00Z", Some(2 * day), "exited");
        // Started 40 days ago but ran 15 days: ended 25 days ago.
        record(&p, ids[1], "2026-09-09T09:00:00Z", Some(15 * day), "exited");
        // Started 31 days ago, no exit time (interrupted): ended when it started.
        record(&p, ids[2], "2026-09-18T09:00:00Z", None, "interrupted");
        // Still running according to its record.
        record(&p, ids[3], "2026-07-31T09:00:00Z", None, "running");
        // An event folder of no run (a probe marker left by a crash).
        let orphan =
            p.t.app_data()
                .join("events/0b000000-0000-4000-8000-0000000000ff");
        std::fs::create_dir_all(&orphan).unwrap();
        foreman_app::runs::interrupted(&p.t.app).unwrap();
        let ev = p.t.app_data().join("events");
        assert!(!ev.join(ids[0]).exists(), "38 days after the end");
        assert!(ev.join(ids[1]).exists(), "25 days after the end");
        assert!(
            !ev.join(ids[2]).exists(),
            "31 days after an interrupted start"
        );
        assert!(!orphan.exists());
        // The startup scan first marked the `running` one interrupted (no live session); it ended
        // at its start, 80 days ago, so its events went too.
        assert!(!ev.join(ids[3]).exists());
        assert_eq!(foreman_app::sensor::prune_events(&p.t.app), 0);
    }
}

mod fs_policy {
    use super::*;

    /// AC-F011-10, SR-10 (sensor setup paths): installing the sensor, launching with hooks, the
    /// probes and Doctor never write `~/.claude/settings.json` (present or absent).
    #[test]
    fn denies_claude_settings_writes() {
        let p = with_sensor(json!({}));
        let claude = p.t.home().join(".claude");
        std::fs::create_dir_all(&claude).unwrap();
        let user = r#"{"cleanupPeriodDays": 20, "permissions": {"allow": ["Bash(*)"]}}"#;
        std::fs::write(claude.join("settings.json"), user).unwrap();
        let mtime = std::fs::metadata(claude.join("settings.json"))
            .unwrap()
            .modified()
            .unwrap();
        let _ = foreman_app::sensor::install(&p.t.app);
        let (r, _) = start(&p, "2.2");
        say(&p, r.run_id.as_str(), "hi");
        let _ = foreman_app::ipc::commands::doctor::doctor_run(
            &p.t.app,
            req(json!({ "scope": "full" })),
        )
        .unwrap();
        let _ = foreman_app::ipc::commands::doctor::probe_run(&p.t.app, req(json!({})), channel());
        p.t.app.runs().hangup_all();
        assert_eq!(
            std::fs::read_to_string(claude.join("settings.json")).unwrap(),
            user
        );
        assert_eq!(
            std::fs::metadata(claude.join("settings.json"))
                .unwrap()
                .modified()
                .unwrap(),
            mtime
        );
        assert!(!claude.join("settings.local.json").exists());
        // Absent stays absent.
        let q = with_sensor(json!({}));
        let _ = foreman_app::sensor::install(&q.t.app);
        let (r, _) = start(&q, "2.2");
        say(&q, r.run_id.as_str(), "hi");
        q.t.app.runs().hangup_all();
        assert!(!q.t.home().join(".claude/settings.json").exists());
    }
}

mod capture {
    use super::*;

    /// AC-F013-14 end to end (M4-23 with M4-08/M4-09): a real run gets a sensor token, the fake
    /// session's reply reaches `last_assistant_message` through the hooks and the real sensor,
    /// and paste-back "Capture from session" previews exactly that reply (nothing written).
    #[test]
    fn reply_captured_end_to_end() {
        let p = with_sensor(json!({}));
        let (r, _) = start(&p, "2.2");
        let id = r.run_id.as_str().to_owned();
        assert!(run_json(&p, &id)["sensorToken"].is_string());
        say(&p, &id, "the council is done");
        wait_until("idle", || statuses(&p, &id).contains(&RunStatus::Idle));
        let root = foreman_core::store::Root::open_app_data(&p.t.app_data()).unwrap();
        assert_eq!(
            foreman_app::sensor_ingest::last_assistant_message(&root, &id).as_deref(),
            Some("ECHO: the council is done")
        );
        let pv = foreman_app::ipc::commands::chat::paste_back_preview(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "2.2", "kind": "output", "source": "sensor" })),
        )
        .map(|v| serde_json::to_value(v).unwrap());
        let pv = pv.unwrap_or_else(|e| panic!("{e:?}"));
        assert!(pv.to_string().contains("ECHO: the council is done"), "{pv}");
        assert!(!p.project.join("spec/council/r1-market.md").exists());
        p.t.app.runs().hangup_all();
    }
}
