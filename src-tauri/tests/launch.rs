//! M2-08, M2-10, M3-02, M3-03, M3-10…M3-15, M3-22: the launch and terminal commands against a
//! real project and fake-claude — the run record before spawn, native confirmations (a scripted
//! Cancel changes nothing), the PTY commands, interrupted runs after a crash, read-only refusal
//! and config changes under a live run.
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

use common::project::{ProjectApp, channel, collect, req};
use foreman_app::ipc::IpcError;
use foreman_app::ipc::commands::{app as app_cmd, launch, projects, steps, terminal};
use foreman_app::ipc::dto::launch::{LaunchPreview, PrepareOutcome, RunStarted};
use foreman_app::ipc::error::{IpcErrorCode, IpcErrorDetail};
use serde_json::{Value, json};

fn ready(p: &ProjectApp, step: &str) -> LaunchPreview {
    match p.prepare(step) {
        PrepareOutcome::Ready(pv) => pv,
        PrepareOutcome::Blocked { blockers, .. } => panic!("blocked: {blockers:?}"),
    }
}

fn wait_for(buf: &Arc<Mutex<Vec<u8>>>, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if String::from_utf8_lossy(&buf.lock().unwrap()).contains(needle) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "timed out waiting for {needle:?}: {}",
        String::from_utf8_lossy(&buf.lock().unwrap())
    );
}

/// Prepares and starts `step`; waits until fake-claude is up.
fn start(p: &ProjectApp, step: &str) -> (RunStarted, Arc<Mutex<Vec<u8>>>) {
    let pv = ready(p, step);
    let (ch, buf) = collect();
    let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
    wait_for(&buf, "FAKE-CLAUDE READY");
    (r, buf)
}

fn code(e: &IpcError) -> IpcErrorCode {
    e.code
}

fn run_json(p: &ProjectApp, run_id: &str) -> Value {
    let f = p.t.app_data().join("runs").join(run_id).join("run.json");
    serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap()
}

fn calls(p: &ProjectApp) -> usize {
    let mut path = p.record.as_os_str().to_owned();
    path.push(".calls.jsonl");
    std::fs::read_to_string(std::path::PathBuf::from(path))
        .unwrap_or_default()
        .lines()
        .count()
}

mod lifecycle {
    use super::*;

    /// SR-57, ADR-010 §6: a run that was `running` when Foreman died is `interrupted` after the
    /// restart, listed by `app_bootstrap` and never relaunched.
    #[test]
    fn crash_leaves_interrupted() {
        let p = ProjectApp::new(json!({}));
        let (r, _) = start(&p, "2.2");
        let id = r.run_id.as_str().to_owned();
        assert_eq!(run_json(&p, &id)["status"], "running");
        // The crash: the process table is gone, run.json still says running.
        let os = Arc::new(foreman_app::os::ScriptedOs::default());
        let again = foreman_app::state::App::new(
            os,
            foreman_app::state::Paths {
                app_data: p.t.app_data(),
                home: p.t.home(),
            },
        );
        let calls_before = calls(&p);
        let boot = app_cmd::app_bootstrap(&again, req(json!({}))).unwrap();
        let listed: Vec<&str> = boot.interrupted.iter().map(|s| s.run_id.as_str()).collect();
        assert_eq!(listed, vec![id.as_str()]);
        assert_eq!(run_json(&p, &id)["status"], "interrupted");
        let index: String = std::fs::read_dir(p.t.app_data().join("runs"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|f| {
                f.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("index-")
            })
            .map(|f| std::fs::read_to_string(f).unwrap())
            .collect();
        assert!(
            index.contains(&id) && index.contains("interrupted"),
            "{index}"
        );
        // Listed again on the next bootstrap, never relaunched.
        let boot = app_cmd::app_bootstrap(&again, req(json!({}))).unwrap();
        assert_eq!(boot.interrupted.len(), 1);
        assert_eq!(calls(&p), calls_before, "nothing was spawned");
        // The live session of the first app is still ours to end.
        p.t.app.runs().hangup_all();
    }
}

mod runlog {
    use super::*;

    /// SR-19: the run record is on disk before the spawn — a spawn that fails leaves it,
    /// `errored`.
    #[test]
    fn record_before_spawn() {
        let p = ProjectApp::new(json!({}));
        let pv = ready(&p, "2.2");
        // The binary vanishes between the preview and the start.
        std::fs::remove_file(p.root.join("bin/claude")).unwrap();
        let e = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), channel())
            .unwrap_err();
        assert_eq!(code(&e), IpcErrorCode::ToolFailed, "{e:?}");
        let files = p.run_files();
        assert_eq!(files.len(), 1, "{files:?}");
        assert_eq!(files[0].1["status"], "errored");
        assert_eq!(files[0].1["argv"][1], "--session-id");
    }

    /// SR-52: the record's argv has the prompt replaced by its hash.
    #[test]
    fn argv_prompt_redacted() {
        let p = ProjectApp::new(json!({}));
        let (r, _) = start(&p, "2.2");
        let raw = std::fs::read_to_string(
            p.t.app_data()
                .join("runs")
                .join(r.run_id.as_str())
                .join("run.json"),
        )
        .unwrap();
        assert!(
            !raw.contains("chair of the product design council"),
            "{raw}"
        );
        let rec: Value = serde_json::from_str(&raw).unwrap();
        let sha = rec["promptSha256"].as_str().unwrap();
        assert_eq!(sha.len(), 64);
        assert!(
            rec["argv"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a.as_str().unwrap().contains(sha)),
            "{rec}"
        );
        p.t.app.runs().hangup_all();
    }

    /// SR-52, SR-57: environment names only, never values.
    #[test]
    fn no_env_values() {
        let p = ProjectApp::new(json!({}));
        let (r, _) = start(&p, "2.2");
        let raw = std::fs::read_to_string(
            p.t.app_data()
                .join("runs")
                .join(r.run_id.as_str())
                .join("run.json"),
        )
        .unwrap();
        let rec: Value = serde_json::from_str(&raw).unwrap();
        let names: Vec<&str> = rec["envNames"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(
            names.contains(&"HOME") && names.contains(&"PATH"),
            "{names:?}"
        );
        assert!(names.iter().all(|n| !n.contains('=')));
        let home = p.t.home().display().to_string();
        assert!(
            !raw.contains(&format!("\"{home}\"")),
            "HOME's value leaked: {raw}"
        );
        assert!(!raw.contains("/usr/bin:"), "PATH's value leaked: {raw}");
        p.t.app.runs().hangup_all();
    }
}

mod ipc {
    use super::*;

    /// SR-24a, IPC-R11: every security-relevant confirmation is a native dialog; a scripted
    /// Cancel leaves everything as it was.
    #[test]
    fn security_confirmations_are_native() {
        let p = ProjectApp::new(json!({}));

        // launch_start: Cancel spawns nothing and writes no record.
        let pv = ready(&p, "2.2");
        let before = calls(&p);
        let asked = p.t.os.asked().len();
        p.t.os.answer_next(false);
        let e = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), channel())
            .unwrap_err();
        assert_eq!(code(&e), IpcErrorCode::Cancelled);
        assert_eq!(p.t.os.asked().len(), asked + 1, "a native dialog was shown");
        assert!(p.run_files().is_empty());
        assert_eq!(calls(&p), before);

        // pty_terminate: Cancel sends no signal.
        let (r, buf) = start(&p, "2.2");
        p.t.os.answer_next(false);
        let e = terminal::pty_terminate(&p.t.app, req(json!({ "runId": r.run_id }))).unwrap_err();
        assert_eq!(code(&e), IpcErrorCode::Cancelled);
        terminal::pty_write(
            &p.t.app,
            req(json!({ "runId": r.run_id, "data": "still\r" })),
        )
        .unwrap();
        wait_for(&buf, "ECHO: still");

        // rearm_start: the idle assertion is native; Cancel sends nothing to the old session.
        let pv = ready(&p, "2.3");
        assert!(pv.rearm.is_some(), "2.3 continues the live 2.2 chain");
        let e = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), channel())
            .unwrap_err();
        assert_eq!(code(&e), IpcErrorCode::NotAllowed, "{e:?}");
        let pv = ready(&p, "2.3");
        p.t.os.answer_next(false);
        let e = terminal::rearm_start(
            &p.t.app,
            req(json!({ "planId": pv.plan_id })),
            channel(),
            channel(),
        )
        .unwrap_err();
        assert_eq!(code(&e), IpcErrorCode::Cancelled, "{e:?}");
        let live = p.t.app.runs().get(r.run_id.as_str()).unwrap();
        assert!(live.session.is_live());
        assert!(!String::from_utf8_lossy(&buf.lock().unwrap()).contains("/exit"));

        // Terminate confirmed: the session ends.
        p.t.os.answer_next(true);
        terminal::pty_terminate(&p.t.app, req(json!({ "runId": r.run_id }))).unwrap();
        assert!(live.session.wait_exit(Duration::from_secs(10)).is_some());

        // project_remove: Cancel keeps the registry entry.
        p.t.os.answer_next(false);
        let e = projects::project_remove(&p.t.app, req(json!({ "projectId": p.id }))).unwrap_err();
        assert_eq!(code(&e), IpcErrorCode::Cancelled);
        assert_eq!(p.t.app.registry().unwrap().0.projects.len(), 1);
    }

    /// SR-06, ADR-021: `config_ack` is a native confirmation; Cancel leaves the change pending.
    #[test]
    fn config_ack_cancel_keeps_blocking() {
        let p = ProjectApp::new(json!({}));
        let first = launch::launch_prepare(
            &p.t.app,
            req(
                json!({ "projectId": p.id, "step": "2.2", "entry": "stageView", "mode": "normal" }),
            ),
            channel(),
        )
        .unwrap();
        let PrepareOutcome::Blocked { blockers, .. } = first else {
            panic!("the first scan needs an acknowledgement")
        };
        let b = serde_json::to_value(&blockers[0]).unwrap();
        assert_eq!(b["kind"], "configChanged", "{b}");
        p.t.os.answer_next(false);
        let e = launch::config_ack(
            &p.t.app,
            req(json!({ "projectId": p.id, "cwd": b["cwd"], "fingerprint": b["fingerprint"] })),
        )
        .unwrap_err();
        assert_eq!(code(&e), IpcErrorCode::Cancelled);
        let again = launch::launch_prepare(
            &p.t.app,
            req(
                json!({ "projectId": p.id, "step": "2.2", "entry": "stageView", "mode": "normal" }),
            ),
            channel(),
        )
        .unwrap();
        assert!(matches!(again, PrepareOutcome::Blocked { .. }), "{again:?}");
        // A stale fingerprint is refused without a dialog.
        let asked = p.t.os.asked().len();
        let e = launch::config_ack(
            &p.t.app,
            req(json!({ "projectId": p.id, "cwd": b["cwd"], "fingerprint": "ab".repeat(32) })),
        )
        .unwrap_err();
        assert_eq!(code(&e), IpcErrorCode::Stale);
        assert_eq!(p.t.os.asked().len(), asked);
    }

    /// IPC-R9: a read-only project refuses every mutation with `ReadOnly`.
    #[test]
    fn read_only_refuses_mutation() {
        let p = ProjectApp::new(json!({}));
        // The stored blueprint text no longer matches the lock (AC-F002-05).
        let templates = p.t.app_data().join("templates");
        for e in std::fs::read_dir(&templates).unwrap() {
            let f = e.unwrap().path();
            if f.extension().is_some_and(|x| x == "md") {
                std::fs::write(&f, "tampered").unwrap();
            }
        }
        let refused = |r: Result<Value, IpcError>| {
            let e = r.unwrap_err();
            assert_eq!(code(&e), IpcErrorCode::ReadOnly, "{e:?}");
            assert!(
                matches!(e.detail, Some(IpcErrorDetail::ReadOnly(_))),
                "{e:?}"
            );
        };
        let id = p.id.clone();
        refused(
            steps::step_mark_done(&p.t.app, req(json!({ "projectId": id, "step": "2.4b" })))
                .map(|v| serde_json::to_value(v).unwrap()),
        );
        refused(
            projects::variable_set(
                &p.t.app,
                req(json!({ "projectId": id, "name": "QUALITY_BAR", "value": "Production-ready" })),
            )
            .map(|v| serde_json::to_value(v).unwrap()),
        );
        refused(
            projects::condition_set(
                &p.t.app,
                req(
                    json!({ "projectId": id, "condition": "payments", "to": "no", "note": "free" }),
                ),
            )
            .map(|v| serde_json::to_value(v).unwrap()),
        );
        match launch::launch_prepare(
            &p.t.app,
            req(json!({ "projectId": id, "step": "2.2", "entry": "stageView", "mode": "normal" })),
            channel(),
        ) {
            Err(e) => assert_eq!(code(&e), IpcErrorCode::ReadOnly, "{e:?}"),
            Ok(PrepareOutcome::Blocked { blockers, .. }) => assert!(
                blockers
                    .iter()
                    .any(|b| serde_json::to_value(b).unwrap()["kind"] == "readOnly"),
                "{blockers:?}"
            ),
            Ok(other) => panic!("a read-only project launched: {other:?}"),
        }
        assert!(p.run_files().is_empty());
    }
}

mod pty {
    use super::*;

    /// SR-24: keystrokes only into a live session; an unknown run is `NotFound`.
    #[test]
    fn write_requires_live_run() {
        let p = ProjectApp::new(json!({}));
        let (r, buf) = start(&p, "2.2");
        terminal::pty_write(
            &p.t.app,
            req(json!({ "runId": r.run_id, "data": "hello\r" })),
        )
        .unwrap();
        wait_for(&buf, "ECHO: hello");
        // Attach replays what was printed.
        let (ch, replay) = collect();
        let info = terminal::pty_attach(&p.t.app, req(json!({ "runId": r.run_id })), ch).unwrap();
        assert!(String::from_utf8_lossy(&replay.lock().unwrap()).contains("ECHO: hello"));
        assert_eq!(info.tab_name, "fm-tradelog-s02-2-2");
        terminal::pty_resize(
            &p.t.app,
            req(json!({ "runId": r.run_id, "cols": 100, "rows": 30 })),
        )
        .unwrap();
        terminal::pty_write(
            &p.t.app,
            req(json!({ "runId": r.run_id, "data": "/exit\r" })),
        )
        .unwrap();
        let live = p.t.app.runs().get(r.run_id.as_str()).unwrap();
        assert_eq!(
            live.session
                .wait_exit(Duration::from_secs(10))
                .unwrap()
                .code,
            Some(0)
        );
        let e = terminal::pty_write(&p.t.app, req(json!({ "runId": r.run_id, "data": "x" })))
            .unwrap_err();
        assert_eq!(code(&e), IpcErrorCode::NotAllowed);
        assert!(
            matches!(&e.detail, Some(IpcErrorDetail::NotAllowed { rule }) if rule == "pty-not-live"),
            "{e:?}"
        );
        assert_eq!(run_json(&p, r.run_id.as_str())["status"], "exited");
        let e = terminal::pty_write(
            &p.t.app,
            req(json!({ "runId": "a0000000-0000-4000-8000-000000000023", "data": "x" })),
        )
        .unwrap_err();
        assert_eq!(code(&e), IpcErrorCode::NotFound);
    }

    /// SR-28: a paste comes back without control characters, with the count removed.
    #[test]
    fn paste_sanitized() {
        let p = ProjectApp::new(json!({}));
        let (r, _) = start(&p, "2.2");
        p.t.os
            .set_clipboard(Some("\u{1b}[201~\u{3}!touch /tmp/pwn\r"));
        let t = terminal::pty_paste(&p.t.app, req(json!({ "runId": r.run_id }))).unwrap();
        assert!(
            !t.text.contains('\u{1b}') && !t.text.contains('\u{3}'),
            "{t:?}"
        );
        assert_eq!(t.removed_controls, 2);
        p.t.app.runs().hangup_all();
    }
}

mod watcher {
    use super::*;

    /// SR-06, ADR-021 §7: a fingerprinted file that changes under a live run marks the run and
    /// emits `config:changed` once; the run keeps going.
    #[test]
    fn config_changed_marks_open_runs() {
        let p = ProjectApp::new(json!({}));
        let (r, _) = start(&p, "2.2");
        assert!(
            foreman_app::watch::recheck_live_runs(&p.t.app).is_empty(),
            "nothing changed yet"
        );
        std::fs::write(p.project.join("CLAUDE.md"), "# changed under the run\n").unwrap();
        let marked = foreman_app::watch::recheck_live_runs(&p.t.app);
        assert_eq!(marked, vec![r.run_id.as_str().to_owned()]);
        let live = p.t.app.runs().get(r.run_id.as_str()).unwrap();
        assert!(
            live.config_changed
                .load(std::sync::atomic::Ordering::SeqCst)
        );
        assert!(live.session.is_live(), "a warning only");
        let events: Vec<Value> =
            p.t.os
                .events()
                .iter()
                .filter(|e| e.name() == "config:changed")
                .map(|e| e.payload())
                .collect();
        assert_eq!(events, vec![json!({ "projectId": p.id, "cwd": "spec" })]);
        // Once per run.
        assert!(foreman_app::watch::recheck_live_runs(&p.t.app).is_empty());
        p.t.app.runs().hangup_all();
    }
}

mod rearm {
    use super::*;
    use foreman_app::ipc::dto::launch::RearmOutcome;

    /// F-010, DR-116: after Billy's native idle assertion, the old session gets Ctrl-C and
    /// `/exit`, the step is prepared again and relaunched in the same tab Channel, after a
    /// divider; the old run is recorded as exited.
    #[test]
    fn relaunches_after_idle_confirm() {
        let p = ProjectApp::new(json!({}));
        let (old, _) = start(&p, "2.2");
        let pv = ready(&p, "2.3");
        assert!(pv.rearm.is_some());
        let asked = p.t.os.asked().len();
        let (out, buf) = collect();
        let r = terminal::rearm_start(
            &p.t.app,
            req(json!({ "planId": pv.plan_id })),
            out,
            channel(),
        )
        .unwrap();
        assert!(
            p.t.os.asked().len() > asked,
            "the idle assertion was a native dialog"
        );
        let RearmOutcome::Relaunched(new) = r else {
            panic!("{r:?}")
        };
        assert_ne!(new.run_id, old.run_id);
        assert_eq!(new.chain, old.chain, "the same chain continues");
        wait_for(&buf, "FAKE-CLAUDE READY");
        let prev = p.t.app.runs().get(old.run_id.as_str()).unwrap();
        assert!(!prev.session.is_live());
        assert!(
            p.t.app
                .runs()
                .get(new.run_id.as_str())
                .unwrap()
                .session
                .is_live()
        );
        assert_eq!(run_json(&p, new.run_id.as_str())["kind"], "rearm");
        // A continue with nothing to wait for is refused.
        let e = terminal::rearm_continue(
            &p.t.app,
            req(json!({ "chain": new.chain })),
            channel(),
            channel(),
        )
        .unwrap_err();
        assert_eq!(code(&e), IpcErrorCode::Expired);
        p.t.app.runs().hangup_all();
    }
}
