//! M2-24: the outside-change review (AC-F022-02, AC-F022-05, AC-F004-11; T-003, T-063, SR-24a,
//! SR-33, SR-40). Accept raises a native dialog from Rust and Cancel changes nothing; Reject puts
//! the approved content back by compare-and-swap; a file that changed again refuses both and
//! returns the new diff.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::time::{Duration, Instant};

use common::project::{ProjectApp, git_identity, req};
use foreman_app::ipc::IpcErrorCode;
use foreman_app::ipc::commands::{decisions as dec, projects};
use serde_json::{Value, json};

fn open(p: &ProjectApp) -> Value {
    serde_json::to_value(
        projects::project_open(&p.t.app, req(json!({ "projectId": p.id }))).unwrap(),
    )
    .unwrap()
}

fn pending_events(p: &ProjectApp) -> usize {
    p.t.os
        .events()
        .iter()
        .filter(|e| e.name() == "decision:pending")
        .count()
}

fn wait_pending(p: &ProjectApp, at_least: usize) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if pending_events(p) >= at_least {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no decision:pending ({at_least})");
}

/// Writes `.foreman/project.json` with `commercial` set, as an editor would.
fn edit_condition(p: &ProjectApp, value: &str) -> Vec<u8> {
    let file = p.project.join(".foreman/project.json");
    let mut pj: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    pj["conditions"]["commercial"] = json!(value);
    let bytes = format!("{}\n", serde_json::to_string_pretty(&pj).unwrap()).into_bytes();
    std::fs::write(&file, &bytes).unwrap();
    bytes
}

fn approved_copy(p: &ProjectApp) -> Vec<u8> {
    std::fs::read(
        p.t.app_data()
            .join("approvals")
            .join(&p.id)
            .join("project.json"),
    )
    .unwrap()
}

fn list(p: &ProjectApp) -> Vec<Value> {
    dec::pending_changes_list(&p.t.app, req(json!({ "projectId": p.id })))
        .unwrap()
        .iter()
        .map(|c| serde_json::to_value(c).unwrap())
        .collect()
}

fn args(p: &ProjectApp, c: &Value) -> Value {
    json!({ "projectId": p.id, "change": c["id"], "seenSha256": c["seenSha256"] })
}

mod decisions {
    use super::*;

    /// DR-152, SR-33, AC-F022-02: Reject writes the approved content back through the store's
    /// compare-and-swap (the file's read stamp is the expected one), leaves no temp file, drops
    /// the pending change, and never touches the approved copy.
    #[test]
    fn reject_restores_via_cas() {
        let p = ProjectApp::new(json!({}));
        open(&p);
        let file = p.project.join(".foreman/project.json");
        let approved_before = approved_copy(&p);
        let original = std::fs::read(&file).unwrap();
        assert_eq!(original, approved_before, "setup writes the approved copy");

        let seen = pending_events(&p);
        let edited = edit_condition(&p, "yes");
        wait_pending(&p, seen + 1);
        assert_ne!(std::fs::read(&file).unwrap(), original);
        let c = list(&p).pop().unwrap();
        // The review's context is the redacted diff (AC-F022-02 comp part): the key, not a dump.
        assert_eq!(c["diff"]["source"], ".foreman/project.json");
        assert_eq!(c["diff"]["entries"][0]["keyPath"], "conditions.commercial");
        assert!(
            c["context"]["changed"]
                .as_str()
                .unwrap()
                .contains("conditions.commercial")
        );

        // The CAS: a stale `seenSha256` (the file isn't what the review showed) is refused before
        // any write, and the file still holds the edit.
        let mut stale = args(&p, &c);
        stale["seenSha256"] = json!("0".repeat(64));
        let e = dec::pending_change_reject(&p.t.app, req(stale)).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Stale);
        assert_eq!(std::fs::read(&file).unwrap(), edited);

        let v = dec::pending_change_reject(&p.t.app, req(args(&p, &c))).unwrap();
        let v = serde_json::to_value(v).unwrap();
        assert_eq!(v["pendingChanges"], 0);
        assert_eq!(
            std::fs::read(&file).unwrap(),
            original,
            "restored byte for byte"
        );
        assert_eq!(
            approved_copy(&p),
            approved_before,
            "the approved copy is untouched"
        );
        assert_eq!(v["conditions"]["commercial"]["value"], "no");
        assert!(list(&p).is_empty());
        // Atomic: nothing left beside it, and it's no wider than the repo default.
        let names: Vec<String> = std::fs::read_dir(p.project.join(".foreman"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|n| !n.contains(".tmp")), "{names:?}");
        // No native dialog for Reject (it restores what Billy already approved).
        assert!(p.t.os.asked().is_empty());
        // Rejecting twice: the change is gone.
        let e = dec::pending_change_reject(&p.t.app, req(args(&p, &c))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotFound);
    }

    /// AC-F022-02 (int): Accept raises a native dialog from Rust naming the file and the decision
    /// keys; Cancel leaves the approved copy, its hash and the pending change exactly as they
    /// were; OK stores the new approved copy and clears the pending change.
    #[test]
    fn accept_is_native_and_cancel_changes_nothing() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        open(&p);
        let seen = pending_events(&p);
        let edited = edit_condition(&p, "yes");
        wait_pending(&p, seen + 1);
        let c = list(&p).pop().unwrap();
        let approved_before = approved_copy(&p);
        let pending_before = open(&p)["pendingChanges"].clone();

        p.t.os.answer_next(false);
        let e = dec::pending_change_accept(&p.t.app, req(args(&p, &c))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        let asked = p.t.os.asked();
        assert_eq!(asked.len(), 1);
        assert!(
            asked[0].title.contains(".foreman/project.json"),
            "{:?}",
            asked[0]
        );
        assert!(
            asked[0].message.contains("conditions.commercial"),
            "{:?}",
            asked[0]
        );
        assert_eq!(approved_copy(&p), approved_before);
        assert_eq!(open(&p)["pendingChanges"], pending_before);
        assert_eq!(list(&p).len(), 1);
        assert_eq!(
            std::fs::read(p.project.join(".foreman/project.json")).unwrap(),
            edited
        );

        p.t.os.answer_next(true);
        dec::pending_change_accept(&p.t.app, req(args(&p, &c))).unwrap();
        assert_eq!(approved_copy(&p), edited);
        assert!(list(&p).is_empty());
    }

    /// Security review 2026-10-20, W-2: the Accept dialog shows the content Foreman will
    /// approve, not only the changed keys: SHA-256 prefix, size and the file's first lines with
    /// escapes visible.
    #[test]
    fn accept_dialog_shows_the_content() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        open(&p);
        let seen = pending_events(&p);
        let edited = edit_condition(&p, "yes");
        wait_pending(&p, seen + 1);
        let c = list(&p).pop().unwrap();
        p.t.os.answer_next(false);
        let _ = dec::pending_change_accept(&p.t.app, req(args(&p, &c)));
        let asked = p.t.os.asked();
        let m = &asked[0].message;
        let sha = foreman_core::store::sha256_hex(&edited);
        assert!(m.contains(&format!("SHA-256 {}", &sha[..12])), "{m}");
        assert!(m.contains(&format!("{} bytes", edited.len())), "{m}");
        assert!(m.contains("│ {"), "{m}");
    }

    /// AC-F022-05: the file changes again while the review is open; Reject and Accept are both
    /// refused with `Stale`, the answer carries the new review, no dialog is raised for Accept,
    /// and neither the file nor the approved copy changed.
    #[test]
    fn second_change_refuses_accept_and_reject() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        open(&p);
        let seen = pending_events(&p);
        edit_condition(&p, "yes");
        wait_pending(&p, seen + 1);
        let c = list(&p).pop().unwrap();
        let approved_before = approved_copy(&p);

        // Billy's editor saves again while the review is open.
        let second = edit_condition(&p, "undecided");
        let file = p.project.join(".foreman/project.json");

        for which in ["reject", "accept"] {
            let r = if which == "reject" {
                dec::pending_change_reject(&p.t.app, req(args(&p, &c))).map(drop)
            } else {
                dec::pending_change_accept(&p.t.app, req(args(&p, &c))).map(drop)
            };
            let e = r.unwrap_err();
            assert_eq!(e.code, IpcErrorCode::Stale, "{which}");
            let detail = serde_json::to_value(&e).unwrap();
            assert!(
                detail.to_string().contains("changed again"),
                "{which}: {detail}"
            );
            assert_eq!(std::fs::read(&file).unwrap(), second, "{which}");
            assert_eq!(approved_copy(&p), approved_before, "{which}");
        }
        assert!(p.t.os.asked().is_empty(), "no dialog for a stale review");
    }

    /// AC-F004-11, DR-052: once Stage 2 is signed a condition can't change; once Stage 4 is
    /// signed a late condition can't either. The refusal says "Change requires a Change Request
    /// (v1.1)" with the template path, and the decision file stays as it was.
    #[test]
    fn condition_change_after_sign_needs_a_change_request() {
        let p = ProjectApp::new(json!({}));
        open(&p);
        let set = |condition: &str| {
            projects::condition_set(
                &p.t.app,
                req(json!({ "projectId": p.id, "condition": condition, "to": "yes", "note": "n" })),
            )
        };
        let sign = |stage: &str| {
            let path =
                p.t.app_data()
                    .join("projects")
                    .join(&p.id)
                    .join("progress.json");
            let mut prog: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            prog["signing"][stage] = json!({
                "state": "handSigned", "commit": null, "branch": "main",
                "packetSha256": "0".repeat(64), "at": "2026-10-20T12:00:00Z"
            });
            std::fs::write(&path, serde_json::to_string_pretty(&prog).unwrap()).unwrap();
        };
        let project_json = || std::fs::read(p.project.join(".foreman/project.json")).unwrap();

        // Before any signing: allowed (control).
        git_identity(&p);
        set("payments").unwrap();
        let before = project_json();

        sign("2");
        let e = set("commercial").unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        assert!(
            e.message
                .contains("Change requires a Change Request (v1.1)"),
            "{}",
            e.message
        );
        assert!(
            e.message.contains("docs/templates/change-request.md"),
            "{}",
            e.message
        );
        assert_eq!(project_json(), before);
        // A late condition is still open until Stage 4 is signed.
        set("has_database").unwrap();
        let before = project_json();
        sign("4");
        let e = set("has_email").unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        assert!(
            e.message
                .contains("Change requires a Change Request (v1.1)"),
            "{}",
            e.message
        );
        assert_eq!(project_json(), before);
    }
}

fn progress_bytes(p: &ProjectApp) -> Vec<u8> {
    std::fs::read(
        p.t.app_data()
            .join(format!("projects/{}/progress.json", p.id)),
    )
    .unwrap()
}

/// Code review W1: one locked path writes `progress.json`.
mod writes {
    use super::*;

    /// A `Read` open has no side effects: an outside change it classifies is in the view (with
    /// the same change ID every time), and `progress.json` is written only by `project_open`
    /// (or a mutation), under the write lock.
    #[test]
    fn read_open_writes_nothing() {
        let p = ProjectApp::new(json!({}));
        open(&p);
        let before = progress_bytes(&p);
        edit_condition(&p, "yes");
        let a = foreman_app::project::open(&p.t.app, &p.id).unwrap();
        let b = foreman_app::project::open(&p.t.app, &p.id).unwrap();
        assert_eq!(a.progress.pending_outside.len(), 1);
        assert!(a.dirty);
        assert_eq!(
            a.progress.pending_outside[0].id, b.progress.pending_outside[0].id,
            "the same change has the same ID"
        );
        assert_eq!(
            progress_bytes(&p),
            before,
            "a Read open wrote progress.json"
        );
        let v = open(&p);
        assert_eq!(v["pendingChanges"], 1);
        assert_ne!(progress_bytes(&p), before);
        assert_eq!(list(&p)[0]["id"], json!(a.progress.pending_outside[0].id));
    }

    /// `with_project_mut` reopens and runs the change again once when `progress.json` changed
    /// underneath (an outside writer), so the change isn't lost.
    #[test]
    fn with_project_mut_retries_once_on_conflict() {
        let p = ProjectApp::new(json!({}));
        open(&p);
        let file =
            p.t.app_data()
                .join(format!("projects/{}/progress.json", p.id));
        let mut runs = 0;
        foreman_app::project::with_project_mut(&p.t.app, &p.id, |o| {
            runs += 1;
            if runs == 1 {
                // Another writer rewrites the file between this open and its save.
                let t = std::fs::read_to_string(&file).unwrap();
                std::fs::write(&file, format!("{t}\n")).unwrap();
            }
            o.progress.first_launch_done = true;
            Ok(())
        })
        .unwrap();
        assert_eq!(runs, 2, "reopened and applied again");
        let pr: Value = serde_json::from_slice(&progress_bytes(&p)).unwrap();
        assert_eq!(pr["firstLaunchDone"], true);
    }
}

/// Code review W3: no native dialog is shown while the write lock is held.
mod dialogs {
    use super::*;

    fn lock_probe(p: &ProjectApp) -> std::sync::Arc<std::sync::Mutex<Vec<bool>>> {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let s = std::sync::Arc::clone(&seen);
        let app = std::sync::Arc::clone(&p.t.app);
        p.t.os.while_dialog_open(Some(Box::new(move |_| {
            s.lock().unwrap().push(app.write_lock_held());
        })));
        seen
    }

    /// Accept's dialog, then the change is checked again under the lock.
    #[test]
    fn accept_dialog_without_write_lock() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        open(&p);
        let seen = pending_events(&p);
        edit_condition(&p, "yes");
        wait_pending(&p, seen + 1);
        let c = list(&p).pop().unwrap();
        let held = lock_probe(&p);
        dec::pending_change_accept(&p.t.app, req(args(&p, &c))).unwrap();
        p.t.os.while_dialog_open(None);
        assert_eq!(*held.lock().unwrap(), vec![false]);
        assert!(list(&p).is_empty());
    }

    /// Re-pin's dialog (security review N-8), then the project and blueprint are read again
    /// under the lock.
    #[test]
    fn repin_dialog_without_write_lock() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        open(&p);
        let held = lock_probe(&p);
        projects::blueprint_repin(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        p.t.os.while_dialog_open(None);
        assert_eq!(*held.lock().unwrap(), vec![false]);
    }

    /// Remove's dialog, then the registry and live runs are checked again under the lock.
    #[test]
    fn remove_dialog_without_write_lock() {
        let p = ProjectApp::new(json!({}));
        open(&p);
        let held = lock_probe(&p);
        projects::project_remove(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        p.t.os.while_dialog_open(None);
        assert_eq!(*held.lock().unwrap(), vec![false]);
        assert!(projects::summaries(&p.t.app).unwrap().is_empty());
    }
}
