//! M2-25: the launch plan (ADR-021, IPC-R7, AC-F008-18, AC-F008-03). A `planId` is single-use,
//! valid for 60 s and bound to the project revision; `launch_start` takes only the `planId`;
//! `freshWithContext` is refused on a step that starts its own session.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::project::{ProjectApp, channel, collect, req};
use foreman_app::ipc::IpcErrorCode;
use foreman_app::ipc::commands::{decisions as dec, launch, projects};
use foreman_app::ipc::dto::launch::{LaunchPreview, PrepareOutcome};
use foreman_app::ipc::error::{IpcErrorDetail, StaleWhat};
use foreman_app::state::{OneTime, PLAN_TTL};
use serde_json::{Value, json};

fn ready(p: &ProjectApp, step: &str) -> LaunchPreview {
    match p.prepare(step) {
        PrepareOutcome::Ready(pv) => pv,
        PrepareOutcome::Blocked { blockers, .. } => panic!("blocked: {blockers:?}"),
    }
}

mod launch_plan {
    use super::*;

    /// The plan is single-use: the second start with the same ID is `Expired`, and only one
    /// session was spawned.
    #[test]
    fn plan_id_is_single_use() {
        let p = ProjectApp::new(json!({}));
        p.t.os.answer_all(true);
        let pv = ready(&p, "2.2");
        let (ch, _buf) = collect();
        launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
        let e = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), channel())
            .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Expired, "{e:?}");
        assert_eq!(p.run_files().len(), 1, "one run record");
    }

    /// A plan is good for 60 s: the constant, the holder that enforces it, and its use for plans.
    #[test]
    fn plan_id_is_valid_for_60_seconds() {
        assert_eq!(PLAN_TTL, Duration::from_secs(60));
        let mut held = OneTime::<u8>::default();
        let id = held.hold(1, Duration::from_millis(30));
        assert_eq!(held.peek(&id).unwrap(), &1);
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(held.take(&id).unwrap_err().code, IpcErrorCode::Expired);
        let src = include_str!("../src/launch.rs");
        assert!(
            src.contains("plans.hold(held, PLAN_TTL)"),
            "plans are held for PLAN_TTL"
        );
    }

    /// The plan is bound to the project revision: any change after the review makes the start
    /// `Stale` and spawns nothing.
    #[test]
    fn plan_id_is_bound_to_the_project_revision() {
        let p = ProjectApp::new(json!({}));
        p.t.os.answer_all(true);
        common::project::git_identity(&p);
        let pv = ready(&p, "2.2");
        projects::variable_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "name": "QUALITY_BAR", "value": "Production-ready" })),
        )
        .unwrap();
        let e = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), channel())
            .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Stale, "{e:?}");
        assert!(p.run_files().is_empty(), "nothing spawned, no record");
    }

    /// `freshWithContext` on a step that starts its own session is `NotAllowed`
    /// ("fresh-not-continuing"); on a continuing step it is accepted.
    #[test]
    fn fresh_on_a_non_continuing_step_is_refused() {
        let p = ProjectApp::new(json!({}));
        let e = launch::launch_prepare(
            &p.t.app,
            req(json!({
                "projectId": p.id, "step": "2.2", "entry": "stageView", "mode": "freshWithContext"
            })),
            channel(),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed, "{e:?}");
        assert!(format!("{e:?}").contains("fresh-not-continuing"), "{e:?}");
    }
}

/// `claude` sessions started (a session runs with `--settings`; `-v` and `auth status` don't).
fn calls(p: &ProjectApp) -> usize {
    std::fs::read_to_string(format!("{}.calls.jsonl", p.record.display()))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains("--settings"))
        .count()
}

/// Runs `f` while the next native dialog is open; returns how many dialogs it ran in.
fn during_dialog(p: &ProjectApp, f: impl Fn() + Send + Sync + 'static) -> Arc<Mutex<usize>> {
    let n = Arc::new(Mutex::new(0));
    let m = Arc::clone(&n);
    p.t.os.while_dialog_open(Some(Box::new(move |_| {
        *m.lock().unwrap() += 1;
        f();
    })));
    n
}

/// IPC-R7, the contract's names (docs/contracts/ipc.md §1).
mod ipc {
    use super::*;

    /// A two-phase preview is taken once: the second apply of the same ID is `Expired`.
    #[test]
    fn preview_single_use() {
        let p = ProjectApp::new(json!({}));
        let pv =
            dec::progress_rebuild_preview(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        dec::progress_rebuild_apply(&p.t.app, req(json!({ "previewId": pv.preview_id }))).unwrap();
        let e = dec::progress_rebuild_apply(&p.t.app, req(json!({ "previewId": pv.preview_id })))
            .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Expired, "{e:?}");
    }

    /// A launch plan lives 60 s; one held past its time is `Expired` and starts nothing.
    #[test]
    fn plan_expires_60s() {
        assert_eq!(PLAN_TTL, Duration::from_secs(60));
        let p = ProjectApp::new(json!({}));
        let pv = ready(&p, "2.2");
        let held = p.t.app.launch().plans.take(pv.plan_id.as_str()).unwrap();
        let short = p.t.app.launch().plans.hold(held, Duration::ZERO);
        let e =
            launch::launch_start(&p.t.app, req(json!({ "planId": short })), channel()).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Expired, "{e:?}");
        assert!(p.run_files().is_empty(), "nothing started");
    }

    /// Code review C2: the plan is checked again after the native launch dialog, under the write
    /// lock. A `progress.json` rewritten while the dialog was open (another session's exit, any
    /// command) is `Stale` with a fresh preview, and nothing spawns: no run record, no `claude`,
    /// no session without a tab.
    #[test]
    fn stale_plan_rejected() {
        let p = ProjectApp::new(json!({}));
        common::project::git_identity(&p);
        let pv = ready(&p, "2.2");
        assert!(!pv.confirm.is_empty(), "the first launch asks");
        let app = Arc::clone(&p.t.app);
        let id = p.id.clone();
        let asked = during_dialog(&p, move || {
            projects::variable_set(
                &app,
                req(json!({ "projectId": id, "name": "QUALITY_BAR", "value": "Production-ready" })),
            )
            .unwrap();
        });
        let before = calls(&p);
        let e = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), channel())
            .unwrap_err();
        p.t.os.while_dialog_open(None);
        assert_eq!(*asked.lock().unwrap(), 1);
        assert_eq!(e.code, IpcErrorCode::Stale, "{e:?}");
        assert!(
            matches!(
                &e.detail,
                Some(IpcErrorDetail::Stale {
                    what: StaleWhat::ProjectRev,
                    fresh: Some(_)
                })
            ),
            "{e:?}"
        );
        assert!(p.run_files().is_empty(), "no run record");
        assert_eq!(calls(&p), before, "claude never ran");
        assert!(p.t.app.runs().live().is_empty(), "no hidden session");
    }

    /// Code review C2, IPC-R7: a configuration change while the launch dialog is open (a hook
    /// added to `.claude/settings.local.json`) is `Stale` for the fingerprint, so the launch
    /// can't skip the configuration acknowledgement (SR-06).
    #[test]
    fn stale_fingerprint_rejected() {
        let p = ProjectApp::new(json!({}));
        let pv = ready(&p, "2.2");
        let dir = p.project.join(".claude");
        let asked = during_dialog(&p, move || {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("settings.local.json"),
                r#"{ "hooks": { "SessionStart": [] } }"#,
            )
            .unwrap();
        });
        let e = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), channel())
            .unwrap_err();
        p.t.os.while_dialog_open(None);
        assert_eq!(*asked.lock().unwrap(), 1);
        assert!(
            matches!(
                &e.detail,
                Some(IpcErrorDetail::Stale {
                    what: StaleWhat::Fingerprint,
                    ..
                })
            ),
            "{e:?}"
        );
        assert!(p.run_files().is_empty(), "nothing spawned");
        // The fresh preparation asks for the acknowledgement.
        let out = launch::launch_prepare(
            &p.t.app,
            req(
                json!({ "projectId": p.id, "step": "2.2", "entry": "stageView", "mode": "normal" }),
            ),
            channel(),
        )
        .unwrap();
        let v: Value = serde_json::to_value(&out).unwrap();
        assert!(v.to_string().contains("configChanged"), "{v}");
    }
}

/// Security review N-1, folded into code review C2: the sensor's hash is checked again right
/// before the run settings are written. A sensor that no longer matches (and can't be
/// re-installed) gives the run no hooks and no token.
mod sensor_recheck {
    use super::*;

    #[test]
    fn sensor_rechecked_at_spawn() {
        let p = ProjectApp::new(json!({}));
        common::sensor::enable(&p.t.app);
        let pv = ready(&p, "2.2");
        let held =
            p.t.app
                .launch()
                .plans
                .peek(pv.plan_id.as_str())
                .unwrap()
                .clone();
        assert!(held.plan.sensor_command().is_some(), "prepared with hooks");
        // The installed binary goes and `bin/` can't take a new one.
        let bin = p.t.app_data().join("bin");
        std::fs::remove_dir_all(&bin).unwrap();
        std::fs::write(&bin, "not a folder").unwrap();
        let (ch, _buf) = collect();
        let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
        let rs = std::fs::read_to_string(
            p.t.app_data()
                .join(format!("runs/{}/settings.json", r.run_id.as_str())),
        )
        .unwrap();
        assert!(!rs.contains("hooks"), "{rs}");
        let rj: Value = serde_json::from_str(
            &std::fs::read_to_string(
                p.t.app_data()
                    .join(format!("runs/{}/run.json", r.run_id.as_str())),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(rj["sensorToken"].is_null(), "{rj}");
        assert_eq!(
            rj["runSettingsSha256"],
            json!(foreman_core::store::sha256_hex(rs.as_bytes()))
        );
        assert!(p.t.app.sensor().token(r.run_id.as_str()).is_none());
        p.t.app.runs().hangup_all();
    }
}
