//! Prompt hygiene, paste stripping and visible escapes (SR-04, SR-26, SR-28; DR-094, DR-095).

use serde::{Deserialize, Serialize};

/// A launch blocker raised by prompt hygiene (`docs/contracts/ipc.md` `Blocker`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Blocker {
    /// After trimming leading whitespace the prompt starts with `-`, `/` or `!` (SR-04).
    #[error(
        "the prompt starts with `{first}`, which Claude Code would read as an option or command"
    )]
    PromptOptionLike {
        /// The character.
        first: char,
    },
    /// A C0 control (other than TAB, LF, CR), DEL or a C1 control (SR-28).
    #[error("control character U+{codepoint:04X} at offset {offset}")]
    PromptControlChar {
        /// Byte offset.
        offset: u32,
        /// The code point.
        codepoint: u32,
    },
}

fn is_blocked_control(c: char) -> bool {
    let cp = u32::from(c);
    (cp < 0x20 && !matches!(c, '\t' | '\n' | '\r')) || cp == 0x7f || (0x80..=0x9f).contains(&cp)
}

/// DR-094: checks a resolved prompt before it becomes the final argv element.
///
/// # Errors
/// The first blocker, with the offset of the offending character.
pub fn check_prompt(p: &str) -> Result<(), Blocker> {
    if let Some((i, c)) = p.char_indices().find(|(_, c)| is_blocked_control(*c)) {
        return Err(Blocker::PromptControlChar {
            offset: u32::try_from(i).unwrap_or(u32::MAX),
            codepoint: u32::from(c),
        });
    }
    match p.trim_start().chars().next() {
        Some(c @ ('-' | '/' | '!')) => Err(Blocker::PromptOptionLike { first: c }),
        _ => Ok(()),
    }
}

/// What `for_paste` removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SanitizeReport {
    /// Characters removed.
    pub removed: usize,
}

impl SanitizeReport {
    /// "removed N control characters" (shown visibly).
    #[must_use]
    pub fn message(&self) -> Option<String> {
        (self.removed > 0).then(|| {
            format!(
                "removed {} control character{}",
                self.removed,
                if self.removed == 1 { "" } else { "s" }
            )
        })
    }
}

/// DR-095: text for terminals loses ESC, C0 (except TAB, LF, CR), DEL and C1, with a count.
#[must_use]
pub fn for_paste(text: &str) -> (String, SanitizeReport) {
    let mut out = String::with_capacity(text.len());
    let mut removed = 0usize;
    for c in text.chars() {
        if is_blocked_control(c) {
            removed += 1;
        } else {
            out.push(c);
        }
    }
    (out, SanitizeReport { removed })
}

/// A character shown as a visible escape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    /// Byte offset.
    pub start: usize,
    /// Byte end.
    pub end: usize,
    /// Code point.
    pub codepoint: u32,
    /// `U+202E RIGHT-TO-LEFT OVERRIDE`-style label (kind only).
    pub label: String,
}

/// Default_Ignorable_Code_Point (Unicode DerivedCoreProperties), as ranges.
pub const DEFAULT_IGNORABLE: [(u32, u32); 17] = [
    (0x00AD, 0x00AD),
    (0x034F, 0x034F),
    (0x061C, 0x061C),
    (0x115F, 0x1160),
    (0x17B4, 0x17B5),
    (0x180B, 0x180F),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x206F),
    (0x3164, 0x3164),
    (0xFE00, 0xFE0F),
    (0xFEFF, 0xFEFF),
    (0xFFA0, 0xFFA0),
    (0xFFF0, 0xFFF8),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0000, 0xE0FFF),
];

fn label(cp: u32) -> Option<&'static str> {
    Some(match cp {
        0x202A..=0x202E | 0x2066..=0x2069 | 0x200E | 0x200F | 0x061C => "bidi control",
        0x200B..=0x200D | 0x2060 | 0xFEFF => "zero-width",
        0xE0000..=0xE007F => "tag character",
        0x00..=0x08 | 0x0B | 0x0C | 0x0E..=0x1F | 0x7F | 0x80..=0x9F => "control",
        _ if DEFAULT_IGNORABLE
            .iter()
            .any(|(a, b)| (*a..=*b).contains(&cp)) =>
        {
            "invisible"
        }
        _ => return None,
    })
}

/// SR-26: every bidi control, zero-width, tag character, other Default_Ignorable code point and
/// control (except TAB, LF, CR) as a span to show visibly.
#[must_use]
pub fn visible_escapes(text: &str) -> Vec<Span> {
    text.char_indices()
        .filter_map(|(i, c)| {
            let cp = u32::from(c);
            label(cp).map(|l| Span {
                start: i,
                end: i + c.len_utf8(),
                codepoint: cp,
                label: format!("U+{cp:04X} {l}"),
            })
        })
        .collect()
}
