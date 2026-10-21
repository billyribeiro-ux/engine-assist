//! M3-21: packet landing and hand-signed packets — the watcher's `packet:changed`, outside edits
//! re-validated, normalization shown before it is written, and an outside signature that counts
//! only after Billy's native confirmation (AC-F013-06…-10, AC-F004-12, EX-M3-03; SR-47, DR-135).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

/// Every file under `dir` (relative path → bytes), the diagnostic log aside.
fn tree(dir: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let path = e.unwrap().path();
            let rel = path.strip_prefix(dir).unwrap().display().to_string();
            if rel == "logs" {
                continue;
            }
            if std::fs::symlink_metadata(&path).unwrap().is_dir() {
                stack.push(path);
            } else {
                out.insert(rel, std::fs::read(&path).unwrap_or_default());
            }
        }
    }
    out
}

use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::project::{ProjectApp, req};
use foreman_app::ipc::commands::{packets, projects};
use foreman_app::ipc::dto::app::ProjectView;
use foreman_app::ipc::dto::records::PacketView;
use foreman_app::ipc::error::{IpcErrorCode, IpcErrorDetail};
use serde_json::{Value, json};

fn tradelog_packet(n: u8) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../crates/foreman-core/tests/fixtures/tradelog/handoff/stage-{n:02}.md"
    )))
    .unwrap()
}

fn packet_path(p: &ProjectApp, n: u8) -> PathBuf {
    p.project.join(format!("docs/handoff/stage-{n:02}.md"))
}

fn write_packet(p: &ProjectApp, n: u8, text: &str) {
    let f = packet_path(p, n);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    // Write and rename, the way editors and Claude Code save.
    let tmp = f.with_extension("tmp-writer");
    std::fs::write(&tmp, text).unwrap();
    std::fs::rename(&tmp, &f).unwrap();
}

fn view(p: &ProjectApp, n: u8) -> PacketView {
    packets::packet_view(&p.t.app, req(json!({ "projectId": p.id, "stage": n }))).unwrap()
}

fn open(p: &ProjectApp) -> ProjectView {
    projects::project_open(&p.t.app, req(json!({ "projectId": p.id }))).unwrap()
}

fn sha(v: &PacketView) -> String {
    v.sha256.as_ref().unwrap().as_str().to_owned()
}

fn packet_events(p: &ProjectApp) -> Vec<Value> {
    p.t.os
        .events()
        .iter()
        .filter(|e| e.name() == "packet:changed")
        .map(|e| e.payload())
        .collect()
}

fn wait_events(p: &ProjectApp, at_least: usize, within: Duration) -> Option<Duration> {
    let start = Instant::now();
    while start.elapsed() < within {
        if packet_events(p).len() >= at_least {
            return Some(start.elapsed());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

fn unsigned(text: &str) -> String {
    text.replacen(
        "APPROVED BY: Billy Ribeiro on 2026-10-20",
        "APPROVED BY: <blank>",
        1,
    )
}

mod packet {
    use super::*;

    /// AC-F013-09, SR-47, DR-135: a packet in the signed form that Foreman's signing didn't write
    /// counts only after Billy confirms it in a native dialog that binds the packet's SHA-256.
    /// Cancel leaves it unsigned; after OK the Next step moves on; any later change voids it.
    #[test]
    fn outside_signature_needs_confirmation() {
        let p = ProjectApp::new(json!({}));
        write_packet(&p, 1, &tradelog_packet(1));
        let before = open(&p);
        assert_eq!(before.summary.current_stage.map(|s| s.0), Some(1));
        let v = view(&p, 1);
        let s = v.signed.as_ref().expect("the signed form is shown");
        assert!(!s.confirmed && !s.by_foreman, "{s:?}");
        assert!(v.summary.contains("isn't confirmed"), "{}", v.summary);
        // Cancel: nothing recorded, still unsigned.
        p.t.os.answer_next(false);
        let asked = p.t.os.asked().len();
        let e = packets::packet_signature_confirm(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "seenSha256": sha(&v) })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert_eq!(p.t.os.asked().len(), asked + 1, "a native dialog asked");
        assert!(
            p.t.os.asked()[asked]
                .title
                .contains("Did you sign Stage 1 yourself?")
        );
        assert!(!view(&p, 1).signed.unwrap().confirmed);
        assert_eq!(open(&p).summary.current_stage.map(|s| s.0), Some(1));
        // A stale hash is refused before any dialog.
        let e = packets::packet_signature_confirm(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "seenSha256": "0".repeat(64) })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Stale);
        assert_eq!(p.t.os.asked().len(), asked + 1);
        // OK: signed, bound to the hash; the Next step moves to Stage 2.
        p.t.os.answer_next(true);
        let v = packets::packet_signature_confirm(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "seenSha256": sha(&v) })),
        )
        .unwrap();
        assert!(v.signed.as_ref().unwrap().confirmed);
        assert_eq!(v.summary, "Valid and signed.");
        let after = open(&p);
        assert_eq!(after.summary.current_stage.map(|s| s.0), Some(2));
        let approvals: Value = serde_json::from_str(
            &std::fs::read_to_string(p.t.app_data().join(format!("approvals/{}.json", p.id)))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            approvals["outsideSignatures"]["1"]["packetSha256"],
            json!(sha(&v))
        );
        // Any later change voids it.
        write_packet(&p, 1, &format!("{}\n", tradelog_packet(1)));
        assert!(!view(&p, 1).signed.unwrap().confirmed);
        assert_eq!(open(&p).summary.current_stage.map(|s| s.0), Some(1));
        // Claude writing `<blank>` is no signature at all.
        write_packet(&p, 1, &unsigned(&tradelog_packet(1)));
        let e = packets::packet_signature_confirm(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "seenSha256": sha(&view(&p, 1)) })),
        )
        .unwrap_err();
        assert!(
            matches!(&e.detail, Some(IpcErrorDetail::NotAllowed { rule }) if rule == "not-signed-form"),
            "{e:?}"
        );
    }

    /// AC-F004-12, DR-135: a Stage 2 packet signed by hand while an early condition is undecided
    /// shows "Decide <condition> before signing Stage 2" and can't be confirmed; Stage 2 isn't
    /// counted as signed.
    #[test]
    fn hand_signed_stage_2_needs_decided_conditions() {
        let p = ProjectApp::new(json!({}));
        let s2 = tradelog_packet(2);
        write_packet(&p, 2, &s2);
        let v = view(&p, 2);
        let pr = v
            .problems
            .iter()
            .find(|x| x.rule == "packet.undecided-early-condition")
            .expect("the undecided problem is listed");
        assert!(
            pr.message.starts_with("Decide ") && pr.message.ends_with(" before signing Stage 2"),
            "{}",
            pr.message
        );
        let asked = p.t.os.asked().len();
        let e = packets::packet_signature_confirm(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 2, "seenSha256": sha(&v) })),
        )
        .unwrap_err();
        assert!(
            matches!(&e.detail, Some(IpcErrorDetail::NotAllowed { rule }) if rule == "undecided-early-condition"),
            "{e:?}"
        );
        assert_eq!(p.t.os.asked().len(), asked, "no dialog");
        let stage2 = open(&p)
            .stages
            .into_iter()
            .find(|s| s.stage.0 == 2)
            .unwrap();
        assert!(stage2.signed.is_none());
    }

    /// AC-F013-08, DR-131: a CLI-written packet needing normalization shows the changes and is
    /// written (atomically, by compare-and-swap) only when Billy applies it; a stale review is
    /// refused with the fresh view.
    #[test]
    fn normalization_shown_then_written() {
        let p = ProjectApp::new(json!({}));
        let good = unsigned(&tradelog_packet(1));
        let messy = good
            .replacen(
                "APPROVED BY: <blank>",
                "APPROVED BY: <leave blank for me to sign>",
                1,
            )
            .replacen("OPEN ITEMS: none", "OPEN ITEMS: None.", 1)
            .replace(" — ", "\u{a0}— ");
        write_packet(&p, 1, &messy);
        let v = view(&p, 1);
        let notes: Vec<(String, String)> = v
            .normalization
            .iter()
            .map(|n| (n.field.clone(), n.message.clone()))
            .collect();
        assert!(
            notes.contains(&("APPROVED BY".into(), "unsigned".into())),
            "{notes:?}"
        );
        assert!(
            notes.contains(&("OPEN ITEMS".into(), "OPEN ITEMS normalized to none".into())),
            "{notes:?}"
        );
        assert!(notes.iter().any(|(_, m)| m == "fixed non-breaking spaces"));
        // Shown only: the file is unchanged.
        assert_eq!(std::fs::read_to_string(packet_path(&p, 1)).unwrap(), messy);
        // A review of an older version is stale.
        let e = packets::packet_normalize_apply(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "seenSha256": "0".repeat(64) })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Stale);
        assert_eq!(std::fs::read_to_string(packet_path(&p, 1)).unwrap(), messy);
        let events = packet_events(&p).len();
        let v = packets::packet_normalize_apply(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "seenSha256": sha(&v) })),
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(packet_path(&p, 1)).unwrap(), good);
        assert!(v.normalization.is_empty());
        assert!(
            v.problems
                .iter()
                .all(|x| x.rule != "packet.approved-by" && x.rule != "packet.open-items"),
            "{:?}",
            v.problems
        );
        assert!(packet_events(&p).len() > events, "packet:changed emitted");
        // No temp file left behind.
        let left: Vec<String> = std::fs::read_dir(p.project.join("docs/handoff"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec!["stage-01.md".to_owned()]);
    }
}

mod watcher {
    use super::*;

    /// AC-F013-06, AC-F013-07, EX-M3-03, NFR-06: with the project open, a fake writer's packet
    /// emits `packet:changed` and is visible and validated at p95 ≤ 1 s; an outside edit is
    /// re-validated and its problems update.
    #[test]
    fn packet_visible_within_1s() {
        let p = ProjectApp::new(json!({}));
        open(&p);
        assert!(p.t.app.watchers().is_watching(&p.id));
        let good = unsigned(&tradelog_packet(1));
        let bad = good.replacen("OPEN ITEMS: none", "OPEN ITEMS: pricing", 1);
        let mut times = Vec::new();
        for i in 0..20 {
            let seen = packet_events(&p).len();
            let start = Instant::now();
            write_packet(&p, 1, if i % 2 == 0 { &bad } else { &good });
            assert!(
                wait_events(&p, seen + 1, Duration::from_secs(5)).is_some(),
                "write {i}: no packet:changed"
            );
            let v = view(&p, 1);
            times.push(start.elapsed());
            let open_items = v.problems.iter().any(|x| x.rule == "packet.open-items");
            assert_eq!(open_items, i % 2 == 0, "write {i}: re-validated");
            // Let the debouncer settle before the next write.
            std::thread::sleep(Duration::from_millis(300));
        }
        times.sort();
        let p95 = times[(times.len() * 95).div_ceil(100) - 1];
        assert!(p95 <= Duration::from_secs(1), "p95 {p95:?}: {times:?}");
        let last = packet_events(&p).pop().unwrap();
        assert_eq!(last["projectId"], json!(p.id));
        assert_eq!(last["stage"], json!(1));
        // Lookalike names are ignored.
        let seen = packet_events(&p).len();
        std::fs::write(p.project.join("docs/handoff/stage-1.md"), "x").unwrap();
        std::fs::write(p.project.join("docs/handoff/stage-01.md.bak"), "x").unwrap();
        assert!(wait_events(&p, seen + 1, Duration::from_millis(800)).is_none());
    }

    /// AC-F013-10: when the watcher stops, Doctor's level turns to a warning, and the packet is
    /// still validated when Billy opens the stage.
    #[test]
    fn stopped_watcher_warns_and_packets_still_validate() {
        let p = ProjectApp::new(json!({}));
        open(&p);
        let handled = foreman_app::watch::handle(
            &p.t.app,
            &p.id,
            &foreman_sys::watch::Change::RootRefused {
                root: p.project.clone(),
            },
        );
        assert!(!handled);
        let failures = p.t.app.watchers().failures();
        assert!(
            failures[&p.id].contains("Packets appear when you focus Foreman"),
            "{failures:?}"
        );
        let doctor: Vec<Value> =
            p.t.os
                .events()
                .iter()
                .filter(|e| e.name() == "doctor:status")
                .map(|e| e.payload())
                .collect();
        assert_eq!(doctor.last(), Some(&json!({ "level": "warning" })));
        write_packet(
            &p,
            1,
            &unsigned(&tradelog_packet(1)).replacen("OPEN ITEMS: none", "OPEN ITEMS: x", 1),
        );
        let v = view(&p, 1);
        assert!(v.exists && v.problems.iter().any(|x| x.rule == "packet.open-items"));
    }

    /// ADR-013 (watcher disabled): missed events are tolerated. With no watcher running, a packet
    /// that lands is still seen at the next interaction: the interest-set re-stat on window focus,
    /// before a launch (`launch_prepare`, `launch_start`) and before a sign (`sign_preview`,
    /// `sign_apply`).
    #[test]
    fn refocus_rescan_catches_missed_event() {
        let p = ProjectApp::new(json!({}));
        // `project_open` whose watcher couldn't start: the first re-stat records the stamps.
        let o = foreman_app::project::open(&p.t.app, &p.id).unwrap();
        assert_eq!(foreman_app::watch::restat_project(&p.t.app, &o), 0);
        assert!(!p.t.app.watchers().is_watching(&p.id));
        let stages = |p: &ProjectApp| -> Vec<u64> {
            packet_events(p)
                .iter()
                .map(|e| e["stage"].as_u64().unwrap())
                .collect()
        };

        // A packet lands; nothing is watching, so nothing is emitted…
        write_packet(&p, 1, &unsigned(&tradelog_packet(1)));
        std::thread::sleep(Duration::from_millis(300));
        assert!(stages(&p).is_empty());
        // …until the window is focused.
        foreman_app::watch::on_focus(&p.t.app);
        assert_eq!(stages(&p), [1]);
        foreman_app::watch::on_focus(&p.t.app);
        assert_eq!(stages(&p), [1], "an unchanged file is not a change");

        // Before a launch.
        write_packet(&p, 2, &unsigned(&tradelog_packet(2)));
        let _ = p.prepare("2.2");
        assert_eq!(stages(&p), [1, 2]);

        // Before a sign (the preview's answer doesn't matter here).
        write_packet(
            &p,
            1,
            &unsigned(&tradelog_packet(1)).replacen("OPEN ITEMS: none", "OPEN ITEMS: x", 1),
        );
        let _ = foreman_app::ipc::commands::signing::sign_preview(
            &p.t.app,
            req(
                json!({ "projectId": p.id, "stage": 1, "branch": { "kind": "current" }, "name": "Billy Ribeiro", "date": "2026-10-20" }),
            ),
        );
        assert_eq!(stages(&p), [1, 2, 1]);
        // The packet view re-reads it (the event is only a hint).
        assert!(
            view(&p, 1)
                .problems
                .iter()
                .any(|x| x.rule == "packet.open-items")
        );
    }

    /// ADR-013, SR-18, IPC-R6 (code review W8: a runtime proof, not a text search): what the
    /// app's watch module does with each kind of change. A packet, an `out` file and the lessons
    /// file give events only: the repo (`.git` included) and app data stay byte for byte, no run
    /// starts and `claude` never runs. A decision file updates the
    /// pending review in `progress.json` (as opening the project would) and nothing else.
    #[test]
    fn app_events_never_act() {
        use foreman_core::interest::Interest;
        use foreman_sys::watch::Change;
        let p = ProjectApp::new(json!({}));
        projects::project_open(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        let out = foreman_app::ipc::commands::steps::step_view(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "2.2" })),
        )
        .unwrap()
        .outs[0]
            .path
            .as_str()
            .to_owned();
        let files = [
            ("docs/handoff/stage-02.md", "=== HANDOFF PACKET\nbody\n"),
            (out.as_str(), "# out\n"),
            ("docs/lessons.md", "# Lessons\n"),
        ];
        for (rel, body) in files {
            let f = p.project.join(rel);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(&f, body).unwrap();
        }
        let calls = || {
            std::fs::read_to_string(format!("{}.calls.jsonl", p.record.display()))
                .unwrap_or_default()
                .lines()
                .count()
        };
        let (repo, data, n, events) = (
            tree(&p.project),
            tree(&p.t.app_data()),
            calls(),
            p.t.os.events().len(),
        );
        let change = |interest: Interest, path: &str| Change::Changed {
            interest,
            path: path.to_owned(),
            stamp: foreman_core::store::ReadStamp::Absent,
        };
        for c in [
            change(Interest::Packet { stage: 2 }, "docs/handoff/stage-02.md"),
            change(Interest::Out { path: out.clone() }, &out),
            change(Interest::Lessons, "docs/lessons.md"),
        ] {
            foreman_app::watch::handle(&p.t.app, &p.id, &c);
        }
        assert_eq!(tree(&p.project), repo, "the repo changed");
        assert_eq!(tree(&p.t.app_data()), data, "app data changed");
        assert_eq!(calls(), n, "claude ran");
        assert!(p.t.app.runs().live().is_empty(), "a run started");
        let names: Vec<&str> = p.t.os.events()[events..].iter().map(|e| e.name()).collect();
        assert!(
            names
                .iter()
                .all(|n| ["packet:changed", "project:changed"].contains(n)),
            "{names:?}"
        );
        // A decision file: the pending review only.
        let pj = p.project.join(".foreman/project.json");
        let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&pj).unwrap()).unwrap();
        v["conditions"]["commercial"] = json!("yes");
        std::fs::write(&pj, serde_json::to_string_pretty(&v).unwrap()).unwrap();
        let repo = tree(&p.project);
        let mut data = tree(&p.t.app_data());
        foreman_app::watch::handle(
            &p.t.app,
            &p.id,
            &change(
                Interest::Decision {
                    file: "project".into(),
                },
                ".foreman/project.json",
            ),
        );
        assert_eq!(tree(&p.project), repo, "the repo changed");
        let mut now = tree(&p.t.app_data());
        let progress = format!("projects/{}/progress.json", p.id);
        assert!(now.remove(&progress).is_some() && data.remove(&progress).is_some());
        assert_eq!(now, data, "app data beyond progress.json changed");
        assert_eq!(calls(), n, "claude ran");
    }
}
