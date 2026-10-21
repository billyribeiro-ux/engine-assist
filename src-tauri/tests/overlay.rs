//! M4-31 (with M4-30's rules): adding a prompt or an inspection item for one project through the
//! IPC commands: the overlay file, the project view, the confirm-before-run rule and the refusals.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::project::{ProjectApp, git_identity, req};
use foreman_app::ipc::IpcErrorCode;
use foreman_app::ipc::commands::{desktop, gates};
use foreman_app::ipc::dto::launch::PrepareOutcome;
use serde_json::{Value, json};

fn add_req(p: &ProjectApp, after: &str, title: &str, text: &str) -> Value {
    json!({
        "projectId": p.id, "stage": 2, "after": after, "copyConfigFrom": "2.2",
        "title": title, "text": text,
        "config": {
            "surface": "cli", "cwd": "spec",
            "start": { "model": "opus", "effort": "high", "mode": "plan" },
            "out": []
        },
        "tag": "optional"
    })
}

fn overlay_json(p: &ProjectApp) -> Value {
    let text = std::fs::read_to_string(p.project.join(".foreman/overlay.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

mod overlay {
    use super::*;

    /// AC-F025-01: the step is stored in `overlay.json` as `x-<stage>-<n>`, optional by default,
    /// and the project view lists it after the step it was added to, tagged `added`.
    #[test]
    fn add_step_is_stored_and_listed() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        let v = desktop::add_step(
            &p.t.app,
            req(add_req(
                &p,
                "2.6",
                "Name check",
                "Read spec/council and list any name used two ways.",
            )),
        )
        .unwrap();
        let o = overlay_json(&p);
        assert_eq!(o["addedSteps"][0]["id"], "x-2-1");
        assert_eq!(o["addedSteps"][0]["tag"], "optional");
        assert_eq!(o["addedSteps"][0]["copiedFrom"], "2.2");
        let stage = v.stages.iter().find(|s| s.stage.0 == 2).unwrap();
        let keys: Vec<&str> = stage.steps.iter().map(|s| s.key.as_str()).collect();
        let at = keys.iter().position(|k| *k == "2.6").unwrap();
        assert_eq!(keys[at + 1], "x-2-1", "{keys:?}");
        let added = &stage.steps[at + 1];
        assert_eq!(serde_json::to_value(added.tag).unwrap(), "added");
        assert_eq!(added.title, "Name check");
        // The second one is x-2-2 and sits directly after the same step.
        desktop::add_step(
            &p.t.app,
            req(add_req(&p, "2.6", "Second", "Another check.")),
        )
        .unwrap();
        assert_eq!(overlay_json(&p)["addedSteps"][1]["id"], "x-2-2");
    }

    /// AC-F025-03 / DR-059 / SR-19: an added step always confirms before running.
    #[test]
    fn added_step_always_confirms() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        desktop::add_step(
            &p.t.app,
            req(add_req(&p, "2.2", "Name check", "Check the names.")),
        )
        .unwrap();
        match p.prepare("x-2-1") {
            PrepareOutcome::Ready(pv) => {
                let v = serde_json::to_value(&pv).unwrap();
                let kinds: Vec<&str> = v["confirm"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| c["kind"].as_str().unwrap())
                    .collect();
                assert!(kinds.contains(&"overlayStep"), "{kinds:?}");
                // The added prompt is what would be sent, and the run config is the one saved.
                assert!(
                    v["prompt"].to_string().contains("Check the names."),
                    "{}",
                    v["prompt"]
                );
                assert!(
                    v["argv"].to_string().contains("plan"),
                    "the saved mode reaches the command: {}",
                    v["argv"]
                );
            }
            PrepareOutcome::Blocked { blockers, .. } => {
                // A blocker is fine only when it is not about the added step itself.
                let v = serde_json::to_value(&blockers).unwrap();
                panic!("the added step is blocked: {v}");
            }
        }
    }

    /// AC-F025-02: an added item is `attest` and appears in the stage's gate.
    #[test]
    fn add_item_lands_in_the_gate() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        let g = desktop::add_item(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 2, "text": "Billing sandbox keys rotated" })),
        )
        .unwrap();
        let o = overlay_json(&p);
        assert_eq!(o["addedItems"][0]["id"], "xi-2-1");
        assert_eq!(o["addedItems"][0]["kind"], "attest");
        let v = serde_json::to_value(&g).unwrap();
        assert!(
            v.to_string().contains("Billing sandbox keys rotated"),
            "the gate lists it: {v}"
        );
        let again =
            gates::gate_view(&p.t.app, req(json!({ "projectId": p.id, "stage": 2 }))).unwrap();
        assert_eq!(serde_json::to_value(&again).unwrap(), v);
    }

    /// AC-F025-05: `bypassPermissions`, a refused `out` path and a bad config are rejected and
    /// nothing is written.
    #[test]
    fn refusals_write_nothing() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        let before = overlay_json(&p);
        let mut bad = add_req(&p, "2.6", "x", "use bypassPermissions");
        let e = desktop::add_step(&p.t.app, req(bad.clone())).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        assert!(e.message.contains("bypassPermissions"), "{}", e.message);
        bad = add_req(&p, "2.6", "x", "fine");
        bad["config"]["out"] = json!([".github/workflows/x.yml"]);
        let e = desktop::add_step(&p.t.app, req(bad)).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::PolicyViolation);
        let mut bad = add_req(&p, "2.6", "x", "fine");
        bad["config"]["start"]["mode"] = Value::Null;
        let e = desktop::add_step(&p.t.app, req(bad)).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        let e = desktop::add_step(&p.t.app, req(add_req(&p, "9.9", "x", "fine"))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotFound);
        let e = desktop::add_item(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 0, "text": "x" })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        assert_eq!(overlay_json(&p), before, "nothing was written");
    }
}
