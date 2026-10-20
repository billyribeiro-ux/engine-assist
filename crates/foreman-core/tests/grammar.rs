//! M1-12: the `fm:` grammar, directives and values (ADR-005, DR-016, DR-020–DR-028, DR-034).
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
use foreman_core::ids::{ConditionId, Effort, Mode, Model};
use foreman_core::template::{Expr, StepStatus, parse_expr};

/// One chat stage with a single annotated prompt.
fn one(annotation: &str, prompt: &str) -> foreman_core::template::Template {
    parse(&mini(&format!(
        "## Stage 9: A\n\n### Prompt 9.1: First\n\n> **RUN WITH** · Claude chat · **Opus 5.5** · effort adaptive\n{annotation}\n\n```text\n{prompt}\n```\n"
    )))
}

fn has(t: &foreman_core::template::Template, rule: &str) -> bool {
    rules(t).iter().any(|r| r == rule)
}

mod grammar {
    use super::*;
    use proptest::prelude::*;

    /// AC-F001-12: an `fm:` comment in a list item, a blockquote, inline or in a fence is refused.
    #[test]
    fn block_level_only() {
        for body in [
            "> - item <!-- fm:verify a=attest -->\n",
            "- item\n  <!-- fm:step id=1.1 -->\n",
            "text <!-- fm:step id=1.1 --> inline\n",
            "> <!-- fm:step id=1.1 -->\n",
            "```text\n<!-- fm:step id=1.1 -->\n```\n",
        ] {
            let t = parse(&mini(body));
            let d = t
                .diagnostics
                .iter()
                .find(|d| d.rule == "grammar.not-block-level")
                .unwrap_or_else(|| panic!("{body}: {:?}", rules(&t)));
            assert!(
                d.message.contains("annotations must be block-level"),
                "{}",
                d.message
            );
            assert!(d.line >= 6, "{body}: line {}", d.line);
        }
        // The two-line 2.2 comment is one directive.
        let t = parse(&blueprint());
        assert_eq!(
            t.step("2.2")
                .unwrap()
                .annotation
                .as_ref()
                .unwrap()
                .out
                .len(),
            8
        );
    }

    /// DR-021: the grammar version is required; without it nothing runs.
    #[test]
    fn version_required() {
        let bp = blueprint();
        assert_eq!(parse(&bp).grammar_version, Some(1));
        let missing = parse(&replace_once(&bp, "<!-- fm:grammar v=1 -->\n", ""));
        assert!(has(&missing, "grammar.missing-version"));
        assert_eq!(missing.grammar_version, None);
        assert!(missing.steps.iter().all(|s| !s.runnable()));
        let bad = parse(&replace_once(
            &bp,
            "<!-- fm:grammar v=1 -->",
            "<!-- fm:grammar v=0 -->",
        ));
        assert!(has(&bad, "grammar.missing-version"));
        // Not the first annotation: an fm:grammar later on is misplaced.
        let late = parse(&format!("{bp}\n<!-- fm:grammar v=1 -->\n"));
        assert!(has(&late, "grammar.misplaced-version"));
    }

    /// DR-022: the key table (allowed kinds, repeatability, unknown keys).
    #[test]
    fn key_table() {
        let t = parse(&mini(
            "## Stage 2: A\n\n### Council setup (once): x\n\nCreate these six files.\n<!-- fm:step id=council-setup kind=action tag=core start=opus@high/plan -->\n",
        ));
        assert!(has(&t, "grammar.key-not-allowed"), "{:?}", rules(&t));
        let t = one(
            "<!-- fm:step id=9.1 surface=chat surface=cli start=opus@adaptive -->",
            "x",
        );
        assert!(has(&t, "grammar.duplicate-key"));
        let t = one(
            "<!-- fm:step id=9.1 surface=chat start=opus@adaptive in=x:text -->",
            "[x]",
        );
        assert!(!has(&t, "grammar.duplicate-key"));
        let t = one(
            "<!-- fm:step id=9.1 surfaces=chat start=opus@adaptive -->",
            "x",
        );
        assert!(has(&t, "grammar.unknown-key"));
        let t = parse(&mini(
            "## Stage 2: A\n\n### Prompt 2.4: x\n\nRecord it.\n<!-- fm:step id=2.4b kind=manual out=spec/a.md in=x:text -->\n",
        ));
        assert!(has(&t, "grammar.key-not-allowed"));
        // Every annotated step of the real blueprint uses only allowed keys.
        assert!(!has(&parse(&blueprint()), "grammar.key-not-allowed"));
    }

    /// AC-F001-09 (values): models, efforts and modes; `manual` is stored as `default`.
    #[test]
    fn start_vocabulary() {
        let t = parse(&blueprint());
        let s21 = t.step("2.6").unwrap().start.unwrap();
        assert_eq!(
            (s21.model, s21.effort, s21.mode),
            (Model::Fable, Effort::High, Some(Mode::Default))
        );
        let s51 = t.step("5.1").unwrap().start.unwrap();
        assert_eq!(
            (s51.model, s51.effort, s51.mode),
            (Model::Opusplan, Effort::Medium, Some(Mode::Plan))
        );
        assert_eq!(
            t.step("1.1").unwrap().start.unwrap().effort,
            Effort::Adaptive
        );
        for (ann, rule) in [
            ("start=gpt@high", "grammar.unknown-value"),
            ("start=opus@extreme", "grammar.unknown-value"),
            ("start=opus@adaptive/yolo", "grammar.unknown-value"),
            ("start=opus@adaptive/plan", "grammar.mode-on-chat"),
            ("start=opus@high", "grammar.effort-surface"),
        ] {
            let t = one(&format!("<!-- fm:step id=9.1 surface=chat {ann} -->"), "x");
            assert!(has(&t, rule), "{ann}: {:?}", rules(&t));
        }
        let cli = |start: &str| {
            parse(&mini(&format!(
                "## Stage 5: A\n\n### Prompt 5.1: x\n\n> **RUN WITH** · CLI\n<!-- fm:step id=5.1 surface=cli cwd=project {start} -->\n\n```text\nx\n```\n"
            )))
        };
        assert!(has(&cli("start=opus@high"), "grammar.mode-required"));
        assert!(has(
            &cli("start=opus@adaptive/plan"),
            "grammar.effort-surface"
        ));
        let ok = cli("start=opus@ultracode/manual");
        assert!(ok.errors().next().is_none(), "{:?}", rules(&ok));
        assert_eq!(
            ok.step("5.1").unwrap().start.unwrap().mode,
            Some(Mode::Default)
        );
        for m in ["auto", "dontAsk", "acceptEdits", "plan", "default"] {
            assert!(
                cli(&format!("start=sonnet@low/{m}"))
                    .errors()
                    .next()
                    .is_none(),
                "{m}"
            );
        }
    }

    /// DR-024: alternatives come from a closed set of rules.
    #[test]
    fn alt_rules_closed_set() {
        let t = parse(&blueprint());
        let a = t.step("2.2").unwrap().alts();
        assert_eq!(a.len(), 1);
        assert_eq!((a[0].model, a[0].effort), (Model::Opus, Effort::Xhigh));
        let t = one(
            "<!-- fm:step id=9.1 surface=chat start=opus@adaptive alt=team-plan:opus@adaptive -->",
            "x",
        );
        assert!(has(&t, "grammar.unknown-alt-rule"));
        let t = one(
            "<!-- fm:step id=9.1 surface=chat start=opus@adaptive alt=max-plan:sonnet@adaptive alt=pro-plan:haiku@adaptive -->",
            "x",
        );
        assert!(t.errors().next().is_none(), "{:?}", rules(&t));
        let t = one(
            "<!-- fm:step id=9.1 surface=chat start=opus@adaptive alt=pro-plan:opus@adaptive/plan -->",
            "x",
        );
        assert!(has(&t, "grammar.unknown-value"));
    }

    /// DR-025: run inputs bind to their tokens.
    #[test]
    fn input_binding() {
        let t = parse(&blueprint());
        let ins: Vec<&str> = t
            .step("1.2")
            .unwrap()
            .inputs()
            .iter()
            .map(|i| i.name.as_str())
            .collect();
        assert_eq!(ins, ["X", "changes"]);
        assert_eq!(t.step("1.4").unwrap().inputs()[0].name, "paste_numbers");
        assert!(t.step("3.2").unwrap().inputs()[0].remember);
        // A declared input with no token, and a run-input token with no declaration.
        let t = one(
            "<!-- fm:step id=9.1 surface=chat start=opus@adaptive in=jurisdictions:text -->",
            "no token here",
        );
        assert!(has(&t, "grammar.input-binding"));
        let t = one(
            "<!-- fm:step id=9.1 surface=chat start=opus@adaptive -->",
            "We're going with concept [X].",
        );
        assert!(has(&t, "grammar.input-binding"));
        // `in=` moved to 1.2 (gap 5): annotating it on 1.1 instead would fail.
        let moved = replace_once(
            &blueprint(),
            "start=opus@adaptive in=X:text in=changes:text -->",
            "start=opus@adaptive -->",
        );
        assert!(has(&parse(&moved), "grammar.input-binding"));
        let t = one(
            "<!-- fm:step id=9.1 surface=chat start=opus@adaptive in=9bad:text -->",
            "x",
        );
        assert!(has(&t, "grammar.unknown-value"));
    }

    /// DR-026: annotation paths are repo-relative and pass the write policy.
    #[test]
    fn paths_repo_relative() {
        for out in [
            "../docs/x.md",
            "/etc/passwd",
            ".git/hooks/pre-commit",
            "spec/.claude/settings.json",
            "README.md",
            "docs/.MCP.json",
            "spec/CLAUDE.md",
        ] {
            let t = one(
                &format!("<!-- fm:step id=9.1 surface=chat start=opus@adaptive out={out} -->"),
                "x",
            );
            let d = t
                .diagnostics
                .iter()
                .find(|d| d.rule == "grammar.path-policy")
                .unwrap_or_else(|| panic!("{out}"));
            assert!(d.message.contains("path."), "{}", d.message);
        }
        let t = one(
            "<!-- fm:step id=9.1 surface=chat start=opus@adaptive requires=.env.local -->",
            "x",
        );
        assert!(has(&t, "grammar.path-policy"));
        let t = one(
            "<!-- fm:step id=9.1 surface=chat start=opus@adaptive out=docs/a.md,spec/b.md requires=docs/c.md -->",
            "x",
        );
        assert!(t.errors().next().is_none());
        // 2.2's outputs are repo-relative even though it runs in spec/.
        let bp = parse(&blueprint());
        assert!(
            bp.step("2.2")
                .unwrap()
                .out
                .iter()
                .all(|o| o.starts_with("spec/council/"))
        );
    }

    /// DR-026: `NN` in gate and F.2 templates becomes the two-digit stage.
    #[test]
    fn nn_instantiation() {
        let t = parse(&blueprint());
        assert_eq!(t.gate_templates[1].out, ["docs/handoff/stage-NN.md"]);
        assert_eq!(
            t.step("5:gate.B").unwrap().out,
            ["docs/handoff/stage-05.md"]
        );
        assert_eq!(
            t.step("16:gate.B").unwrap().out,
            ["docs/handoff/stage-16.md"]
        );
        assert_eq!(
            t.step("5:gate.C").unwrap().appends.as_deref(),
            Some("docs/lessons.md")
        );
        assert_eq!(
            t.step("5:gate.B").unwrap().chain.as_ref().unwrap().as_str(),
            "5:gate.A"
        );
    }

    proptest! {
        /// DR-027: print/parse round trip.
        #[test]
        fn expression_parser(e in arb_expr()) {
            let s = e.to_string();
            prop_assert_eq!(parse_expr(&s).unwrap(), e);
        }
    }

    fn arb_expr() -> impl Strategy<Value = Expr> {
        let leaf = (0usize..12).prop_map(|i| Expr::Id(ConditionId::ALL[i]));
        leaf.prop_recursive(6, 32, 2, |inner| {
            prop_oneof![
                inner.clone().prop_map(|e| Expr::Not(Box::new(e))),
                (inner.clone(), inner.clone())
                    .prop_map(|(a, b)| Expr::And(Box::new(a), Box::new(b))),
                (inner.clone(), inner).prop_map(|(a, b)| Expr::Or(Box::new(a), Box::new(b))),
            ]
        })
    }

    #[test]
    fn expression_parser_cases() {
        assert_eq!(
            parse_expr("commercial|payments").unwrap().to_string(),
            "commercial|payments"
        );
        assert_eq!(
            parse_expr("!idea_known").unwrap(),
            Expr::Not(Box::new(Expr::Id(ConditionId::IdeaKnown)))
        );
        // ! > & > |
        assert_eq!(
            parse_expr("a_x").unwrap_err().rule(),
            "grammar.unknown-condition"
        );
        let e = parse_expr("payments&!publicx").unwrap_err();
        assert_eq!((e.rule(), e.column()), ("grammar.unknown-condition", 11));
        assert_eq!(parse_expr("payments&").unwrap_err().column(), 10);
        assert_eq!(
            parse_expr("(server").unwrap_err().rule(),
            "grammar.bad-expression"
        );
        assert!(parse_expr("server server").is_err());
        assert!(parse_expr(&format!("{}server{}", "(".repeat(64), ")".repeat(64))).is_err());
        assert!(parse_expr(&format!("{}server{}", "(".repeat(60), ")".repeat(60))).is_ok());
        let p = parse_expr("commercial|payments&!public").unwrap();
        assert_eq!(p.to_string(), "commercial|payments&!public");
        assert!(matches!(p, Expr::Or(..)));
        // In an annotation, the column points into the line.
        let t = one(
            "<!-- fm:step id=9.1 tag=conditional when=commerical surface=chat start=opus@adaptive -->",
            "x",
        );
        assert!(has(&t, "grammar.unknown-condition"));
    }

    /// DR-005: step IDs are unique in the template.
    #[test]
    fn duplicate_id_error() {
        let bp = blueprint();
        let dup = replace_once(
            &bp,
            "<!-- fm:step id=2.5 kind=followup",
            "<!-- fm:step id=2.3 kind=followup",
        );
        let t = parse(&dup);
        assert!(
            has(&t, "grammar.duplicate-id") || has(&t, "grammar.id-mismatch"),
            "{:?}",
            rules(&t)
        );
        let t = parse(&mini(
            "## Stage 9: A\n\nOne.\n<!-- fm:step id=9.3b kind=wait -->\n\n### Prompt 9.4: y\n\nTwo.\n<!-- fm:step id=9.3b kind=wait -->\n",
        ));
        assert!(has(&t, "grammar.duplicate-id"));
    }
}

mod template {
    use super::*;

    /// AC-F001-10 / DR-016: `bypassPermissions` anywhere is an error (case-insensitive).
    #[test]
    fn rejects_bypass_permissions() {
        let bp = blueprint();
        assert!(
            !has(&parse(&bp), "grammar.bypass-permissions"),
            "the Bypass table row is fine"
        );
        let a = parse(&replace_once(
            &bp,
            "start=opus@high/plan -->\n\n```text\nRun the Stage",
            "start=opus@high/bypassPermissions -->\n\n```text\nRun the Stage",
        ));
        let d = a
            .diagnostics
            .iter()
            .find(|d| d.rule == "grammar.bypass-permissions")
            .unwrap();
        assert!(d.column > 1);
        let b = parse(&replace_once(
            &bp,
            "Use the test-runner agent for every automated check.",
            "Use --permission-mode BYPASSPERMISSIONS for every automated check.",
        ));
        assert!(has(&b, "grammar.bypass-permissions"));
        // An error in the gate template makes every gate instance not runnable.
        assert!(
            a.steps
                .iter()
                .filter(|s| s.template_id.as_deref() == Some("gate.A"))
                .all(|s| s.status == StepStatus::Invalid)
        );
    }
}
