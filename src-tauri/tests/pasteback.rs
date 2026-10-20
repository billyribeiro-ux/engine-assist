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

fn preview_kind(p: &ProjectApp, step: &str, kind: &str, source: &str) -> Result<Value, IpcError> {
    chat::paste_back_preview(
        &p.t.app,
        req(json!({ "projectId": p.id, "step": step, "kind": kind, "source": source })),
    )
    .map(|v| serde_json::to_value(v).unwrap())
}

fn target_paths(pv: &Value) -> Vec<String> {
    pv["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["path"].as_str().unwrap().to_owned())
        .collect()
}

fn not_allowed_rule(e: &IpcError) -> String {
    match &e.detail {
        Some(IpcErrorDetail::NotAllowed { rule }) => rule.clone(),
        other => panic!("not NotAllowed: {e:?} {other:?}"),
    }
}

/// F-016 (M4-18): `kind: "lessons"` appends to `docs/lessons.md` through `foreman_core::lessons`.
mod lessons {
    use super::*;

    const ENTRY: &str = "## L-001 · Stage 1 · 2026-10-20 · Packets ran long\nWHAT HAPPENED: the brief repeated the scope\nCAUGHT BY: me\nSTATUS: open\n";

    pub(super) fn file(p: &ProjectApp) -> std::path::PathBuf {
        p.project.join("docs/lessons.md")
    }

    /// The setup header plus one entry already there, so the paste's `L-001` is renumbered.
    pub(super) fn seed(p: &ProjectApp) -> String {
        let header = std::fs::read_to_string(file(p)).unwrap();
        assert!(header.starts_with("# Lessons — TradeLog"), "{header}");
        let text =
            format!("{header}\n## L-001 · Stage 0 · 2026-10-19 · Idea was vague\nSTATUS: open\n");
        std::fs::write(file(p), &text).unwrap();
        text
    }

    mod pasteback {
        use super::*;

        /// AC-F016-02, SR-35: the one target (`docs/lessons.md`) is listed with its diff before
        /// anything is written; entries take the next free `L-NNN`; the header and the existing
        /// entries are kept byte for byte; a valid append makes the F.2 step done.
        #[test]
        fn lists_targets_before_write() {
            let p = ProjectApp::new(json!({}));
            let before = seed(&p);
            p.t.os.set_clipboard(Some(&format!(
                "Here are this stage's lessons:\n```markdown\n{ENTRY}```\n"
            )));
            let pv = preview_kind(&p, "1:F.2", "lessons", "clipboard").unwrap();
            assert_eq!(pv["kind"], "lessons");
            assert_eq!(target_paths(&pv), ["docs/lessons.md"]);
            assert_eq!(pv["targets"][0]["exists"], true);
            let hunks = pv["targets"][0]["hunks"].to_string();
            assert!(hunks.contains("L-002 · Stage 1"), "{hunks}");
            assert!(
                !hunks.contains("\"op\":\"del\""),
                "an append deletes nothing: {hunks}"
            );
            assert!(
                pv["notes"].to_string().contains("L-001 becomes L-002"),
                "{}",
                pv["notes"]
            );
            assert_eq!(pv["problems"], json!([]));
            assert_eq!(std::fs::read_to_string(file(&p)).unwrap(), before);

            let r = apply(&p, &pv["previewId"]).unwrap();
            assert_eq!(r["written"], json!(["docs/lessons.md"]));
            let after = std::fs::read_to_string(file(&p)).unwrap();
            assert_eq!(
                after,
                format!("{before}\n{}", ENTRY.replacen("L-001", "L-002", 1))
            );
            assert_eq!(r["stepDone"], true);
            assert_eq!(state(&p, "1:F.2"), "done");
        }
    }

    /// AC-F016-03: the file changed on disk after the preview: the write is refused (nothing
    /// written), and a new preview diffs against the new content.
    #[test]
    fn changed_on_disk_is_refused_and_rediffed() {
        let p = ProjectApp::new(json!({}));
        let before = seed(&p);
        p.t.os.set_clipboard(Some(ENTRY));
        let pv = preview_kind(&p, "1:F.2", "lessons", "clipboard").unwrap();
        let edited = format!("{before}\n## L-002 · Stage 1 · 2026-10-20 · Added by hand\n");
        std::fs::write(file(&p), &edited).unwrap();
        let e = apply(&p, &pv["previewId"]).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Conflict, "{e:?}");
        assert!(
            matches!(&e.detail, Some(IpcErrorDetail::Conflict { path, written }) if path.as_str() == "docs/lessons.md" && written.is_empty())
        );
        assert_eq!(std::fs::read_to_string(file(&p)).unwrap(), edited);
        let again = preview_kind(&p, "1:F.2", "lessons", "clipboard").unwrap();
        assert!(again["notes"].to_string().contains("L-001 becomes L-003"));
        apply(&p, &again["previewId"]).unwrap();
        assert!(
            std::fs::read_to_string(file(&p))
                .unwrap()
                .ends_with(&ENTRY.replacen("L-001", "L-003", 1))
        );
    }

    /// AC-F016-04: a missing `docs/lessons.md` refuses; nothing is written (it isn't recreated).
    #[test]
    fn missing_file_writes_nothing() {
        let p = ProjectApp::new(json!({}));
        std::fs::remove_file(file(&p)).unwrap();
        p.t.os.set_clipboard(Some(ENTRY));
        let e = preview_kind(&p, "1:F.2", "lessons", "clipboard").unwrap_err();
        assert_eq!(not_allowed_rule(&e), "lessons-missing");
        assert!(
            e.message.contains("docs/lessons.md is missing"),
            "{}",
            e.message
        );
        assert!(!file(&p).exists());
    }

    /// Only a step that appends to `docs/lessons.md` takes lessons; a paste without an entry is
    /// refused.
    #[test]
    fn refusals() {
        let p = ProjectApp::new(json!({}));
        p.t.os.set_clipboard(Some(ENTRY));
        let e = preview_kind(&p, "1.4", "lessons", "clipboard").unwrap_err();
        assert_eq!(not_allowed_rule(&e), "not-a-lessons-step");
        p.t.os.set_clipboard(Some("No lessons this time."));
        let e = preview_kind(&p, "1:F.2", "lessons", "clipboard").unwrap_err();
        assert_eq!(not_allowed_rule(&e), "no-lessons-entries");
    }
}

/// AC-F012-15 (M4-24): `kind: "adrSet"` splits on `^# ADR-\d{3}:` through
/// `foreman_core::packet::adr_split`.
mod adr_set {
    use super::*;

    const ADRS: &str = "Here are the ADRs.\n\n# ADR-001: Tauri for the shell\nStatus: Proposed\nDecision: Tauri 2.\n\n# ADR-002: SQLite for storage\nStatus: Proposed\nDecision: one file <script>alert(1)</script>.\n";

    fn adr(p: &ProjectApp, n: &str) -> std::path::PathBuf {
        p.project.join(format!("docs/adr/ADR-{n}.md"))
    }

    mod pasteback {
        use super::*;

        /// AC-F012-15, SR-35: every ADR file is listed, an existing one diffed against its
        /// current text, before anything is written; apply writes exactly those files.
        #[test]
        fn lists_targets_before_write() {
            let p = ProjectApp::new(json!({}));
            std::fs::create_dir_all(p.project.join("docs/adr")).unwrap();
            std::fs::write(adr(&p, "002"), "# ADR-002: Postgres\nStatus: Proposed\n").unwrap();
            p.t.os.set_clipboard(Some(ADRS));
            let pv = preview_kind(&p, "4.2", "adrSet", "clipboard").unwrap();
            assert_eq!(pv["kind"], "adrSet");
            assert_eq!(
                target_paths(&pv),
                ["docs/adr/ADR-001.md", "docs/adr/ADR-002.md"]
            );
            assert_eq!(pv["targets"][0]["exists"], false);
            assert_eq!(pv["targets"][1]["exists"], true);
            let second = pv["targets"][1]["hunks"].to_string();
            assert!(
                second.contains("\"op\":\"del\"") && second.contains("Postgres"),
                "{second}"
            );
            assert!(
                second.contains("<script>alert(1)</script>"),
                "escaped text, not HTML"
            );
            assert!(pv["notes"].to_string().contains("split into 2 ADR files"));
            // Nothing written by the preview.
            assert!(!adr(&p, "001").exists());
            assert!(
                std::fs::read_to_string(adr(&p, "002"))
                    .unwrap()
                    .contains("Postgres")
            );

            let r = apply(&p, &pv["previewId"]).unwrap();
            assert_eq!(
                r["written"],
                json!(["docs/adr/ADR-001.md", "docs/adr/ADR-002.md"])
            );
            assert_eq!(
                std::fs::read_to_string(adr(&p, "001")).unwrap(),
                "# ADR-001: Tauri for the shell\nStatus: Proposed\nDecision: Tauri 2.\n"
            );
            assert!(
                std::fs::read_to_string(adr(&p, "002"))
                    .unwrap()
                    .starts_with("# ADR-002: SQLite for storage\n")
            );
        }
    }

    /// AC-F012-08: an ADR changed on disk after the preview writes nothing at all (compare-and-
    /// swap, every stamp checked before the first write).
    #[test]
    fn changed_target_writes_nothing() {
        let p = ProjectApp::new(json!({}));
        std::fs::create_dir_all(p.project.join("docs/adr")).unwrap();
        std::fs::write(adr(&p, "002"), "old\n").unwrap();
        p.t.os.set_clipboard(Some(ADRS));
        let pv = preview_kind(&p, "4.2", "adrSet", "clipboard").unwrap();
        std::fs::write(adr(&p, "002"), "edited elsewhere\n").unwrap();
        let e = apply(&p, &pv["previewId"]).unwrap_err();
        assert!(
            matches!(&e.detail, Some(IpcErrorDetail::Conflict { path, written }) if path.as_str() == "docs/adr/ADR-002.md" && written.is_empty()),
            "{e:?}"
        );
        assert!(!adr(&p, "001").exists());
        assert_eq!(
            std::fs::read_to_string(adr(&p, "002")).unwrap(),
            "edited elsewhere\n"
        );
    }

    /// No ADR heading, or one ADR twice: refused, nothing written.
    #[test]
    fn refusals() {
        let p = ProjectApp::new(json!({}));
        p.t.os
            .set_clipboard(Some("## ADR-001: wrong level\n# ADR-01: two digits\n"));
        let e = preview_kind(&p, "4.2", "adrSet", "clipboard").unwrap_err();
        assert_eq!(not_allowed_rule(&e), "no-adrs");
        p.t.os.set_clipboard(Some("# ADR-001: a\n\n# ADR-001: b\n"));
        let e = preview_kind(&p, "4.2", "adrSet", "clipboard").unwrap_err();
        assert_eq!(not_allowed_rule(&e), "duplicate-adr");
        assert!(!p.project.join("docs/adr/ADR-001.md").exists());
    }
}

/// AC-F013-14 (M4-23): Capture from session is `paste_back_preview { source: "sensor" }`: the
/// step's last run's `last_assistant_message`, then the same path as a clipboard paste.
mod capture {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;
    use foreman_app::project;
    use foreman_core::ids::StepKey;
    use foreman_core::schema::progress::StoredState;

    /// A finished run of `step` with a sensor token, its `Stop` event carrying `message`, and the
    /// step's `lastRunId` pointing at it.
    fn run_with_message(p: &ProjectApp, step: &str, token: Option<&str>, message: &str) {
        let run_id = uuid::Uuid::new_v4().to_string();
        let mode = |f: &std::path::Path, m: u32| {
            std::fs::set_permissions(f, std::fs::Permissions::from_mode(m)).unwrap();
        };
        let dir = p.t.app_data().join("runs").join(&run_id);
        std::fs::create_dir_all(&dir).unwrap();
        mode(&dir, 0o700);
        let run = json!({
            "argv": ["claude"], "chainId": null, "claudeVersion": "2.1.304", "copyPromptOnCli": false,
            "cwd": "spec", "entryPoint": "stageView", "envNames": ["HOME"], "exitCode": 0,
            "intended": { "effort": "high", "mode": "default", "model": "opus" }, "kind": "launch",
            "name": "fm-tradelog-s02-2-6", "observed": null, "override": false, "pid": null,
            "projectId": p.id, "promptSha256": null, "runId": run_id, "runInputInterventions": 0,
            "runSettingsSha256": null, "schemaVersion": 1, "sensorToken": token, "stage": 2,
            "status": "exited", "stepId": step, "surface": "cli",
            "t": { "confirmed": null, "exited": null, "lookStart": null, "prepared": null, "spawned": null, "wallStart": "2026-10-20T09:00:00Z" },
            "timeToStartMs": null
        });
        let f = dir.join("run.json");
        std::fs::write(&f, serde_json::to_string_pretty(&run).unwrap()).unwrap();
        mode(&f, 0o600);
        if let Some(token) = token {
            let events = p.t.app_data().join("events");
            let ev = events.join(&run_id);
            std::fs::create_dir_all(&ev).unwrap();
            mode(&events, 0o700);
            mode(&ev, 0o700);
            let e = json!({
                "event": "Stop", "payload": { "hook_event_name": "Stop", "last_assistant_message": message },
                "receivedAt": "2026-10-20T09:05:00Z", "schemaVersion": 1, "token": token, "truncated": false
            });
            let f = ev.join("00000000000000000001-0000000001-Stop.json");
            std::fs::write(&f, e.to_string()).unwrap();
            mode(&f, 0o600);
        }
        let mut o = project::open(&p.t.app, &p.id).unwrap();
        let mut s = o.step_progress(step);
        s.state = StoredState::Done;
        s.last_run_id = Some(run_id);
        o.progress.steps.insert(StepKey::parse(step).unwrap(), s);
        o.save_progress(&p.t.app).unwrap();
    }

    fn stage_2_packet() -> String {
        std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../crates/foreman-core/tests/fixtures/tradelog/handoff/stage-02.md"),
        )
        .unwrap()
    }

    mod pasteback {
        use super::*;

        /// AC-F013-14, SR-18, SR-35 (capture path): the packet Claude printed instead of writing
        /// is taken from the session sensor, normalized and split like a paste, and every target
        /// (the packet and the step's other `out`) is listed before anything is written; only
        /// Billy's apply writes. The clipboard isn't read.
        #[test]
        fn lists_targets_before_write() {
            let p = ProjectApp::new(json!({}));
            let packet = stage_2_packet();
            run_with_message(
                &p,
                "2.6",
                Some(&"c".repeat(64)),
                &format!("Done. Here is the packet:\n\n{packet}"),
            );
            p.t.os.set_clipboard(Some("clipboard text is not used"));
            let master = p.project.join("spec/MASTER-SPEC.md");
            let master_before = std::fs::read(&master).ok();
            let pv = preview_kind(&p, "2.6", "auto", "sensor").unwrap();
            assert_eq!(pv["kind"], "packet");
            let paths = target_paths(&pv);
            assert_eq!(paths[0], "docs/handoff/stage-02.md");
            assert!(
                paths.contains(&"spec/MASTER-SPEC.md".to_owned()),
                "{paths:?}"
            );
            assert!(pv["notes"].to_string().contains("before the packet header"));
            assert!(!pv.to_string().contains("clipboard text is not used"));
            // Nothing is written by the preview.
            assert!(!p.project.join("docs/handoff/stage-02.md").exists());
            assert_eq!(std::fs::read(&master).ok(), master_before);

            let r = apply(&p, &pv["previewId"]).unwrap();
            let written: Vec<String> = r["written"]
                .as_array()
                .unwrap()
                .iter()
                .map(|w| w.as_str().unwrap().to_owned())
                .collect();
            assert_eq!(written, paths);
            let landed =
                std::fs::read_to_string(p.project.join("docs/handoff/stage-02.md")).unwrap();
            assert!(
                landed.starts_with("=== HANDOFF PACKET · Stage 2"),
                "{landed}"
            );
        }
    }

    /// Without the sensor's reply (no run, a run without a sensor token): `Unsupported`
    /// (`noSensor`), and paste-back is offered instead. Nothing is written.
    #[test]
    fn without_the_sensor_is_unsupported() {
        let p = ProjectApp::new(json!({}));
        let e = preview_kind(&p, "2.6", "auto", "sensor").unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Unsupported);
        assert!(
            matches!(&e.detail, Some(IpcErrorDetail::Unsupported { feature, why }) if feature == "sensor" && why == "noSensor")
        );
        assert!(e.message.contains("paste it back"), "{}", e.message);
        run_with_message(&p, "2.6", None, "ignored");
        let e = preview_kind(&p, "2.6", "auto", "sensor").unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Unsupported);
        assert!(!p.project.join("docs/handoff/stage-02.md").exists());
    }
}
