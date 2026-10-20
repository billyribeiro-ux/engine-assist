//! M3-25: the P1 close-out checklist step (MASTER-SPEC §8, DR-070, AC-F015-01).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]

use foreman_core::steps::closeout::{StageType, checklist, sequence, stage_type};

mod steps {
    use super::*;

    /// AC-F015-01 / DR-070: each stage type lists its own sequence, in order, ending with Sign;
    /// the checklist shows only while the packet is valid and the stage unsigned.
    #[test]
    fn closeout_sequences() {
        for n in [0, 1, 3, 4] {
            assert_eq!(stage_type(n), Some(StageType::Chat));
        }
        assert_eq!(stage_type(2), Some(StageType::Council));
        for n in 5..=17 {
            assert_eq!(stage_type(n), Some(StageType::Cli));
        }
        assert_eq!(stage_type(18), None);
        assert!(sequence(18).is_empty());

        let chat = sequence(1);
        assert_eq!(chat.len(), 4);
        assert!(chat[0].starts_with("Packet pasted back and valid"));
        assert!(chat[0].contains("docs/handoff/stage-01.md"));
        assert!(chat[1].starts_with("Attest the inspection items"));
        assert!(chat[2].starts_with("F.2 lessons in the same chat"));
        assert!(chat[3].starts_with("Sign"));

        let council = sequence(2);
        assert!(council[0].starts_with("2.6 writes the packet"));
        assert!(council[1].starts_with("Attest"));
        assert!(council[2].contains("followup in the 2.2 chain"));
        assert!(council[3].starts_with("Sign"));

        let cli = sequence(7);
        assert!(cli[0].starts_with("Gate A") && cli[0].contains("7:gate.A"));
        assert!(cli[1].starts_with("Gate B writes the packet") && cli[1].contains("stage-07.md"));
        assert!(cli[2].starts_with("Gate C lessons") && cli[2].contains("7:gate.C"));
        assert!(cli[3].starts_with("Sign") && cli[3].contains("stage-07.md"));

        // Shown only for a valid, unsigned packet.
        assert_eq!(checklist(1, true, false), chat);
        assert!(checklist(1, false, false).is_empty());
        assert!(checklist(1, true, true).is_empty());
    }
}
