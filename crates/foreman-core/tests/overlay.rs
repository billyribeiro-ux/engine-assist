//! M4-30: overlay additions (F-025, DR-058–DR-062): add only, added-step rules, attest-only items,
//! validation, and the round trip.
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

use common::{blueprint, parse};
use foreman_core::ids::{
    ConditionId as C, CwdKind, Effort, Mode, Model, Start, Surface, TriState as T,
};
use foreman_core::rules::overlay::{self as ov, NewStep, OverlayError};
use foreman_core::rules::{self as r, Applicability as A, ApplicablePath, StageState};
use foreman_core::schema::overlay::{AddedTag, OverlayFile, Skip};
use foreman_core::template::{Session, Tag, Template};

fn conditions() -> foreman_core::schema::project::Conditions {
    let mut v: Vec<(C, T)> = C::ALL.iter().map(|c| (*c, T::No)).collect();
    v.push((C::IdeaKnown, T::Yes));
    v.push((C::HasUi, T::Yes));
    r::conditions(&v)
}

fn tpl() -> Template {
    parse(&blueprint())
}

fn path(t: &Template, o: &OverlayFile) -> ApplicablePath {
    r::applicable_path(t, &conditions(), o)
}

fn fable_default() -> Start {
    Start {
        model: Model::Fable,
        effort: Effort::High,
        mode: Some(Mode::Default),
    }
}

/// 2.6's config, copied: cli, `spec/`, fable@high/default.
fn new_after(after: &str) -> NewStep {
    NewStep {
        stage: 2,
        after: after.to_owned(),
        copied_from: "2.6".to_owned(),
        title: "Check the council's naming".to_owned(),
        text: "Read spec/council and list any name used two ways.".to_owned(),
        surface: Surface::Cli,
        cwd: Some(CwdKind::Spec),
        start: fable_default(),
        out: vec![],
        tag: AddedTag::Optional,
    }
}

mod rules {
    use super::*;
    use proptest::prelude::*;

    /// DR-058: the overlay only adds. A recorded skip on a core step is ignored and flagged, an
    /// unknown top-level key fails the schema, and nothing in the API removes, reorders or edits a
    /// template step: the template steps keep their order and values with additions in place.
    #[test]
    fn overlay_add_only() {
        let t = tpl();
        let mut o = OverlayFile::empty();
        o.skips.insert(
            foreman_core::ids::StepKey::parse("2.2").unwrap(),
            Skip {
                reason: "tries to skip core".into(),
                at: "2026-10-20T12:00:00Z".into(),
            },
        );
        let p = path(&t, &o);
        assert_eq!(
            p.step("2.2"),
            Some(&A::Applicable),
            "a core step can't be skipped"
        );
        assert!(
            p.flags.iter().any(|f| f.starts_with("rules.stale-skip")),
            "{:?}",
            p.flags
        );
        let bad =
            r#"{"schemaVersion":1,"skips":{},"addedSteps":[],"addedItems":[],"removed":["2.2"]}"#;
        assert!(foreman_core::json::parse::<OverlayFile>(bad.as_bytes()).is_err());

        let (o2, _) = ov::add_step(
            &t,
            &path(&t, &OverlayFile::empty()),
            &OverlayFile::empty(),
            &new_after("2.2"),
        )
        .unwrap();
        let (eff, flags) = ov::effective_template(&t, &o2);
        assert!(flags.is_empty(), "{flags:?}");
        let template_only: Vec<_> = eff
            .steps
            .iter()
            .filter(|s| !s.key.as_str().starts_with("x-"))
            .collect();
        assert_eq!(template_only.len(), t.steps.len());
        assert!(
            template_only.iter().zip(&t.steps).all(|(a, b)| *a == b),
            "template steps are untouched"
        );
    }

    /// DR-059 (AC-F025-01, -03): the ID, the position, the copied config, a new session, optional
    /// by default and `core` on request, and the applicable path lists it in place.
    #[test]
    fn added_step_rules() {
        let t = tpl();
        let none = OverlayFile::empty();
        let p = path(&t, &none);
        let (o1, id1) = ov::add_step(&t, &p, &none, &new_after("2.6")).unwrap();
        assert_eq!(id1, "x-2-1");
        let a = &o1.added_steps[0];
        assert_eq!(
            (a.stage, a.after.as_str(), a.copied_from.as_str()),
            (2, "2.6", "2.6")
        );
        assert_eq!(a.tag, AddedTag::Optional, "optional by default");
        assert_eq!(a.surface, Surface::Cli);
        assert_eq!(a.cwd, Some(CwdKind::Spec));
        assert_eq!(a.start, fable_default());

        let (eff, flags) = ov::effective_template(&t, &o1);
        assert!(flags.is_empty(), "{flags:?}");
        let s = eff.step("x-2-1").unwrap();
        assert_eq!(s.session, Session::New, "an added step never joins a chain");
        assert!(s.chain.is_none());
        assert_eq!(s.tag, Tag::Optional);
        assert!(
            s.prompt
                .as_ref()
                .unwrap()
                .joined()
                .starts_with("Read spec/council")
        );
        let keys: Vec<&str> = eff.stage_steps(2).map(|s| s.key.as_str()).collect();
        let at = keys.iter().position(|k| *k == "2.6").unwrap();
        assert_eq!(
            keys[at + 1],
            "x-2-1",
            "sits right after the step it names: {keys:?}"
        );

        // The path lists it in position, skippable because it is optional.
        let pe = path(&eff, &o1);
        assert_eq!(pe.step("x-2-1"), Some(&A::Skippable));
        let order: Vec<&str> = pe.steps.iter().map(|(k, _)| k.as_str()).collect();
        let i2_6 = order.iter().position(|k| *k == "2.6").unwrap();
        assert_eq!(order[i2_6 + 1], "x-2-1");

        // n = 1 + the highest in the stage; core on request is not skippable; a later addition
        // after the same step sits directly below it.
        let mut again = new_after("2.6");
        again.tag = AddedTag::Core;
        let (o2, id2) = ov::add_step(&eff, &pe, &o1, &again).unwrap();
        assert_eq!(id2, "x-2-2");
        let (eff2, _) = ov::effective_template(&t, &o2);
        assert_eq!(path(&eff2, &o2).step("x-2-2"), Some(&A::Applicable));
        let keys: Vec<&str> = eff2.stage_steps(2).map(|s| s.key.as_str()).collect();
        let at = keys.iter().position(|k| *k == "2.6").unwrap();
        assert_eq!(&keys[at + 1..at + 3], ["x-2-2", "x-2-1"]);
        // An added step can be skipped like an optional template step, with a reason.
        let pe2 = path(&eff2, &o2);
        let skipped = r::skip(
            &eff2,
            &pe2,
            &o2,
            "x-2-1",
            "not needed here",
            "2026-10-20T12:00:00Z",
        )
        .unwrap();
        assert!(matches!(
            path(&eff2, &skipped).step("x-2-1"),
            Some(A::Skipped(_))
        ));
        // It can go after another added step, and only in its own stage.
        let mut after_added = new_after("x-2-1");
        after_added.title = "Second".into();
        assert!(ov::add_step(&eff2, &pe2, &o2, &after_added).is_ok());
        let mut wrong = new_after("5.1");
        wrong.stage = 2;
        assert!(matches!(
            ov::add_step(&eff2, &pe2, &o2, &wrong),
            Err(OverlayError::WrongStage(..))
        ));
        assert!(matches!(
            ov::add_step(&eff2, &pe2, &o2, &new_after("9.9")),
            Err(OverlayError::UnknownStep(_))
        ));
        // A stage that doesn't apply takes nothing: Stage 0 (idea_known = yes).
        let mut s0 = new_after("0.1");
        s0.stage = 0;
        assert_eq!(
            ov::add_step(&eff2, &pe2, &o2, &s0).unwrap_err(),
            OverlayError::StageNotApplicable(0)
        );
        assert!(matches!(pe2.stage(0), Some(StageState::NotApplicable(_))));
    }

    /// DR-060 (AC-F025-02): `xi-<stage>-<n>`, kind `attest` only, one line, numbered per stage.
    #[test]
    fn added_item_attest_only() {
        let t = tpl();
        let none = OverlayFile::empty();
        let p = path(&t, &none);
        let (o1, id) = ov::add_item(&p, &none, 5, "  Billing sandbox keys rotated ").unwrap();
        assert_eq!(id, "xi-5-1");
        let i = &o1.added_items[0];
        assert_eq!(
            (i.kind.as_str(), i.stage, i.text.as_str()),
            ("attest", 5, "Billing sandbox keys rotated")
        );
        let (o2, id2) = ov::add_item(&p, &o1, 5, "Second").unwrap();
        assert_eq!(id2, "xi-5-2");
        let (_, other) = ov::add_item(&p, &o2, 2, "Another stage").unwrap();
        assert_eq!(other, "xi-2-1");
        for bad in ["", "   ", "two\nlines", &"x".repeat(201)] {
            assert_eq!(
                ov::add_item(&p, &o2, 5, bad).unwrap_err(),
                OverlayError::BadText,
                "{bad:?}"
            );
        }
        assert_eq!(
            ov::add_item(&p, &o2, 5, "use bypassPermissions").unwrap_err(),
            OverlayError::BypassPermissions
        );
        assert_eq!(
            ov::add_item(&p, &o2, 0, "x").unwrap_err(),
            OverlayError::StageNotApplicable(0)
        );
    }

    /// DR-061 (AC-F025-05): the same checks as a template step: `bypassPermissions`, the run-config
    /// vocabulary and the write allowlist.
    #[test]
    fn added_step_validated() {
        let t = tpl();
        let none = OverlayFile::empty();
        let p = path(&t, &none);
        let try_with = |f: &dyn Fn(&mut NewStep)| {
            let mut n = new_after("2.6");
            f(&mut n);
            ov::add_step(&t, &p, &none, &n).map(|(_, id)| id)
        };
        assert!(try_with(&|_| {}).is_ok());
        // DR-016, in any field, any case.
        for f in [
            (|n: &mut NewStep| n.text = "run with --permission-mode BYPASSPERMISSIONS".into())
                as fn(&mut NewStep),
            |n| n.title = "bypassPermissions".into(),
            |n| n.out = vec!["docs/bypasspermissions.md".into()],
        ] {
            assert_eq!(try_with(&f).unwrap_err(), OverlayError::BypassPermissions);
        }
        // DR-026 / DR-160: out paths go through the write allowlist.
        for bad in [
            ".github/workflows/x.yml",
            "src/main.rs",
            "../x.md",
            "/etc/x.md",
            "docs/../../x.md",
            "CLAUDE.md",
            "docs/.claude/x.md",
        ] {
            let e = try_with(&|n| n.out = vec![bad.to_owned()]).unwrap_err();
            assert!(matches!(e, OverlayError::Path(_)), "{bad}: {e:?}");
        }
        assert!(try_with(&|n| n.out = vec!["docs/notes/council-names.md".into()]).is_ok());
        assert_eq!(
            try_with(&|n| n.out = vec!["docs/a.md".into(), "docs/a.md".into()]).unwrap_err(),
            OverlayError::BadOut
        );
        // DR-023 by surface: cli needs a mode and cwd; chat has neither and uses adaptive.
        assert!(matches!(
            try_with(&|n| n.start.mode = None),
            Err(OverlayError::InvalidStart(_))
        ));
        assert!(matches!(
            try_with(&|n| n.cwd = None),
            Err(OverlayError::InvalidStart(_))
        ));
        assert!(matches!(
            try_with(&|n| n.start.effort = Effort::Adaptive),
            Err(OverlayError::InvalidStart(_))
        ));
        let chat = |n: &mut NewStep| {
            n.surface = Surface::Chat;
            n.cwd = None;
            n.start = Start {
                model: Model::Opus,
                effort: Effort::Adaptive,
                mode: None,
            };
        };
        assert!(try_with(&chat).is_ok());
        assert!(matches!(
            try_with(&|n| {
                chat(n);
                n.start.mode = Some(Mode::Plan);
            }),
            Err(OverlayError::InvalidStart(_))
        ));
        assert_eq!(
            try_with(&|n| n.title = String::new()).unwrap_err(),
            OverlayError::BadTitle
        );
        assert_eq!(
            try_with(&|n| n.title = "a\nb".into()).unwrap_err(),
            OverlayError::BadTitle
        );
        assert_eq!(
            try_with(&|n| n.text = "  ".into()).unwrap_err(),
            OverlayError::BadText
        );
        assert_eq!(
            try_with(&|n| n.text = "x".repeat(4001)).unwrap_err(),
            OverlayError::BadText
        );

        // A hand-edited overlay with a bypass step is left out of the project and flagged.
        let (good, _) = ov::add_step(&t, &p, &none, &new_after("2.6")).unwrap();
        let mut edited = good.clone();
        edited.added_steps[0].text = "use bypassPermissions".into();
        let (eff, flags) = ov::effective_template(&t, &edited);
        assert!(eff.step("x-2-1").is_none(), "never listed, so never run");
        assert!(
            flags
                .iter()
                .any(|f| f.starts_with("rules.invalid-added-step: x-2-1")),
            "{flags:?}"
        );
        let mut orphan = good;
        orphan.added_steps[0].after = foreman_core::ids::StepKey::parse("2.99").unwrap();
        let (eff, flags) = ov::effective_template(&t, &orphan);
        assert!(eff.step("x-2-1").is_none() && flags.len() == 1, "{flags:?}");
    }

    fn arb_start() -> impl Strategy<Value = Start> {
        (
            prop::sample::select(vec![Model::Fable, Model::Opus, Model::Sonnet, Model::Haiku]),
            prop::sample::select(vec![
                Effort::Low,
                Effort::Medium,
                Effort::High,
                Effort::Xhigh,
            ]),
            prop::sample::select(vec![
                Mode::Plan,
                Mode::Default,
                Mode::AcceptEdits,
                Mode::Auto,
                Mode::DontAsk,
            ]),
        )
            .prop_map(|(model, effort, mode)| Start {
                model,
                effort,
                mode: Some(mode),
            })
    }

    proptest! {
        /// DR-062 (AC-F025-04): an overlay round-trips through JSON unchanged, and every added step
        /// appears in the applicable path, right after the step it names.
        #[test]
        fn overlay_roundtrip(
            adds in proptest::collection::vec((prop::sample::select(vec!["2.2", "2.4", "2.6", "5.1", "5.4"]), arb_start(), any::<bool>(), "[a-zA-Z0-9 ]{1,40}"), 0..6),
            items in proptest::collection::vec("[a-zA-Z0-9 ]{1,40}", 0..4),
        ) {
            let t = tpl();
            let mut o = OverlayFile::empty();
            let mut eff = t.clone();
            for (after, start, core, title) in &adds {
                let stage = after.split('.').next().unwrap().parse::<u8>().unwrap();
                prop_assume!(!title.trim().is_empty());
                let n = NewStep {
                    stage,
                    after: (*after).to_owned(),
                    copied_from: (*after).to_owned(),
                    title: title.clone(),
                    text: format!("Do the thing: {title}"),
                    surface: Surface::Cli,
                    cwd: Some(CwdKind::Project),
                    start: *start,
                    out: vec![],
                    tag: if *core { AddedTag::Core } else { AddedTag::Optional },
                };
                let (o2, _) = ov::add_step(&eff, &path(&eff, &o), &o, &n).unwrap();
                o = o2;
                eff = ov::effective_template(&t, &o).0;
            }
            for text in &items {
                prop_assume!(!text.trim().is_empty());
                o = ov::add_item(&path(&t, &o), &o, 5, text).unwrap().0;
            }
            let json = serde_json::to_string(&o).unwrap();
            let back: OverlayFile = foreman_core::json::parse(json.as_bytes()).unwrap();
            prop_assert_eq!(&back, &o);
            let (eff, flags) = ov::effective_template(&t, &back);
            prop_assert!(flags.is_empty(), "{:?}", flags);
            let p = path(&eff, &back);
            for a in &back.added_steps {
                let order: Vec<&str> = p.steps.iter().map(|(k, _)| k.as_str()).collect();
                let at = order.iter().position(|k| *k == a.id).unwrap();
                let after = order.iter().position(|k| *k == a.after.as_str()).unwrap();
                prop_assert!(at > after, "{} must follow {}", a.id, a.after);
                prop_assert_eq!(eff.step(&a.id).unwrap().stage, a.stage);
            }
            prop_assert_eq!(eff.steps.len(), t.steps.len() + back.added_steps.len());
        }
    }
}
