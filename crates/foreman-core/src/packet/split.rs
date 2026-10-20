//! Splitting a pasted packet (DR-132, AC-F012-06, AC-F012-10). The input is **normalized** text
//! (`normalize_packet` first). The packet is the header line through the end of the stage
//! format's last key's value, which ends at the first blank line; everything after it is the
//! attachments. Two headers or another stage's header refuse the paste, so nothing is written.

use serde::{Deserialize, Serialize};

use super::normalize::loose_header;
use crate::template::PacketFormat;

/// A paste split into its parts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Split {
    /// Text before the header line (chat preamble), trimmed; empty when there is none. DR-132
    /// doesn't send it anywhere: the paste-back preview shows it as not written.
    pub before: String,
    /// The packet: header through the end of the format's last key's value, ending in `\n`.
    pub packet: String,
    /// Everything after the packet (leading and trailing blank lines trimmed), for
    /// `docs/handoff/stage-NN-attachments.md`; `None` when only blank lines follow.
    pub attachments: Option<String>,
}

/// Why a paste can't be split. Nothing is written in any case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum SplitError {
    /// No header line at all.
    #[error("No packet found: a packet starts with `=== HANDOFF PACKET · Stage N → Stage M ===`.")]
    NoHeader,
    /// More than one header line.
    #[error("paste one packet at a time")]
    TwoPackets,
    /// The header names another stage.
    #[error("This is the Stage {found} packet; this step expects Stage {expected}.")]
    WrongStage {
        /// The stage in the header.
        found: u8,
        /// The stage the step writes.
        expected: u8,
    },
}

fn is_header_like(l: &str) -> bool {
    l.trim_start().starts_with("=== HANDOFF PACKET") || loose_header(l).is_some()
}

fn join(lines: &[&str]) -> String {
    let mut s = lines.join("\n");
    s.push('\n');
    s
}

fn trim_blank<'a, 'b>(lines: &'b [&'a str]) -> &'b [&'a str] {
    let start = lines
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(start, |e| e + 1);
    &lines[start..end.max(start)]
}

/// Splits a normalized paste for `expected`'s packet (DR-132).
///
/// # Errors
/// [`SplitError`]: no header, two headers, or another stage's header.
pub fn split_paste(
    normalized: &str,
    fmt: &PacketFormat,
    expected: u8,
) -> Result<Split, SplitError> {
    let lines: Vec<&str> = normalized.lines().collect();
    let headers: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| is_header_like(l))
        .map(|(i, _)| i)
        .collect();
    let start = match headers.as_slice() {
        [] => return Err(SplitError::NoHeader),
        [h] => *h,
        _ => return Err(SplitError::TwoPackets),
    };
    if let Some((n, _)) = loose_header(lines[start])
        && n != expected
    {
        return Err(SplitError::WrongStage { found: n, expected });
    }
    let end = fmt
        .keys
        .last()
        .and_then(|last| {
            let prefix = format!("{}:", last.key);
            lines
                .iter()
                .enumerate()
                .skip(start + 1)
                .find(|(_, l)| l.starts_with(&prefix))
                .map(|(i, _)| i)
        })
        .map_or(lines.len(), |k| {
            lines
                .iter()
                .enumerate()
                .skip(k + 1)
                .find(|(_, l)| l.trim().is_empty())
                .map_or(lines.len(), |(i, _)| i)
        });
    let before = trim_blank(&lines[..start]).join("\n");
    let rest = trim_blank(&lines[end..]);
    Ok(Split {
        before,
        packet: join(&lines[start..end]),
        attachments: (!rest.is_empty()).then(|| join(rest)),
    })
}
