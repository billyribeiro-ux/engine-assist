//! M4-27 and M4-28: the knowledge manifest and the build-package assembler through the IPC
//! commands (AC-F023-01…-06, AC-F021-06; T-003, T-051, T-052, T-090).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "integration-test code (tests/): unwrap/expect are denied only outside tests (SR-62)"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::path::PathBuf;

use common::project::{ProjectApp, git_identity, req};
use foreman_app::ipc::commands::{assembler as asm, packets, projects};
use foreman_app::ipc::dto::gates::{Assembler, AssemblerState};
use foreman_app::ipc::dto::records::PacketView;
use foreman_app::ipc::error::IpcErrorCode;
use foreman_app::ipc::types::TriState;
use serde_json::json;

fn fixture_packet(n: u8) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../crates/foreman-core/tests/fixtures/tradelog/handoff/stage-{n:02}.md"
    )))
    .unwrap()
}

fn write(p: &ProjectApp, rel: &str, text: &str) {
    let f = p.project.join(rel);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, text).unwrap();
}

fn stage4_packet(entries: &[&str]) -> String {
    let mut s = String::from(
        "=== HANDOFF PACKET · Stage 4 → Stage 5 ===\nPROJECT: TradeLog\nAPPROVED BY: <blank>\nREPO FILES (create exactly these):\n",
    );
    for e in entries {
        s.push_str("  ");
        s.push_str(e);
        s.push('\n');
    }
    s.push_str("STACK + VERSIONS: SvelteKit 3.1.0\nMILESTONES: M0\nFIRST STAGE: Stage 5 · Site Prep · Claude Code CLI · opusplan · medium · Plan\nBLUEPRINT VERSION: 2.2\nOPEN ITEMS: none\n");
    s
}

fn view(p: &ProjectApp) -> Assembler {
    asm::assembler_view(&p.t.app, req(json!({ "projectId": p.id }))).unwrap()
}

fn row<'a>(a: &'a Assembler, path: &str) -> &'a foreman_app::ipc::dto::gates::AssemblerRow {
    a.rows
        .iter()
        .find(|r| r.path == path)
        .unwrap_or_else(|| panic!("{path} not a row"))
}

/// Signs Stage 1 by confirming an outside signature (the M3-21 path).
fn sign_stage_1(p: &ProjectApp) {
    write(p, "docs/handoff/stage-01.md", &fixture_packet(1));
    let v: PacketView =
        packets::packet_view(&p.t.app, req(json!({ "projectId": p.id, "stage": 1 }))).unwrap();
    p.t.os.answer_next(true);
    packets::packet_signature_confirm(
        &p.t.app,
        req(json!({ "projectId": p.id, "stage": 1, "seenSha256": v.sha256.unwrap().as_str() })),
    )
    .unwrap();
}

mod manifest {
    use super::*;

    /// AC-F023-02: no signed packets or chat outputs → an empty manifest pointing at the first
    /// step that produces one.
    #[test]
    fn empty_points_to_first_producer() {
        let p = ProjectApp::new(json!({}));
        let m = asm::manifest_view(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        // Nothing is signed, and the chat output on the path (2.1) doesn't apply to a product
        // that isn't commercial: the manifest is empty and names 1.4, the step whose packet
        // will be the first file to upload.
        assert!(m.files.is_empty(), "{:?}", m.files);
        assert_eq!((m.ready, m.total), (0, 0));
        assert_eq!(m.first_producer.as_ref().map(|k| k.as_str()), Some("1.4"));
    }

    /// AC-F023-01: a signed packet and a declared chat output are listed with their producer;
    /// marking one "in the claude.ai Project" counts it ready; only listed files can be marked.
    #[test]
    fn lists_signed_packets_and_chat_outputs() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        // 2.1 (the teardown, a chat step) applies only to a commercial product.
        projects::condition_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "condition": "commercial", "to": "yes", "note": "Sold" })),
        )
        .unwrap();
        sign_stage_1(&p);
        write(&p, "spec/02-teardown.md", "# Teardown\n");
        let m = asm::manifest_view(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        let packet = m
            .files
            .iter()
            .find(|f| f.path.as_str() == "docs/handoff/stage-01.md")
            .expect("the signed Stage 1 packet is listed");
        assert!(packet.exists);
        assert_eq!(packet.producer.as_ref().unwrap().as_str(), "1.4");
        let teardown = m
            .files
            .iter()
            .find(|f| f.path.as_str() == "spec/02-teardown.md")
            .expect("2.1's chat output is listed");
        assert!(teardown.exists);
        assert_eq!(teardown.producer.as_ref().unwrap().as_str(), "2.1");
        // CLI outputs aren't chat knowledge; unsigned packets aren't listed.
        assert!(!m.files.iter().any(|f| f.path.as_str().contains("council/")));
        assert!(
            !m.files
                .iter()
                .any(|f| f.path.as_str() == "docs/handoff/stage-02.md")
        );
        assert_eq!(m.ready, 0);
        assert_eq!(m.total, u32::try_from(m.files.len()).unwrap());
        assert!(m.first_producer.is_none());
        let m = asm::manifest_mark(
            &p.t.app,
            req(
                json!({ "projectId": p.id, "path": "docs/handoff/stage-01.md", "inProject": true }),
            ),
        )
        .unwrap();
        assert_eq!(m.ready, 1);
        assert!(
            m.files
                .iter()
                .find(|f| f.path.as_str() == "docs/handoff/stage-01.md")
                .unwrap()
                .in_project
        );
        let again = asm::manifest_view(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        assert_eq!(again.ready, 1, "the mark holds for the session");
        let m = asm::manifest_mark(
            &p.t.app,
            req(json!({ "projectId": p.id, "path": "docs/handoff/stage-01.md", "inProject": false })),
        )
        .unwrap();
        assert_eq!(m.ready, 0);
        let e = asm::manifest_mark(
            &p.t.app,
            req(
                json!({ "projectId": p.id, "path": "docs/handoff/stage-02.md", "inProject": true }),
            ),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotFound);
    }
}

mod assembler_view {
    use super::*;

    /// AC-F023-03, -05: the 4.8 packet's REPO FILES with exists, missing and empty; the package
    /// is complete only when every row Foreman may write exists; a refused row doesn't block.
    #[test]
    fn rows_and_completeness() {
        let p = ProjectApp::new(json!({}));
        let e = asm::assembler_view(&p.t.app, req(json!({ "projectId": p.id }))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        write(
            &p,
            "docs/handoff/stage-04.md",
            &stage4_packet(&[
                "CLAUDE.md",
                "docs/MASTER-SPEC.md",
                "docs/tickets.md",
                "docs/migrations/",
                ".github/workflows/ci.yml",
            ]),
        );
        write(&p, "docs/MASTER-SPEC.md", "# Spec\n");
        write(&p, "docs/tickets.md", "");
        let a = view(&p);
        assert_eq!(a.rows.len(), 5);
        assert_eq!(row(&a, "CLAUDE.md").state, AssemblerState::Missing);
        assert!(
            row(&a, "CLAUDE.md")
                .note
                .as_deref()
                .unwrap()
                .contains("Claude Code loads this file in every session")
        );
        assert_eq!(row(&a, "docs/MASTER-SPEC.md").state, AssemblerState::Exists);
        assert_eq!(row(&a, "docs/tickets.md").state, AssemblerState::Empty);
        assert_eq!(row(&a, "docs/migrations/").state, AssemblerState::Missing);
        let gh = row(&a, ".github/workflows/ci.yml");
        assert_eq!(gh.state, AssemblerState::Refused);
        assert_eq!(gh.rule.as_deref(), Some("path.denied-component"));
        assert!(!a.complete);
        // Fill everything Foreman may write; the refused row stays and doesn't block.
        write(&p, "CLAUDE.md", "# TradeLog\n");
        write(&p, "docs/tickets.md", "# Tickets\n");
        write(&p, "docs/migrations/0001_init.sql", "create table t();\n");
        let a = view(&p);
        assert_eq!(row(&a, "docs/migrations/").state, AssemblerState::Exists);
        assert_eq!(
            row(&a, ".github/workflows/ci.yml").state,
            AssemblerState::Refused
        );
        assert!(a.complete);
    }

    /// AC-F023-06 / DR-063: undecided late conditions get proposals from paths only; no
    /// evidence stays undecided (never `no`); nothing is recorded until Billy confirms.
    #[test]
    fn late_conditions_from_path_evidence() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        write(&p, "docs/handoff/stage-04.md", &fixture_packet(4));
        let a = view(&p);
        assert_eq!(a.late_conditions.len(), 4);
        let db = a
            .late_conditions
            .iter()
            .find(|c| c.condition == foreman_app::ipc::types::ConditionId::HasDatabase)
            .unwrap();
        assert_eq!(db.value, TriState::Undecided);
        assert_eq!(db.proposed, Some(TriState::Yes));
        assert_eq!(
            db.evidence.as_deref(),
            Some("docs/migrations/0001_init.sql")
        );
        for c in a
            .late_conditions
            .iter()
            .filter(|c| c.condition != foreman_app::ipc::types::ConditionId::HasDatabase)
        {
            assert_eq!(c.value, TriState::Undecided);
            assert_eq!(c.proposed, None, "{:?}", c.condition);
            assert_eq!(c.evidence, None);
        }
        // Path evidence in the project itself (never contents).
        write(&p, "src/webhooks/broker.ts", "export {}\n");
        let a = view(&p);
        let i = a
            .late_conditions
            .iter()
            .find(|c| c.condition == foreman_app::ipc::types::ConditionId::HasIntegrations)
            .unwrap();
        assert_eq!(i.proposed, Some(TriState::Yes));
        assert_eq!(i.evidence.as_deref(), Some("src/webhooks/"));
        // The proposal took no effect: project.json still says undecided.
        let project: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(p.project.join(".foreman/project.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(project["conditions"]["has_database"], "undecided");
        // Billy confirms through condition_set with the evidence note.
        projects::condition_set(
            &p.t.app,
            req(
                json!({ "projectId": p.id, "condition": "has_database", "to": "yes",
                        "note": "from evidence: docs/migrations/0001_init.sql" }),
            ),
        )
        .unwrap();
        let a = view(&p);
        let db = a
            .late_conditions
            .iter()
            .find(|c| c.condition == foreman_app::ipc::types::ConditionId::HasDatabase)
            .unwrap();
        assert_eq!(db.value, TriState::Yes);
        assert_eq!(db.proposed, None, "a decided condition gets no proposal");
        assert!(
            db.decided
                .as_deref()
                .unwrap()
                .contains("from evidence: docs/migrations/0001_init.sql")
        );
    }
}

mod assembler {
    use super::*;

    /// AC-F023-04 (T-051, T-052, T-090): a malicious REPO FILES entry for each policy case is
    /// a refused row, and Paste into refuses it before reading the clipboard; nothing is written.
    #[test]
    fn rejects_policy_violations() {
        let p = ProjectApp::new(json!({}));
        let bad = [
            ("../escape.md", "path.dotdot"),
            ("/etc/passwd", "path.absolute"),
            ("docs/a\\b.md", "path.backslash"),
            (".git/hooks/pre-commit", "path.denied-component"),
            (".claude/settings.json", "path.denied-component"),
            ("spec/.claude/agents/x.md", "path.denied-component"),
            (".github/workflows/ci.yml", "path.denied-component"),
            ("docs/.mcp.json", "path.denied-name"),
            ("docs/CLAUDE.md", "path.claude-md"),
            ("src/main.rs", "path.not-allowlisted"),
            ("docs/.env", "path.secret"),
            ("docs/\u{202e}dm.md", "path.bidi"),
        ];
        let entries: Vec<&str> = bad.iter().map(|(e, _)| *e).collect();
        write(&p, "docs/handoff/stage-04.md", &stage4_packet(&entries));
        let a = view(&p);
        p.t.os.set_clipboard(Some("planted"));
        for (path, rule) in bad {
            let r = a
                .rows
                .iter()
                .find(|r| r.rule.as_deref() == Some(rule) && r.state == AssemblerState::Refused);
            assert!(r.is_some(), "{path:?} → {rule}: {:?}", a.rows);
            let e = asm::assembler_paste_preview(
                &p.t.app,
                req(json!({ "projectId": p.id, "path": path })),
            )
            .unwrap_err();
            assert_eq!(e.code, IpcErrorCode::PolicyViolation, "{path:?}");
        }
        // Not a REPO FILES row at all.
        let e = asm::assembler_paste_preview(
            &p.t.app,
            req(json!({ "projectId": p.id, "path": "docs/other.md" })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotFound);
        assert!(!p.project.join("escape.md").exists());
        assert!(!p.project.join("src/main.rs").exists());
        assert!(!p.project.join(".github").exists());
        // Every row is refused: none blocks "the package is complete" (Billy creates them).
        assert!(a.complete);
    }

    /// AC-F023-03: Paste into lists the target with its diff before writing, overwrites only
    /// by compare-and-swap, and a file changed since the preview writes nothing.
    #[test]
    fn paste_into_lists_target_and_writes_by_cas() {
        let p = ProjectApp::new(json!({}));
        write(
            &p,
            "docs/handoff/stage-04.md",
            &stage4_packet(&["docs/MASTER-SPEC.md", "docs/tickets.md"]),
        );
        write(&p, "docs/MASTER-SPEC.md", "old spec\n");
        p.t.os.set_clipboard(Some("```\n# Master spec\nv1\n```"));
        let pv = asm::assembler_paste_preview(
            &p.t.app,
            req(json!({ "projectId": p.id, "path": "docs/MASTER-SPEC.md" })),
        )
        .unwrap();
        assert_eq!(pv.targets.len(), 1);
        assert_eq!(pv.targets[0].path.as_str(), "docs/MASTER-SPEC.md");
        assert!(pv.notes.iter().any(|n| n.contains("Overwrites")));
        assert_eq!(
            std::fs::read_to_string(p.project.join("docs/MASTER-SPEC.md")).unwrap(),
            "old spec\n",
            "the preview writes nothing"
        );
        let a = asm::assembler_paste_apply(
            &p.t.app,
            req(json!({ "previewId": pv.preview_id.as_str() })),
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(p.project.join("docs/MASTER-SPEC.md")).unwrap(),
            "# Master spec\nv1\n"
        );
        assert_eq!(row(&a, "docs/MASTER-SPEC.md").state, AssemblerState::Exists);
        // A one-time preview.
        let e = asm::assembler_paste_apply(
            &p.t.app,
            req(json!({ "previewId": pv.preview_id.as_str() })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Expired);
        // Changed on disk since the preview: Conflict, nothing written.
        p.t.os.set_clipboard(Some("tickets v1"));
        let pv = asm::assembler_paste_preview(
            &p.t.app,
            req(json!({ "projectId": p.id, "path": "docs/tickets.md" })),
        )
        .unwrap();
        write(&p, "docs/tickets.md", "written by an editor\n");
        let e = asm::assembler_paste_apply(
            &p.t.app,
            req(json!({ "previewId": pv.preview_id.as_str() })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Conflict);
        assert_eq!(
            std::fs::read_to_string(p.project.join("docs/tickets.md")).unwrap(),
            "written by an editor\n"
        );
    }
}

mod ipc {
    use super::*;

    /// AC-F021-06 (T-003, SR-24a): root `CLAUDE.md` is written only after the native
    /// confirmation Rust raises ("Claude Code loads this file in every session"); Cancel writes
    /// nothing. The request carries no approval.
    #[test]
    fn security_confirmations_are_native() {
        let p = ProjectApp::new(json!({}));
        write(
            &p,
            "docs/handoff/stage-04.md",
            &stage4_packet(&["CLAUDE.md", "docs/MASTER-SPEC.md"]),
        );
        p.t.os
            .set_clipboard(Some("# TradeLog\nNever float money.\n"));
        let preview = || {
            asm::assembler_paste_preview(
                &p.t.app,
                req(json!({ "projectId": p.id, "path": "CLAUDE.md" })),
            )
            .unwrap()
        };
        // An approval field is refused by the request type itself (IPC-R1, IPC-R11).
        assert!(
            serde_json::from_value::<foreman_app::ipc::dto::requests::AssemblerPasteApplyReq>(
                json!({ "previewId": "a3bb189e-8bf9-4888-9912-ace4e6543002", "approved": true })
            )
            .is_err()
        );
        let pv = preview();
        assert!(
            pv.notes
                .iter()
                .any(|n| n.contains("Claude Code loads this file in every session"))
        );
        // Cancel: nothing written.
        let asked = p.t.os.asked().len();
        p.t.os.answer_next(false);
        let e = asm::assembler_paste_apply(
            &p.t.app,
            req(json!({ "previewId": pv.preview_id.as_str() })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert_eq!(p.t.os.asked().len(), asked + 1, "a native dialog asked");
        let c = &p.t.os.asked()[asked];
        assert!(
            c.message
                .contains("Claude Code loads this file in every session"),
            "{c:?}"
        );
        assert!(!p.project.join("CLAUDE.md").exists());
        // OK: written.
        let pv = preview();
        p.t.os.answer_next(true);
        let a = asm::assembler_paste_apply(
            &p.t.app,
            req(json!({ "previewId": pv.preview_id.as_str() })),
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(p.project.join("CLAUDE.md")).unwrap(),
            "# TradeLog\nNever float money.\n"
        );
        assert_eq!(row(&a, "CLAUDE.md").state, AssemblerState::Exists);
        // Any other row writes without a dialog.
        let asked = p.t.os.asked().len();
        let pv = asm::assembler_paste_preview(
            &p.t.app,
            req(json!({ "projectId": p.id, "path": "docs/MASTER-SPEC.md" })),
        )
        .unwrap();
        asm::assembler_paste_apply(
            &p.t.app,
            req(json!({ "previewId": pv.preview_id.as_str() })),
        )
        .unwrap();
        assert_eq!(p.t.os.asked().len(), asked);
    }
}
