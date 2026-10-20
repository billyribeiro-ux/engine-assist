//! M3-24: paste-back preview and apply against a real project (F-012; AC-F012-06…-09, -12…-14,
//! AC-XC-16; SR-26, SR-33, SR-35, SR-56; T-060, T-089, T-090).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::project::{ProjectApp, git_identity, req};
use foreman_app::ipc::IpcError;
use foreman_app::ipc::commands::{chat, projects, steps};
use foreman_app::ipc::dto::events::Event;
use foreman_app::ipc::error::{IpcErrorCode, IpcErrorDetail};
use serde_json::{Value, json};

const PACKET: &str = "=== HANDOFF PACKET · Stage 1 → Stage 2 ===
PROJECT: TradeLog
APPROVED BY: <blank>
APP: TradeLog — a journal that imports every fill
FOR: active options traders | also: coaches
PROBLEM: \"I lose track of why I took a trade\" (41 of 60 polled)
DOES:
  - imports fills from the broker
  - tags each trade with its setup
CORE LOOP: log after the close, review on Sunday
WOW MOMENT: the first import shows last month's P&L by setup
WITH (must integrate): broker CSV exports
WITH (constraints): evenings only, three months
STACK PREFERENCE: Svelte and Rust, decided in Stage 2
NOT DOING (v1): social features
MONETIZATION HYPOTHESIS: free tool for the trading room
SUCCESS METRICS: 30 weekly active members by week 6
KILL CRITERION: fewer than 10 weekly users after a month
RISKIEST ASSUMPTION: members log every week; v1 tests it with reminders
PRESS RELEASE + FAQ: attached below the packet
OPEN ITEMS: none
NEXT: Stage 2 · Architect's Design Studio · chat + Claude Code council";

const PRESS: &str = "# TradeLog launches\n\nTradeLog <script>alert(1)</script> imports [every fill](https://evil.example/) today.\u{202e}";

fn preview(p: &ProjectApp, step: &str) -> Result<Value, IpcError> {
    chat::paste_back_preview(
        &p.t.app,
        req(json!({ "projectId": p.id, "step": step, "kind": "auto", "source": "clipboard" })),
    )
    .map(|v| serde_json::to_value(v).unwrap())
}

fn apply(p: &ProjectApp, id: &Value) -> Result<Value, IpcError> {
    chat::paste_back_apply(&p.t.app, req(json!({ "previewId": id })))
        .map(|v| serde_json::to_value(v).unwrap())
}

fn state(p: &ProjectApp, step: &str) -> Value {
    let v = steps::step_view(&p.t.app, req(json!({ "projectId": p.id, "step": step }))).unwrap();
    serde_json::to_value(v.summary.state).unwrap()
}

fn handoff(p: &ProjectApp, name: &str) -> std::path::PathBuf {
    p.project.join("docs/handoff").join(name)
}

mod pasteback {
    use super::*;

    /// AC-F012-06, -12, SR-35: every target (packet, attachments) is listed with its diff before
    /// anything is written; apply writes exactly those, and a valid packet makes the step done.
    #[test]
    fn lists_targets_before_write() {
        let p = ProjectApp::new(json!({}));
        p.t.os.set_clipboard(Some(&format!(
            "Here it is:\n```text\n{PACKET}\n```\n\n{PRESS}\n"
        )));
        let pv = preview(&p, "1.4").unwrap();
        assert_eq!(pv["kind"], "packet");
        let paths: Vec<&str> = pv["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["path"].as_str().unwrap())
            .collect();
        assert_eq!(
            paths,
            [
                "docs/handoff/stage-01.md",
                "docs/handoff/stage-01-attachments.md"
            ]
        );
        assert_eq!(pv["problems"], json!([]), "{}", pv["problems"]);
        assert_eq!(pv["packetFields"], json!({ "fromStage": 1, "toStage": 2 }));
        assert!(pv["notes"].to_string().contains("removed code fence"));
        // Nothing is written by the preview.
        assert!(!handoff(&p, "stage-01.md").exists());
        assert!(!handoff(&p, "stage-01-attachments.md").exists());

        // AC-F012-13 / SR-26: pasted HTML and links are text spans; the bidi override is an
        // escape span, never raw.
        let att = pv["targets"][1]["hunks"].to_string();
        assert!(att.contains("<script>alert(1)</script>"));
        assert!(att.contains("[every fill](https://evil.example/)"));
        assert!(att.contains("\"kind\":\"escape\""), "{att}");
        assert!(!att.contains('\u{202e}'));
        assert!(!att.contains("\"kind\":\"link\""));

        let r = apply(&p, &pv["previewId"]).unwrap();
        assert_eq!(
            r,
            json!({ "written": ["docs/handoff/stage-01.md", "docs/handoff/stage-01-attachments.md"], "stepDone": true })
        );
        assert_eq!(
            std::fs::read_to_string(handoff(&p, "stage-01.md")).unwrap(),
            format!("{PACKET}\n")
        );
        assert!(
            std::fs::read_to_string(handoff(&p, "stage-01-attachments.md"))
                .unwrap()
                .starts_with("# TradeLog launches")
        );
        assert_eq!(state(&p, "1.4"), "done");
        assert!(
            p.t.os
                .events()
                .iter()
                .any(|e| matches!(e, Event::PacketChanged(c) if c.stage.0 == 1))
        );
        // One-time preview.
        assert_eq!(
            apply(&p, &pv["previewId"]).unwrap_err().code,
            IpcErrorCode::Expired
        );
    }

    /// AC-F012-11 / DR-137: an invalid packet lists its problems with the fixes offered; written,
    /// it leaves the step open and Copy fix request copies the problems.
    #[test]
    fn invalid_packet_offers_fix_request() {
        let p = ProjectApp::new(json!({}));
        let bad = PACKET
            .replace("OPEN ITEMS: none", "OPEN ITEMS: pricing")
            .replace(
                "KILL CRITERION: fewer than 10 weekly users after a month\n",
                "",
            );
        p.t.os.set_clipboard(Some(&bad));
        let pv = preview(&p, "1.4").unwrap();
        let problems = pv["problems"].as_array().unwrap();
        assert!(!problems.is_empty());
        assert!(
            problems
                .iter()
                .all(|x| x["fix"] == json!(["openInEditor", "copyFixRequest"]))
        );
        let r = apply(&p, &pv["previewId"]).unwrap();
        assert_eq!(r["stepDone"], false);
        assert_ne!(state(&p, "1.4"), "done");
        steps::clipboard_copy(
            &p.t.app,
            req(json!({ "projectId": p.id, "target": "fixRequest", "step": null, "chain": null, "path": null, "stage": 1 })),
        )
        .unwrap();
        let fix = p.t.os.clipboard().unwrap();
        assert!(fix.starts_with("Your packet is missing: "), "{fix}");
        assert!(fix.contains("KILL CRITERION"), "{fix}");
        assert!(fix.contains("OPEN ITEMS must be exactly `none`."), "{fix}");
    }

    /// AC-F012-10: the wrong stage's packet and two packets write nothing.
    #[test]
    fn wrong_stage_and_two_packets_write_nothing() {
        let p = ProjectApp::new(json!({}));
        p.t.os.set_clipboard(Some(
            &PACKET.replace("Stage 1 → Stage 2", "Stage 3 → Stage 4"),
        ));
        let e = preview(&p, "1.4").unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        assert_eq!(
            e.message,
            "This is the Stage 3 packet; this step expects Stage 1."
        );
        assert!(
            matches!(&e.detail, Some(IpcErrorDetail::NotAllowed { rule }) if rule == "wrong-stage-packet")
        );
        p.t.os.set_clipboard(Some(&format!("{PACKET}\n\n{PACKET}")));
        let e = preview(&p, "1.4").unwrap_err();
        assert!(
            matches!(&e.detail, Some(IpcErrorDetail::NotAllowed { rule }) if rule == "two-packets")
        );
        assert!(!p.project.join("docs/handoff/stage-01.md").exists());
    }

    /// AC-F012-09 / SR-56: empty, over 1 MB: an inline error and nothing written.
    #[test]
    fn clipboard_problems() {
        let p = ProjectApp::new(json!({}));
        p.t.os.set_clipboard(None);
        let e = preview(&p, "1.4").unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Clipboard);
        p.t.os.set_clipboard(Some(&"x".repeat(1024 * 1024 + 1)));
        let e = preview(&p, "1.4").unwrap_err();
        assert!(
            matches!(e.detail, Some(IpcErrorDetail::Clipboard { .. })),
            "{e:?}"
        );
        // P2 sources and kinds are typed answers, not errors in disguise.
        let e = chat::paste_back_preview(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "1.4", "kind": "auto", "source": "sensor" })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Unsupported);
    }

    /// AC-F012-14: F.2 appends to docs/lessons.md; P1 paste-back offers no write.
    #[test]
    fn appends_offer_no_write() {
        let p = ProjectApp::new(json!({}));
        p.t.os
            .set_clipboard(Some("## L-001 · Stage 1\nKeep packets short."));
        let e = preview(&p, "1:F.2").unwrap_err();
        assert!(
            matches!(&e.detail, Some(IpcErrorDetail::NotAllowed { rule }) if rule == "appends-in-p1"),
            "{e:?}"
        );
        assert!(e.message.contains("docs/lessons.md"));
    }

    /// AC-F012-08 (path policy): a target folder that is a symlink is refused; nothing written.
    #[test]
    fn policy_refusal_writes_nothing() {
        let p = ProjectApp::new(json!({}));
        let elsewhere = p.root.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        let _ = std::fs::remove_dir_all(p.project.join("docs/handoff"));
        std::os::unix::fs::symlink(&elsewhere, p.project.join("docs/handoff")).unwrap();
        p.t.os.set_clipboard(Some(PACKET));
        let e = preview(&p, "1.4").unwrap_err();
        assert_eq!(e.code, IpcErrorCode::PolicyViolation, "{e:?}");
        assert!(std::fs::read_dir(&elsewhere).unwrap().next().is_none());
    }

    /// AC-XC-16 / PR-03: Foreman keeps no chat content: app data holds neither the prompt nor the
    /// pasted reply (only hashes); the reply lives only in the declared repo files.
    #[test]
    fn no_chat_content_stored() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        for (name, value) in [
            ("MY_ADVANTAGES", "SECRET-ADVANTAGE-42"),
            ("BUDGET_TIME", "x"),
        ] {
            projects::variable_set(
                &p.t.app,
                req(json!({ "projectId": p.id, "name": name, "value": value })),
            )
            .unwrap();
        }
        chat::chat_copy_and_open(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "1.1", "entry": "stageView" })),
        )
        .unwrap();
        p.t.os
            .set_clipboard(Some(&format!("{PACKET}\n\nREPLY-ONLY-IN-REPO")));
        let pv = preview(&p, "1.4").unwrap();
        apply(&p, &pv["previewId"]).unwrap();
        for e in walk(&p.t.app_data()) {
            let text = String::from_utf8_lossy(&std::fs::read(&e).unwrap()).into_owned();
            // The resolved prompt (the template text alone is the cached blueprint).
            assert!(
                !text.contains("My advantages: SECRET-ADVANTAGE-42"),
                "prompt text in {e:?}"
            );
            assert!(!text.contains("REPLY-ONLY-IN-REPO"), "reply in {e:?}");
        }
    }

    fn walk(d: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(d).unwrap() {
            let path = e.unwrap().path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
        out
    }
}

mod atomic {
    use super::*;

    /// AC-F012-08 / SR-33 (paste-back conflict): a target changed on disk after the preview
    /// writes nothing at all (not even the targets before it), and the conflict names the file.
    #[test]
    fn refuses_newer_file() {
        let p = ProjectApp::new(json!({}));
        p.t.os.set_clipboard(Some(&format!("{PACKET}\n\n{PRESS}")));
        let pv = preview(&p, "1.4").unwrap();
        std::fs::create_dir_all(p.project.join("docs/handoff")).unwrap();
        std::fs::write(
            handoff(&p, "stage-01-attachments.md"),
            "Billy's own notes\n",
        )
        .unwrap();
        let e = apply(&p, &pv["previewId"]).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Conflict);
        assert!(
            matches!(&e.detail, Some(IpcErrorDetail::Conflict { path, written })
                if path.as_str() == "docs/handoff/stage-01-attachments.md" && written.is_empty()),
            "{e:?}"
        );
        assert!(!handoff(&p, "stage-01.md").exists(), "nothing written");
        assert_eq!(
            std::fs::read_to_string(handoff(&p, "stage-01-attachments.md")).unwrap(),
            "Billy's own notes\n"
        );
        assert_ne!(state(&p, "1.4"), "done");
        // A fresh preview shows the existing file as an overwrite with its diff (AC-F012-07).
        let pv = preview(&p, "1.4").unwrap();
        assert_eq!(pv["targets"][1]["exists"], true);
        assert!(
            pv["targets"][1]["hunks"]
                .to_string()
                .contains("\"op\":\"del\"")
        );
    }
}
