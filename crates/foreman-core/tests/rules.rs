//! M1-18: the rule engine: tags, tri-state conditions, the applicable path (DR-050–DR-058).
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

use std::collections::BTreeSet;

use common::{blueprint, parse};
use foreman_core::ids::{ConditionId as C, StepKey, TriState as T};
use foreman_core::rules::{self as r, Applicability as A, RuleError, StageState};
use foreman_core::schema::overlay::{OverlayFile, Skip};
use foreman_core::template::{Tag, Template};

/// Foreman's own conditions (MASTER-SPEC §5): idea_known and has_ui yes, everything else no.
fn foreman() -> foreman_core::schema::project::Conditions {
    let mut v: Vec<(C, T)> = C::ALL.iter().map(|c| (*c, T::No)).collect();
    v.push((C::IdeaKnown, T::Yes));
    v.push((C::HasUi, T::Yes));
    r::conditions(&v)
}

fn tpl() -> Template {
    parse(&blueprint())
}

fn project(
    conds: foreman_core::schema::project::Conditions,
) -> foreman_core::schema::project::ProjectFile {
    let text = common::read_fixture("schema/project/v1.json");
    let mut p: foreman_core::schema::project::ProjectFile = serde_json::from_str(&text).unwrap();
    p.conditions = conds;
    p.condition_audit.clear();
    p
}

mod rules {
    use super::*;
    use proptest::prelude::*;

    /// AC-F004-01: every row of DR-050's table, and MASTER-SPEC §5's default tagging.
    #[test]
    fn tag_condition_table() {
        let t = tpl();
        let s = |k: &str| t.step(k).unwrap().clone();
        let mut core = s("2.2");
        let mut cond = s("1.3"); // conditional when=commercial
        let mut opt = s("0.1"); // optional when=!idea_known
        let mut opt_plain = s("0.1");
        if let Some(a) = opt_plain.annotation.as_mut() {
            a.when = None;
        }
        core.tag = Tag::Core;
        cond.tag = Tag::Conditional;
        opt.tag = Tag::Optional;
        opt_plain.tag = Tag::Optional;
        let c = |pairs: &[(C, T)]| r::conditions(pairs);
        for v in [T::Yes, T::No, T::Undecided] {
            assert_eq!(
                r::step_state(&core, &c(&[(C::Commercial, v)])),
                A::Applicable
            );
            assert_eq!(
                r::step_state(&opt_plain, &c(&[(C::IdeaKnown, v)])),
                A::Skippable
            );
        }
        assert_eq!(
            r::step_state(&cond, &c(&[(C::Commercial, T::Yes)])),
            A::Applicable
        );
        assert_eq!(
            r::step_state(&cond, &c(&[(C::Commercial, T::No)])),
            A::NotApplicable("commercial = no".into())
        );
        assert_eq!(
            r::step_state(&cond, &c(&[(C::Commercial, T::Undecided)])),
            A::Undecided(vec![C::Commercial])
        );
        assert_eq!(
            r::step_state(&opt, &c(&[(C::IdeaKnown, T::No)])),
            A::Skippable
        );
        assert_eq!(
            r::step_state(&opt, &c(&[(C::IdeaKnown, T::Yes)])),
            A::NotApplicable("idea_known = yes".into())
        );
        assert_eq!(
            r::step_state(&opt, &c(&[(C::IdeaKnown, T::Undecided)])),
            A::Undecided(vec![C::IdeaKnown])
        );
        // MASTER-SPEC §5 defaults with Foreman's own conditions.
        let p = r::applicable_path(&t, &foreman(), &OverlayFile::empty());
        for (k, want) in [
            ("0.1", A::NotApplicable("idea_known = yes".into())),
            ("1.1", A::Applicable),
            ("1.3", A::NotApplicable("commercial = no".into())),
            ("1.3b", A::NotApplicable("commercial = no".into())),
            ("1.4", A::Applicable),
            ("2.1", A::NotApplicable("commercial = no".into())),
            ("2.2", A::Applicable),
            ("2.6", A::Applicable),
            (
                "3.1",
                A::NotApplicable("commercial = no, payments = no".into()),
            ),
            ("3.2", A::Applicable),
            ("4.5", A::Applicable),
            ("5.5", A::NotApplicable("server = no".into())),
            ("5:gate.A", A::Applicable),
            ("2:F.2", A::Applicable),
            ("6.1", A::NeedsAnnotation),
        ] {
            assert_eq!(p.step(k), Some(&want), "{k}");
        }
        insta::assert_json_snapshot!(
            r::summary(&p)
                .into_iter()
                .filter(|(k, _)| !k.contains(':') && k.as_str() < "6")
                .collect::<std::collections::BTreeMap<_, _>>()
        );
    }

    fn arb_tri() -> impl Strategy<Value = T> {
        prop_oneof![Just(T::Yes), Just(T::No), Just(T::Undecided)]
    }

    proptest! {
        /// AC-F004-02: whatever the conditions, skips and overlay, no core step is ever dropped.
        #[test]
        fn core_never_dropped(vals in proptest::collection::vec(arb_tri(), 12), skips in proptest::collection::vec(0usize..100, 0..20)) {
            let t = tpl();
            let c = r::conditions(&C::ALL.iter().copied().zip(vals).collect::<Vec<_>>());
            let mut o = OverlayFile::empty();
            for i in skips {
                let s = &t.steps[i % t.steps.len()];
                o.skips.insert(s.key.clone(), Skip { reason: "nope".into(), at: "2026-10-20T00:00:00Z".into() });
            }
            let p = r::applicable_path(&t, &c, &o);
            for s in t.steps.iter().filter(|s| s.tag == Tag::Core && s.template_id.is_none() && s.runnable()) {
                prop_assert_eq!(p.step(s.key.as_str()), Some(&A::Applicable), "{}", s.key);
            }
            // A stage with a core step always applies, with its gate and F.2.
            for s in t.steps.iter().filter(|s| s.template_id.is_some() && t.steps.iter().any(|x| x.stage == s.stage && x.tag == Tag::Core && x.template_id.is_none() && x.runnable())) {
                prop_assert_eq!(p.step(s.key.as_str()), Some(&A::Applicable));
            }
        }
    }

    /// DR-051: exhaustive over 3×3 per operator.
    #[test]
    fn kleene_truth_table() {
        use foreman_core::template::Expr;
        let a = || Box::new(Expr::Id(C::Commercial));
        let b = || Box::new(Expr::Id(C::Payments));
        let vals = [T::Yes, T::No, T::Undecided];
        let and = |x: T, y: T| match (x, y) {
            (T::No, _) | (_, T::No) => T::No,
            (T::Yes, T::Yes) => T::Yes,
            _ => T::Undecided,
        };
        let or = |x: T, y: T| match (x, y) {
            (T::Yes, _) | (_, T::Yes) => T::Yes,
            (T::No, T::No) => T::No,
            _ => T::Undecided,
        };
        let mut rows = Vec::new();
        for x in vals {
            for y in vals {
                let c = r::conditions(&[(C::Commercial, x), (C::Payments, y)]);
                assert_eq!(r::eval(&Expr::And(a(), b()), &c), and(x, y));
                assert_eq!(r::eval(&Expr::Or(a(), b()), &c), or(x, y));
                rows.push(format!(
                    "{} {} → & {} | {}",
                    x.as_str(),
                    y.as_str(),
                    r::eval(&Expr::And(a(), b()), &c).as_str(),
                    r::eval(&Expr::Or(a(), b()), &c).as_str()
                ));
            }
            let c = r::conditions(&[(C::Commercial, x)]);
            let not = r::eval(&Expr::Not(a()), &c);
            assert_eq!(
                not,
                match x {
                    T::Yes => T::No,
                    T::No => T::Yes,
                    T::Undecided => T::Undecided,
                }
            );
        }
        // DR-051's own examples: Y | U = Y; N | U = U.
        let e = r::parse_expr("commercial|payments").unwrap();
        assert_eq!(
            r::eval(&e, &r::conditions(&[(C::Commercial, T::Yes)])),
            T::Yes
        );
        assert_eq!(
            r::eval(&e, &r::conditions(&[(C::Commercial, T::No)])),
            T::Undecided
        );
        insta::assert_snapshot!(rows.join("\n"));
    }

    /// AC-F004-10 / DR-052.
    #[test]
    fn condition_edit_windows() {
        let p = project(foreman());
        let none = BTreeSet::new();
        let q = r::set_condition(
            &p,
            C::Payments,
            T::Yes,
            "we'll charge",
            "2026-10-20T10:00:00Z",
            &none,
        )
        .unwrap();
        assert_eq!(q.conditions[&C::Payments], T::Yes);
        let audit = q.condition_audit.last().unwrap();
        assert_eq!(
            (audit.condition, audit.from, audit.to, audit.note.as_str()),
            (C::Payments, T::No, T::Yes, "we'll charge")
        );
        // The path updates at once (no restart): 3.1 now applies.
        let t = tpl();
        assert_eq!(
            r::applicable_path(&t, &q.conditions, &OverlayFile::empty()).step("3.1"),
            Some(&A::Applicable)
        );
        let signed2: BTreeSet<u8> = [1, 2].into();
        let e = r::set_condition(&p, C::Payments, T::Yes, "x", "t", &signed2).unwrap_err();
        assert_eq!(e, RuleError::ConditionLocked);
        assert!(
            e.to_string().contains("Change Request (v1.1)")
                && e.to_string().contains("docs/templates/change-request.md")
        );
        // Late conditions stay open until Stage 4 is signed.
        assert!(r::set_condition(&p, C::HasDatabase, T::Yes, "migrations", "t", &signed2).is_ok());
        let signed4: BTreeSet<u8> = [1, 2, 3, 4].into();
        assert!(r::set_condition(&p, C::HasDatabase, T::Yes, "x", "t", &signed4).is_err());
    }

    /// DR-053.
    #[test]
    fn packet_producer() {
        let t = tpl();
        let p = r::applicable_path(&t, &foreman(), &OverlayFile::empty());
        assert_eq!(r::packet_producer(&t, &p, 2).unwrap().key.as_str(), "2.6");
        assert_eq!(r::packet_producer(&t, &p, 1).unwrap().key.as_str(), "1.4");
        assert_eq!(
            r::packet_producer(&t, &p, 5).unwrap().key.as_str(),
            "5:gate.B"
        );
        assert!(
            r::packet_producer(&t, &p, 0).is_none(),
            "Stage 0 doesn't apply for Foreman"
        );
        let p0 = r::applicable_path(
            &t,
            &r::conditions(&[(C::IdeaKnown, T::No)]),
            &OverlayFile::empty(),
        );
        assert_eq!(r::packet_producer(&t, &p0, 0).unwrap().key.as_str(), "0.2");
    }

    /// AC-F004-09 / DR-054.
    #[test]
    fn stage_not_applicable() {
        let t = tpl();
        let p = r::applicable_path(&t, &foreman(), &OverlayFile::empty());
        assert_eq!(
            p.stage(0),
            Some(&StageState::NotApplicable("idea_known = yes".into()))
        );
        assert!(
            matches!(p.step("0:F.2"), Some(A::NotApplicable(_))),
            "no F.2 for a stage that doesn't apply"
        );
        assert_eq!(p.next_applicable_stage(0), Some(1));
        assert_eq!(
            p.previous_applicable_stage(1),
            None,
            "Stage 1's packet is the first"
        );
        assert_eq!(p.stage(6), Some(&StageState::NeedsAnnotation));
        assert!(p.stage_applies(3));
        // Everything skipped: optional Stage 0 with both steps skipped doesn't apply either.
        let c0 = r::conditions(&[(C::IdeaKnown, T::No)]);
        let mut o = OverlayFile::empty();
        for k in ["0.1", "0.2"] {
            o.skips.insert(
                StepKey::parse(k).unwrap(),
                Skip {
                    reason: "had an idea".into(),
                    at: "t".into(),
                },
            );
        }
        assert!(matches!(
            r::applicable_path(&t, &c0, &o).stage(0),
            Some(StageState::NotApplicable(_))
        ));
    }

    /// AC-F013-15 / DR-055.
    #[test]
    fn skipped_queue() {
        let t = tpl();
        let p = r::applicable_path(&t, &foreman(), &OverlayFile::empty());
        assert_eq!(
            r::skipped_queue(&t, &p, 2, Some(1), &[]),
            ["2.1 Competitor teardown: not applicable (commercial = no)"]
        );
        let q1 = r::skipped_queue(
            &t,
            &p,
            1,
            None,
            &["1.4 part \"opt-1\": skipped: Include turned off".into()],
        );
        assert_eq!(
            q1[0],
            "Stage 0 Land Survey. Find where the money is: not applicable (idea_known = yes)"
        );
        assert!(
            q1.contains(&"1.3 Demand validation kit: not applicable (commercial = no)".to_owned())
        );
        assert_eq!(
            q1.last().unwrap(),
            "1.4 part \"opt-1\": skipped: Include turned off"
        );
        // Not re-queued once a later packet is signed.
        assert!(
            !r::skipped_queue(&t, &p, 3, Some(2), &[])
                .iter()
                .any(|l| l.starts_with("Stage 0"))
        );
    }

    /// AC-F004-05/-07/-08 / DR-056.
    #[test]
    fn skip_rules() {
        let t = tpl();
        let c = r::conditions(&[(C::IdeaKnown, T::No)]);
        let p = r::applicable_path(&t, &c, &OverlayFile::empty());
        let o = r::skip(
            &t,
            &p,
            &OverlayFile::empty(),
            "0.1",
            "I already know the idea",
            "2026-10-20T10:00:00Z",
        )
        .unwrap();
        assert_eq!(
            o.skips[&StepKey::parse("0.1").unwrap()].reason,
            "I already know the idea"
        );
        assert_eq!(
            r::applicable_path(&t, &c, &o).step("0.1"),
            Some(&A::Skipped("I already know the idea".into()))
        );
        assert_eq!(
            r::skip(&t, &p, &OverlayFile::empty(), "0.1", "  ", "t"),
            Err(RuleError::BadReason)
        );
        assert_eq!(
            r::skip(&t, &p, &OverlayFile::empty(), "0.1", &"x".repeat(201), "t"),
            Err(RuleError::BadReason)
        );
        assert_eq!(
            r::skip(&t, &p, &OverlayFile::empty(), "2.2", "nah", "t"),
            Err(RuleError::CoreStepSkip)
        );
        assert_eq!(
            r::skip(&t, &p, &OverlayFile::empty(), "1.3", "nah", "t"),
            Err(RuleError::ConditionalStepSkip)
        );
        assert!(
            RuleError::CoreStepSkip
                .to_string()
                .contains("Core: protects production quality")
        );
        // Not applicable optional steps can't be skipped either.
        let pf = r::applicable_path(&t, &foreman(), &OverlayFile::empty());
        assert!(matches!(
            r::skip(&t, &pf, &OverlayFile::empty(), "0.1", "x", "t"),
            Err(RuleError::NotSkippable(_))
        ));
        // A recorded skip on a step that is now core is ignored and flagged.
        let mut stale = OverlayFile::empty();
        stale.skips.insert(
            StepKey::parse("2.2").unwrap(),
            Skip {
                reason: "old".into(),
                at: "t".into(),
            },
        );
        let ps = r::applicable_path(&t, &foreman(), &stale);
        assert_eq!(ps.step("2.2"), Some(&A::Applicable));
        assert!(ps.flags[0].starts_with("rules.stale-skip: 2.2"));
    }

    /// DR-057: gate steps, F.2, packets and inspection items are never skipped.
    #[test]
    fn gate_items_unskippable() {
        let t = tpl();
        let p = r::applicable_path(&t, &foreman(), &OverlayFile::empty());
        for k in ["5:gate.A", "5:gate.C", "2:F.2"] {
            assert_eq!(
                r::skip(&t, &p, &OverlayFile::empty(), k, "x", "t"),
                Err(RuleError::GateItemSkip),
                "{k}"
            );
        }
        assert_eq!(r::skip_item("staging"), Err(RuleError::GateItemSkip));
        assert!(
            RuleError::GateItemSkip
                .to_string()
                .contains("attest \"not applicable\"")
        );
        // A skip recorded for a gate step in the overlay is ignored and flagged.
        let mut o = OverlayFile::empty();
        o.skips.insert(
            StepKey::parse("5:gate.A").unwrap(),
            Skip {
                reason: "x".into(),
                at: "t".into(),
            },
        );
        let p2 = r::applicable_path(&t, &foreman(), &o);
        assert_eq!(p2.step("5:gate.A"), Some(&A::Applicable));
        assert!(!p2.flags.is_empty());
    }
}
