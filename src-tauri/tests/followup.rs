//! M3-17: followup and manual steps, switches and overrides, against a real project and
//! fake-claude (DR-066, DR-068, DR-103, DR-105; AC-F007-04, -07, AC-F005-05, -07).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::project::{ProjectApp, collect, req};
use foreman_app::ipc::commands::{files, launch, steps, terminal};
use foreman_app::ipc::dto::launch::{PrepareOutcome, RunStarted};
use serde_json::{Value, json};

fn wait_for(buf: &Arc<Mutex<Vec<u8>>>, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if String::from_utf8_lossy(&buf.lock().unwrap()).contains(needle) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for {needle:?}");
}

fn start(p: &ProjectApp, step: &str) -> (RunStarted, Arc<Mutex<Vec<u8>>>) {
    let PrepareOutcome::Ready(pv) = p.prepare(step) else {
        panic!("{step} is blocked");
    };
    let (ch, buf) = collect();
    let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
    wait_for(&buf, "FAKE-CLAUDE READY");
    (r, buf)
}

fn view(p: &ProjectApp, step: &str) -> Value {
    serde_json::to_value(
        steps::step_view(&p.t.app, req(json!({ "projectId": p.id, "step": step }))).unwrap(),
    )
    .unwrap()
}

fn copy_prompt(p: &ProjectApp, step: &str) -> Value {
    serde_json::to_value(
        steps::clipboard_copy(
            &p.t.app,
            req(json!({ "projectId": p.id, "target": "prompt", "step": step, "chain": null, "path": null, "stage": null })),
        )
        .unwrap(),
    )
    .unwrap()
}

mod steps_m3 {
    use super::*;

    /// AC-F007-07 / DR-068: a followup with its chain live copies its prompt and names the chain's
    /// tab to focus; nothing reaches the PTY. Once the chain's process is gone there is no tab to
    /// focus (the UI relaunches it as a continuing launch).
    #[test]
    fn followup_copy_or_relaunch() {
        let p = ProjectApp::new(json!({}));
        let (r, buf) = start(&p, "2.2");
        let before = buf.lock().unwrap().len();
        let c = copy_prompt(&p, "2.3");
        assert_eq!(c["focusRun"], json!(r.run_id), "{c}");
        assert!(p.t.os.clipboard().unwrap().starts_with("ultrathink."));
        assert_eq!(view(&p, "2.3")["summary"]["state"], "copied");
        std::thread::sleep(Duration::from_millis(400));
        let after = String::from_utf8_lossy(&buf.lock().unwrap()[before..]).into_owned();
        assert!(
            !after.contains("ECHO"),
            "nothing typed into the PTY: {after:?}"
        );
        // The chain ends: no live tab.
        p.t.os.answer_next(true);
        terminal::pty_terminate(&p.t.app, req(json!({ "runId": r.run_id }))).unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while p.t.app.runs().of_chain(r.chain.as_str()).is_some() {
            assert!(Instant::now() < deadline, "the session didn't end");
            std::thread::sleep(Duration::from_millis(50));
        }
        let c = copy_prompt(&p, "2.3");
        assert!(c["focusRun"].is_null(), "{c}");
    }

    /// AC-F007-04 / DR-066: Open file on a manual step's file starts it; a change to that file
    /// afterwards makes it done (re-stat on open).
    #[test]
    fn finish_rules() {
        let p = ProjectApp::new(json!({}));
        let (mut s, stamp, _) = p.t.app.settings().unwrap();
        s.editor_bundle_id = Some("com.example.none".into());
        p.t.app.save_settings(&s, &stamp).unwrap();
        let file = p.project.join("spec/council/r7-stack.md");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "# Stack\n").unwrap();
        // The file predates the step's start.
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(std::time::SystemTime::now() - Duration::from_secs(7200))
            .unwrap();
        assert_eq!(view(&p, "2.4b")["summary"]["state"], "notStarted");
        // Started an hour before the edit below (the scripted clock).
        p.t.os
            .set_now(jiff::Timestamp::now() - jiff::SignedDuration::from_hours(1));
        // `open` itself may fail in the sandbox (no /usr/bin/open, or no such editor); the step
        // starts before the editor is asked.
        let _ = files::file_open(
            &p.t.app,
            req(json!({ "projectId": p.id, "path": "spec/council/r7-stack.md" })),
        );
        assert_eq!(view(&p, "2.4b")["summary"]["state"], "running");
        std::fs::write(&file, "# Stack\n\nFinal choice: Rust, because…\n").unwrap();
        assert_eq!(view(&p, "2.4b")["summary"]["state"], "done");
        // A CLI step can't be marked done by hand.
        let e = steps::step_mark_done(&p.t.app, req(json!({ "projectId": p.id, "step": "2.2" })))
            .unwrap_err();
        assert_eq!(e.code, foreman_app::ipc::error::IpcErrorCode::NotAllowed);
    }
}

mod policy {
    use super::*;

    /// AC-F005-05 / DR-103: an override is logged in the run record and applies to that run only.
    #[test]
    fn override_scope() {
        let p = ProjectApp::new(json!({}));
        let v = serde_json::to_value(
            steps::run_override_set(
                &p.t.app,
                req(json!({ "projectId": p.id, "step": "2.2", "over": { "model": "opus", "effort": "max", "mode": null } })),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(v["override"]["effort"], "max");
        assert_eq!(v["start"]["origin"]["kind"], "override", "{}", v["start"]);
        // A mode on a chat step doesn't fit (NotAllowed, nothing stored).
        let e = steps::run_override_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "1.1", "over": { "model": null, "effort": null, "mode": "plan" } })),
        )
        .unwrap_err();
        assert_eq!(e.code, foreman_app::ipc::error::IpcErrorCode::NotAllowed);
        let (r, _) = start(&p, "2.2");
        let rec: Value = serde_json::from_str(
            &std::fs::read_to_string(
                p.t.app_data()
                    .join("runs")
                    .join(r.run_id.as_str())
                    .join("run.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(rec["override"], true);
        assert!(rec["argv"].to_string().contains("max"), "{}", rec["argv"]);
        // Used once: the next run is back to the template's start.
        assert!(view(&p, "2.2")["override"].is_null());
    }

    /// AC-F005-07 / DR-105: a step's switches are cues on the step view; launching sends none of
    /// them into the PTY as keystrokes.
    #[test]
    fn switches_never_typed() {
        let p = ProjectApp::new(json!({}));
        let v = view(&p, "2.2");
        let cues = v["switches"].as_array().unwrap();
        assert!(!cues.is_empty(), "2.2 has a plan-approved switch");
        let (_, buf) = start(&p, "2.2");
        // fake-claude echoes its argv prompt once; any keystroke would echo again.
        std::thread::sleep(Duration::from_millis(400));
        let out = String::from_utf8_lossy(&buf.lock().unwrap()).into_owned();
        assert_eq!(out.matches("ECHO:").count(), 1, "only the prompt: {out:?}");
        assert!(!out.contains("\u{1b}[Z"), "no Shift+Tab was sent");
    }
}
