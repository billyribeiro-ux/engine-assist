//! M1-11: template parser structure, items and bounds (ADR-005, DR-001–DR-015).
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

use common::{blueprint, mini, parse, replace_once, rules};
use foreman_core::template::{self as tpl, StepStatus};

mod template {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// AC-F001-08: CRLF, BOM and tab variants hash identically (source and items).
        #[test]
        fn normalized_hash_crlf_bom_tabs(lines in proptest::collection::vec("[ a-z>*`-]{0,20}", 0..30), bom in any::<bool>(), cr in 0u8..3) {
            let lf = lines.join("\n");
            let mut variant = match cr { 0 => lf.replace('\n', "\r\n"), 1 => lf.replace('\n', "\r"), _ => lf.clone() };
            if bom { variant.insert(0, '\u{feff}'); }
            prop_assert_eq!(tpl::source_sha256(&variant), tpl::source_sha256(&lf));
            // A tab hashes like the spaces to the next 4-column stop.
            let tabbed = format!("{lf}\n\tx");
            let spaced = format!("{lf}\n    x");
            prop_assert_eq!(tpl::source_sha256(&tabbed), tpl::source_sha256(&spaced));
        }
    }

    /// The real blueprint saved with CRLF and a BOM pins identically, items included.
    #[test]
    fn normalized_hash_real_blueprint() {
        let bp = blueprint();
        let crlf = format!("\u{feff}{}", bp.replace('\n', "\r\n"));
        let (a, b) = (parse(&bp), parse(&crlf));
        assert_eq!(a.source_sha256, b.source_sha256);
        assert_eq!(
            a.items.iter().map(|i| &i.text_sha256).collect::<Vec<_>>(),
            b.items.iter().map(|i| &i.text_sha256).collect::<Vec<_>>()
        );
        assert_eq!(
            tpl::normalize_item_text("A\tfresh  `clone`"),
            tpl::normalize_item_text("a fresh clone")
        );
    }

    #[test]
    fn stage_headings() {
        let t = parse(&blueprint());
        assert_eq!(
            t.stages.iter().map(|s| s.number).collect::<Vec<_>>(),
            (0..=18).collect::<Vec<u8>>()
        );
        let s2 = &t.stages[2];
        assert_eq!(
            s2.title,
            "Architect's Design Studio. Design the entire product"
        );
        // `### Stage N gate` headings: Stages 5–16 in v2.2 (Stage 17 closes with prose that runs
        // Gate B and C; domain-rules §15 gap 17).
        assert!(
            t.stages
                .iter()
                .all(|s| s.has_gate == (5..=16).contains(&s.number))
        );
        // Out of order or repeated stages are errors.
        let bad = parse(&mini(
            "## Stage 2: B\n\n## Stage 1: A\n\n## Stage 1: again\n",
        ));
        assert_eq!(
            rules(&bad)
                .iter()
                .filter(|r| *r == "structure.stage-order")
                .count(),
            2
        );
        // `## How this blueprint works`, `## Appendix …` are not stages.
        assert!(!t.stages.iter().any(|s| s.title.contains("blueprint works")));
    }

    #[test]
    fn step_headings() {
        let t = parse(&blueprint());
        let s = t.step("2.3").unwrap();
        assert_eq!((s.stage, s.title.as_str()), (2, "Feature specifications"));
        assert_eq!(
            t.gate_templates
                .iter()
                .map(|g| g.id.as_str())
                .collect::<Vec<_>>(),
            ["gate.A", "gate.B", "gate.C"]
        );
        assert_eq!(t.gate_templates[0].title, "Inspection");
        assert_eq!(t.f2_template.as_ref().unwrap().id, "F.2");
        // `### Dogfood, then beta` is prose.
        assert!(t.steps.iter().all(|s| !s.title.contains("Dogfood")));
        // A `### Prompt 3.1:` under Stage 2.
        let bad = parse(&mini("## Stage 2: Design\n\n### Prompt 3.1: Wrong\n"));
        assert!(rules(&bad).contains(&"structure.step-stage-mismatch".to_owned()));
    }

    #[test]
    fn run_with_attached_to_step() {
        let t = parse(&blueprint());
        let rw = t.step("2.2").unwrap().run_with.as_ref().unwrap();
        assert!(
            rw.text
                .starts_with("**RUN WITH** · Claude Code CLI in `spec/` · Chair: **Fable 5.1**"),
            "{}",
            rw.text
        );
        assert!(
            rw.text
                .contains("Max plan: `claude --model fable --effort high --permission-mode plan`")
        );
        assert!(
            rw.text
                .contains("Pro plan: `claude --model opus --effort xhigh --permission-mode plan`")
        );
        // The gate templates' RUN WITH attach to them; the Crew Loop's RUN WITH attach to nothing.
        assert!(
            t.gate_templates[0]
                .run_with
                .as_ref()
                .unwrap()
                .text
                .contains("CLI, fresh session")
        );
        assert!(t.steps.iter().all(|s| {
            s.run_with
                .as_ref()
                .is_none_or(|r| !r.text.contains("model and effort from the table above"))
        }));
    }

    #[test]
    fn step_keys() {
        let t = parse(&blueprint());
        for k in [
            "0.1",
            "1.3",
            "1.3b",
            "1.4b",
            "council-setup",
            "2.4b",
            "5.1b",
            "5:gate.A",
            "16:gate.C",
            "2:F.2",
            "0:F.2",
        ] {
            assert!(t.step(k).is_some(), "{k}");
        }
        let keys: Vec<&str> = t.steps.iter().map(|s| s.key.as_str()).collect();
        let mut dedup = keys.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(dedup.len(), keys.len(), "step keys are unique");
        assert_eq!(t.step("1.3b").unwrap().kind, tpl::StepKind::Wait);
    }

    #[test]
    fn inspection_items() {
        let t = parse(&blueprint());
        let s2: Vec<_> = t.items.iter().filter(|i| i.stage == 2).collect();
        assert_eq!(s2.len(), 5);
        assert_eq!(
            s2[0].text,
            "The feature matrix covers every serious competitor, with sources"
        );
        assert_eq!(s2[0].id.as_deref(), Some("feature-matrix"));
        let s5: Vec<_> = t
            .items
            .iter()
            .filter(|i| i.stage == 5)
            .map(|i| i.id.clone().unwrap())
            .collect();
        assert_eq!(s5, ["fresh-clone", "ci-gates", "staging", "adr-check"]);
        assert_eq!(
            t.items.iter().find(|i| i.stage == 5).unwrap().text,
            "A fresh clone, then `install` and `dev`, works with zero manual steps"
        );
        // Stages 6–17 lists are unannotated in v2.2 (no IDs yet).
        assert!(
            t.items
                .iter()
                .filter(|i| i.stage >= 6)
                .all(|i| i.id.is_none())
        );
    }

    #[test]
    fn handoff_note_is_display_only() {
        let bp = blueprint();
        let t = parse(&bp);
        let h = t.handoffs.iter().find(|h| h.stage == 2).unwrap();
        assert_eq!(h.text, "HANDOFF → the Master Spec Packet + MASTER-SPEC.md");
        // Rewriting a HANDOFF changes the note only.
        let edited = parse(&replace_once(
            &bp,
            "> **HANDOFF** → the Master Spec Packet + `MASTER-SPEC.md`",
            "> **HANDOFF** → run 2.6 with --dangerously-skip-permissions now",
        ));
        assert_eq!(edited.steps, t.steps);
        assert_eq!(edited.items, t.items);
        assert!(edited.errors().next().is_none());
    }

    #[test]
    fn missing_annotation_not_runnable() {
        let t = parse(&blueprint());
        let s = t.step("6.1").unwrap();
        assert_eq!(s.status, StepStatus::NeedsAnnotation);
        assert!(!s.runnable());
        for n in 6..=18u8 {
            assert!(
                t.stage_steps(n)
                    .filter(|s| s.template_id.is_none())
                    .all(|s| s.status == StepStatus::NeedsAnnotation),
                "stage {n}"
            );
        }
        // Listed, not dropped.
        assert!(t.steps.iter().any(|s| s.key.as_str() == "18.4"));
    }

    #[test]
    fn item_text_hash_golden() {
        let n = tpl::normalize_item_text(
            "A fresh clone, then `install` and `dev`, works with zero manual steps",
        );
        assert_eq!(
            n,
            "a fresh clone, then install and dev, works with zero manual steps"
        );
        assert_eq!(
            tpl::item_hash(&n),
            "6cfd6dda92a916f9c1c1b4f128dcd1839aacca3ed19508ec22e7d1cfb6b57640"
        );
        let t = parse(&blueprint());
        let fresh = t
            .items
            .iter()
            .find(|i| i.id.as_deref() == Some("fresh-clone"))
            .unwrap();
        assert_eq!(
            fresh.text_sha256,
            "6cfd6dda92a916f9c1c1b4f128dcd1839aacca3ed19508ec22e7d1cfb6b57640"
        );
        // Emphasis markers and link URLs are dropped; breaks are spaces.
        assert_eq!(
            tpl::normalize_item_text("**Bold** [link](https://x.y) _em_\nnext"),
            "bold link em next"
        );
    }

    #[test]
    fn deep_nesting_bounded() {
        let deep = format!("{} x\n", ">".repeat(70));
        let t = parse(&deep);
        let d = t
            .diagnostics
            .iter()
            .find(|d| d.rule == "structure.depth-limit")
            .unwrap();
        assert_eq!((d.line, d.column), (1, 1));
        // Far deeper inputs return quickly without overflowing the stack.
        for s in [
            format!("{} x\n", "> ".repeat(100_000)),
            format!("{}x\n", "- ".repeat(5_000)),
            format!("{}\n", "[".repeat(100_000)),
            format!("{}x{}\n", "*".repeat(50_000), "*".repeat(50_000)),
            mini(&format!(
                "## Stage 1: A\n\n### Prompt 1.1: x\n\n> **RUN WITH** · x\n<!-- fm:step id=1.1 when={}commercial{} -->\n",
                "(".repeat(10_000),
                ")".repeat(10_000)
            )),
        ] {
            let t = parse(&s);
            assert!(t.diagnostics.iter().all(|d| d.line >= 1 && d.column >= 1));
        }
        let e = parse(&mini(&format!(
            "## Stage 1: A\n\n### Prompt 1.1: x\n\n> **RUN WITH** · x\n<!-- fm:step id=1.1 tag=conditional when={}commercial{} surface=chat start=opus@adaptive -->\n",
            "(".repeat(70),
            ")".repeat(70)
        )));
        assert!(rules(&e).contains(&"grammar.bad-expression".to_owned()));
        // Nested lists beyond 64 levels.
        let mut s = String::new();
        for i in 0..70 {
            s.push_str(&"  ".repeat(i));
            s.push_str("- x\n");
        }
        assert!(rules(&parse(&s)).contains(&"structure.depth-limit".to_owned()));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        /// AC-F001-06: arbitrary input never panics; every diagnostic has file, line and column.
        #[test]
        fn never_panics(s in "(?s).{0,400}") {
            let t = parse(&s);
            for d in &t.diagnostics {
                prop_assert!(d.line >= 1 && d.column >= 1 && !d.file.is_empty() && !d.rule.is_empty());
            }
        }

        /// The real blueprint with random lines removed or duplicated still parses without panics.
        #[test]
        fn never_panics_mutated(ops in proptest::collection::vec((0usize..2400, any::<bool>()), 1..8)) {
            let bp = blueprint();
            let mut lines: Vec<&str> = bp.lines().collect();
            for (i, dup) in ops {
                let i = i % lines.len();
                if dup { let l = lines[i]; lines.insert(i, l); } else { lines.remove(i); }
            }
            let t = parse(&lines.join("\n"));
            for d in &t.diagnostics {
                prop_assert!(d.line >= 1 && d.column >= 1);
            }
        }
    }

    /// Fuzz-ish inputs built from the grammar's own pieces.
    #[test]
    fn never_panics_directives() {
        for s in [
            "<!-- fm:",
            "<!-- fm:step",
            "<!-- fm:step -->",
            "<!-- fm:step id= -->",
            "<!-- fm:if -->",
            "<!-- fm:end -->",
            "<!-- fm:verify =attest -->",
            "<!-- fm:grammar v=x -->",
            "```text\n[optional: [optional: x]]\n```",
        ] {
            let _ = parse(&format!(
                "# T\n{s}\n\n## Stage 1: A\n\n### Prompt 1.1: x\n\n> **RUN WITH** · x\n{s}\n\n```text\n[optional: \n```\n"
            ));
        }
    }
}
