//! M1-21: token classification and the blueprint inventory (DR-080–DR-083, AC-F006-01/-02/-03).
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

use common::{Fixture, blueprint, parse, replace_once};
use foreman_core::resolver::tokens::{TokenClass as C, classify, scan};
use foreman_core::resolver::{Unresolved, resolve};

mod resolver {
    use super::*;

    /// AC-F006-02: an unknown token is a template error, never a guess.
    #[test]
    fn unknown_token_is_error() {
        let t = parse(&replace_once(
            &blueprint(),
            "Inputs: the design doc, the ADRs, and the Stage 3 packet (Project knowledge).",
            "Inputs: [paste the threat model] and the Stage 3 packet (Project knowledge).",
        ));
        let d = t
            .diagnostics
            .iter()
            .find(|d| d.rule == "resolver.unknown-token")
            .unwrap();
        assert!(d.message.contains("[paste the threat model]"));
        assert_eq!(d.stage, Some(4));
        assert!(t.step("4.3").unwrap().status == foreman_core::template::StepStatus::Invalid);
        assert_eq!(classify("paste the threat model", None, None, &[]), None);
    }

    /// DR-081: the two syntactic rules and the scanning limits.
    #[test]
    fn token_syntax() {
        let (t, _) = scan(
            "use sonnet[1m] and see [the docs](https://x) for [APP_NAME]",
            &[],
        );
        let classes: Vec<_> = t.iter().map(|x| (x.text.as_str(), x.class)).collect();
        assert_eq!(
            classes,
            [
                ("[1m]", Some(C::Literal)),
                ("[the docs]", Some(C::Literal)),
                ("[APP_NAME]", Some(C::ProjectVariable))
            ]
        );
        // No line break inside, no nested `[`.
        let (t, _) = scan("a [broken\ntoken] b [[APP_NAME]]", &[]);
        assert_eq!(
            t.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(),
            ["[APP_NAME]"]
        );
        // `[optional: …]` may span lines and contain tokens.
        let (t, e) = scan("x [optional: one\ntwo [APP_NAME]] y", &[]);
        assert!(e.is_empty());
        assert_eq!(t[0].class, Some(C::OptionalPart));
        assert_eq!(t[1].part.as_deref(), Some("opt-1"));
        // `docs/handoff/stage-NN.md` has no brackets: no token.
        assert!(scan("docs/handoff/stage-NN.md", &[]).0.is_empty());
        // A declared input binds by name with spaces as underscores (DR-025).
        assert_eq!(
            classify("new thing", None, None, &["new_thing".into()]),
            Some(C::RunInput)
        );
    }

    /// AC-F006-01 / DR-082: every token in Stages 0–5, the Gate and F.2 falls into exactly one
    /// class (golden of the inventory).
    #[test]
    fn blueprint_token_inventory() {
        let t = parse(&blueprint());
        let mut inv: BTreeMap<String, (String, usize)> = BTreeMap::new();
        let mut occurrences = 0;
        let steps = t
            .steps
            .iter()
            .filter(|s| s.stage <= 5 && s.template_id.is_none())
            .chain(t.gate_templates.iter())
            .chain(t.f2_template.iter());
        for s in steps {
            let Some(p) = &s.prompt else { continue };
            let declared: Vec<String> = s.inputs().iter().map(|i| i.name.clone()).collect();
            let (toks, errs) = scan(&p.joined(), &declared);
            assert!(errs.is_empty());
            for tk in toks {
                let class = tk
                    .class
                    .unwrap_or_else(|| panic!("{} in {} is unclassified", tk.text, s.key));
                let key = if class == C::OptionalPart {
                    "[optional: …]".to_owned()
                } else {
                    tk.text.clone()
                };
                let e = inv.entry(key).or_insert((format!("{class:?}"), 0));
                assert_eq!(e.0, format!("{class:?}"), "one class per token");
                e.1 += 1;
                occurrences += 1;
            }
        }
        // 21 of DR-082's 22 tokens appear in v2.2 prompt text (`[app]` was only in prose and B-8
        // removed it), plus the optional part B-5 adds.
        assert_eq!(inv.len(), 22, "{inv:#?}");
        // DR-082's 41 prompt occurrences, minus the `[N]` in Gate B's old commit line (B-3), plus
        // the optional part (B-5).
        assert_eq!(occurrences, 41);
        let lines: Vec<String> = inv
            .iter()
            .map(|(k, (c, n))| format!("{k} · {c} · {n}"))
            .collect();
        insta::assert_snapshot!(lines.join("\n"));
        // Every annotated step in Stages 1–2, the Gate and F.2 resolves for TradeLog.
        let f = Fixture::tradelog();
        for k in [
            "1.1", "2.2", "2.6", "5:gate.A", "5:gate.B", "5:gate.C", "1:F.2",
        ] {
            assert!(resolve(&f.ctx(k)).is_ok() || k == "1.2", "{k}");
        }
        // M4-05 (AC-F006-15): Stages 3–5 have tokens of their own, each classified above, and
        // every runnable step there resolves with zero unresolved tokens once its declared run
        // inputs are filled.
        let mut f = Fixture::tradelog();
        let mut by_stage: BTreeMap<u8, usize> = BTreeMap::new();
        let keys: Vec<(u8, String, Vec<String>)> = f
            .template
            .steps
            .iter()
            .filter(|s| (3..=5).contains(&s.stage) && s.runnable() && s.prompt.is_some())
            .map(|s| {
                (
                    s.stage,
                    s.key.to_string(),
                    s.inputs().iter().map(|i| i.name.clone()).collect(),
                )
            })
            .collect();
        for (stage, key, declared) in &keys {
            let step = f.template.step(key).unwrap();
            let (toks, _) = scan(&step.prompt.as_ref().unwrap().joined(), declared);
            *by_stage.entry(*stage).or_default() += toks.len();
            f.inputs = declared
                .iter()
                .map(|n| (n.clone(), format!("value of {n}")))
                .collect();
            let r = resolve(&f.ctx(key));
            assert!(r.is_ok(), "{key}: {r:?}");
        }
        for n in 3..=5 {
            assert!(
                by_stage.get(&n).copied().unwrap_or(0) > 0,
                "Stage {n}: {by_stage:?}"
            );
        }
    }

    /// AC-F006-03 / DR-083: empty, whitespace, the token itself, or a bracketed placeholder.
    #[test]
    fn placeholder_is_unresolved() {
        for v in ["", "   ", "[BUDGET_TIME]", "[your numbers]"] {
            let mut f = Fixture::tradelog();
            f.project.variables.insert("BUDGET_TIME".into(), v.into());
            let e = resolve(&f.ctx("1.1")).unwrap_err();
            assert_eq!(
                e,
                [Unresolved::Variable {
                    token: "[BUDGET_TIME]".into()
                }],
                "{v:?}"
            );
        }
        let mut f = Fixture::tradelog();
        f.project.variables.remove("APP_NAME");
        assert!(
            resolve(&f.ctx("2.2"))
                .unwrap_err()
                .contains(&Unresolved::Variable {
                    token: "[APP_NAME]".into()
                })
        );
        let f = Fixture::tradelog();
        assert!(
            f.resolve("1.1")
                .text
                .contains("Budget and time: $0 cash; 10 hours a week for 12 weeks")
        );
    }

    /// DR-087: `[N]`, `[N-1]`, `[N+1]` with two-digit padding inside `stage-…` file names.
    #[test]
    fn stage_tokens_padding() {
        let mut f = Fixture::tradelog();
        f.packets.insert(
            4,
            foreman_core::resolver::PacketText {
                text: "=== HANDOFF PACKET · Stage 4 → Stage 5 ===\n".into(),
                signed: true,
            },
        );
        let a = f.resolve("5:gate.A").text;
        assert!(
            a.starts_with(
                "Run the Stage 5 inspection for TradeLog. Read docs/handoff/stage-04.md\n"
            ),
            "{a}"
        );
        let b = f.resolve("5:gate.B").text;
        assert!(b.contains("Write docs/handoff/stage-05.md in exactly this format:"));
        assert!(b.contains("=== HANDOFF PACKET · Stage 5 → Stage 6 ===\n"));
        assert!(b.contains("NEXT: Stage 6 · <name>"));
        let c = f.resolve("12:gate.C").text;
        assert!(c.starts_with("Before we close Stage 12, capture"));
        let a12 = f.resolve("12:gate.A").text;
        assert!(
            a12.contains("stage-04.md"),
            "the latest signed packet before 12 is Stage 4's (remap): {a12}"
        );
        assert!(
            f.resolve("12:gate.A")
                .notes
                .iter()
                .any(|n| n.contains("Stage 11 has no packet; using Stage 4's"))
        );
    }
}
