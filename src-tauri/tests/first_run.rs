//! M2-16: first run (AC-F001-18, AC-F019-01, AC-F019-03; SR-31a, PR-08). A blueprint repo that is
//! gone sends Billy to first run to choose it again, while existing projects keep opening from
//! their pinned template; auto-detection looks only at the fixed candidates.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::TestApp;
use common::project::{ProjectApp, req};
use foreman_app::ipc::IpcErrorCode;
use foreman_app::ipc::commands::first_run as cmd;
use foreman_app::ipc::commands::{app, projects};
use serde_json::{Value, json};

fn boot(t: &TestApp) -> Value {
    serde_json::to_value(app::app_bootstrap(&t.app, req(json!({}))).unwrap()).unwrap()
}

mod first_run {
    use super::*;

    /// AC-F001-18: the repo goes missing; the next start lands on first run, and the project
    /// still opens from the template pinned in app data. With that gone too it opens read-only
    /// and says where the blueprint should be (AC-F002-06).
    #[test]
    fn missing_blueprint_repo_returns_to_first_run() {
        let p = ProjectApp::new(json!({}));
        assert_eq!(boot(&p.t)["firstRun"], json!(false), "set up: no first run");
        std::fs::rename(p.root.join("blueprint"), p.root.join("blueprint-moved")).unwrap();
        let b = boot(&p.t);
        assert_eq!(b["firstRun"], json!(true), "choose the repo again");
        assert_eq!(
            b["projects"].as_array().unwrap().len(),
            1,
            "the project is still listed"
        );
        // Opens from the cached lock: the template is in app data, verified by its hash.
        let view = serde_json::to_value(
            projects::project_open(&p.t.app, req(json!({ "projectId": p.id }))).unwrap(),
        )
        .unwrap();
        assert!(view["readOnly"].is_null(), "{}", view["readOnly"]);
        assert_eq!(view["blueprint"]["version"], json!("2.2"));
        // Without the cached text, only the repo has it: read-only, with the reason.
        for e in std::fs::read_dir(p.t.app_data().join("templates")).unwrap() {
            let f = e.unwrap().path();
            if f.extension().is_some_and(|x| x == "md") {
                std::fs::remove_file(f).unwrap();
            }
        }
        let view = serde_json::to_value(
            projects::project_open(&p.t.app, req(json!({ "projectId": p.id }))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            view["readOnly"]["reason"],
            json!("blueprintMissing"),
            "{view}"
        );
        // Choosing the repo again ends it.
        p.t.os.pick_next(Some(p.root.join("blueprint-moved")));
        cmd::blueprint_pick_repo(&p.t.app, req(json!({ "mode": "choose" }))).unwrap();
        assert_eq!(boot(&p.t)["firstRun"], json!(false));
    }

    /// AC-F019-03, SR-31a, PR-08: auto-detection checks only `~/claude-blueprint` and
    /// `~/Developer/claude-blueprint`; a repo anywhere else is never found, and no folder is
    /// indexed.
    #[test]
    fn auto_detect_checks_only_the_fixed_candidates() {
        let t = TestApp::new();
        let home = t.home();
        // A blueprint repo in a place that is not a candidate.
        let elsewhere = home.join("Projects/claude-blueprint");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(elsewhere.join("claude-build-blueprint.md"), "# x\n").unwrap();
        let e = cmd::blueprint_pick_repo(&t.app, req(json!({ "mode": "autoDetect" }))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotFound, "{e:?}");
        let (s, _, _) = t.app.settings().unwrap();
        assert!(s.blueprint_repo.is_none(), "nothing was stored");
    }
}
