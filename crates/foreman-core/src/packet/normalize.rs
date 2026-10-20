//! Packet normalization (DR-131, `docs/contracts/files.md` §5.2). Applied to pasted text before
//! splitting and validation, and offered for files Claude wrote (shown first, written only after
//! Billy confirms, AC-F013-08). Each change that carries a note in §5.2 is recorded; nothing outside
//! §5.2's table is altered. Bidi and zero-width characters are kept (they are shown as visible
//! escapes and flagged elsewhere, SR-26). Pure; never panics.

use serde::{Deserialize, Serialize};

/// What a recorded change was (§5.2's "Note recorded" column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteKind {
    /// A wrapping code fence was removed.
    CodeFence,
    /// NBSP or narrow NBSP became a space.
    NonBreakingSpaces,
    /// Smart quotes became straight quotes.
    SmartQuotes,
    /// A header variant became the canonical header.
    Header,
    /// An unsigned `APPROVED BY:` variant became `<blank>`.
    Unsigned,
    /// `OPEN ITEMS:` `None`, `NONE`, `none.` … became `none`.
    OpenItemsNone,
}

impl NoteKind {
    /// The note's words, exactly as §5.2 gives them.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::CodeFence => "removed code fence",
            Self::NonBreakingSpaces => "fixed non-breaking spaces",
            Self::SmartQuotes => "fixed smart quotes",
            Self::Header => "normalized header",
            Self::Unsigned => "unsigned",
            Self::OpenItemsNone => "OPEN ITEMS normalized to none",
        }
    }
}

/// One recorded change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    /// What changed.
    pub kind: NoteKind,
    /// The words (`NoteKind::message`).
    pub message: String,
    /// The 1-based lines of the **normalized** text it touched (empty for the fence).
    pub lines: Vec<u32>,
}

/// Normalized text and its notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Normalized {
    /// The normalized text.
    pub text: String,
    /// One note per kind of change, in §5.2's order.
    pub notes: Vec<Note>,
}

const FENCES: [&str; 2] = ["```", "```text"];

fn is_fence_open(l: &str) -> bool {
    FENCES.contains(&l.trim())
}

/// `=== HANDOFF PACKET · Stage N → Stage M ===` with `->` for `→`, `-`/`•`/`|` for `·`, or extra
/// spaces. Returns `(N, M)` for any such variant, including the canonical form.
#[must_use]
pub fn loose_header(line: &str) -> Option<(u8, u8)> {
    let inner = line.trim().strip_prefix("===")?.strip_suffix("===")?;
    let inner = inner.split_whitespace().collect::<Vec<_>>().join(" ");
    let rest = inner.strip_prefix("HANDOFF PACKET")?.trim_start();
    let rest = rest.strip_prefix(['·', '-', '•', '|'])?.trim_start();
    let rest = rest.strip_prefix("Stage")?.trim_start();
    let (n, rest) = digits(rest)?;
    let rest = rest.trim_start();
    let rest = rest
        .strip_prefix('→')
        .or_else(|| rest.strip_prefix("->"))?
        .trim_start();
    let rest = rest.strip_prefix("Stage")?.trim_start();
    let (m, rest) = digits(rest)?;
    rest.trim().is_empty().then_some((n, m))
}

fn digits(s: &str) -> Option<(u8, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if end == 0 || end > 3 {
        return None;
    }
    Some((s[..end].parse().ok()?, &s[end..]))
}

/// The canonical header line.
#[must_use]
pub fn canonical_header(n: u8, m: u8) -> String {
    format!("=== HANDOFF PACKET · Stage {n} → Stage {m} ===")
}

fn is_unsigned_variant(v: &str) -> bool {
    let l = v.trim().to_lowercase();
    l.is_empty()
        || l == "<blank>"
        || l == "[blank]"
        || (l.starts_with("<leave blank") && l.ends_with('>'))
        || (l.starts_with("<sign here") && l.ends_with('>'))
}

fn is_none_variant(v: &str) -> bool {
    matches!(v.trim().to_lowercase().as_str(), "none" | "none.")
}

/// True when the field starting at `i` has no continuation (its next non-blank line starts at
/// column 1, or there is none).
fn single_line(lines: &[String], i: usize) -> bool {
    lines
        .iter()
        .skip(i + 1)
        .find(|l| !l.trim().is_empty())
        .is_none_or(|l| !l.starts_with([' ', '\t']))
}

fn line_no(i: usize) -> u32 {
    u32::try_from(i + 1).unwrap_or(u32::MAX)
}

/// Removes a wrapping fence: it opens on the first non-blank line, or on the last non-blank line
/// before the header (a chat preamble may come first); the next fence line closes it. Blank lines
/// before a fence that opens the text go with it.
fn remove_fence(lines: &mut Vec<String>) -> bool {
    let first = lines.iter().position(|l| !l.trim().is_empty());
    let before_header = lines
        .iter()
        .position(|l| l.trim_start().starts_with("=== HANDOFF PACKET") || loose_header(l).is_some())
        .and_then(|h| lines[..h].iter().rposition(|l| !l.trim().is_empty()));
    let open = [first, before_header]
        .into_iter()
        .flatten()
        .find(|&i| is_fence_open(&lines[i]));
    if let Some(open) = open
        && let Some(close) = lines
            .iter()
            .skip(open + 1)
            .position(|l| l.trim() == "```")
            .map(|k| k + open + 1)
    {
        lines.remove(close);
        lines.remove(open);
        if Some(open) == first {
            lines.drain(..open);
        }
        return true;
    }
    false
}

/// Normalizes packet text (DR-131). Idempotent: normalizing the result changes nothing.
#[must_use]
pub fn normalize_packet(input: &str) -> Normalized {
    let text = input.strip_prefix('\u{feff}').unwrap_or(input);
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let ends_with_newline = text.ends_with('\n');
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut notes = Vec::new();

    if remove_fence(&mut lines) {
        notes.push((NoteKind::CodeFence, Vec::new()));
    }

    let mut nbsp = Vec::new();
    let mut quotes = Vec::new();
    for (i, l) in lines.iter_mut().enumerate() {
        if l.contains(['\u{a0}', '\u{202f}']) {
            *l = l.replace(['\u{a0}', '\u{202f}'], " ");
            nbsp.push(line_no(i));
        }
        if l.contains(['“', '”', '‘', '’']) {
            *l = l.replace(['“', '”'], "\"").replace(['‘', '’'], "'");
            quotes.push(line_no(i));
        }
        let trimmed = l.trim_end().len();
        l.truncate(trimmed);
    }

    let mut header = Vec::new();
    let mut unsigned = Vec::new();
    let mut open_items = Vec::new();
    for i in 0..lines.len() {
        let l = &lines[i];
        if l.trim_start().starts_with("===")
            && let Some((n, m)) = loose_header(l)
        {
            let canon = canonical_header(n, m);
            if *l != canon {
                lines[i] = canon;
                header.push(line_no(i));
            }
            continue;
        }
        if let Some(v) = l.strip_prefix("APPROVED BY:")
            && v.trim() != "<blank>"
            && is_unsigned_variant(v)
            && single_line(&lines, i)
        {
            "APPROVED BY: <blank>".clone_into(&mut lines[i]);
            unsigned.push(line_no(i));
            continue;
        }
        if let Some(v) = l.strip_prefix("OPEN ITEMS:")
            && v.trim() != "none"
            && is_none_variant(v)
            && single_line(&lines, i)
        {
            "OPEN ITEMS: none".clone_into(&mut lines[i]);
            open_items.push(line_no(i));
        }
    }
    for (k, v) in [
        (NoteKind::NonBreakingSpaces, nbsp),
        (NoteKind::SmartQuotes, quotes),
        (NoteKind::Header, header),
        (NoteKind::Unsigned, unsigned),
        (NoteKind::OpenItemsNone, open_items),
    ] {
        if !v.is_empty() {
            notes.push((k, v));
        }
    }
    let mut text = lines.join("\n");
    if ends_with_newline && !text.is_empty() {
        text.push('\n');
    }
    Normalized {
        text,
        notes: notes
            .into_iter()
            .map(|(kind, lines)| Note {
                kind,
                message: kind.message().to_owned(),
                lines,
            })
            .collect(),
    }
}

/// [`normalize_packet`] for a paste (DR-131, `pasteback::Normalize`): blank lines around the paste
/// are dropped too, and the notes are their words. The paste-back planner splits the result.
#[must_use]
pub fn normalize_paste(text: &str) -> (String, Vec<String>) {
    let n = normalize_packet(text);
    let trimmed = n.text.trim_matches('\n').to_owned();
    (trimmed, n.notes.into_iter().map(|x| x.message).collect())
}
