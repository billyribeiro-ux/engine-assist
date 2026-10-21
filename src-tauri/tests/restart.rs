//! M3-16: restart, interrupted sessions and rebuild from the repo (AC-F010-12, AC-F020-10,
//! AC-F009-05, AC-F020-11, AC-F020-12; NFR-11, NFR-17; T-059).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::project::{ProjectApp, channel, collect, req};
use foreman_app::ipc::commands::{
    app as app_cmd, decisions as dec, first_run, launch, packets, projects,
};
use foreman_app::ipc::dto::launch::PrepareOutcome;
use foreman_app::ipc::error::IpcErrorCode;
use foreman_app::os::ScriptedOs;
use foreman_app::state::{App, Paths};
use foreman_core::schema::settings::ToolSource;
use serde_json::{Value, json};

fn restart(p: &ProjectApp) -> (Arc<App>, Arc<ScriptedOs>) {
    let os = Arc::new(ScriptedOs::default());
    let app = App::new(
        os.clone(),
        Paths {
            app_data: p.t.app_data(),
            home: p.t.home(),
        },
    );
    (app, os)
}

fn open(app: &App, id: &str) -> Value {
    serde_json::to_value(projects::project_open(app, req(json!({ "projectId": id }))).unwrap())
        .unwrap()
}

fn calls(p: &ProjectApp) -> usize {
    let mut path = p.record.as_os_str().to_owned();
    path.push(".calls.jsonl");
    std::fs::read_to_string(PathBuf::from(path))
        .unwrap_or_default()
        .lines()
        .count()
}

fn progress(p: &ProjectApp) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(
            p.t.app_data()
                .join(format!("projects/{}/progress.json", p.id)),
        )
        .unwrap(),
    )
    .unwrap()
}

fn stage(v: &Value, n: u64) -> Value {
    v["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["stage"] == n)
        .unwrap()
        .clone()
}

fn done_steps(v: &Value, n: u64) -> Vec<String> {
    stage(v, n)["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["state"] == "done")
        .map(|s| s["key"].as_str().unwrap().to_owned())
        .collect()
}

fn signed_packet() -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../crates/foreman-core/tests/fixtures/tradelog/handoff/stage-01.md"),
    )
    .unwrap()
}

/// A project with Stage 1's packet written by hand and Billy's signature confirmed.
fn with_signed_stage_1() -> ProjectApp {
    let p = ProjectApp::new(json!({}));
    std::fs::write(p.project.join("docs/handoff/stage-01.md"), signed_packet()).unwrap();
    let v = packets::packet_view(&p.t.app, req(json!({ "projectId": p.id, "stage": 1 }))).unwrap();
    packets::packet_signature_confirm(
        &p.t.app,
        req(json!({ "projectId": p.id, "stage": 1, "seenSha256": v.sha256.unwrap() })),
    )
    .unwrap();
    p
}

mod lifecycle {
    use super::*;

    /// AC-F010-12, AC-F020-10, AC-F009-05, SR-57 (restart case): Foreman dies with a live
    /// fake-claude; after the restart each chain's status and steps are back, the session is
    /// listed as interrupted and resumable, nothing is relaunched by itself, and Resume goes
    /// through `chain_resume_prepare` (the full preflight, `--resume` of the same chain) and then
    /// the normal `launch_start` with its native confirmation.
    #[test]
    fn crash_leaves_interrupted() {
        let p = ProjectApp::new(json!({}));
        let pv = match p.prepare("2.2") {
            PrepareOutcome::Ready(pv) => pv,
            PrepareOutcome::Blocked { blockers, .. } => panic!("{blockers:?}"),
        };
        let (ch, buf) = collect();
        let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
        let t0 = Instant::now();
        while !String::from_utf8_lossy(&buf.lock().unwrap()).contains("FAKE-CLAUDE READY") {
            assert!(t0.elapsed() < Duration::from_secs(20));
            std::thread::sleep(Duration::from_millis(20));
        }
        let chain = r.chain.as_str().to_owned();
        let before = progress(&p)["chains"].clone();
        // The crash: a new process over the same app data.
        let (again, os) = restart(&p);
        let calls_before = calls(&p);
        let runs_before = p.run_files().len();
        let boot = app_cmd::app_bootstrap(&again, req(json!({}))).unwrap();
        let listed: Vec<Value> = boot
            .interrupted
            .iter()
            .map(|s| serde_json::to_value(s).unwrap())
            .collect();
        assert_eq!(listed.len(), 1, "{listed:?}");
        assert_eq!(listed[0]["chain"], json!(chain));
        assert_eq!(listed[0]["status"], "interrupted");
        assert_eq!(listed[0]["resumable"], true, "{listed:?}");
        // Chains restored exactly (AC-F010-12).
        let v = open(&again, &p.id);
        assert_eq!(progress(&p)["chains"], before);
        let c = before
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == json!(chain))
            .unwrap()
            .clone();
        assert_eq!(c["status"], "open");
        assert_eq!(c["steps"], json!(["2.2"]));
        assert!(v["stages"].is_array());
        // C3 (DR-065): the interrupted run's step is back to "run again", not stuck running.
        let s22 = stage(&v, 2)["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["key"] == "2.2")
            .unwrap()
            .clone();
        assert_eq!(s22["state"], "notStarted", "{s22}");
        assert_eq!(calls(&p), calls_before, "nothing relaunched by itself");
        assert_eq!(p.run_files().len(), runs_before);
        // Resume: the preflight again, then launch_start's native confirmation.
        let out = launch::chain_resume_prepare(&again, req(json!({ "chain": chain })), channel())
            .unwrap();
        let PrepareOutcome::Ready(rv) = out else {
            panic!("{out:?}")
        };
        let argv = serde_json::to_value(&rv).unwrap().to_string();
        assert!(argv.contains("--resume") && argv.contains(&chain), "{argv}");
        assert_eq!(
            p.run_files().len(),
            runs_before,
            "preparing starts no session"
        );
        // launch_start's confirmation (SR-19) asks only when the plan has a reason to; either
        // way the resumed run goes through the same command.
        let asked = os.asked().len();
        let (ch, buf) = collect();
        let r2 = launch::launch_start(&again, req(json!({ "planId": rv.plan_id })), ch).unwrap();
        assert_eq!(
            os.asked().len() > asked,
            !rv.confirm.is_empty(),
            "a native confirmation exactly when the plan has reasons"
        );
        assert_eq!(r2.chain.as_str(), chain, "the same chain continues");
        let t0 = Instant::now();
        while !String::from_utf8_lossy(&buf.lock().unwrap()).contains("FAKE-CLAUDE READY") {
            assert!(t0.elapsed() < Duration::from_secs(20));
            std::thread::sleep(Duration::from_millis(20));
        }
        again.runs().hangup_all();
        p.t.app.runs().hangup_all();
    }
}

mod quit {
    use super::*;

    /// C3: after Billy confirms the quit, the hung-up sessions' exit handlers finish before the
    /// app may exit: each run's record and its step are final when the guard returns.
    #[test]
    fn guard_waits_for_exit_handlers() {
        let p = ProjectApp::new(json!({}));
        let pv = match p.prepare("2.2") {
            PrepareOutcome::Ready(pv) => pv,
            PrepareOutcome::Blocked { blockers, .. } => panic!("{blockers:?}"),
        };
        let (ch, buf) = collect();
        let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
        let t0 = Instant::now();
        while !String::from_utf8_lossy(&buf.lock().unwrap()).contains("FAKE-CLAUDE READY") {
            assert!(t0.elapsed() < Duration::from_secs(20));
            std::thread::sleep(Duration::from_millis(20));
        }
        p.t.os.answer_all(true);
        assert!(foreman_app::quit::guard(&p.t.app));
        assert!(
            p.t.app.runs().exit_handled(r.run_id.as_str()),
            "exit handler done"
        );
        let run = p
            .run_files()
            .into_iter()
            .find(|(_, v)| v["runId"] == json!(r.run_id.as_str()))
            .unwrap()
            .1;
        assert_ne!(run["status"], "running", "{run}");
        let pr = progress(&p);
        assert_ne!(pr["steps"]["2.2"]["state"], "running", "{pr}");
    }
}

/// Code review W9: a rebuild on open continues the revision this run has seen.
mod revision {
    use super::*;

    #[test]
    fn auto_rebuild_keeps_the_revision_moving() {
        let p = with_signed_stage_1();
        let v = open(&p.t.app, &p.id);
        let seen = v["summary"]["rev"].as_u64().unwrap();
        assert!(seen > 0);
        std::fs::remove_file(
            p.t.app_data()
                .join(format!("projects/{}/progress.json", p.id)),
        )
        .unwrap();
        let v = open(&p.t.app, &p.id);
        assert!(
            v["summary"]["rev"].as_u64().unwrap() > seen,
            "the rebuilt revision is newer than {seen}: {}",
            v["summary"]["rev"]
        );
        assert!(progress(&p)["rev"].as_u64().unwrap() > seen);
    }
}

mod rebuild {
    use super::*;

    /// AC-F020-11, NFR-17: with the project's progress and the run log gone, opening the project
    /// rebuilds stage and step status from the repo's packets within a minute; only in-progress
    /// markers, chains and the run log are lost.
    #[test]
    fn app_data_deleted_rebuilds_from_packets() {
        let p = with_signed_stage_1();
        let v = open(&p.t.app, &p.id);
        assert!(stage(&v, 1)["signed"].is_object());
        std::fs::remove_dir_all(p.t.app_data().join(format!("projects/{}", p.id))).unwrap();
        let _ = std::fs::remove_dir_all(p.t.app_data().join("runs"));
        let t0 = Instant::now();
        let (again, _) = restart(&p);
        let v = open(&again, &p.id);
        assert!(t0.elapsed() < Duration::from_secs(60));
        assert!(
            stage(&v, 1)["signed"].is_object(),
            "the confirmed signature stands"
        );
        assert_eq!(v["summary"]["currentStage"], 2);
        let done = done_steps(&v, 1);
        assert!(!done.is_empty(), "stage 1's steps are done: {v}");
        assert!(done_steps(&v, 2).is_empty());
        let pr = progress(&p);
        assert_eq!(pr["chains"], json!([]));
        assert!(pr["steps"].as_object().unwrap().len() >= done.len());
    }

    /// AC-F020-11 with all of app data gone: the folder is registered again as it is (AC-F018-07)
    /// and its packets give back the step that wrote each one. A hand signature needs Billy's
    /// confirmation again, because that confirmation lived in app data (DR-135, SR-47).
    #[test]
    fn all_app_data_deleted() {
        let p = with_signed_stage_1();
        std::fs::remove_dir_all(p.t.app_data()).unwrap();
        let (again, os) = restart(&p);
        // First run again: settings, the blueprint repo, then the existing folder.
        let (mut s, stamp, _) = again.settings().unwrap();
        s.tools.claude.path = Some(p.root.join("bin/claude").display().to_string());
        s.tools.claude.source = ToolSource::Settings;
        again.save_settings(&s, &stamp).unwrap();
        os.pick_next(Some(p.root.join("blueprint")));
        first_run::blueprint_pick_repo(&again, req(json!({ "mode": "choose" }))).unwrap();
        os.pick_next(Some(p.project.clone()));
        let pick = projects::setup_pick_folder(&again, req(json!({}))).unwrap();
        let preview = projects::setup_preview(
            &again,
            req(json!({
                "pickId": pick.pick_id,
                "name": "TradeLog",
                "slug": "tradelog",
                "variables": {},
                "conditions": {},
                "chatProjectUrl": null
            })),
        )
        .unwrap();
        let t0 = Instant::now();
        let summary =
            projects::setup_apply(&again, req(json!({ "previewId": preview.preview_id }))).unwrap();
        assert_eq!(summary.id.as_str(), p.id, "the same project");
        let v = open(&again, &p.id);
        assert!(t0.elapsed() < Duration::from_secs(60));
        assert!(
            !done_steps(&v, 1).is_empty(),
            "the packet's step is done: {v}"
        );
        assert!(stage(&v, 1)["signed"].is_null());
        let pk =
            packets::packet_view(&again, req(json!({ "projectId": p.id, "stage": 1 }))).unwrap();
        assert!(!pk.signed.unwrap().confirmed, "asks again");
    }

    /// AC-F020-12, T-059: a corrupt `progress.json` is a clear error naming it; the file is kept;
    /// the preview writes nothing; Rebuild from repo writes the rebuilt state only on apply, keeps
    /// the corrupt file beside it, and a preview is single-use.
    #[test]
    fn corrupt_progress_kept_until_confirmed() {
        let p = with_signed_stage_1();
        let file =
            p.t.app_data()
                .join(format!("projects/{}/progress.json", p.id));
        std::fs::write(&file, "{ not json").unwrap();
        let (again, _) = restart(&p);
        let e = projects::project_open(&again, req(json!({ "projectId": p.id }))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Io);
        assert!(
            e.message.contains("progress.json") && e.message.contains("unreadable"),
            "{}",
            e.message
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{ not json");
        let pv = dec::progress_rebuild_preview(&again, req(json!({ "projectId": p.id }))).unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "{ not json",
            "preview writes nothing"
        );
        assert!(
            pv.lost.iter().any(|l| l.contains("corrupt")),
            "{:?}",
            pv.lost
        );
        let pvj = serde_json::to_value(&pv).unwrap();
        assert!(stage(&pvj, 1)["signed"].is_object());
        let view = dec::progress_rebuild_apply(&again, req(json!({ "previewId": pv.preview_id })))
            .unwrap();
        assert!(
            view.stages
                .iter()
                .any(|s| s.stage.0 == 1 && s.signed.is_some())
        );
        let kept: Vec<String> = std::fs::read_dir(file.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("progress.json.corrupt-"))
            .collect();
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert_eq!(
            std::fs::read_to_string(file.parent().unwrap().join(&kept[0])).unwrap(),
            "{ not json"
        );
        assert!(progress(&p)["schemaVersion"] == 1);
        open(&again, &p.id);
        let e = dec::progress_rebuild_apply(&again, req(json!({ "previewId": pv.preview_id })))
            .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Expired);
        // A file changed between preview and apply is a conflict, not overwritten.
        let pv = dec::progress_rebuild_preview(&again, req(json!({ "projectId": p.id }))).unwrap();
        std::fs::write(&file, "{ changed").unwrap();
        let e = dec::progress_rebuild_apply(&again, req(json!({ "previewId": pv.preview_id })))
            .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Conflict);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{ changed");
    }
}
