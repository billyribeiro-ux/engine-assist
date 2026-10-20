//! M4-15, M4-18, M4-24: the gate model (DR-070–DR-073), the signable predicate (DR-071) and the
//! lessons requirement (DR-074).
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

use std::collections::BTreeMap;

use foreman_core::gate::{
    self as g, Attest, AttestCtx, AttestError, ChainSession, CloseoutFacts, ItemKind, ItemStatus,
    SignBlocker, SignFacts, StageType,
};
use foreman_core::ids::{ConditionId as C, ItemId};
use foreman_core::lessons;
use foreman_core::schema::progress::{GateState as S, ItemOutcome, ItemResult};
use jiff::civil::date;
use proptest::prelude::*;

fn id(s: &str) -> ItemId {
    ItemId::parse(s).unwrap()
}

fn result(outcome: ItemOutcome, text: &str) -> ItemResult {
    ItemResult {
        outcome,
        text: text.into(),
        link: None,
        at: "2026-10-20T00:00:00Z".into(),
    }
}

fn facts() -> SignFacts {
    SignFacts {
        stage: 5,
        closeout: S::Signable,
        unresolved: Vec::new(),
        lessons_found: true,
        chain: ChainSession::Idle,
        undecided_early: Vec::new(),
        undecided_late: Vec::new(),
        packet_valid: true,
        git_found: true,
        signed: false,
    }
}

fn arb_state() -> impl Strategy<Value = S> {
    proptest::sample::select(g::SEQUENCE.to_vec())
}

fn arb_chain() -> impl Strategy<Value = ChainSession> {
    prop_oneof![
        Just(ChainSession::Idle),
        Just(ChainSession::Busy),
        Just(ChainSession::BusyConfirmed)
    ]
}

fn arb_cond() -> impl Strategy<Value = C> {
    proptest::sample::select(C::ALL.to_vec())
}

prop_compose! {
    fn arb_facts()(
        stage in 0u8..=17,
        closeout in arb_state(),
        unresolved in proptest::collection::vec("[a-z]{1,8}", 0..3),
        lessons_found in any::<bool>(),
        chain in arb_chain(),
        undecided in proptest::collection::vec(arb_cond(), 0..3),
        packet_valid in any::<bool>(),
        git_found in any::<bool>(),
        signed in any::<bool>(),
    ) -> SignFacts {
        SignFacts {
            stage,
            closeout,
            unresolved,
            lessons_found,
            chain,
            undecided_early: undecided.iter().copied().filter(|c| !c.is_late()).collect(),
            undecided_late: undecided.into_iter().filter(|c| c.is_late()).collect(),
            packet_valid,
            git_found,
            signed,
        }
    }
}

prop_compose! {
    fn arb_closeout()(
        steps_done in any::<bool>(),
        gate_a_ran in any::<bool>(),
        items_resolved in any::<bool>(),
        gate_b_ran in any::<bool>(),
        packet_valid in any::<bool>(),
        gate_c_ran in any::<bool>(),
        lessons_found in any::<bool>(),
    ) -> CloseoutFacts {
        CloseoutFacts { steps_done, gate_a_ran, items_resolved, gate_b_ran, packet_valid, gate_c_ran, lessons_found }
    }
}

fn arb_type() -> impl Strategy<Value = StageType> {
    prop_oneof![
        Just(StageType::Chat),
        Just(StageType::Council),
        Just(StageType::Gated)
    ]
}

mod gate {
    use super::*;

    proptest! {
        /// DR-071, AC-F014-01, AC-F015-10, AC-F004-13: signable only when every condition holds,
        /// and always when they do. The gate view's Sign is enabled with only a busy chain left,
        /// which the native sign dialog confirms.
        #[test]
        fn signable_requires_all(f in arb_facts()) {
            let all = !f.signed
                && f.git_found
                && f.packet_valid
                && f.unresolved.is_empty()
                && f.lessons_found
                && f.closeout == S::Signable
                && f.chain != ChainSession::Busy
                && !(f.stage == 2 && !f.undecided_early.is_empty())
                && !(f.stage == 4 && !f.undecided_late.is_empty());
            prop_assert_eq!(g::signable(&f), all);
            let enabled = g::sign_enabled(&f);
            let busy_only = all || (f.chain == ChainSession::Busy && g::signable(&SignFacts { chain: ChainSession::BusyConfirmed, ..f.clone() }));
            prop_assert_eq!(enabled, busy_only);
            // Every blocker has a reason Billy can read.
            for b in g::sign_blockers(&f) {
                prop_assert!(!b.text(f.stage).is_empty());
            }
        }

        /// AC-F015-10: no facts reach `signable` with an applicable item unresolved (or with any
        /// earlier fact missing).
        #[test]
        fn never_signable_with_unresolved_items(k in arb_type(), f in arb_closeout()) {
            if g::derive(k, &f) == S::Signable {
                prop_assert!(f.items_resolved && f.packet_valid && f.lessons_found);
                if k == StageType::Gated {
                    prop_assert!(f.steps_done && f.gate_a_ran && f.gate_b_ran && f.gate_c_ran);
                }
            }
        }
    }

    /// AC-F014-03, AC-F004-13: the reasons name git and the undecided early condition.
    #[test]
    fn reasons_name_git_and_conditions() {
        let f = SignFacts {
            git_found: false,
            ..facts()
        };
        let text: Vec<String> = g::sign_blockers(&f).iter().map(|b| b.text(5)).collect();
        assert!(text.iter().any(|t| t.contains("git")), "{text:?}");
        let f = SignFacts {
            stage: 2,
            undecided_early: vec![C::Server],
            ..facts()
        };
        assert_eq!(
            g::sign_blockers(&f),
            vec![SignBlocker::UndecidedEarly(C::Server)]
        );
        assert_eq!(
            g::sign_blockers(&f)[0].text(2),
            "Decide server before signing Stage 2."
        );
        // Stage 4 checks the late conditions only.
        let f = SignFacts {
            stage: 4,
            undecided_late: vec![C::HasDatabase],
            ..facts()
        };
        assert_eq!(
            g::sign_blockers(&f),
            vec![SignBlocker::UndecidedLate(C::HasDatabase)]
        );
    }

    /// AC-F015-08: every out-of-order transition is rejected; only the next state is allowed.
    #[test]
    fn state_machine_rejects_out_of_order() {
        for (i, from) in g::SEQUENCE.iter().enumerate() {
            for (j, to) in g::SEQUENCE.iter().enumerate() {
                let r = g::transition(*from, *to);
                if j == i + 1 {
                    assert_eq!(r, Ok(*to));
                } else {
                    assert!(r.is_err(), "{from:?} → {to:?} must be rejected");
                }
            }
        }
        assert_eq!(
            g::transition(S::A, S::B).unwrap_err().0,
            "Resolve every inspection item first"
        );
        assert_eq!(
            g::transition(S::NotReady, S::Results).unwrap_err().0,
            "Run Gate A first"
        );
    }

    /// AC-F015-09: not-applicable items are excluded from the gate count and count as resolved.
    #[test]
    fn not_applicable_excluded() {
        let st = [
            ItemStatus::Pass,
            ItemStatus::NotApplicable,
            ItemStatus::Pass,
        ];
        assert_eq!(g::counted(&st), 2);
        assert!(g::all_resolved(&st));
        assert!(!g::all_resolved(&[
            ItemStatus::NotApplicable,
            ItemStatus::Waiting
        ]));
    }
}

mod steps {
    use super::*;

    fn ctx<'a>(
        item: &'a ItemId,
        kind: ItemKind,
        results: &'a BTreeMap<ItemId, ItemResult>,
    ) -> AttestCtx<'a> {
        AttestCtx {
            item,
            kind,
            needs_reconfirm: false,
            first_run: Some(date(2026, 10, 1)),
            today: date(2026, 10, 20),
            results,
        }
    }

    fn done(note: &str) -> Attest {
        Attest::Done {
            note: note.into(),
            link: None,
        }
    }

    /// DR-073, AC-F015-02/-03: Done needs ≥ 15 characters that repeat no other note in the gate
    /// (compared lowercased and whitespace-collapsed); Not applicable needs a reason; a `time`
    /// item is available only `n` days after the stage's first run.
    #[test]
    fn attest_note_rules() {
        let a = id("fresh-clone");
        let b = id("ci-gates");
        let mut results = BTreeMap::new();
        let c = ctx(&a, ItemKind::Attest, &results);
        assert_eq!(
            g::check_attest(&c, &done("checked it")),
            Err(AttestError::NoteTooShort)
        );
        assert_eq!(
            g::check_attest(&c, &done("   checked it, ok    ")),
            Err(AttestError::NoteTooShort),
            "trimmed before counting"
        );
        assert_eq!(g::check_attest(&c, &done("checked it, all good")), Ok(()));
        results.insert(b.clone(), result(ItemOutcome::Done, "checked it, all good"));
        let c = ctx(&a, ItemKind::Attest, &results);
        assert_eq!(
            g::check_attest(&c, &done("Checked  it,   ALL good")),
            Err(AttestError::NoteRepeated)
        );
        assert_eq!(AttestError::NoteRepeated.rule(), "note-repeated");
        // Its own earlier note isn't a repeat.
        let c = ctx(&b, ItemKind::Attest, &results);
        assert_eq!(g::check_attest(&c, &done("checked it, all good")), Ok(()));
        // Not applicable: any non-empty reason.
        let c = ctx(&a, ItemKind::Attest, &results);
        assert_eq!(
            g::check_attest(
                &c,
                &Attest::NotApplicable {
                    reason: "  ".into()
                }
            ),
            Err(AttestError::ReasonRequired)
        );
        assert_eq!(
            g::check_attest(&c, &Attest::NotApplicable { reason: "n".into() }),
            Ok(())
        );
        // time:30d from 2026-10-01: not before 2026-10-31.
        let t = ItemKind::Time {
            days: 30,
            then_attest: true,
        };
        let c = ctx(&a, t, &results);
        assert_eq!(
            g::check_attest(&c, &done("thirty days of logs, no gaps")),
            Err(AttestError::TimeNotElapsed(date(2026, 10, 31)))
        );
        let c = AttestCtx {
            today: date(2026, 10, 31),
            ..ctx(&a, t, &results)
        };
        assert_eq!(
            g::check_attest(&c, &done("thirty days of logs, no gaps")),
            Ok(())
        );
        // `time:<n>d` without `>attest` is done once the date has come, without a note.
        let plain = ItemKind::Time {
            days: 3,
            then_attest: false,
        };
        let c = ctx(&a, plain, &results);
        assert_eq!(g::check_attest(&c, &done("")), Ok(()));
        // Not run yet: never available.
        let c = AttestCtx {
            first_run: None,
            ..ctx(&a, plain, &results)
        };
        assert!(matches!(
            g::check_attest(&c, &done("")),
            Err(AttestError::TimeNotElapsed(_))
        ));
        // A drifted item is blocked until its kind is re-confirmed (AC-F001-17).
        let c = AttestCtx {
            needs_reconfirm: true,
            ..ctx(&a, ItemKind::Attest, &results)
        };
        assert_eq!(
            g::check_attest(&c, &done("checked it in a new way")),
            Err(AttestError::KindNeedsReconfirm)
        );
        // Clear is always allowed.
        assert_eq!(g::check_attest(&c, &Attest::Clear), Ok(()));
        // Kinds parse as DR-031 writes them.
        assert_eq!(g::parse_kind("attest"), ItemKind::Attest);
        assert_eq!(g::parse_kind("auto:cmd=adr-lint"), ItemKind::Attest);
        assert_eq!(
            g::parse_kind("time:7d>attest"),
            ItemKind::Time {
                days: 7,
                then_attest: true
            }
        );
        assert_eq!(g::parse_kind("time:0d"), ItemKind::Attest);
        // The third failure of one item shows F.4.
        let mut f = BTreeMap::new();
        assert!(!g::shows_f4(g::record_failure(&mut f, &a)));
        assert!(!g::shows_f4(g::record_failure(&mut f, &a)));
        assert!(g::shows_f4(g::record_failure(&mut f, &a)));
        assert!(!g::shows_f4(g::record_failure(&mut f, &b)));
    }

    /// DR-072, AC-F015-06: FAIL, then waiting, then PASS (and not applicable), each in template
    /// order.
    #[test]
    fn gate_item_order() {
        let r = |o| Some(result(o, "note long enough here"));
        let st: Vec<ItemStatus> = [
            r(ItemOutcome::Pass),
            None,
            r(ItemOutcome::Fail),
            r(ItemOutcome::NotApplicable),
            None,
            r(ItemOutcome::Fail),
            r(ItemOutcome::Done),
        ]
        .iter()
        .map(|x| g::status(x.as_ref()))
        .collect();
        assert_eq!(g::order(&st), vec![2, 5, 1, 4, 0, 3, 6]);
        assert_eq!(g::counted(&st), 6);
    }

    /// DR-074, AC-F016-05/-06: only `## L-NNN · Stage N · …` for that stage meets it ("clean
    /// stage" in that form counts; another stage's entry doesn't); the next ID is max + 1.
    #[test]
    fn lessons_entry_detected() {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/lessons.md"),
        )
        .unwrap();
        let s2 = lessons::status(&text, 2);
        assert!(s2.met);
        assert_eq!(s2.entries.len(), 6);
        assert_eq!(s2.entries[5].id, "L-006");
        assert!(
            s2.entries[5]
                .title
                .starts_with("2026-10-19 · The chair's resolution")
        );
        assert!(!lessons::status(&text, 5).met);
        // The header's `## L-NNN · Stage [N]` template line is not an entry.
        assert!(lessons::entries(&text).iter().all(|e| e.number > 0));
        let next = lessons::next_id(&text);
        let max = lessons::entries(&text)
            .iter()
            .map(|e| e.number)
            .max()
            .unwrap();
        assert_eq!(next, format!("L-{:03}", max + 1));
        // A clean-stage entry counts; a Stage 50 or a malformed heading doesn't.
        let t = "# Lessons\n\n## L-012 · Stage 5 · 2026-11-02 · clean stage\n## L-13 · Stage 6 · x\n## L-014 · Stage 6 2026\n";
        assert!(lessons::status(t, 5).met);
        assert!(!lessons::status(t, 6).met);
        assert_eq!(lessons::next_id(t), "L-013");
        assert_eq!(lessons::next_id("# Lessons\n"), "L-001");
    }

    /// DR-070, AC-F015-01/-08, AC-F012-16: each stage type's close-out runs in its order, and a
    /// later fact never counts before an earlier one (chat: attest → F.2 lessons → Sign).
    #[test]
    fn closeout_sequences() {
        assert_eq!(g::stage_type(0), StageType::Chat);
        assert_eq!(g::stage_type(3), StageType::Chat);
        assert_eq!(g::stage_type(2), StageType::Council);
        assert_eq!(g::stage_type(5), StageType::Gated);
        assert_eq!(g::stage_type(17), StageType::Gated);
        let all = CloseoutFacts {
            steps_done: true,
            gate_a_ran: true,
            items_resolved: true,
            gate_b_ran: true,
            packet_valid: true,
            gate_c_ran: true,
            lessons_found: true,
        };
        for k in [StageType::Chat, StageType::Council, StageType::Gated] {
            assert_eq!(g::derive(k, &all), S::Signable);
        }
        // Chat: a valid packet, then the items, then the lessons.
        let chat = |p, i, l| CloseoutFacts {
            packet_valid: p,
            items_resolved: i,
            lessons_found: l,
            ..CloseoutFacts::default()
        };
        assert_eq!(
            g::derive(StageType::Chat, &chat(false, true, true)),
            S::NotReady
        );
        assert_eq!(g::derive(StageType::Chat, &chat(true, false, true)), S::A);
        assert_eq!(g::derive(StageType::Chat, &chat(true, true, false)), S::C);
        assert_eq!(
            g::derive(StageType::Chat, &chat(true, true, true)),
            S::Signable
        );
        // Gated: Gate B without Gate A's results stays at `a`.
        let b_first = CloseoutFacts {
            items_resolved: false,
            ..all
        };
        assert_eq!(g::derive(StageType::Gated, &b_first), S::A);
        let no_a = CloseoutFacts {
            gate_a_ran: false,
            ..all
        };
        assert_eq!(g::derive(StageType::Gated, &no_a), S::NotReady);
        let no_c = CloseoutFacts {
            gate_c_ran: false,
            ..all
        };
        assert_eq!(g::derive(StageType::Gated, &no_c), S::PacketValid);
        let no_lessons = CloseoutFacts {
            lessons_found: false,
            ..all
        };
        assert_eq!(g::derive(StageType::Gated, &no_lessons), S::C);
    }
}
