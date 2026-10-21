//! M4-28: the build-package assembler's REPO FILES parsing and policy (AC-F023-03, -04, -05;
//! T-051, T-052, T-090) and DR-063's late-condition evidence (AC-F023-06).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::collections::BTreeMap;

use foreman_core::assembler::{self as asm, RepoFile, RowState};
use foreman_core::ids::{ConditionId, TriState};
use foreman_core::pathpolicy::{PolicyViolation, WritePurpose};
use foreman_core::store::dirs::EntryKind;

fn packet(entries: &[&str]) -> String {
    let mut s = String::from(
        "=== HANDOFF PACKET · Stage 4 → Stage 5 ===\nPROJECT: TradeLog\nAPPROVED BY: <blank>\nREPO FILES (create exactly these):\n",
    );
    for e in entries {
        s.push_str("  ");
        s.push_str(e);
        s.push('\n');
    }
    s.push_str("STACK + VERSIONS: x\nMILESTONES: M0\nOPEN ITEMS: none\n");
    s
}

mod assembler_parse {
    use super::*;

    /// AC-F023-03: REPO FILES of the TradeLog 4.8 fixture, in packet order; remarks dropped.
    #[test]
    fn repo_files_in_packet_order() {
        let text = common::read_fixture("fixtures/tradelog/handoff/stage-04.md");
        let list = asm::repo_files(&text, &[]).unwrap();
        let paths: Vec<&str> = list.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "CLAUDE.md",
                "docs/MASTER-SPEC.md",
                "docs/migrations/0001_init.sql",
                "docs/lessons.md"
            ]
        );
        // The v2.2 template's own list: remarks after the path, `…`, and a folder entry.
        let tpl = packet(&[
            "docs/adr/ADR-001.md …",
            "docs/migrations/",
            "docs/requirements.md   (SR/PR/LC lists)",
            "docs/lessons.md        (empty, with the template header from Appendix F)",
            "docs/lessons.md",
        ]);
        let list = asm::repo_files(&tpl, &[]).unwrap();
        assert_eq!(
            list,
            [
                RepoFile {
                    path: "docs/adr/ADR-001.md".into(),
                    dir: false
                },
                RepoFile {
                    path: "docs/migrations/".into(),
                    dir: true
                },
                RepoFile {
                    path: "docs/requirements.md".into(),
                    dir: false
                },
                RepoFile {
                    path: "docs/lessons.md".into(),
                    dir: false
                },
            ]
        );
        // No REPO FILES field.
        assert!(
            asm::repo_files(
                "=== HANDOFF PACKET · Stage 4 → Stage 5 ===\nPROJECT: x\n",
                &[]
            )
            .is_none()
        );
    }

    /// AC-F023-03, -05: row states, and completeness (a refused row never blocks it).
    #[test]
    fn row_states_and_completeness() {
        let file = RepoFile {
            path: "docs/x.md".into(),
            dir: false,
        };
        let dir = RepoFile {
            path: "docs/migrations/".into(),
            dir: true,
        };
        assert_eq!(asm::row_state(&file, EntryKind::Missing), RowState::Missing);
        assert_eq!(asm::row_state(&file, EntryKind::File(0)), RowState::Empty);
        assert_eq!(asm::row_state(&file, EntryKind::File(9)), RowState::Exists);
        assert_eq!(asm::row_state(&dir, EntryKind::Dir(0)), RowState::Empty);
        assert_eq!(asm::row_state(&dir, EntryKind::Dir(2)), RowState::Exists);
        assert!(matches!(
            asm::row_state(&file, EntryKind::Other),
            RowState::Refused(_)
        ));
        assert!(matches!(
            asm::row_state(&file, EntryKind::Dir(1)),
            RowState::Refused(_)
        ));
        let refused = RowState::Refused("path.denied-component".into());
        assert!(asm::complete(&[RowState::Exists, refused.clone()]));
        assert!(!asm::complete(&[RowState::Exists, RowState::Missing]));
        assert!(!asm::complete(&[RowState::Exists, RowState::Empty]));
        assert!(!asm::complete(&[]));
    }
}

mod assembler {
    use super::*;

    /// AC-F023-04, T-051, T-052, T-090: a malicious REPO FILES entry for each path-policy case is
    /// refused with its rule; only root `CLAUDE.md` takes the assembler's purpose.
    #[test]
    fn rejects_policy_violations() {
        let cases: [(&str, &str); 20] = [
            ("../escape.md", "path.dotdot"),
            ("docs/../../etc/passwd", "path.dotdot"),
            ("/etc/passwd", "path.absolute"),
            ("~/.zshrc", "path.absolute"),
            ("docs/a\\b.md", "path.backslash"),
            ("docs/\u{202e}dm.exe", "path.bidi"),
            ("docs/zero\u{200b}width.md", "path.zero-width"),
            ("docs/\u{7}bell.md", "path.control"),
            ("docs//x.md", "path.empty-component"),
            ("docs/./x.md", "path.empty-component"),
            (".git/hooks/pre-commit", "path.denied-component"),
            (".github/workflows/ci.yml", "path.denied-component"),
            (".claude/settings.json", "path.denied-component"),
            ("spec/.claude/settings.json", "path.denied-component"),
            (".husky/pre-commit", "path.denied-component"),
            ("docs/.mcp.json", "path.denied-name"),
            ("docs/CLAUDE.local.md", "path.denied-name"),
            ("docs/CLAUDE.md", "path.claude-md"),
            ("src/main.rs", "path.not-allowlisted"),
            ("docs/.env", "path.secret"),
        ];
        let mut entries: Vec<&str> = cases.iter().map(|(p, _)| *p).collect();
        entries.push("CLAUDE.md");
        entries.push("docs/MASTER-SPEC.md");
        let list = asm::repo_files(&packet(&entries), &[]).unwrap();
        for (path, rule) in cases {
            // The list keeps the entry (so the row shows as refused) …
            assert!(list.iter().any(|f| f.path == path), "{path:?} not listed");
            // … and the policy refuses it with the expected rule.
            let err = asm::target(path).unwrap_err();
            assert_eq!(err.rule(), rule, "{path:?}: {err}");
            assert!(asm::refused_note(&err).contains("Create this file yourself"));
        }
        let (rel, purpose) = asm::target("CLAUDE.md").unwrap();
        assert_eq!(rel.as_str(), "CLAUDE.md");
        assert_eq!(purpose, WritePurpose::AssemblerClaudeMd);
        let (rel, purpose) = asm::target("docs/MASTER-SPEC.md").unwrap();
        assert_eq!(rel.as_str(), "docs/MASTER-SPEC.md");
        assert_eq!(purpose, WritePurpose::Repo);
        // A folder entry is checked without its trailing slash.
        assert!(asm::target("docs/migrations/").is_ok());
        assert_eq!(
            asm::target(".foreman/project.json").unwrap_err(),
            PolicyViolation::DeniedComponent(".foreman".into())
        );
    }
}

mod rules {
    use super::*;

    fn undecided() -> BTreeMap<ConditionId, TriState> {
        ConditionId::ALL
            .iter()
            .map(|c| (*c, TriState::Undecided))
            .collect()
    }

    /// AC-F023-06 / DR-063: proposals from paths only; no evidence stays undecided (never `no`);
    /// a condition already decided gets no proposal.
    #[test]
    fn late_condition_evidence() {
        use foreman_core::rules::evidence::{late_condition_evidence, matches};
        // TradeLog's fixture: docs/migrations/0001_init.sql → has_database = yes.
        let text = common::read_fixture("fixtures/tradelog/handoff/stage-04.md");
        let list = asm::repo_files(&text, &[]).unwrap();
        let paths: Vec<String> = list.iter().map(|f| f.path.clone()).collect();
        let p = late_condition_evidence(&paths, &undecided());
        assert_eq!(p.len(), 1, "{p:?}");
        let db = &p[&ConditionId::HasDatabase];
        assert_eq!(db.value, TriState::Yes);
        assert_eq!(db.evidence, "docs/migrations/0001_init.sql");
        // Foreman's own Stage 4 list: no migrations, uploads, email or integration paths.
        let foreman = [
            "CLAUDE.md",
            "docs/MASTER-SPEC.md",
            "docs/scope.md",
            "docs/design-doc.md",
            "docs/adr/ADR-001.md",
            "docs/threat-model.md",
            "docs/contracts/ipc.md",
            "docs/lessons.md",
        ];
        assert!(late_condition_evidence(&foreman, &undecided()).is_empty());
        // Each rule of the table.
        for (c, yes) in [
            (ConditionId::HasDatabase, "docs/migrations/0002.sql"),
            (ConditionId::HasDatabase, "db/schema.SQL"),
            (ConditionId::HasDatabase, "prisma/schema.prisma"),
            (ConditionId::HasUploads, "src/uploads/"),
            (ConditionId::HasUploads, "docs/attachments-storage.md"),
            (ConditionId::HasEmail, "docs/email-templates/welcome.md"),
            (ConditionId::HasEmail, "src/Mailer.ts"),
            (ConditionId::HasIntegrations, "docs/integrations/broker.md"),
            (ConditionId::HasIntegrations, "src/webhooks/stripe.ts"),
        ] {
            assert!(matches(c, yes), "{c:?} {yes}");
            let p = late_condition_evidence(&[yes], &undecided());
            assert_eq!(p[&c].evidence, yes);
            assert!(p.values().all(|x| x.value == TriState::Yes), "never no");
        }
        for (c, no) in [
            (ConditionId::HasDatabase, "docs/migrations/"),
            (ConditionId::HasDatabase, "docs/sql-notes/"),
            (ConditionId::HasUploads, "docs/design.md"),
            (ConditionId::HasIntegrations, "docs/integration"),
        ] {
            let _ = no;
            if c == ConditionId::HasIntegrations {
                // A component containing "integration" counts (DR-063).
                assert!(matches(c, no));
            } else {
                assert!(!matches(c, no), "{c:?} {no}");
            }
        }
        // Already decided: never changed, no proposal.
        let mut decided = undecided();
        decided.insert(ConditionId::HasDatabase, TriState::No);
        assert!(late_condition_evidence(&paths, &decided).is_empty());
        // Not a late condition: never proposed.
        let p = late_condition_evidence(&["docs/migrations/1.sql", "payments/"], &undecided());
        assert!(p.keys().all(|c| c.is_late()));
    }
}
