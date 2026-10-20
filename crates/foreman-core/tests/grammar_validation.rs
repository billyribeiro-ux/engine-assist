//! M1-14: grammar validation and the diagnostic catalog (DR-028–DR-037, AC-F001-09/-11/-14/-15).
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
use foreman_core::ids::CwdKind;
use foreman_core::template::StepStatus;

fn diag<'a>(
    t: &'a foreman_core::template::Template,
    rule: &str,
) -> Option<&'a foreman_core::diagnostic::Diagnostic> {
    t.diagnostics.iter().find(|d| d.rule == rule)
}

/// A Stage 9 with a chat chain start and a second step whose annotation is `second`.
fn chain_fixture(second: &str) -> foreman_core::template::Template {
    parse(&mini(&format!(
        "## Stage 9: A\n\n### Prompt 9.1: a\n\n> **RUN WITH** · chat\n<!-- fm:step id=9.1 surface=chat start=opus@adaptive -->\n\n```text\none\n```\n\n\
         ### Prompt 9.2: b\n\n> **RUN WITH** · same chat\n{second}\n\n```text\ntwo\n```\n\n\
         ### Prompt 9.3: c\n\n> **RUN WITH** · cli\n<!-- fm:step id=9.3 surface=cli cwd=spec start=opus@high/plan -->\n\n```text\nthree\n```\n"
    )))
}

mod grammar {
    use super::*;

    /// DR-028 / AC-F001-09: every chain rule has its own error with a position.
    #[test]
    fn chain_validation() {
        let ok =
            chain_fixture("<!-- fm:step id=9.2 session=continue chain=9.1 start=opus@adaptive -->");
        assert!(ok.errors().next().is_none(), "{:?}", rules(&ok));
        assert_eq!(
            ok.step("9.2").unwrap().chain.as_ref().unwrap().as_str(),
            "9.1"
        );
        for (ann, rule) in [
            (
                "<!-- fm:step id=9.2 session=continue chain=9.3 start=opus@adaptive -->",
                "grammar.bad-chain",
            ),
            (
                "<!-- fm:step id=9.2 session=continue chain=8.1 start=opus@adaptive -->",
                "grammar.bad-chain",
            ),
            (
                "<!-- fm:step id=9.2 surface=chat chain=9.1 start=opus@adaptive -->",
                "grammar.chain-without-continue",
            ),
            (
                "<!-- fm:step id=9.2 session=continue chain=@stage start=opus@adaptive -->",
                "grammar.stage-chain-outside-f2",
            ),
        ] {
            let t = chain_fixture(ann);
            let d = diag(&t, rule).unwrap_or_else(|| panic!("{ann}: {:?}", rules(&t)));
            assert!(d.line > 1 && d.column >= 1 && d.file == "claude-build-blueprint.md");
        }
        // session=continue with no earlier step in the stage.
        let t = parse(&mini(
            "## Stage 9: A\n\n### Prompt 9.1: a\n\n> **RUN WITH** · same chat\n<!-- fm:step id=9.1 surface=chat session=continue start=opus@adaptive -->\n\n```text\none\n```\n",
        ));
        assert!(diag(&t, "grammar.bad-chain").is_some());
        // Without chain=, the nearest earlier prompt step is used, with a lint.
        let t = chain_fixture("<!-- fm:step id=9.2 session=continue start=opus@adaptive -->");
        assert!(diag(&t, "lint.implicit-chain").is_some());
        assert_eq!(
            t.step("9.2").unwrap().chain.as_ref().unwrap().as_str(),
            "9.1"
        );
        // 2.3 with chain=2.4 (a later step) in the real blueprint.
        let bad = parse(&replace_once(
            &blueprint(),
            "session=continue chain=2.2\n     start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r6",
            "session=continue chain=2.4\n     start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r6",
        ));
        assert!(
            diag(&bad, "grammar.bad-chain")
                .unwrap()
                .message
                .contains("later step")
        );
    }

    /// DR-029: a continuing step inherits its chain's surface and cwd; a different one is an error.
    #[test]
    fn continuing_inherits_cwd_surface() {
        let t = parse(&blueprint());
        let s = t.step("2.3").unwrap();
        assert_eq!(s.cwd, Some(CwdKind::Spec));
        assert!(
            s.annotation.as_ref().unwrap().cwd.is_none(),
            "2.3 doesn't declare cwd"
        );
        assert_eq!(t.step("5.4").unwrap().cwd, Some(CwdKind::Project));
        let bad = parse(&replace_once(
            &blueprint(),
            "<!-- fm:step id=2.3 kind=followup tag=core surface=cli session=continue",
            "<!-- fm:step id=2.3 kind=followup tag=core surface=cli cwd=project session=continue",
        ));
        assert!(
            diag(&bad, "grammar.chain-cwd-mismatch").is_some(),
            "{:?}",
            rules(&bad)
        );
        let bad = chain_fixture(
            "<!-- fm:step id=9.2 surface=cli cwd=spec session=continue chain=9.1 start=opus@high/plan -->",
        );
        assert!(diag(&bad, "grammar.chain-surface-mismatch").is_some());
    }

    /// DR-030: the entry count must equal the bullet count.
    #[test]
    fn verify_count() {
        let bad = parse(&replace_once(
            &blueprint(),
            "fresh-clone=attest ci-gates=attest staging=attest adr-check=attest",
            "fresh-clone=attest ci-gates=attest staging=attest",
        ));
        let d = diag(&bad, "grammar.verify-count").unwrap();
        assert!(d.message.contains("3 fm:verify entries for 4 bullets"));
        assert_eq!(d.stage, Some(5));
        // fm:verify not directly after its INSPECTION.
        let t = parse(&mini(
            "## Stage 9: A\n\n> **INSPECTION: x**\n>\n> - one\n\n<!-- fm:verify one=attest -->\n",
        ));
        assert!(diag(&t, "grammar.misplaced-verify").is_some());
    }

    /// AC-F001-11 / DR-031.
    #[test]
    fn auto_treated_as_attest() {
        let t = parse(&replace_once(
            &blueprint(),
            "adr-check=attest -->",
            "adr-check=auto:cmd=adr-lint -->",
        ));
        let d = diag(&t, "grammar.auto-needs-v1-1").unwrap();
        assert_eq!(d.message, "auto items need Foreman v1.1; treated as attest");
        assert!(!d.is_error());
        let item = t
            .items
            .iter()
            .find(|i| i.id.as_deref() == Some("adr-check"))
            .unwrap();
        assert_eq!(item.kind, "auto:cmd=adr-lint");
        assert_eq!(item.effective_kind(), "attest");
        assert!(t.errors().next().is_none());
        // time:<n>d and time:<n>d>attest are valid; others aren't.
        for (k, ok) in [
            ("time:7d", true),
            ("time:7d>attest", true),
            ("time:0d", false),
            ("time:366d", false),
            ("guess", false),
        ] {
            let t = parse(&replace_once(
                &blueprint(),
                "adr-check=attest -->",
                &format!("adr-check={k} -->"),
            ));
            assert_eq!(t.errors().next().is_none(), ok, "{k}");
        }
    }

    /// AC-F001-14 / DR-032: fm:if blocks nest never and always close.
    #[test]
    fn fm_if_blocks() {
        let t = parse(&blueprint());
        assert!(
            t.step("3.2").unwrap().prompt.as_ref().unwrap().parts[1]
                .cond
                .is_some()
        );
        let nested = parse(&replace_once(
            &blueprint(),
            "<!-- fm:if commercial|others_data|multi_user -->\n",
            "<!-- fm:if commercial -->\n<!-- fm:if others_data -->\n",
        ));
        assert!(
            diag(&nested, "grammar.nested-if").is_some(),
            "{:?}",
            rules(&nested)
        );
        let unmatched = parse(&mini(
            "## Stage 9: A\n\n### Prompt 9.1: a\n\n> **RUN WITH** · chat\n<!-- fm:step id=9.1 surface=chat start=opus@adaptive -->\n\n```text\none\n```\n<!-- fm:if server -->\n```text\ntwo\n```\n\nprose\n",
        ));
        assert!(diag(&unmatched, "grammar.unmatched-if").is_some());
        let end_only = parse(&mini(
            "## Stage 9: A\n\n### Prompt 9.1: a\n\n> **RUN WITH** · chat\n<!-- fm:step id=9.1 surface=chat start=opus@adaptive -->\n\n```text\none\n```\n<!-- fm:end -->\n",
        ));
        assert!(diag(&end_only, "grammar.unmatched-if").is_some());
        let unknown = parse(&replace_once(
            &blueprint(),
            "<!-- fm:if commercial|others_data|multi_user -->",
            "<!-- fm:if commercial|lawyers -->",
        ));
        assert!(diag(&unknown, "grammar.unknown-condition").is_some());
        let outside = parse(&mini("## Stage 9: A\n\n<!-- fm:if server -->\n"));
        assert!(diag(&outside, "grammar.misplaced-if").is_some());
    }

    /// DR-033: optional parts are numbered and never nested.
    #[test]
    fn optional_parts() {
        let t = parse(&blueprint());
        let p = t.step("1.4").unwrap().prompt.as_ref().unwrap().joined();
        let (toks, errs) = foreman_core::resolver::tokens::scan(&p, &["paste_numbers".into()]);
        assert!(errs.is_empty());
        let opt: Vec<_> = toks
            .iter()
            .filter(|t| t.class == Some(foreman_core::resolver::tokens::TokenClass::OptionalPart))
            .collect();
        assert_eq!(opt.len(), 1);
        assert_eq!(opt[0].part.as_deref(), Some("opt-1"));
        assert!(
            opt[0].text.starts_with("[optional: write a press release")
                && opt[0].text.ends_with("ask, with answers.]")
        );
        let nested = parse(&replace_once(
            &blueprint(),
            "[optional: write a press release",
            "[optional: [optional: write a press release",
        ));
        assert!(diag(&nested, "grammar.nested-optional").is_some());
        let unclosed = parse(&replace_once(
            &blueprint(),
            "ask, with answers.]\n",
            "ask, with answers.\n",
        ));
        assert!(diag(&unclosed, "grammar.unclosed-optional").is_some());
    }

    /// DR-034: item IDs are unique across the whole template.
    #[test]
    fn item_ids_unique() {
        let t = parse(&replace_once(
            &blueprint(),
            "fresh-clone=attest ci-gates=attest staging=attest",
            "fresh-clone=attest ci-gates=attest frozen=attest",
        ));
        let d = diag(&t, "grammar.duplicate-id").unwrap();
        assert!(d.message.contains("frozen"));
        assert!(parse(&blueprint()).errors().next().is_none());
    }

    /// AC-F001-13 / DR-035.
    #[test]
    fn unknown_key_by_version() {
        let bp = blueprint();
        let v1 = parse(&replace_once(
            &bp,
            "<!-- fm:step id=1.1 tag=core",
            "<!-- fm:step id=1.1 surfaces=chat tag=core",
        ));
        assert!(diag(&v1, "grammar.unknown-key").is_some());
        let v2src = replace_once(
            &replace_once(&bp, "<!-- fm:grammar v=1 -->", "<!-- fm:grammar v=2 -->"),
            "<!-- fm:step id=1.1 tag=core",
            "<!-- fm:step id=1.1 surfaces=chat tag=core",
        );
        let v2src = replace_once(
            &v2src,
            "# PART A: PRE-CONSTRUCTION\n",
            "# PART A: PRE-CONSTRUCTION\n\n<!-- fm:note text=x -->\n",
        );
        let v2 = parse(&v2src);
        assert_eq!(
            v2.step("1.1").unwrap().status,
            StepStatus::NeedsNewerForeman
        );
        assert!(!v2.step("1.1").unwrap().runnable());
        assert_eq!(v2.needs_newer_count(), 1);
        let w = diag(&v2, "grammar.newer-grammar").unwrap();
        assert!(!w.is_error());
        assert!(v2.errors().next().is_none(), "{:?}", rules(&v2));
        assert_eq!(v2.step("1.2").unwrap().status, StepStatus::Runnable);
        // Under v1 an unknown directive is an error.
        let d = parse(&replace_once(
            &bp,
            "# PART A: PRE-CONSTRUCTION\n",
            "# PART A: PRE-CONSTRUCTION\n\n<!-- fm:note text=x -->\n",
        ));
        assert!(diag(&d, "grammar.unknown-directive").is_some());
    }
}

mod template {
    use super::*;

    /// AC-F001-15 / DR-037: errors only make their own stage's steps not runnable.
    #[test]
    fn errors_scoped_to_stage() {
        let bad = parse(&replace_once(
            &blueprint(),
            "<!-- fm:step id=4.3 tag=core surface=chat session=new",
            "<!-- fm:step id=4.3 tag=core surface=chat session=continue chain=4.9",
        ));
        let d = diag(&bad, "grammar.bad-chain").unwrap();
        assert_eq!(d.stage, Some(4));
        assert!(
            bad.stage_steps(4)
                .filter(|s| s.annotation.is_some())
                .all(|s| s.status == StepStatus::Invalid)
        );
        for n in [1u8, 2, 3, 5] {
            assert!(
                bad.stage_steps(n)
                    .filter(|s| s.annotation.is_some())
                    .all(|s| s.runnable()),
                "stage {n}"
            );
        }
        // Every catalog entry has a stable rule, file, line and column.
        for d in &bad.diagnostics {
            assert!(d.rule.contains('.') && d.line >= 1 && d.column >= 1);
        }
    }
}
