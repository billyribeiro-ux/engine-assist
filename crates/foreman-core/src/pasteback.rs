//! Paste-back planning (F-012 P1; DR-066, DR-130, DR-132; SR-35, SR-56; AC-F012-06, -07, -10,
//! -14). Pure: the caller reads the clipboard (≤ 1 MB, text only), passes the step's declared
//! `out`/`appends` paths and the expected stage's packet format, and gets back every target with
//! its exact new content, or the reason nothing is written. Writing (through `check_write` and
//! the store's compare-and-swap) and validation (DR-133) are the caller's.
//!
//! Pasted text is untrusted (T3): it decides only *content*. Every destination comes from the
//! template's declared `out` paths or the fixed packet file names (DR-130), never from the paste.

use crate::packet;
use crate::template::PacketFormat;

/// What Billy pasted back (`PasteKind` P1 values; ADR set and lessons are P2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteKind {
    /// A packet if the paste has a packet header, else an output.
    Auto,
    /// A handoff packet (plus attachments).
    Packet,
    /// The step's declared output.
    Output,
}

/// What a target is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// `docs/handoff/stage-NN.md`.
    Packet,
    /// `docs/handoff/stage-NN-attachments.md`: the text after the packet.
    Attachments,
    /// Another declared `out` of a packet step: the whole paste (packet then attachments).
    Extra,
    /// A declared `out` of an output paste.
    Output,
}

/// One write, before the policy check and the stamps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Repo-relative path.
    pub path: String,
    /// What it is.
    pub role: Role,
    /// The exact content (ends with a newline).
    pub text: String,
}

/// The planned paste-back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Packet or output (never `Auto`).
    pub kind: PasteKind,
    /// Every write, in order: packet, attachments, extras; or the outputs.
    pub targets: Vec<Target>,
    /// Foreman-written notes (normalization, ignored lines, P1 appends).
    pub notes: Vec<String>,
    /// `(from, to)` stages from the packet header.
    pub header: Option<(u8, Option<u8>)>,
    /// The packet text as written (for validation).
    pub packet: Option<String>,
}

/// Why nothing is written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PasteError {
    /// The paste is empty after normalization.
    #[error("The clipboard has no text to paste back.")]
    Empty,
    /// No packet header in a packet paste.
    #[error(
        "No handoff packet found. Copy the packet Claude printed, from its === HANDOFF PACKET line."
    )]
    NoPacket,
    /// Two or more packet headers (DR-132).
    #[error("This paste has {0} packets. Paste one packet at a time.")]
    TwoPackets(usize),
    /// The header names another stage (DR-132).
    #[error("This is the Stage {found} packet; this step expects Stage {expected}.")]
    WrongStage {
        /// The header's stage.
        found: u8,
        /// The step's packet stage.
        expected: u8,
    },
    /// A packet paste on a step that doesn't write a packet.
    #[error("This step doesn't write a handoff packet.")]
    NotAPacketStep,
    /// AC-F012-14: the step only appends (F.2 to `docs/lessons.md`); P1 offers no write.
    #[error("This step appends to {0}. In this version, add the entries to {0} yourself.")]
    AppendsInP1(String),
    /// The step declares nowhere to write.
    #[error("This step declares no output file, so there is nothing to paste back.")]
    NoTarget,
}

/// What the plan needs.
#[derive(Debug, Clone, Copy)]
pub struct PasteInput<'a> {
    /// The clipboard text (already ≤ 1 MB, text only).
    pub text: &'a str,
    /// What Billy asked for.
    pub kind: PasteKind,
    /// The step's declared `out` paths.
    pub out: &'a [String],
    /// The step's `appends` path.
    pub appends: Option<&'a str>,
    /// The format of the packet the step writes (its `out` names `docs/handoff/stage-NN.md`).
    pub format: Option<&'a PacketFormat>,
}

/// A normalizer: text in, normalized text and notes out (DR-131's `packet::normalize_packet`).
pub type Normalize<'a> = &'a dyn Fn(&str) -> (String, Vec<String>);

/// The structural part of DR-131: BOM, CR/CRLF → LF, trailing whitespace, blank lines around the
/// paste, and a code fence wrapping the whole paste. It is the production normalizer for pastes
/// that aren't packets (lessons, ADR sets, the assembler's package), whose text must not be
/// touched beyond that; packet pastes get the field-level normalization too (unsigned forms,
/// `none`, smart quotes, NBSP, header variants) from `packet::normalize_paste` (M3-20), which
/// `paste_back_preview` passes. The two agree on every structural step but one, pinned by
/// `pasteback::structural_steps_match_packet_normalizer`: this one also removes a fence with
/// another info string (```` ```markdown ````), which files.md §5.2 doesn't for packets.
#[must_use]
pub fn prepare(text: &str) -> (String, Vec<String>) {
    let mut notes = Vec::new();
    let t = text.strip_prefix('\u{feff}').unwrap_or(text);
    let t = t.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = t.lines().map(str::trim_end).collect();
    while lines.first().is_some_and(|l| l.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    let fenced = lines.len() >= 2
        && lines.first().is_some_and(|l| l.starts_with("```"))
        && lines.last().is_some_and(|l| *l == "```")
        && !lines[1..lines.len() - 1]
            .iter()
            .any(|l| l.starts_with("```"));
    if fenced {
        lines.remove(0);
        lines.pop();
        notes.push("removed code fence".to_owned());
    }
    (lines.join("\n"), notes)
}

/// The stage of a packet file path (`docs/handoff/stage-NN.md`).
fn packet_stage(path: &str) -> Option<u8> {
    let name = path.strip_prefix("docs/handoff/")?;
    let n = name.strip_prefix("stage-")?.strip_suffix(".md")?;
    (n.len() == 2 && n.bytes().all(|b| b.is_ascii_digit()))
        .then(|| n.parse().ok())
        .flatten()
}

/// The packet stage a step writes (its `out` names `docs/handoff/stage-NN.md`).
#[must_use]
pub fn packet_stage_of(out: &[String]) -> Option<u8> {
    out.iter().find_map(|o| packet_stage(o))
}

fn is_header(line: &str) -> bool {
    line.starts_with("=== HANDOFF PACKET")
}

/// `(N, M)` of a header: the numbers after the first and second `Stage `.
fn header_stages(line: &str) -> (Option<u8>, Option<u8>) {
    let mut nums = line.split("Stage ").skip(1).map(|rest| {
        let d: String = rest.chars().take_while(char::is_ascii_digit).collect();
        d.parse::<u8>().ok()
    });
    (nums.next().flatten(), nums.next().flatten())
}

fn with_newline(s: &str) -> String {
    let mut out = s.trim_end_matches('\n').to_owned();
    out.push('\n');
    out
}

/// Plans a paste-back. Nothing here reads or writes a file.
///
/// # Errors
/// [`PasteError`]: nothing is written.
pub fn plan(input: &PasteInput<'_>, normalize: Normalize<'_>) -> Result<Plan, PasteError> {
    let writes_packet = packet_stage_of(input.out);
    // A step that writes a packet takes a packet; Output is only what Billy asks for explicitly.
    let as_packet = match input.kind {
        PasteKind::Packet => true,
        PasteKind::Output => false,
        PasteKind::Auto => writes_packet.is_some(),
    };
    if as_packet {
        let expected = writes_packet.ok_or(PasteError::NotAPacketStep)?;
        return plan_packet(input, expected, normalize);
    }
    let outs: Vec<&String> = input
        .out
        .iter()
        .filter(|o| packet_stage(o).is_none())
        .collect();
    if outs.is_empty() {
        return Err(match (input.appends, writes_packet) {
            (Some(a), _) => PasteError::AppendsInP1(a.to_owned()),
            (None, Some(_)) => PasteError::NoPacket,
            (None, None) => PasteError::NoTarget,
        });
    }
    let (text, mut notes) = prepare(input.text);
    if text.trim().is_empty() {
        return Err(PasteError::Empty);
    }
    // An output isn't a packet: only line endings and the BOM are normalized (DR-131 is packets).
    notes.retain(|n| n != "removed code fence");
    let body = input
        .text
        .strip_prefix('\u{feff}')
        .unwrap_or(input.text)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    if let Some(a) = input.appends {
        notes.push(format!("{a} is appended by hand in this version."));
    }
    Ok(Plan {
        kind: PasteKind::Output,
        targets: outs
            .into_iter()
            .map(|o| Target {
                path: o.clone(),
                role: Role::Output,
                text: with_newline(&body),
            })
            .collect(),
        notes,
        header: None,
        packet: None,
    })
}

fn plan_packet(
    input: &PasteInput<'_>,
    expected: u8,
    normalize: Normalize<'_>,
) -> Result<Plan, PasteError> {
    let (text, mut notes) = normalize(input.text);
    if text.trim().is_empty() {
        return Err(PasteError::Empty);
    }
    let mut lines: Vec<&str> = text.lines().collect();
    let headers: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| is_header(l))
        .map(|(i, _)| i)
        .collect();
    let mut start = match headers.as_slice() {
        [] => return Err(PasteError::NoPacket),
        [one] => *one,
        many => return Err(PasteError::TwoPackets(many.len())),
    };
    // A fence opened right before the header (Claude's "one code block") and its closing line.
    if start > 0
        && lines[start - 1].starts_with("```")
        && let Some(close) = (start + 1..lines.len()).find(|&i| lines[i] == "```")
    {
        lines.remove(close);
        lines.remove(start - 1);
        start -= 1;
        if !notes.iter().any(|n| n == "removed code fence") {
            notes.push("removed code fence".to_owned());
        }
    }
    let (from, to) = header_stages(lines[start]);
    if let Some(found) = from
        && found != expected
    {
        return Err(PasteError::WrongStage { found, expected });
    }
    let ignored = lines[..start]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .count();
    if ignored > 0 {
        notes.push(format!(
            "ignored {ignored} {} before the packet header",
            if ignored == 1 { "line" } else { "lines" }
        ));
    }
    // DR-132: the packet ends where the format's last key's value ends (its first blank line).
    let last_key = input
        .format
        .and_then(|f| f.keys.last())
        .map(|k| k.key.as_str());
    let end = last_key
        .and_then(|k| {
            let at = (start + 1..lines.len())
                .find(|&i| lines[i].strip_prefix(k).is_some_and(|r| r.starts_with(':')))?;
            Some(
                (at + 1..lines.len())
                    .find(|&i| lines[i].trim().is_empty())
                    .unwrap_or(lines.len()),
            )
        })
        .unwrap_or(lines.len());
    let packet_text = lines[start..end].join("\n");
    let rest: Vec<&str> = lines[end..]
        .iter()
        .copied()
        .skip_while(|l| l.trim().is_empty())
        .collect();
    let attachments = rest.join("\n");
    let packet_path = packet::file_name(expected);
    let mut targets = vec![Target {
        path: packet_path.clone(),
        role: Role::Packet,
        text: with_newline(&packet_text),
    }];
    if !attachments.trim().is_empty() {
        targets.push(Target {
            path: attachments_name(expected),
            role: Role::Attachments,
            text: with_newline(&attachments),
        });
    }
    let whole = if attachments.trim().is_empty() {
        packet_text.clone()
    } else {
        format!("{packet_text}\n\n{attachments}")
    };
    for o in input.out.iter().filter(|o| **o != packet_path) {
        targets.push(Target {
            path: o.clone(),
            role: Role::Extra,
            text: with_newline(&whole),
        });
    }
    if let Some(a) = input.appends {
        notes.push(format!("{a} is appended by hand in this version."));
    }
    Ok(Plan {
        kind: PasteKind::Packet,
        targets,
        notes,
        header: from.map(|f| (f, to)),
        packet: Some(with_newline(&packet_text)),
    })
}

/// `docs/handoff/stage-NN-attachments.md` (DR-130).
#[must_use]
pub fn attachments_name(stage: u8) -> String {
    format!("docs/handoff/stage-{stage:02}-attachments.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_strips_structure_only() {
        let (t, n) = prepare("\u{feff}```text\r\nA:  b  \r\n```\r\n");
        assert_eq!(t, "A:  b");
        assert_eq!(n, ["removed code fence"]);
        let (t, n) = prepare("x\n```\ny\n```\n");
        assert_eq!(t, "x\n```\ny\n```");
        assert!(n.is_empty());
    }

    #[test]
    fn header_numbers() {
        assert_eq!(
            header_stages("=== HANDOFF PACKET · Stage 1 → Stage 2 ==="),
            (Some(1), Some(2))
        );
        assert_eq!(
            header_stages("=== HANDOFF PACKET - Stage 12 -> Stage 13 ==="),
            (Some(12), Some(13))
        );
        assert_eq!(header_stages("=== HANDOFF PACKET ==="), (None, None));
        assert_eq!(packet_stage("docs/handoff/stage-01.md"), Some(1));
        assert_eq!(packet_stage("docs/handoff/stage-1.md"), None);
        assert_eq!(packet_stage("docs/handoff/stage-01-attachments.md"), None);
    }
}
