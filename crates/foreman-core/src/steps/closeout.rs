//! The P1 close-out checklist (F-015 P1; MASTER-SPEC §8, DR-070; AC-F015-01). Phase 1 doesn't
//! automate gates, lessons or signing: once a stage's packet is valid, the stage shows a `manual`
//! checklist step with the close-out sequence for its stage type, and Billy works through it by
//! hand. P2 replaces it with the gate state machine (`GateView.closeout`).

use serde::{Deserialize, Serialize};

/// The stage types of MASTER-SPEC §8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StageType {
    /// Chat stages 0, 1, 3, 4.
    Chat,
    /// Stage 2, the CLI council.
    Council,
    /// Stages 5–17 (CLI).
    Cli,
}

/// The stage type of a stage (`None` outside 0–17).
#[must_use]
pub fn stage_type(stage: u8) -> Option<StageType> {
    match stage {
        0 | 1 | 3 | 4 => Some(StageType::Chat),
        2 => Some(StageType::Council),
        5..=17 => Some(StageType::Cli),
        _ => None,
    }
}

/// The close-out sequence of a stage type, in order (MASTER-SPEC §8), worded for doing it by hand
/// in P1: the F.2 paste-back offers no write in P1 (AC-F012-14) and a hand signature counts only
/// after Billy confirms it in Foreman's native dialog (DR-135).
#[must_use]
pub fn sequence(stage: u8) -> Vec<String> {
    let nn = format!("{stage:02}");
    let sign = format!(
        "Sign: write APPROVED BY: <your name> on <date> in docs/handoff/stage-{nn}.md, confirm the signature in Foreman, then commit the stage"
    );
    match stage_type(stage) {
        None => Vec::new(),
        Some(StageType::Chat) => vec![
            format!("Packet pasted back and valid (docs/handoff/stage-{nn}.md)"),
            "Attest the inspection items: a note for each, or why it doesn't apply".to_owned(),
            format!(
                "F.2 lessons in the same chat; add the entries to docs/lessons.md yourself (Stage {stage})"
            ),
            sign,
        ],
        Some(StageType::Council) => vec![
            "2.6 writes the packet (docs/handoff/stage-02.md) and it's valid".to_owned(),
            "Attest the inspection items: a note for each, or why it doesn't apply".to_owned(),
            "F.2 as a followup in the 2.2 chain, writing to docs/lessons.md".to_owned(),
            sign,
        ],
        Some(StageType::Cli) => vec![
            format!("Gate A (interactive): run {stage}:gate.A and confirm the results"),
            format!("Gate B writes the packet (docs/handoff/stage-{nn}.md) and it's valid"),
            format!("Gate C lessons: run {stage}:gate.C, writing to docs/lessons.md"),
            sign,
        ],
    }
}

/// AC-F015-01: the checklist a stage shows in P1: its sequence once the packet is valid, until
/// the stage is signed; empty otherwise.
#[must_use]
pub fn checklist(stage: u8, packet_valid: bool, signed: bool) -> Vec<String> {
    if packet_valid && !signed {
        sequence(stage)
    } else {
        Vec::new()
    }
}
