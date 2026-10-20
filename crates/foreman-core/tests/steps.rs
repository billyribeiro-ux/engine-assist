//! M1-24: step kinds and states (DR-065–DR-069, AC-F007-01/-05/-06).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]

use foreman_core::ids::Surface;
use foreman_core::rules::Applicability as A;
use foreman_core::schema::progress::StoredState as S;
use foreman_core::steps::{
    FollowupAction, StepCtx, StepEvent as E, StepState, blocked_by_wait, display_state,
    followup_action, fresh, transition,
};
use foreman_core::template::StepKind as K;
use jiff::civil::date;

fn ctx(kind: K, surface: Option<Surface>, a: A) -> StepCtx {
    StepCtx {
        kind,
        surface,
        applicability: a,
        today: date(2026, 10, 20),
        wait_until: None,
        at: "2026-10-20T10:00:00Z".into(),
    }
}

fn at(state: S) -> foreman_core::schema::progress::StepProgress {
    let mut p = fresh();
    p.state = state;
    p
}

mod steps {
    use super::*;
    use proptest::prelude::*;

    fn all_events() -> Vec<E> {
        vec![
            E::Copy,
            E::Spawn,
            E::Exited(0),
            E::Exited(1),
            E::Errored,
            E::Interrupted,
            E::MarkDone,
            E::PasteBackValid,
            E::OutChanged,
            E::ActionApplied,
            E::StartWait("2026-10-26".into()),
            E::Start,
        ]
    }

    fn derived() -> Vec<A> {
        vec![
            A::Applicable,
            A::Skippable,
            A::Skipped("r".into()),
            A::NotApplicable("commercial = no".into()),
            A::Undecided(vec![]),
            A::NeedsAnnotation,
            A::NeedsNewerForeman,
        ]
    }

    /// AC-F007-01: every (state, event) pair is legal or rejected with a reason; no `adopted`.
    #[test]
    fn transitions_exhaustive() {
        let kinds = [
            K::Prompt,
            K::Followup,
            K::Action,
            K::Manual,
            K::Wait,
            K::Repeatable,
        ];
        let mut table = Vec::new();
        for kind in kinds {
            for surface in [Some(Surface::Chat), Some(Surface::Cli), None] {
                for a in derived() {
                    for s in [S::NotStarted, S::Copied, S::Running, S::Done] {
                        for e in all_events() {
                            let c = ctx(kind, surface, a.clone());
                            match transition(&at(s), &c, &e) {
                                Ok(n) => {
                                    assert!(a.on_path(), "{kind:?} {a:?} accepted {e:?}");
                                    table.push(format!(
                                        "{} {:?} {:?} {:?} → {:?}",
                                        kind.as_str(),
                                        surface,
                                        s,
                                        e,
                                        n.state
                                    ));
                                }
                                Err(r) => assert!(!r.0.is_empty()),
                            }
                        }
                    }
                }
            }
        }
        let json = serde_json::to_string(&StepState::Done).unwrap();
        assert!(!json.contains("adopted"));
        // Specific DR-065 rows.
        let chat = ctx(K::Prompt, Some(Surface::Chat), A::Applicable);
        let cli = ctx(K::Prompt, Some(Surface::Cli), A::Applicable);
        assert_eq!(
            transition(&at(S::NotStarted), &chat, &E::Copy)
                .unwrap()
                .state,
            S::Copied
        );
        assert_eq!(
            transition(&at(S::Copied), &chat, &E::MarkDone)
                .unwrap()
                .state,
            S::Done
        );
        assert_eq!(
            transition(&at(S::NotStarted), &cli, &E::Spawn)
                .unwrap()
                .state,
            S::Running
        );
        assert_eq!(
            transition(&at(S::Running), &cli, &E::Exited(0))
                .unwrap()
                .state,
            S::Done
        );
        assert_eq!(
            transition(&at(S::Running), &cli, &E::Interrupted)
                .unwrap()
                .state,
            S::NotStarted
        );
        assert_eq!(
            transition(&at(S::Done), &cli, &E::Spawn).unwrap().state,
            S::Running,
            "re-run"
        );
        assert_eq!(
            transition(
                &at(S::NotStarted),
                &ctx(K::Prompt, None, A::NeedsAnnotation),
                &E::MarkDone
            )
            .unwrap_err()
            .0,
            "needs annotation"
        );
        insta::assert_snapshot!(table.join("\n"));
    }

    proptest! {
        /// Random event sequences never reach a state outside the four stored ones and never
        /// change a derived (off-path) step.
        #[test]
        fn transitions_never_escape(evs in proptest::collection::vec(0usize..12, 0..30), k in 0usize..6, cli in any::<bool>()) {
            let kinds = [K::Prompt, K::Followup, K::Action, K::Manual, K::Wait, K::Repeatable];
            let c = ctx(kinds[k], Some(if cli { Surface::Cli } else { Surface::Chat }), A::Applicable);
            let mut p = fresh();
            for i in evs {
                if let Ok(n) = transition(&p, &c, &all_events()[i]) { p = n; }
                prop_assert!(matches!(p.state, S::NotStarted | S::Copied | S::Running | S::Done));
            }
        }
    }

    /// AC-F007-05 / DR-067.
    #[test]
    fn wait_until_date() {
        let mut c = ctx(K::Wait, None, A::Applicable);
        let started = transition(&fresh(), &c, &E::StartWait("2026-10-26".into())).unwrap();
        assert_eq!(started.state, S::Running);
        assert!(transition(&fresh(), &c, &E::StartWait("next week".into())).is_err());
        c.wait_until = Some(date(2026, 10, 26));
        assert_eq!(
            display_state(&started, &c),
            StepState::WaitingUntil("2026-10-26".into())
        );
        assert_eq!(
            transition(&started, &c, &E::MarkDone).unwrap_err().0,
            "waiting until 2026-10-26"
        );
        assert_eq!(
            blocked_by_wait(K::Wait, Some(date(2026, 10, 26)), false, c.today),
            Some("Waiting until 2026-10-26".into())
        );
        c.today = date(2026, 10, 26);
        assert_eq!(display_state(&started, &c), StepState::Running);
        assert_eq!(
            transition(&started, &c, &E::MarkDone).unwrap().state,
            S::Done
        );
        assert_eq!(
            blocked_by_wait(K::Wait, Some(date(2026, 10, 26)), false, c.today),
            None
        );
    }

    /// AC-F007-06 / DR-069.
    #[test]
    fn repeatable_counter() {
        let c = ctx(K::Repeatable, Some(Surface::Cli), A::Applicable);
        let mut p = fresh();
        for n in 1..=3u32 {
            p = transition(&p, &c, &E::Spawn).unwrap();
            p = transition(&p, &c, &E::Exited(0)).unwrap();
            assert_eq!((p.state, p.run_count), (S::NotStarted, n));
        }
    }

    /// DR-066 (AC-F007-04/-08, AC-F012-12).
    #[test]
    fn finish_rules() {
        // Chat: Mark done or a valid paste-back.
        let chat = ctx(K::Prompt, Some(Surface::Chat), A::Applicable);
        assert_eq!(
            transition(&at(S::Copied), &chat, &E::PasteBackValid)
                .unwrap()
                .state,
            S::Done
        );
        // CLI: on exit, any code (recorded elsewhere); never by hand.
        let cli = ctx(K::Prompt, Some(Surface::Cli), A::Applicable);
        assert_eq!(
            transition(&at(S::Running), &cli, &E::Exited(2))
                .unwrap()
                .state,
            S::Done
        );
        assert!(
            transition(&at(S::Running), &cli, &E::MarkDone)
                .unwrap_err()
                .0
                .contains("can't be marked done by hand")
        );
        // Manual: when a declared out file changes after it started, or Mark done.
        let man = ctx(K::Manual, None, A::Applicable);
        let started = transition(&fresh(), &man, &E::Start).unwrap();
        assert_eq!(
            transition(&started, &man, &E::OutChanged).unwrap().state,
            S::Done
        );
        assert!(
            transition(&fresh(), &man, &E::OutChanged).is_err(),
            "a change before it started doesn't count"
        );
        assert_eq!(
            transition(&fresh(), &man, &E::MarkDone).unwrap().state,
            S::Done
        );
        // Action: on a successful apply.
        let act = ctx(K::Action, None, A::Applicable);
        assert_eq!(
            transition(&fresh(), &act, &E::ActionApplied).unwrap().state,
            S::Done
        );
        assert!(transition(&fresh(), &act, &E::MarkDone).is_err());
    }

    /// AC-F007-07 / DR-068.
    #[test]
    fn followup_copy_or_relaunch() {
        assert_eq!(followup_action(true), FollowupAction::CopyAndFocus);
        assert_eq!(followup_action(false), FollowupAction::Relaunch);
        let live = ctx(K::Followup, Some(Surface::Cli), A::Applicable);
        assert_eq!(
            transition(&fresh(), &live, &E::Copy).unwrap().state,
            S::Copied
        );
        assert_eq!(
            transition(&fresh(), &live, &E::Spawn).unwrap().state,
            S::Running,
            "relaunch as a continuing launch"
        );
    }
}
