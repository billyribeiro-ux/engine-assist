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

use std::time::Duration;

use common::project::{ProjectApp, channel, collect, req};
use foreman_app::ipc::IpcErrorCode;
use foreman_app::ipc::commands::{launch, projects};
use foreman_app::ipc::dto::launch::{LaunchPreview, PrepareOutcome};
use foreman_app::state::{OneTime, PLAN_TTL};
use serde_json::json;

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
