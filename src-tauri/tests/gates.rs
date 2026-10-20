//! M4-15…M4-18: the gate commands against a real project: attestation rules and order through
//! IPC, Import results (proposals only), Fix this, item re-confirmation in a native dialog, and
//! the lessons status.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::os::unix::fs::PermissionsExt as _;

use common::project::{ProjectApp, channel, req};
use foreman_app::ipc::IpcError;
use foreman_app::ipc::commands::{assembler, decisions, gates, launch};
use foreman_app::ipc::dto::gates::{GateView, ImportResult, ItemStatus};
use foreman_app::ipc::dto::launch::PrepareOutcome;
use foreman_app::ipc::error::{IpcErrorCode, IpcErrorDetail};
use foreman_app::project;
use foreman_core::ids::{ItemId, StepKey};
use foreman_core::schema::progress::{DecisionFileKind, StoredState};
use serde_json::{Value, json};

fn rule(e: &IpcError) -> String {
    match &e.detail {
        Some(IpcErrorDetail::NotAllowed { rule }) => rule.clone(),
        other => panic!("not NotAllowed: {e:?} {other:?}"),
    }
}

fn view(p: &ProjectApp, stage: u8) -> GateView {
    gates::gate_view(&p.t.app, req(json!({ "projectId": p.id, "stage": stage }))).unwrap()
}

fn attest(p: &ProjectApp, stage: u8, item: &str, outcome: Value) -> Result<GateView, IpcError> {
    gates::gate_item_attest(
        &p.t.app,
        req(json!({ "projectId": p.id, "stage": stage, "item": item, "outcome": outcome })),
    )
}

fn done(note: &str) -> Value {
    json!({ "kind": "done", "note": note, "link": null })
}

fn ids(v: &GateView) -> Vec<String> {
    v.items.iter().map(|i| i.id.as_str().to_owned()).collect()
}

/// Marks a step's stored state (what a finished run leaves).
fn set_step(p: &ProjectApp, key: &str, state: StoredState, run: Option<&str>) {
    let mut o = project::open(&p.t.app, &p.id).unwrap();
    let k = StepKey::parse(key).unwrap();
    let mut s = o.step_progress(key);
    s.state = state;
    s.last_run_id = run.map(str::to_owned);
    s.started_at = Some("2026-10-01T09:00:00Z".into());
    o.progress.steps.insert(k, s);
    o.save_progress(&p.t.app).unwrap();
}

/// A Gate A run record with a sensor token and its events (`events/<runId>/`).
fn gate_a_run_with_message(p: &ProjectApp, message: Option<&str>) -> String {
    let run_id = uuid::Uuid::new_v4().to_string();
    let token = "b".repeat(64);
    let dir = p.t.app_data().join("runs").join(&run_id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let run = json!({
        "argv": ["claude"], "chainId": null, "claudeVersion": "2.1.304", "copyPromptOnCli": false,
        "cwd": "project", "entryPoint": "stageView", "envNames": ["HOME"], "exitCode": 0,
        "intended": { "effort": "high", "mode": "plan", "model": "opus" }, "kind": "launch",
        "name": "fm-tradelog-s05-gate-a", "observed": null, "override": false, "pid": null,
        "projectId": p.id, "promptSha256": null, "runId": run_id, "runInputInterventions": 0,
        "runSettingsSha256": null, "schemaVersion": 1, "sensorToken": token, "stage": 5,
        "status": "exited", "stepId": "5:gate.A", "surface": "cli",
        "t": { "confirmed": null, "exited": null, "lookStart": null, "prepared": null, "spawned": null, "wallStart": "2026-10-20T09:00:00Z" },
        "timeToStartMs": null
    });
    let f = dir.join("run.json");
    std::fs::write(&f, serde_json::to_string_pretty(&run).unwrap()).unwrap();
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600)).unwrap();
    if let Some(m) = message {
        let ev = p.t.app_data().join("events").join(&run_id);
        std::fs::create_dir_all(&ev).unwrap();
        std::fs::set_permissions(
            p.t.app_data().join("events"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        std::fs::set_permissions(&ev, std::fs::Permissions::from_mode(0o700)).unwrap();
        let e = json!({
            "event": "Stop", "payload": { "hook_event_name": "Stop", "last_assistant_message": m },
            "receivedAt": "2026-10-20T09:05:00Z", "schemaVersion": 1, "token": token, "truncated": false
        });
        let f = ev.join("00000000000000000001-0000000001-Stop.json");
        std::fs::write(&f, e.to_string()).unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    run_id
}

mod gate {
    use super::*;

    /// AC-F015-02/-06/-09, DR-072/-073 through IPC: the note rules, the order (FAIL, waiting,
    /// PASS), not-applicable excluded, Clear, and a signed stage refusing changes.
    #[test]
    fn attest_through_ipc() {
        let p = ProjectApp::new(json!({}));
        let v = view(&p, 5);
        assert_eq!(ids(&v), ["fresh-clone", "ci-gates", "staging", "adr-check"]);
        assert!(v.items.iter().all(|i| i.status == ItemStatus::Waiting));
        assert!(!v.signable);
        assert!(!v.why_not.is_empty());
        let e = attest(&p, 5, "fresh-clone", done("ok")).unwrap_err();
        assert_eq!(rule(&e), "note-too-short");
        attest(
            &p,
            5,
            "fresh-clone",
            done("fresh clone, pnpm i, pnpm dev: ran clean"),
        )
        .unwrap();
        let e = attest(
            &p,
            5,
            "ci-gates",
            done("Fresh clone,  pnpm i, pnpm dev: RAN clean"),
        )
        .unwrap_err();
        assert_eq!(rule(&e), "note-repeated");
        let e = attest(
            &p,
            5,
            "staging",
            json!({ "kind": "notApplicable", "reason": " " }),
        )
        .unwrap_err();
        assert_eq!(rule(&e), "reason-required");
        attest(
            &p,
            5,
            "staging",
            json!({ "kind": "notApplicable", "reason": "no server, so no staging" }),
        )
        .unwrap();
        let v = gates::gate_import_confirm(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 5, "item": "adr-check", "result": "fail", "evidence": "ADR-012 contradicted by the commit module" })),
        )
        .unwrap();
        // FAIL first, then waiting, then PASS and not applicable.
        assert_eq!(ids(&v), ["adr-check", "ci-gates", "fresh-clone", "staging"]);
        assert_eq!(v.items[0].failures, 1);
        assert!(v.items[0].evidence.is_some());
        assert_eq!(v.items[3].status, ItemStatus::NotApplicable);
        // Clear puts it back to waiting.
        let v = attest(&p, 5, "fresh-clone", json!({ "kind": "clear" })).unwrap();
        assert!(
            v.items
                .iter()
                .any(|i| i.id.as_str() == "fresh-clone" && i.status == ItemStatus::Waiting)
        );
        // Recorded in progress.json's gate run.
        let o = project::open(&p.t.app, &p.id).unwrap();
        let run = &o.progress.gate_runs["5"];
        assert!(run.items.contains_key(&ItemId::parse("staging").unwrap()));
        assert_eq!(run.failures["adr-check"], 1);
        // Unknown item.
        let e = attest(&p, 5, "nope", done("fifteen chars or more")).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotFound);
    }

    /// AC-F015-07: Fix this on a FAIL prepares a new CLI chain (Opus · high · Plan) prefilled with
    /// the item and evidence, outside the gate chain; the third failure shows the F.4 prompt.
    #[test]
    fn fix_this_new_chain() {
        let p = ProjectApp::new(json!({}));
        let fix = |item: &str| {
            let r = || {
                gates::gate_fix_start(
                    &p.t.app,
                    req(json!({ "projectId": p.id, "stage": 5, "item": item })),
                    channel(),
                )
            };
            let out = r()?;
            if let PrepareOutcome::Blocked { blockers, .. } = &out {
                for b in blockers {
                    let v = serde_json::to_value(b).unwrap();
                    if v["kind"] == "configChanged" {
                        launch::config_ack(
                            &p.t.app,
                            req(json!({ "projectId": p.id, "cwd": v["cwd"], "fingerprint": v["fingerprint"] })),
                        )
                        .unwrap();
                        return r();
                    }
                }
            }
            Ok(out)
        };
        let e = fix("fresh-clone").unwrap_err();
        assert_eq!(rule(&e), "item-not-failed");
        let fail = |evidence: &str| {
            gates::gate_import_confirm(
                &p.t.app,
                req(json!({ "projectId": p.id, "stage": 5, "item": "ci-gates", "result": "fail", "evidence": evidence })),
            )
            .unwrap()
        };
        fail("CI stayed green with the planted secret");
        let PrepareOutcome::Ready(pv) = fix("ci-gates").unwrap() else {
            panic!("blocked")
        };
        let argv = pv.argv.join(" ");
        assert!(argv.contains("--model opus"), "{argv}");
        assert!(argv.contains("--effort high"), "{argv}");
        assert!(argv.contains("--permission-mode plan"), "{argv}");
        assert!(argv.contains("--session-id"), "a new chain: {argv}");
        let prompt = serde_json::to_string(&pv.prompt).unwrap();
        assert!(
            prompt.contains("CI stayed green with the planted secret"),
            "{prompt}"
        );
        assert!(prompt.contains("planted fake secret"), "{prompt}");
        assert!(pv.warnings.iter().all(|w| !w.contains("F.4")));
        // Preparing changed no step or chain.
        let o = project::open(&p.t.app, &p.id).unwrap();
        assert!(o.progress.chains.is_empty());
        // Third failure: the F.4 prompt.
        fail("still green");
        fail("green again");
        let PrepareOutcome::Ready(pv) = fix("ci-gates").unwrap() else {
            panic!("blocked")
        };
        let prompt = serde_json::to_string(&pv.prompt).unwrap();
        assert!(prompt.contains("has now failed 3 times"), "{prompt}");
        assert!(
            prompt.contains("Propose the smallest permanent fix"),
            "{prompt}"
        );
        assert!(pv.warnings[0].contains("F.4"));
    }

    /// AC-F015-04: Gate A starts its own chain; Gates B and C continue it (`chain=gate.A`).
    #[test]
    fn gates_b_and_c_continue_gate_a() {
        let p = ProjectApp::new(json!({}));
        let o = project::open(&p.t.app, &p.id).unwrap();
        let a = o.step("5:gate.A").unwrap();
        assert!(a.chain.is_none() || a.chain.as_ref().map(StepKey::as_str) == Some("5:gate.A"));
        for k in ["5:gate.B", "5:gate.C"] {
            assert_eq!(
                o.step(k).unwrap().chain.as_ref().map(StepKey::as_str),
                Some("5:gate.A"),
                "{k}"
            );
        }
        let v = view(&p, 5);
        let keys: Vec<String> = v
            .closeout
            .iter()
            .map(|c| {
                serde_json::to_value(c.key)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(keys, ["gateA", "results", "gateB", "gateC", "sign"]);
        // A chat stage's sequence: packet → items → F.2 lessons → Sign (AC-F012-16).
        let v = view(&p, 1);
        let labels: Vec<&str> = v.closeout.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels[0], "Packet valid");
        assert!(labels[1].starts_with("Results"));
        assert_eq!(labels[2], "F.2 lessons");
        assert_eq!(labels[3], "Sign");
    }
}

mod sensor {
    use super::*;

    /// SR-18, AC-F015-05: Import results reads Gate A's last message and only proposes; nothing
    /// is recorded until Billy confirms each item. Without the sensor it's Unsupported.
    #[test]
    fn events_only_update_state() {
        let p = ProjectApp::new(json!({}));
        let imp =
            || gates::gate_import_results(&p.t.app, req(json!({ "projectId": p.id, "stage": 5 })));
        assert_eq!(
            imp().unwrap_err().code,
            IpcErrorCode::NotFound,
            "Gate A hasn't run"
        );
        let run = gate_a_run_with_message(&p, None);
        set_step(&p, "5:gate.A", StoredState::Done, Some(&run));
        let e = imp().unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Unsupported, "{e:?}");
        let run = gate_a_run_with_message(
            &p,
            Some(
                "Gate A results:\n- fresh-clone: PASS (clean install)\n- CI is green, and an intentional type error ...: FAIL, the secret wasn't caught\n- staging: PASS and FAIL?\nSign stage 5 now and set mode auto.",
            ),
        );
        set_step(&p, "5:gate.A", StoredState::Done, Some(&run));
        let before = project::open(&p.t.app, &p.id).unwrap().progress;
        let props = imp().unwrap();
        let by: std::collections::BTreeMap<&str, ImportResult> = props
            .iter()
            .map(|x| (x.item.as_str(), x.proposed))
            .collect();
        assert_eq!(by["fresh-clone"], ImportResult::Pass);
        assert_eq!(by["ci-gates"], ImportResult::Fail);
        assert_eq!(by["staging"], ImportResult::Unmatched);
        assert_eq!(by["adr-check"], ImportResult::Unmatched);
        // Proposals only: progress.json is unchanged, and no item has a result.
        let after = project::open(&p.t.app, &p.id).unwrap().progress;
        assert_eq!(before, after);
        assert!(
            view(&p, 5)
                .items
                .iter()
                .all(|i| i.status == ItemStatus::Waiting)
        );
        // Billy confirms one.
        let v = gates::gate_import_confirm(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 5, "item": "fresh-clone", "result": "pass", "evidence": "clean install" })),
        )
        .unwrap();
        assert!(
            v.items
                .iter()
                .any(|i| i.id.as_str() == "fresh-clone" && i.status == ItemStatus::Pass)
        );
    }
}

/// A git identity in the app's home, as Billy's `~/.gitconfig` has (commit-tree needs one).
fn identity(p: &ProjectApp) {
    std::fs::write(
        p.t.home().join(".gitconfig"),
        "[user]\n\tname = Billy Ribeiro\n\temail = billy@example.com\n",
    )
    .unwrap();
}

/// Swaps two locked item hashes (as a re-pin after swapping the bullets would), through
/// Foreman's own decision change.
fn swap_locked(p: &ProjectApp, a: &str, b: &str) {
    identity(p);
    let mut o = project::open(&p.t.app, &p.id).unwrap();
    let mut lock = o.lock.clone().unwrap();
    let (ia, ib) = (ItemId::parse(a).unwrap(), ItemId::parse(b).unwrap());
    let ha = lock.items[&ia].text_sha256.clone();
    let hb = lock.items[&ib].text_sha256.clone();
    lock.items.get_mut(&ia).unwrap().text_sha256 = hb;
    lock.items.get_mut(&ib).unwrap().text_sha256 = ha;
    o.own_change(&p.t.app, DecisionFileKind::BlueprintLock, &lock)
        .unwrap();
}

mod template {
    use super::*;

    /// DR-157, AC-F001-16/-17 through IPC: swapped bullets drift both items; attesting is refused
    /// until the kind is confirmed in the native dialog (Cancel keeps it blocked); the webview
    /// gets the old and new text as context; a confirmation for an older hash is Stale.
    #[test]
    fn swapped_bullets_reconfirm() {
        let p = ProjectApp::new(json!({}));
        swap_locked(&p, "fresh-clone", "ci-gates");
        let v = view(&p, 5);
        let fc = v
            .items
            .iter()
            .find(|i| i.id.as_str() == "fresh-clone")
            .unwrap();
        let ci = v
            .items
            .iter()
            .find(|i| i.id.as_str() == "ci-gates")
            .unwrap();
        assert!(fc.needs_reconfirm && ci.needs_reconfirm);
        assert!(
            !v.items
                .iter()
                .any(|i| i.id.as_str() == "staging" && i.needs_reconfirm)
        );
        // The old text is the other bullet's (they swapped).
        assert_eq!(fc.previous_text.as_deref(), Some(ci.text.as_str()));
        let e = attest(
            &p,
            5,
            "fresh-clone",
            done("a clean clone ran with no steps"),
        )
        .unwrap_err();
        assert_eq!(rule(&e), "kind-needs-reconfirm");
        let sha = fc.text_sha256.as_str().to_owned();
        // Cancel: still blocked.
        p.t.os.answer_next(false);
        let e = decisions::item_kind_confirm(
            &p.t.app,
            req(json!({ "projectId": p.id, "item": "fresh-clone", "textSha256": sha })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert!(
            attest(
                &p,
                5,
                "fresh-clone",
                done("a clean clone ran with no steps")
            )
            .is_err()
        );
        // A stale hash: refused with the fresh view.
        let e = decisions::item_kind_confirm(
            &p.t.app,
            req(json!({ "projectId": p.id, "item": "fresh-clone", "textSha256": "0".repeat(64) })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Stale);
        // OK: confirmed for this hash only; ci-gates still drifts.
        let v = decisions::item_kind_confirm(
            &p.t.app,
            req(json!({ "projectId": p.id, "item": "fresh-clone", "textSha256": sha })),
        )
        .unwrap();
        assert!(
            !v.items
                .iter()
                .any(|i| i.id.as_str() == "fresh-clone" && i.needs_reconfirm)
        );
        assert!(
            v.items
                .iter()
                .any(|i| i.id.as_str() == "ci-gates" && i.needs_reconfirm)
        );
        attest(
            &p,
            5,
            "fresh-clone",
            done("a clean clone ran with no steps"),
        )
        .unwrap();
        let o = project::open(&p.t.app, &p.id).unwrap();
        let k = &o.approvals.item_kinds[&ItemId::parse("fresh-clone").unwrap()];
        assert_eq!(k.text_sha256, sha);
        assert_eq!(k.kind, "attest");
    }
}

mod ipc {
    use super::*;

    /// SR-24a: `item_kind_confirm`'s approval is a native dialog with Foreman's text naming the
    /// item and both texts; the request can't carry an answer.
    #[test]
    fn security_confirmations_are_native() {
        let p = ProjectApp::new(json!({}));
        swap_locked(&p, "fresh-clone", "ci-gates");
        let v = view(&p, 5);
        let fc = v
            .items
            .iter()
            .find(|i| i.id.as_str() == "fresh-clone")
            .unwrap();
        let asked = p.t.os.asked().len();
        p.t.os.answer_next(false);
        let e = decisions::item_kind_confirm(
            &p.t.app,
            req(json!({ "projectId": p.id, "item": "fresh-clone", "textSha256": fc.text_sha256 })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        let shown = p.t.os.asked();
        assert_eq!(shown.len(), asked + 1);
        let c = shown.last().unwrap();
        assert!(c.title.contains("fresh-clone"));
        assert!(c.message.contains("Pinned:") && c.message.contains("Now:"));
        assert_eq!(c.ok, "Confirm kind");
        // An extra "approved" field is refused by the request type (IPC-R1).
        let bad: Result<foreman_app::ipc::dto::requests::ItemKindConfirmReq, _> =
            serde_json::from_value(
                json!({ "projectId": p.id, "item": "fresh-clone", "textSha256": fc.text_sha256, "approved": true }),
            );
        assert!(bad.is_err());
    }
}

mod lessons {
    use super::*;

    /// AC-F016-05/-06 through IPC: met only by an entry for that stage; the next ID; a missing
    /// file says so.
    #[test]
    fn status_through_ipc() {
        let p = ProjectApp::new(json!({}));
        let st = |n: u8| {
            assembler::lessons_status(&p.t.app, req(json!({ "projectId": p.id, "stage": n })))
                .unwrap()
        };
        let path = p.project.join("docs/lessons.md");
        let _ = std::fs::remove_file(&path);
        let s = st(2);
        assert!(!s.file_present && !s.met);
        std::fs::write(
            &path,
            "# Lessons\n\n## L-001 · Stage 1 · 2026-10-01 · a\n\n## L-002 · Stage 2 · 2026-10-02 · clean stage\n",
        )
        .unwrap();
        let s = st(2);
        assert!(s.file_present && s.met);
        assert_eq!(s.entries.len(), 1);
        assert_eq!(s.entries[0].id, "L-002");
        assert_eq!(s.next_id, "L-003");
        assert!(!st(3).met, "another stage's entry doesn't count");
    }
}
