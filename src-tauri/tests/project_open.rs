//! M2-19: `project_open` end to end (AC-F030-03; T-065; ADR-022, ADR-023). The runtime holds the
//! template, the lock check, the approved decisions and the applicable path; a blueprint that no
//! longer matches its lock, or a decision file from a newer Foreman, opens the project read-only
//! with the reason and nothing runnable; `project:changed` carries only `{projectId, rev}`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::project::{ProjectApp, channel, git_identity, req};
use foreman_app::ipc::commands::{app, launch, projects, steps};
use foreman_app::ipc::dto::launch::PrepareOutcome;
use foreman_app::ipc::{IpcError, IpcErrorCode};
use serde_json::{Value, json};

fn open(p: &ProjectApp) -> Value {
    serde_json::to_value(
        projects::project_open(&p.t.app, req(json!({ "projectId": p.id }))).unwrap(),
    )
    .unwrap()
}

/// No step of a read-only project can run: none is the next step, and a launch is blocked.
fn nothing_runs(p: &ProjectApp, why: &str) {
    // Today offers nothing runnable for the project, and says why; Do next finds nothing.
    let today = serde_json::to_value(app::today_get(&p.t.app, req(json!({}))).unwrap()).unwrap();
    let row = &today["rows"][0];
    assert!(row["next"].is_null(), "no next step: {row}");
    assert!(
        row["whyNone"].as_str().unwrap().contains(why),
        "the row says why: {row}"
    );
    let next = serde_json::to_value(app::do_next(&p.t.app, req(json!({}))).unwrap()).unwrap();
    assert_eq!(next["kind"], json!("nothing"), "{next}");
    let launched = launch::launch_prepare(
        &p.t.app,
        req(json!({ "projectId": p.id, "step": "2.2", "entry": "stageView", "mode": "normal" })),
        channel(),
    );
    match launched {
        Err(e) => assert_eq!(e.code, IpcErrorCode::ReadOnly, "{e:?}"),
        Ok(PrepareOutcome::Blocked { blockers, .. }) => assert!(
            blockers
                .iter()
                .any(|b| serde_json::to_value(b).unwrap()["kind"] == "readOnly"),
            "{blockers:?}"
        ),
        Ok(other) => panic!("a read-only project launched: {other:?}"),
    }
    let e: IpcError =
        steps::step_mark_done(&p.t.app, req(json!({ "projectId": p.id, "step": "2.4b" })))
            .unwrap_err();
    assert_eq!(e.code, IpcErrorCode::ReadOnly, "{e:?}");
    assert!(p.run_files().is_empty(), "nothing spawned or recorded");
}

mod lock {
    use super::*;

    /// AC-F002-05 / AC-F030-03 end to end: the cached blueprint text no longer matches the lock.
    #[test]
    fn hash_mismatch_read_only() {
        let p = ProjectApp::new(json!({}));
        assert!(open(&p)["readOnly"].is_null(), "writable before");
        let templates = p.t.app_data().join("templates");
        for e in std::fs::read_dir(&templates).unwrap() {
            let f = e.unwrap().path();
            if f.extension().is_some_and(|x| x == "md") {
                std::fs::write(&f, "tampered").unwrap();
            }
        }
        // The blueprint repo's file is gone too, so there is no good copy to restore from.
        std::fs::remove_file(p.root.join("blueprint/claude-build-blueprint.md")).unwrap();
        let view = open(&p);
        assert_eq!(view["readOnly"]["reason"], json!("lockMismatch"), "{view}");
        assert!(
            !view["readOnly"]["detail"].as_str().unwrap().is_empty(),
            "the banner has its reason"
        );
        assert_eq!(view["summary"]["readOnly"]["reason"], json!("lockMismatch"));
        nothing_runs(&p, view["readOnly"]["detail"].as_str().unwrap());
    }
}

mod schema {
    use super::*;

    /// AC-F018-07 / AC-F030-03 end to end: a decision file written by a newer Foreman.
    #[test]
    fn newer_version_read_only() {
        let p = ProjectApp::new(json!({}));
        let file = p.project.join(".foreman/project.json");
        let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        v["schemaVersion"] = json!(99);
        let newer = serde_json::to_string_pretty(&v).unwrap();
        std::fs::write(&file, &newer).unwrap();
        let view = open(&p);
        assert_eq!(view["readOnly"]["reason"], json!("newerSchema"), "{view}");
        assert!(
            view["readOnly"]["detail"]
                .as_str()
                .unwrap()
                .contains("newer Foreman"),
            "{}",
            view["readOnly"]
        );
        nothing_runs(&p, "newer Foreman");
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            newer,
            "the newer file is never rewritten"
        );
    }
}

mod project_open {
    use super::*;

    /// The runtime holds the pinned template, the lock check, the approved decisions and the
    /// applicable path (AC-F030-03, M2-19).
    #[test]
    fn view_holds_template_lock_decisions_and_path() {
        let p = ProjectApp::new(json!({}));
        let view = open(&p);
        assert!(view["readOnly"].is_null(), "{}", view["readOnly"]);
        // The pinned template and the lock check.
        assert_eq!(view["blueprint"]["version"], json!("2.2"));
        assert!(!view["blueprint"]["commit"].as_str().unwrap().is_empty());
        // The approved decisions (the conditions set at setup).
        assert_eq!(view["conditions"]["idea_known"]["value"], json!("yes"));
        assert_eq!(view["conditions"]["commercial"]["value"], json!("no"));
        // The applicable path: not-applicable steps carry their condition, undecided ones say so.
        let steps: Vec<&Value> = view["stages"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|s| s["steps"].as_array().unwrap())
            .collect();
        assert!(steps.iter().any(|s| s["state"] == "notApplicable"));
        assert!(steps.iter().any(|s| s["state"] == "undecided"));
        let today =
            serde_json::to_value(app::today_get(&p.t.app, req(json!({}))).unwrap()).unwrap();
        assert!(today["rows"][0]["next"].is_object(), "{}", today["rows"][0]);
    }

    /// `project:changed` carries only `{projectId, rev}` (no content, IPC §6).
    #[test]
    fn changed_event_carries_only_id_and_rev() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        open(&p);
        let before = p.t.os.events().len();
        projects::variable_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "name": "QUALITY_BAR", "value": "Production-ready" })),
        )
        .unwrap();
        let events: Vec<Value> = p.t.os.events()[before..]
            .iter()
            .filter(|e| e.name() == "project:changed")
            .map(|e| serde_json::to_value(e.payload()).unwrap())
            .collect();
        assert!(!events.is_empty(), "a change emits project:changed");
        for e in events {
            let mut keys: Vec<&String> = e.as_object().unwrap().keys().collect();
            keys.sort();
            assert_eq!(keys, ["projectId", "rev"], "{e}");
            assert_eq!(e["projectId"], json!(p.id));
            assert!(e["rev"].is_u64());
        }
    }
}

/// Code review W6: a re-pin whose commit fails says so (`ToolFailed`, as `own_change` does)
/// instead of reporting success; the pin itself stands and the next commit picks it up.
mod repin {
    use super::*;

    #[test]
    fn commit_failure_is_reported() {
        let p = ProjectApp::new(json!({}));
        open(&p);
        // No git identity in this sandbox repo: the plumbing commit can't be made.
        let e: IpcError =
            projects::blueprint_repin(&p.t.app, req(json!({ "projectId": p.id }))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::ToolFailed, "{e:?}");
        git_identity(&p);
        projects::blueprint_repin(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
    }
}
