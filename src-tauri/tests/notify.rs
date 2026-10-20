//! M3-07 / M4-25: notification templates (SR-56, T-098; AC-F007-09, AC-F017-09). Every text is a
//! fixed Foreman sentence; repo, terminal and assistant text never reaches a notification.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]

use foreman_app::ipc::dto::app::{InboxItem, InboxKind};
use foreman_app::notify as nt;
use serde_json::json;

fn item(kind: &str, extra: serde_json::Value) -> InboxItem {
    let mut v = json!({
        "kind": kind,
        "project": "7f0c1d3e-3b6a-4c1f-9a53-2f0d6f6f9b10",
        "step": null,
        "stage": null,
        "at": "2026-10-20T12:00:00Z",
        "projectName": "TradeLog",
        "title": "TITLE TEXT FROM THE REPO",
        "detail": "DETAIL TEXT FROM THE TERMINAL",
        "chain": null,
        "run": null
    });
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    serde_json::from_value(v).unwrap()
}

const KINDS: [&str; 12] = [
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

mod notify {
    use super::*;

    /// T-098, SR-56: the whole set of templates, pinned. Only `session-blocked` and
    /// `packet-landed` notify (AC-F017-09); the inbox's own title and detail never appear in a
    /// notification, whatever they hold.
    #[test]
    fn body_template_snapshot() {
        let blocked = nt::for_item(&item(
            "sessionBlocked",
            json!({ "step": "2.3", "stage": 2 }),
        ))
        .expect("session-blocked notifies");
        assert_eq!(blocked.title, "Claude is waiting for you");
        assert_eq!(
            blocked.body,
            "TradeLog: a session needs your permission in step 2.3."
        );
        let landed = nt::for_item(&item("packetLanded", json!({ "stage": 4 })))
            .expect("packet-landed notifies");
        assert_eq!(landed.title, "A packet landed");
        assert_eq!(
            landed.body,
            "TradeLog: the Stage 4 packet is ready to review."
        );
        // Without a step or stage the sentence still reads.
        assert_eq!(
            nt::for_item(&item("sessionBlocked", json!({})))
                .unwrap()
                .body,
            "TradeLog: a session needs your permission."
        );
        assert_eq!(
            nt::for_item(&item("packetLanded", json!({}))).unwrap().body,
            "TradeLog: the packet is ready to review."
        );
        // Every other inbox kind is silent.
        for k in KINDS {
            let n = nt::for_item(&item(k, json!({ "step": "2.3", "stage": 2 })));
            assert_eq!(
                n.is_some(),
                k == "sessionBlocked" || k == "packetLanded",
                "{k}"
            );
            if let Some(n) = n {
                let all = format!("{} {}", n.title, n.body);
                assert!(!all.contains("REPO") && !all.contains("TERMINAL"), "{all}");
            }
        }
        let wait = nt::wait_elapsed("TradeLog", "1.3b");
        assert_eq!(wait.title, "A wait is over");
        assert_eq!(wait.body, "TradeLog: step 1.3b can start now.");
    }

    /// A project name is the only free text in a template: control characters become spaces, runs
    /// of spaces collapse, and a long name is cut.
    #[test]
    fn project_name_is_flattened_and_cut() {
        assert_eq!(nt::name("  Trade\nLog\t\u{7} app "), "Trade Log app");
        let long = "x".repeat(100);
        let cut = nt::name(&long);
        assert_eq!(cut.chars().count(), 40);
        assert!(cut.ends_with('…'));
        let n = nt::for_item(&item(
            "packetLanded",
            json!({ "projectName": "A\u{1b}[31mB", "stage": 1 }),
        ))
        .unwrap();
        assert!(!n.body.contains('\u{1b}'), "{:?}", n.body);
    }

    /// AC-F017-09: the Dock badge counts the first four kinds.
    #[test]
    fn badge_counts_the_first_four_kinds() {
        let counted: Vec<InboxKind> = nt::BADGED.to_vec();
        assert_eq!(
            counted,
            [
                InboxKind::SessionBlocked,
                InboxKind::SessionYourTurn,
                InboxKind::PacketLanded,
                InboxKind::PacketInvalid
            ]
        );
    }
}
