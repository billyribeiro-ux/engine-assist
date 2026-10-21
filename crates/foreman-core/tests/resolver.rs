//! M1-22: prompt resolution (DR-084–DR-093, AC-F006-04…-11) and M1-23's prompt hygiene checks.
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

use common::{Fixture, blueprint, mini, parse};
use foreman_core::ids::{ConditionId as C, CwdKind, TriState as T};
use foreman_core::resolver::{
    PacketText, PromptSegment, Unresolved, render_for_cwd, required_inputs, resolve,
};
use foreman_core::sanitize::{Blocker, check_prompt};

mod resolver {
    use super::*;
    use proptest::prelude::*;

    /// AC-F006-04/-11 / DR-084: markers become the signed packets joined in order; an unsigned
    /// packet blocks with "Sign Stage N first"; a missing file blocks too.
    #[test]
    fn packet_markers() {
        let f = Fixture::tradelog();
        let p = f.resolve("4.1").text;
        let s2 = f.packets[&2].text.trim_end().to_owned();
        let s3 = f.packets[&3].text.trim_end().to_owned();
        assert!(
            p.starts_with(&format!(
                "Inputs: {s2}\n\n{s3}\nWrite the SCOPE document for TradeLog:"
            )),
            "{p}"
        );
        let mut g = Fixture::tradelog();
        g.packets.get_mut(&2).unwrap().signed = false;
        assert_eq!(
            resolve(&g.ctx("3.1")).unwrap_err(),
            [Unresolved::PacketUnsigned { stage: 2 }]
        );
        g.packets.remove(&2);
        assert_eq!(
            resolve(&g.ctx("3.1")).unwrap_err(),
            [Unresolved::PacketMissing { stage: 2 }]
        );
        // The marker text is exactly the packet: header through the last field.
        assert!(
            f.resolve("2.1")
                .text
                .contains("=== HANDOFF PACKET · Stage 1 → Stage 2 ===\nPROJECT: TradeLog\n")
        );
    }

    /// AC-F006-04 / DR-085: the inspection list holds the stage's items minus those attested N/A.
    #[test]
    fn inspection_list_filtered() {
        let mut f = Fixture::tradelog();
        f.attested_na.insert("staging".into());
        f.overlay_items
            .push(foreman_core::schema::overlay::AddedItem {
                id: "xi-5-1".into(),
                stage: 5,
                text: "Billing sandbox keys rotated".into(),
                kind: "attest".into(),
            });
        let a = f.resolve("5:gate.A").text;
        let list = a.split("INSPECTION ITEMS:\n").nth(1).unwrap();
        assert_eq!(
            list,
            "- A fresh clone, then `install` and `dev`, works with zero manual steps\n- CI is green, and an intentional type error, a planted fake secret, and a known-vulnerable dependency each fail it (prove the gates work, then revert)\n- The ADR reality check has no CONTRADICTED left open\n- Billing sandbox keys rotated\n"
        );
    }

    /// AC-F006-05 / DR-086: `[app]-spec/` → the spec folder, rendered for the cwd, with a note.
    #[test]
    fn app_spec_alias() {
        let body = |cwd: &str| {
            format!(
                "## Stage 9: A\n\n### Prompt 9.1: x\n\n> **RUN WITH** · x\n<!-- fm:step id=9.1 surface={} {cwd} start={} -->\n\n```text\nSave the result to [app]-spec/02-teardown.md and read docs/x.md.\n```\n",
                if cwd.is_empty() { "chat" } else { "cli" },
                if cwd.is_empty() {
                    "opus@adaptive"
                } else {
                    "opus@high/plan"
                }
            )
        };
        for (cwd, want) in [
            (
                "",
                "Save the result to spec/02-teardown.md and read docs/x.md.\n",
            ),
            (
                "cwd=spec",
                "Save the result to 02-teardown.md and read docs/x.md.\n",
            ),
            (
                "cwd=project",
                "Save the result to spec/02-teardown.md and read docs/x.md.\n",
            ),
        ] {
            let t = parse(&mini(&body(cwd)));
            assert!(t.errors().next().is_none(), "{:?}", t.diagnostics);
            let f = Fixture::with(t, common::tradelog_project());
            let r = f.resolve("9.1");
            assert_eq!(r.text, want, "{cwd}");
            assert_eq!(r.notes, ["`[app]-spec/` → `spec/`"]);
        }
    }

    /// AC-F006-10 / DR-088: a marker naming a not-applicable stage uses the latest signed packet
    /// before it, once, with a note.
    #[test]
    fn remap_not_applicable() {
        let mut f = Fixture::tradelog();
        f.project.conditions.insert(C::Commercial, T::No);
        f.project.conditions.insert(C::Payments, T::No);
        f.project.conditions.insert(C::OthersData, T::No);
        f.project.conditions.insert(C::MultiUser, T::No);
        f.repath();
        // Stage 3 still applies (3.2 is core); force it out to exercise the remap.
        f.path.stages.iter_mut().find(|(n, _)| *n == 3).unwrap().1 =
            foreman_core::rules::StageState::NotApplicable("test".into());
        let r = f.resolve("4.2");
        assert!(
            r.text.starts_with(&format!(
                "Inputs: {} + docs/scope.md",
                f.packets[&2].text.trim_end()
            )),
            "{}",
            r.text
        );
        assert!(!r.text.contains("Stage 3 → Stage 4"));
        let note = r.notes.join(" | ");
        assert!(
            note.contains("Stage 3 was not applicable; using Stage 2's packet")
                && note.contains("included once"),
            "{note}"
        );
    }

    /// AC-F006-12/-14 / DR-089: an input is required only if its token survives part removal.
    #[test]
    fn inputs_required_only_if_present() {
        let mut f = Fixture::tradelog();
        assert_eq!(required_inputs(&f.ctx("3.2")), ["jurisdictions"]);
        assert_eq!(
            resolve(&f.ctx("3.2")).unwrap_err(),
            [Unresolved::RunInputMissing {
                name: "jurisdictions".into()
            }]
        );
        for c in [C::Commercial, C::OthersData, C::MultiUser] {
            f.project.conditions.insert(c, T::No);
        }
        assert!(required_inputs(&f.ctx("3.2")).is_empty());
        assert!(resolve(&f.ctx("3.2")).is_ok());
        // Remembered inputs prefill; a large value is accepted with a note.
        let mut g = Fixture::tradelog();
        g.remembered
            .insert("jurisdictions".into(), "the US and the EU".into());
        assert!(g.resolve("3.2").text.contains("in the US and the EU:"));
        g.inputs
            .insert("jurisdictions".into(), "x".repeat(70 * 1024));
        assert!(
            g.resolve("3.2")
                .notes
                .iter()
                .any(|n| n.contains("over 64 KB"))
        );
        // 1.2's inputs.
        let mut h = Fixture::tradelog();
        assert_eq!(required_inputs(&h.ctx("1.2")), ["X", "changes"]);
        h.inputs.insert("X".into(), "B".into());
        h.inputs.insert("changes".into(), "mobile first".into());
        assert!(
            h.resolve("1.2")
                .text
                .starts_with("We're going with concept B (with these changes: mobile first). Now")
        );
    }

    /// AC-F006-07 / DR-090: Include defaults on; off removes the part and queues SKIPPED.
    #[test]
    fn optional_part_toggle() {
        let mut f = Fixture::tradelog();
        f.inputs
            .insert("paste_numbers".into(), "112 signups, 31% would pay".into());
        let on = f.resolve("1.4");
        assert!(on.text.contains(
            "Otherwise, write a press release for launch day as if the\nproduct already exists"
        ));
        assert!(on.text.contains("ask, with answers.\n\nThen, when I say"));
        assert!(!on.text.contains("[optional"));
        assert!(on.segments.contains(&PromptSegment::OptionalPart {
            part_id: "opt-1".into(),
            included: true
        }));
        f.optional.insert("opt-1".into(), false);
        let off = f.resolve("1.4");
        assert!(
            off.text
                .contains("Otherwise, \n\nThen, when I say \"approved\", output the APP BRIEF"),
            "{}",
            off.text
        );
        assert_eq!(
            off.skipped,
            ["1.4 part \"opt-1\": skipped: Include turned off"]
        );
    }

    /// AC-F006-07 / DR-091: a false fm:if part is removed and queued; undecided blocks.
    #[test]
    fn fm_if_removal() {
        let mut f = Fixture::tradelog();
        f.inputs.insert("jurisdictions".into(), "US".into());
        assert!(f.resolve("3.2").text.contains(
            "3. Legal and compliance for selling to Active retail and prop traders in US:"
        ));
        for c in [C::Commercial, C::OthersData, C::MultiUser] {
            f.project.conditions.insert(c, T::No);
        }
        let r = f.resolve("3.2");
        assert!(!r.text.contains("Legal and compliance"));
        assert!(
            r.text
                .contains("   consent, and the processors we'll share data with\nFor each item:")
        );
        assert_eq!(
            r.skipped,
            ["3.2 part \"fm:if commercial|others_data|multi_user\": not applicable (all false)"]
        );
        f.project.conditions.insert(C::MultiUser, T::Undecided);
        assert_eq!(
            resolve(&f.ctx("3.2")).unwrap_err(),
            [Unresolved::Undecided {
                message: "Decide multi_user to continue".into()
            }]
        );
        // 2.2's commercial-only council roles.
        let mut g = Fixture::tradelog();
        g.project.conditions.insert(C::Commercial, T::No);
        let r = g.resolve("2.2");
        assert!(
            !r.text.contains("market-analyst")
                && !r.text.contains("growth-strategist")
                && r.text.contains("user-advocate")
        );
        assert_eq!(r.skipped.len(), 2);
    }

    /// AC-F006-06 / DR-092: inserted paths render relative to the cwd; literal paths never change.
    #[test]
    fn cwd_relative_render() {
        assert_eq!(
            render_for_cwd("docs/handoff/stage-02.md", Some(CwdKind::Spec)),
            "../docs/handoff/stage-02.md"
        );
        assert_eq!(
            render_for_cwd("spec/council/r1.md", Some(CwdKind::Spec)),
            "council/r1.md"
        );
        assert_eq!(
            render_for_cwd("docs/handoff/stage-02.md", Some(CwdKind::Project)),
            "docs/handoff/stage-02.md"
        );
        assert_eq!(render_for_cwd("docs/x.md", None), "docs/x.md");
        let f = Fixture::tradelog();
        let r = f.resolve("2.6");
        assert!(
            r.text
                .contains("Then write the MASTER SPEC PACKET to ../docs/handoff/stage-02.md,"),
            "B-2's literal path is left alone"
        );
    }

    proptest! {
        /// AC-F006-08 / DR-093: resolving twice gives the same output (pure function).
        #[test]
        fn deterministic(x in ".{0,40}", changes in ".{0,40}", packet in ".{0,200}", on in any::<bool>()) {
            let mut f = Fixture::tradelog();
            f.inputs.insert("X".into(), format!("x{x}"));
            f.inputs.insert("changes".into(), format!("c{changes}"));
            f.inputs.insert("paste_numbers".into(), "1".into());
            f.optional.insert("opt-1".into(), on);
            f.packets.insert(2, PacketText { text: packet.clone(), signed: true });
            for k in ["1.2", "1.4", "3.1", "4.1", "2.2", "5:gate.B"] {
                let a = resolve(&f.ctx(k));
                let b = resolve(&f.ctx(k));
                prop_assert_eq!(a, b);
            }
        }
    }

    /// AC-F006-17 / DR-094: control characters from a packet, variable or input are refused with
    /// their position.
    #[test]
    fn rejects_control_chars_in_prompt() {
        let mut f = Fixture::tradelog();
        f.inputs.insert("X".into(), "B\u{1b}[31m".into());
        f.inputs.insert("changes".into(), "none".into());
        let r = f.resolve("1.2");
        let off = r.text.find('\u{1b}').unwrap();
        assert_eq!(
            check_prompt(&r.text),
            Err(Blocker::PromptControlChar {
                offset: u32::try_from(off).unwrap(),
                codepoint: 0x1b
            })
        );
        for (s, cp) in [
            ("a\u{7f}", 0x7f),
            ("a\u{85}b", 0x85),
            ("\u{0}", 0),
            ("x\u{9b}", 0x9b),
        ] {
            assert!(
                matches!(check_prompt(s), Err(Blocker::PromptControlChar { codepoint, .. }) if codepoint == cp),
                "{s:?}"
            );
        }
        assert!(check_prompt("tabs\tand\nnewlines\r\nare fine").is_ok());
        // No v2.2 prompt in Stages 1–2 starts with an option-like character.
        let t = parse(&blueprint());
        for s in t.steps.iter().filter(|s| s.stage <= 2) {
            if let Some(p) = &s.prompt {
                assert!(check_prompt(&p.joined()).is_ok(), "{}", s.key);
            }
        }
    }
}

mod argv {
    use super::*;

    /// AC-F008-12 / SR-04: after trimming, a leading `-`, `/` or `!` is refused.
    #[test]
    fn rejects_option_like_prompt() {
        for (p, c) in [
            ("-p hi", '-'),
            ("  --dangerously-skip-permissions", '-'),
            ("/model haiku", '/'),
            ("\n\t!rm -rf ~", '!'),
        ] {
            assert_eq!(
                check_prompt(p),
                Err(Blocker::PromptOptionLike { first: c }),
                "{p:?}"
            );
        }
        assert!(check_prompt("ultrathink. For each v1 feature").is_ok());
        assert!(check_prompt("a -p in the middle is fine").is_ok());
    }
}
