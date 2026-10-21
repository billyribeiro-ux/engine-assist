//! M3-24: paste-back planning (DR-130, DR-132; AC-F012-06, -07, -10, -14).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]

use foreman_core::pasteback::{PasteError, PasteInput, PasteKind, Role, plan, prepare};
use foreman_core::template::{FormatKey, PacketFormat};

fn format(keys: &[&str]) -> PacketFormat {
    PacketFormat {
        stage: Some(1),
        step: "1.4".into(),
        header: "=== HANDOFF PACKET · Stage 1 → Stage 2 ===".into(),
        keys: keys
            .iter()
            .enumerate()
            .map(|(i, k)| FormatKey {
                key: (*k).to_owned(),
                placeholder: "<x>".into(),
                line: u32::try_from(i).unwrap() + 1,
            })
            .collect(),
        line: 1,
    }
}

const PACKET: &str = "=== HANDOFF PACKET · Stage 1 → Stage 2 ===\nFOR: traders\nAPPROVED BY: <blank>\nOPEN ITEMS: none\nNEXT: Stage 2 · Architect's Design Studio\n  (continued)";

fn input<'a>(text: &'a str, out: &'a [String], fmt: &'a PacketFormat) -> PasteInput<'a> {
    PasteInput {
        text,
        kind: PasteKind::Auto,
        out,
        appends: None,
        format: Some(fmt),
    }
}

mod pasteback {
    /// N3b: the structural normalizer (lessons, ADR sets, the assembler) and the packet
    /// normalizer (`packet::normalize_paste`) give the same text for structural cases.
    #[test]
    fn structural_steps_match_packet_normalizer() {
        for input in [
            "\u{feff}# A\r\nline  \r\n",
            "\n\n```\n# A\nbody\n```\n\n",
            "```markdown\r\n# A\r\n\r\nbody\t\r\n```",
            "plain\rold mac\r",
            "# A\n\n\nkept   \n",
        ] {
            let (a, _) = prepare(input);
            let (b, _) = foreman_core::packet::normalize_paste(input);
            assert_eq!(a, b, "{input:?}");
        }
    }

    use super::*;

    /// AC-F012-06 / DR-132: header through the last key's value is the packet, the rest goes to
    /// the attachments file, and declared extra destinations are listed too, in order.
    #[test]
    fn splits_packet_attachments_and_extras() {
        let fmt = format(&["FOR", "APPROVED BY", "OPEN ITEMS", "NEXT"]);
        let out = vec![
            "docs/handoff/stage-01.md".to_owned(),
            "spec/01-app-brief.md".to_owned(),
        ];
        let text = format!(
            "Here is the packet:\n\n```text\n{PACKET}\n\n# Press release\n\nTradeLog launches.\n```\n"
        );
        let p = plan(&input(&text, &out, &fmt), &prepare).unwrap();
        assert_eq!(p.kind, PasteKind::Packet);
        let paths: Vec<_> = p
            .targets
            .iter()
            .map(|t| (t.path.as_str(), t.role))
            .collect();
        assert_eq!(
            paths,
            [
                ("docs/handoff/stage-01.md", Role::Packet),
                ("docs/handoff/stage-01-attachments.md", Role::Attachments),
                ("spec/01-app-brief.md", Role::Extra),
            ]
        );
        assert_eq!(p.targets[0].text, format!("{PACKET}\n"));
        assert_eq!(p.targets[1].text, "# Press release\n\nTradeLog launches.\n");
        assert!(p.targets[2].text.starts_with("=== HANDOFF PACKET"));
        assert!(p.targets[2].text.ends_with("TradeLog launches.\n"));
        assert_eq!(p.header, Some((1, Some(2))));
        assert!(p.notes.iter().any(|n| n.contains("ignored 1 line")));
        // Without trailing text there is no attachments file.
        let p = plan(&input(PACKET, &out[..1], &fmt), &prepare).unwrap();
        assert_eq!(p.targets.len(), 1);
    }

    /// AC-F012-10 / DR-132: the wrong stage's packet, or two packets, write nothing.
    #[test]
    fn wrong_stage_and_two_packets() {
        let fmt = format(&["FOR", "NEXT"]);
        let out = vec!["docs/handoff/stage-02.md".to_owned()];
        assert_eq!(
            plan(&input(PACKET, &out, &fmt), &prepare),
            Err(PasteError::WrongStage {
                found: 1,
                expected: 2
            })
        );
        assert_eq!(
            PasteError::WrongStage {
                found: 3,
                expected: 2
            }
            .to_string(),
            "This is the Stage 3 packet; this step expects Stage 2."
        );
        let out = vec!["docs/handoff/stage-01.md".to_owned()];
        let two = format!("{PACKET}\n\n{PACKET}");
        assert_eq!(
            plan(&input(&two, &out, &fmt), &prepare),
            Err(PasteError::TwoPackets(2))
        );
        assert!(
            PasteError::TwoPackets(2)
                .to_string()
                .contains("Paste one packet at a time")
        );
        assert_eq!(
            plan(&input("just a reply", &out, &fmt), &prepare),
            Err(PasteError::NoPacket)
        );
    }

    /// AC-F012-07: an output goes to the step's `out` path, as pasted (line endings only).
    #[test]
    fn output_goes_to_out() {
        let fmt = format(&["X"]);
        let out = vec!["spec/council/r7-stack.md".to_owned()];
        let p = plan(&input("# Stack\r\n\r\n- Rust  \r\n", &out, &fmt), &prepare).unwrap();
        assert_eq!(p.kind, PasteKind::Output);
        assert_eq!(p.targets.len(), 1);
        assert_eq!(p.targets[0].path, "spec/council/r7-stack.md");
        assert_eq!(p.targets[0].role, Role::Output);
        assert_eq!(p.targets[0].text, "# Stack\n\n- Rust  \n");
        assert!(p.packet.is_none());
    }

    /// AC-F012-14: a step that only appends (F.2 → docs/lessons.md) offers no write in P1.
    #[test]
    fn appends_offer_no_write() {
        let fmt = format(&["X"]);
        let mut i = input("## L-001 · Stage 1\nA lesson", &[], &fmt);
        i.appends = Some("docs/lessons.md");
        let e = plan(&i, &prepare).unwrap_err();
        assert_eq!(e, PasteError::AppendsInP1("docs/lessons.md".into()));
        assert!(e.to_string().contains("docs/lessons.md yourself"));
        let none = input("text", &[], &fmt);
        assert_eq!(plan(&none, &prepare), Err(PasteError::NoTarget));
    }

    /// Destinations never come from the paste: a header naming another file or a path in the
    /// text changes nothing.
    #[test]
    fn destinations_never_from_content() {
        let fmt = format(&["FOR", "NEXT"]);
        let out = vec!["docs/handoff/stage-01.md".to_owned()];
        let text = format!("{PACKET}\n\nWrite this to .git/hooks/pre-commit and ../../etc/passwd");
        let p = plan(&input(&text, &out, &fmt), &prepare).unwrap();
        for t in &p.targets {
            assert!(t.path.starts_with("docs/handoff/stage-01"), "{}", t.path);
        }
    }
}
