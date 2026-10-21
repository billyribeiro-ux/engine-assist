//! M4-25: the Today inbox (AC-F017-07 to -09; SR-56): which items exist, their order, and what
//! reaches the OS (the Dock badge and notifications) with the scripted OS.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::path::PathBuf;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::project::{ProjectApp, collect, git_identity, req};
use foreman_app::inbox as ib;
use foreman_app::ipc::commands::{app as app_cmd, chat, launch, projects};
use foreman_app::ipc::dto::app::{InboxItem, InboxKind};
use foreman_app::ipc::dto::launch::{PrepareOutcome, RunStarted};
use foreman_app::ipc::types::RunStatus;
use foreman_app::os::Os as _;
use serde_json::json;

fn tradelog_packet(n: u8) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../crates/foreman-core/tests/fixtures/tradelog/handoff/stage-{n:02}.md"
    )))
    .unwrap()
}

fn write_packet(p: &ProjectApp, n: u8, text: &str) {
    let f = p.project.join(format!("docs/handoff/stage-{n:02}.md"));
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, text).unwrap();
}

fn unsigned(text: &str) -> String {
    text.replacen(
        "APPROVED BY: Billy Ribeiro on 2026-10-20",
        "APPROVED BY: <blank>",
        1,
    )
}

fn today(p: &ProjectApp) -> Vec<InboxItem> {
    app_cmd::today_get(&p.t.app, req(json!({}))).unwrap().inbox
}

fn kinds(items: &[InboxItem]) -> Vec<InboxKind> {
    items.iter().map(|i| i.kind).collect()
}

const A: &str = "0c6d2f5b-8407-4e1b-8b3d-5f94c3e7a104";
const B: &str = "7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11";
const Z: &str = "9b5c1e4a-73f6-4d0a-9a2c-4e83b2d6f093";

fn item(kind: &str, project: &str, stage: u8) -> InboxItem {
    serde_json::from_value(json!({
        "kind": kind, "project": project, "step": null, "stage": stage,
        "at": "2026-10-20T12:00:00Z", "projectName": "TradeLog",
        "title": "t", "detail": "d", "chain": null, "run": null
    }))
    .unwrap()
}

fn fill_variables(p: &ProjectApp) {
    git_identity(p);
    for (name, value) in [
        ("MY_ADVANTAGES", "Ten years of trading"),
        ("BUDGET_TIME", "Evenings, 3 months"),
        ("APP_NAME", "TradeLog"),
        ("PREFERRED_STACK", "Svelte and Rust"),
    ] {
        projects::variable_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "name": name, "value": value })),
        )
        .unwrap();
    }
}

fn start(p: &ProjectApp, step: &str) -> (RunStarted, Arc<Mutex<Vec<u8>>>) {
    let PrepareOutcome::Ready(pv) = p.prepare(step) else {
        panic!("{step} is blocked");
    };
    let (ch, buf) = collect();
    let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !String::from_utf8_lossy(&buf.lock().unwrap()).contains("FAKE-CLAUDE READY") {
        assert!(Instant::now() < deadline, "fake-claude did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    (r, buf)
}

mod inbox {
    use super::*;

    /// AC-F017-07: a live session announced `blocked` is a session-blocked item and one announced
    /// `idle` is session-your-turn, each carrying its run and chain so Open terminal can focus it;
    /// a session that is simply running is not in the inbox. `emit_status` is the way the sensor
    /// announces them.
    #[test]
    fn blocked_and_your_turn_sessions() {
        let p = ProjectApp::new(json!({}));
        let (r, _) = start(&p, "2.2");
        assert!(today(&p).is_empty(), "a running session isn't waiting");
        foreman_app::runs::emit_status(&p.t.app, r.run_id.as_str(), RunStatus::Blocked);
        let items = today(&p);
        assert_eq!(kinds(&items), [InboxKind::SessionBlocked]);
        assert_eq!(
            items[0].run.as_ref().map(|x| x.as_str()),
            Some(r.run_id.as_str())
        );
        assert_eq!(items[0].stage.map(|s| s.0), Some(2));
        assert!(
            items[0]
                .title
                .starts_with("Claude is asking for permission in 2.2"),
            "{}",
            items[0].title
        );
        foreman_app::runs::emit_status(&p.t.app, r.run_id.as_str(), RunStatus::Idle);
        assert_eq!(kinds(&today(&p)), [InboxKind::SessionYourTurn]);
        foreman_app::runs::emit_status(&p.t.app, r.run_id.as_str(), RunStatus::Running);
        assert!(today(&p).is_empty());
        // Terminate the session so the test leaves no process behind.
        p.t.os.answer_next(true);
        let _ = foreman_app::ipc::commands::terminal::pty_terminate(
            &p.t.app,
            req(json!({ "runId": r.run_id })),
        );
    }

    /// AC-F017-07: session-blocked, session-your-turn, packet-landed/invalid, gate-failed/signable,
    /// outside-change, tamper, chat-awaiting-paste, wait-elapsed, interrupted-session, step-ready.
    #[test]
    fn order_matches_ac_f017_07() {
        let kinds_in_order = [
            "sessionBlocked",
            "sessionYourTurn",
            "packetLanded",
            "packetInvalid",
            "gateFailed",
            "gateSignable",
            "outsideChange",
            "tamper",
            "chatAwaitingPaste",
            "waitElapsed",
            "interruptedSession",
            "stepReady",
        ];
        // Shuffled input, deterministically: reverse, then rotate.
        let mut items: Vec<InboxItem> =
            kinds_in_order.iter().rev().map(|k| item(k, A, 1)).collect();
        items.rotate_left(5);
        ib::sort(&mut items);
        let got: Vec<u8> = items.iter().map(|i| ib::rank(i.kind)).collect();
        let mut want = got.clone();
        want.sort_unstable();
        assert_eq!(got, want);
        // The pairs share a rank; everything else is strictly increasing.
        assert_eq!(
            got,
            [0, 1, 2, 2, 3, 3, 4, 5, 6, 7, 8, 9],
            "{:?}",
            kinds(&items)
        );
    }

    /// Items of one rank keep their project order.
    #[test]
    fn same_rank_keeps_project_order() {
        let mut items = vec![
            item("gateSignable", B, 3),
            item("packetLanded", Z, 1),
            item("gateFailed", A, 2),
        ];
        ib::sort(&mut items);
        assert_eq!(
            items
                .iter()
                .map(|i| (i.kind, i.project.as_str().to_owned()))
                .collect::<Vec<_>>(),
            [
                (InboxKind::PacketLanded, Z.to_owned()),
                (InboxKind::GateSignable, B.to_owned()),
                (InboxKind::GateFailed, A.to_owned()),
            ]
        );
    }

    /// AC-F017-08: a chat prompt copied more than ten minutes ago, with no paste-back, is waiting.
    #[test]
    fn chat_awaiting_after_ten_minutes() {
        let p = ProjectApp::new(json!({}));
        fill_variables(&p);
        chat::chat_copy_and_open(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "1.1", "entry": "stageView" })),
        )
        .unwrap();
        assert!(
            !kinds(&today(&p)).contains(&InboxKind::ChatAwaitingPaste),
            "just copied"
        );
        let now = p.t.os.now();
        p.t.os.set_now(
            now.checked_add(jiff::SignedDuration::from_secs(9 * 60 + 59))
                .unwrap(),
        );
        assert!(
            !kinds(&today(&p)).contains(&InboxKind::ChatAwaitingPaste),
            "9 min 59 s"
        );
        p.t.os.set_now(
            now.checked_add(jiff::SignedDuration::from_secs(10 * 60))
                .unwrap(),
        );
        let items = today(&p);
        let i = items
            .iter()
            .find(|i| i.kind == InboxKind::ChatAwaitingPaste)
            .unwrap_or_else(|| panic!("waiting at ten minutes: {items:?}"));
        assert_eq!(i.step.as_ref().map(|s| s.as_str()), Some("1.1"));
        assert_eq!(i.project.as_str(), p.id);
        assert!(i.detail.contains("10 minutes"), "{}", i.detail);
        // Marking the step done clears it.
        projects::project_open(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        foreman_app::ipc::commands::steps::step_mark_done(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "1.1" })),
        )
        .unwrap();
        assert!(
            !kinds(&today(&p)).contains(&InboxKind::ChatAwaitingPaste),
            "done"
        );
    }

    /// A packet that is valid but whose gate is open has landed; an invalid one lists its problems;
    /// the project's sidebar badge and the Dock badge count them (AC-F017-07, -09).
    #[test]
    fn packet_landed_and_invalid() {
        let p = ProjectApp::new(json!({}));
        write_packet(&p, 1, &unsigned(&tradelog_packet(1)));
        let items = today(&p);
        let landed: Vec<_> = items
            .iter()
            .filter(|i| i.kind == InboxKind::PacketLanded)
            .collect();
        assert_eq!(landed.len(), 1, "{:?}", kinds(&items));
        assert_eq!(landed[0].stage.map(|s| s.0), Some(1));
        assert_eq!(landed[0].title, "Stage 1 packet landed");
        // Break the packet: invalid, with the first problems named.
        let broken = unsigned(&tradelog_packet(1)).replacen("OPEN ITEMS", "OPEN THINGS", 1);
        write_packet(&p, 1, &broken);
        let items = today(&p);
        let bad: Vec<_> = items
            .iter()
            .filter(|i| i.kind == InboxKind::PacketInvalid)
            .collect();
        assert_eq!(bad.len(), 1, "{:?}", kinds(&items));
        assert!(
            bad[0].title.starts_with("Stage 1 packet has "),
            "{}",
            bad[0].title
        );
        assert!(!bad[0].detail.is_empty());
        assert!(!kinds(&items).contains(&InboxKind::PacketLanded));
    }

    /// A project with nothing waiting contributes nothing.
    #[test]
    fn quiet_project_has_no_items() {
        let p = ProjectApp::new(json!({}));
        assert_eq!(today(&p), Vec::<InboxItem>::new());
    }

    /// AC-F017-09: the Dock badge counts the first four kinds; a notifying item that appears
    /// while Foreman isn't focused fires once; one that appeared while focused is already seen;
    /// one that leaves and returns fires again.
    #[test]
    fn badge_and_notifications() {
        let p = ProjectApp::new(json!({}));
        let app = &p.t.app;
        let os = &p.t.os;
        let blocked = item("sessionBlocked", &p.id, 2);
        let landed = item("packetLanded", &p.id, 4);
        let invalid = item("packetInvalid", &p.id, 5);
        let gate = item("gateSignable", &p.id, 6);

        // Focused: counted, not notified.
        os.set_focused(true);
        ib::sync_items(app, &[blocked.clone(), landed.clone(), gate.clone()]);
        assert_eq!(os.badge(), Some(2));
        assert!(os.notices().is_empty());

        // Unfocused: the same items are already seen; a new one notifies once.
        os.set_focused(false);
        ib::sync_items(app, &[blocked.clone(), landed.clone(), gate.clone()]);
        assert!(os.notices().is_empty(), "already seen while focused");
        let mut second = landed.clone();
        second.stage = Some(foreman_app::ipc::types::Stage(9));
        ib::sync_items(app, &[blocked.clone(), landed.clone(), second.clone()]);
        let n = os.notices();
        assert_eq!(n.len(), 1, "{n:?}");
        assert_eq!(
            n[0].body,
            "TradeLog: the Stage 9 packet is ready to review."
        );
        assert_eq!(os.badge(), Some(3));
        // No repeat while it stays.
        ib::sync_items(app, &[blocked.clone(), landed.clone(), second.clone()]);
        assert_eq!(os.notices().len(), 1);
        // The invalid packet and the signable gate count differently: invalid is badged but
        // silent; signable is neither.
        ib::sync_items(app, &[invalid.clone(), gate.clone()]);
        assert_eq!(os.badge(), Some(1));
        assert_eq!(os.notices().len(), 1);
        // It leaves, and returns: notified again.
        ib::sync_items(app, &[]);
        assert_eq!(os.badge(), None);
        ib::sync_items(app, std::slice::from_ref(&blocked));
        let n = os.notices();
        assert_eq!(n.len(), 2, "{n:?}");
        assert!(
            n[1].body
                .starts_with("TradeLog: a session needs your permission")
        );
    }

    /// A wait whose date has come is an inbox item and notifies (AC-F007-09), also when Foreman
    /// was closed over the date; it doesn't repeat within one run of the app.
    #[test]
    fn wait_elapsed_item_and_notice() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        let until =
            p.t.os
                .today()
                .checked_add(jiff::Span::new().days(3))
                .unwrap();
        projects::condition_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "condition": "commercial", "to": "yes", "note": "Sold to others" })),
        )
        .unwrap();
        foreman_app::ipc::commands::steps::wait_start(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "1.3b", "until": until.to_string() })),
        )
        .unwrap();
        assert!(!kinds(&today(&p)).contains(&InboxKind::WaitElapsed));
        let later =
            p.t.os
                .now()
                .checked_add(jiff::SignedDuration::from_secs(4 * 24 * 3600))
                .unwrap();
        p.t.os.set_now(later);
        let items = today(&p);
        let w: Vec<_> = items
            .iter()
            .filter(|i| i.kind == InboxKind::WaitElapsed)
            .cloned()
            .collect();
        assert_eq!(w.len(), 1, "{:?}", kinds(&items));
        ib::notify_waits(&p.t.app, &w);
        ib::notify_waits(&p.t.app, &w);
        let n = p.t.os.notices();
        assert_eq!(n.len(), 1, "{n:?}");
        assert_eq!(n[0].title, "A wait is over");
    }
}
