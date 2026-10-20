//! M1-13: step binding, prompts, gate and F.2 instances, packet formats (DR-006–DR-012, DR-017).
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

use common::{blueprint, blueprint_v21, mini, parse, replace_once, rules};
use foreman_core::ids::{CwdKind, Surface};
use foreman_core::template::{StepKind, StepStatus};

mod template {
    use super::*;

    /// DR-006: the `fm:step` must be the RUN WITH's next sibling, on the next line.
    #[test]
    fn fm_step_must_follow_run_with() {
        let bp = blueprint();
        let gap = replace_once(
            &bp,
            "· effort unchanged · **Accept edits**\n<!-- fm:step id=2.3",
            "· effort unchanged · **Accept edits**\n\n<!-- fm:step id=2.3",
        );
        let t = parse(&gap);
        assert!(
            rules(&t).contains(&"grammar.misplaced-step".to_owned()),
            "{:?}",
            rules(&t)
        );
        assert_eq!(t.step("2.3").unwrap().status, StepStatus::NeedsAnnotation);
        // An fm:step after the prompt fence instead of under the RUN WITH.
        let t = parse(&mini(
            "## Stage 9: A\n\n### Prompt 9.1: x\n\n> **RUN WITH** · chat\n\n```text\nhi\n```\n<!-- fm:step id=9.1 surface=chat start=opus@adaptive -->\n",
        ));
        assert!(rules(&t).contains(&"grammar.misplaced-step".to_owned()));
        // A different ID under the heading.
        let t = parse(&mini(
            "## Stage 9: A\n\n### Prompt 9.1: x\n\n> **RUN WITH** · chat\n<!-- fm:step id=9.2 surface=chat start=opus@adaptive -->\n\n```text\nhi\n```\n",
        ));
        assert!(rules(&t).contains(&"grammar.id-mismatch".to_owned()));
        // The real blueprint binds every annotated step.
        let t = parse(&bp);
        for k in ["1.1", "2.2", "2.6", "5.5"] {
            assert_eq!(t.step(k).unwrap().status, StepStatus::Runnable, "{k}");
        }
    }

    /// DR-007: standalone steps follow the prose that describes them, in source order.
    #[test]
    fn standalone_steps_in_source_order() {
        let t = parse(&blueprint());
        let order = |n: u8| t.stage_steps(n).map(|s| s.key.as_str()).collect::<Vec<_>>();
        assert_eq!(
            order(1),
            ["1.1", "1.2", "1.3", "1.3b", "1.4", "1:F.2", "1.4b"]
        );
        assert_eq!(
            order(2),
            [
                "2.1",
                "council-setup",
                "2.2",
                "2.3",
                "2.4",
                "2.4b",
                "2.5",
                "2.6",
                "2:F.2"
            ]
        );
        assert_eq!(&order(5)[..3], ["5.1", "5.1b", "5.2"]);
        assert_eq!(t.step("council-setup").unwrap().kind, StepKind::Action);
        assert_eq!(t.step("2.4b").unwrap().kind, StepKind::Manual);
        assert_eq!(t.step("2.4b").unwrap().out, ["spec/council/r7-stack.md"]);
        assert_eq!(
            t.step("1.3b").unwrap().title,
            "Run the test yourself, then paste the numbers into the same chat."
        );
        // A standalone fm:step after a list (not prose) is misplaced.
        let t = parse(&mini(
            "## Stage 9: A\n\n- a list\n<!-- fm:step id=9.1b kind=wait -->\n",
        ));
        assert!(
            rules(&t)
                .iter()
                .any(|r| r == "grammar.misplaced-step" || r == "grammar.not-block-level"),
            "{:?}",
            rules(&t)
        );
    }

    /// DR-008: the first `text` fence after the fm:step; `markdown` fences are never prompts.
    #[test]
    fn prompt_binding() {
        let t = parse(&blueprint());
        let p = t.step("2.3").unwrap().prompt.as_ref().unwrap().joined();
        assert!(p.starts_with(
            "ultrathink. For each v1 feature, write a specification a senior engineer"
        ));
        assert!(p.ends_with("-> council/r6-feature-specs.md\n"));
        assert!(
            t.step("5.1")
                .unwrap()
                .prompt
                .as_ref()
                .unwrap()
                .joined()
                .starts_with("Read docs/handoff/stage-04.md")
        );
        assert_eq!(
            t.step("5.1b").unwrap().prompt.as_ref().unwrap().joined(),
            "Use the test-runner agent to run the full verification suite and fix\neverything until it reports ALL GREEN.\n"
        );
        assert!(t.step("council-setup").unwrap().prompt.is_none());
        assert_eq!(t.council_blocks.len(), 6);
        assert_eq!(t.council_blocks[0].name, "market-analyst");
        for k in ["1.3b", "2.4b", "1.4b"] {
            assert!(t.step(k).unwrap().prompt.is_none(), "{k}");
        }
        let t = parse(&mini(
            "## Stage 9: A\n\n### Prompt 9.1: x\n\n> **RUN WITH** · chat\n<!-- fm:step id=9.1 surface=chat start=opus@adaptive -->\n\n```markdown\nnot a prompt\n```\n",
        ));
        assert!(rules(&t).contains(&"grammar.missing-prompt".to_owned()));
    }

    /// DR-008 / DR-032: a prompt is a run of `text` fences separated only by fm:if/fm:end blocks.
    #[test]
    fn prompt_parts_joined() {
        let t = parse(&blueprint());
        let p22 = t.step("2.2").unwrap().prompt.as_ref().unwrap();
        assert_eq!(p22.parts.len(), 5);
        assert_eq!(p22.parts.iter().filter(|p| p.cond.is_some()).count(), 2);
        assert_eq!(p22.parts[1].cond.as_ref().unwrap().0, "commercial");
        // Joined, the parts are exactly v2.1's prompt text.
        let v21 = blueprint_v21();
        let start = v21
            .find("You are the chair of the product design council")
            .unwrap();
        let end = start + v21[start..].find("```").unwrap();
        assert_eq!(p22.joined(), v21[start..end]);
        let p32 = t.step("3.2").unwrap().prompt.as_ref().unwrap();
        assert_eq!(p32.parts.len(), 3);
        assert_eq!(
            p32.parts[1].cond.as_ref().unwrap().0,
            "commercial|others_data|multi_user"
        );
        assert!(p32.parts[1].text.starts_with(
            "3. Legal and compliance for selling to [TARGET_USERS] in [jurisdictions]:"
        ));
        let start = v21
            .find("Define the requirements we must design for")
            .unwrap();
        let end = start + v21[start..].find("```").unwrap();
        assert_eq!(p32.joined(), v21[start..end]);
    }

    /// DR-011: gate instances for the stages with a `### Stage N gate` heading (5–16 in v2.2,
    /// domain-rules §15 gap 17), never for 0–4 or 18.
    #[test]
    fn gate_instances_stages_5_to_17() {
        let t = parse(&blueprint());
        let stages: Vec<u8> = t
            .steps
            .iter()
            .filter(|s| s.key.as_str().ends_with(":gate.A"))
            .map(|s| s.stage)
            .collect();
        assert_eq!(stages, (5..=16).collect::<Vec<u8>>());
        for n in 5..=16u8 {
            let keys: Vec<String> = t
                .stage_steps(n)
                .filter(|s| s.template_id.is_some())
                .map(|s| s.key.to_string())
                .collect();
            assert_eq!(
                keys,
                [
                    format!("{n}:gate.A"),
                    format!("{n}:gate.B"),
                    format!("{n}:gate.C")
                ]
            );
        }
        // After the stage's last prompt step.
        let s5: Vec<&str> = t.stage_steps(5).map(|s| s.key.as_str()).collect();
        assert_eq!(&s5[s5.len() - 3..], ["5:gate.A", "5:gate.B", "5:gate.C"]);
        assert_eq!(s5[s5.len() - 4], "5.5");
        let g = t.step("5:gate.A").unwrap();
        assert_eq!(
            (g.surface, g.cwd, g.kind),
            (Some(Surface::Cli), Some(CwdKind::Project), StepKind::Prompt)
        );
        assert_eq!(t.step("5:gate.C").unwrap().kind, StepKind::Followup);
        assert!(t.step("4:gate.A").is_none() && t.step("18:gate.A").is_none());
    }

    /// DR-012: F.2 instances for 0–4, after the packet-producing step, continuing its chain.
    #[test]
    fn f2_instances_stages_0_to_4() {
        let t = parse(&blueprint());
        let f2: Vec<u8> = t
            .steps
            .iter()
            .filter(|s| s.template_id.as_deref() == Some("F.2"))
            .map(|s| s.stage)
            .collect();
        assert_eq!(f2, [0, 1, 2, 3, 4]);
        let s = t.step("2:F.2").unwrap();
        assert_eq!(s.chain.as_ref().unwrap().as_str(), "2.2");
        assert_eq!(
            (s.surface, s.cwd, s.kind),
            (Some(Surface::Cli), Some(CwdKind::Spec), StepKind::Followup)
        );
        assert_eq!(s.start.unwrap().to_string(), "fable@high/default");
        assert_eq!(s.appends.as_deref(), Some("docs/lessons.md"));
        let s1 = t.step("1:F.2").unwrap();
        assert_eq!(
            (s1.surface, s1.chain.as_ref().unwrap().as_str()),
            (Some(Surface::Chat), "1.1")
        );
        assert_eq!(
            t.step("4:F.2").unwrap().chain.as_ref().unwrap().as_str(),
            "4.8"
        );
        assert!(t.step("5:F.2").is_none());
    }

    /// DR-017: packet formats come from inside prompt fences.
    #[test]
    fn packet_formats_stages_0_to_4_and_gate() {
        let t = parse(&blueprint());
        let keys = |n: u8| {
            t.packet_format(n)
                .unwrap()
                .keys
                .iter()
                .map(|k| k.key.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            keys(0),
            [
                "PROJECT",
                "APPROVED BY",
                "IDEA",
                "FOR",
                "PROBLEM",
                "EVIDENCE",
                "INCUMBENTS",
                "PRICE POINT",
                "MY EDGE",
                "TOP RISKS",
                "VALIDATION EXPERIMENT",
                "OPEN ITEMS",
                "NEXT"
            ]
        );
        assert!(keys(2).contains(&"NFRs".to_owned()) && keys(2).last().unwrap() == "FILES");
        let s4 = t.packet_format(4).unwrap();
        assert!(!s4.keys.iter().any(|k| k.key == "NEXT"));
        let repo = s4
            .keys
            .iter()
            .find(|k| k.key == "REPO FILES (create exactly these)")
            .unwrap();
        assert!(
            repo.placeholder.contains("docs/MASTER-SPEC.md")
                && repo.placeholder.contains("docs/lessons.md")
        );
        assert_eq!(
            keys(1).iter().filter(|k| k.starts_with("WITH (")).count(),
            2
        );
        // The generic Gate B format serves 5–17; it ends before the blank line.
        let g = t.packet_format(7).unwrap();
        assert_eq!(g.stage, None);
        assert_eq!(g.header, "=== HANDOFF PACKET · Stage [N] → Stage [N+1] ===");
        assert_eq!(g.keys.last().unwrap().key, "INPUTS FOR NEXT STAGE");
        assert_eq!(
            g.keys
                .iter()
                .find(|k| k.key == "APPROVED BY")
                .unwrap()
                .placeholder,
            "<blank>"
        );
        // The relay example (a `handoff` fence outside any stage) is ignored: 6 formats.
        assert_eq!(t.packet_formats.len(), 6);
        assert_eq!(
            t.packet_format(1).unwrap().header,
            "=== HANDOFF PACKET · Stage 1 → Stage 2 ==="
        );
    }
}
