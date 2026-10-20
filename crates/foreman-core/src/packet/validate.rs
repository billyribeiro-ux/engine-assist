//! Packet validity (DR-133, DR-134, DR-136). Every problem has a line; extra keys warn.

use serde::{Deserialize, Serialize};

use super::{Signature, parse_packet, parse_signature};
use crate::template::PacketFormat;

/// What validation needs besides the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketCtx {
    /// The stage from the file name.
    pub stage: u8,
    /// The next applicable stage (DR-054).
    pub next_stage: Option<u8>,
    /// The lock's `blueprintVersion` (checked for Stage 4).
    pub lock_blueprint_version: Option<String>,
    /// The stage's SKIPPED queue is non-empty (DR-136).
    pub has_skips: bool,
}

/// One problem or warning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// Rule code (`packet.missing-field` …).
    pub rule: String,
    /// 1-based line.
    pub line: u32,
    /// Message.
    pub message: String,
    /// Warnings don't make the packet invalid.
    pub warning: bool,
}

fn problem(rule: &str, line: u32, message: impl Into<String>) -> Problem {
    Problem {
        rule: rule.into(),
        line: line.max(1),
        message: message.into(),
        warning: false,
    }
}

fn warning(rule: &str, line: u32, message: impl Into<String>) -> Problem {
    Problem {
        warning: true,
        ..problem(rule, line, message)
    }
}

/// Validates packet text against the stage's format (DR-133).
#[allow(clippy::too_many_lines, reason = "one block per DR-133 rule, in order")]
#[must_use]
pub fn validate_packet(text: &str, fmt: &PacketFormat, ctx: &PacketCtx) -> Vec<Problem> {
    let known: Vec<String> = fmt.keys.iter().map(|k| k.key.clone()).collect();
    let p = parse_packet(text, &known);
    let mut out = Vec::new();
    // (1) header and stages.
    let header_line = p.header.map_or(1, |(_, _, l)| l);
    match p.header {
        None => out.push(problem(
            "packet.header",
            1,
            "the first line must be `=== HANDOFF PACKET · Stage N → Stage M ===`",
        )),
        Some((u8::MAX, _, l)) => out.push(problem(
            "packet.header",
            l,
            "the header isn't in the form `=== HANDOFF PACKET · Stage N → Stage M ===`",
        )),
        Some((n, m, l)) => {
            if n != ctx.stage {
                out.push(problem(
                    "packet.stage-mismatch",
                    l,
                    format!(
                        "This is the Stage {n} packet; the file is Stage {}'s",
                        ctx.stage
                    ),
                ));
            }
            if let Some(next) = ctx.next_stage
                && m != next
            {
                out.push(problem(
                    "packet.next-stage",
                    l,
                    format!("the header says Stage {m}, but the next applicable stage is {next}"),
                ));
            }
        }
    }
    // Duplicates and extras.
    for (i, f) in p.fields.iter().enumerate() {
        if p.fields[..i].iter().any(|g| g.key == f.key) {
            out.push(problem(
                "packet.duplicate-field",
                f.line,
                format!("{} appears twice", f.key),
            ));
        }
        if !known.contains(&f.key) {
            out.push(warning(
                "packet.extra-field",
                f.line,
                format!("extra field {}", f.key),
            ));
        }
    }
    // (2) required keys, (6) placeholders, DR-136 not-applicable values.
    for k in &fmt.keys {
        let Some(f) = p.get(&k.key) else {
            out.push(problem(
                "packet.missing-field",
                header_line,
                format!("missing {}", k.key),
            ));
            continue;
        };
        if f.value.trim().is_empty() && k.key != "APPROVED BY" {
            out.push(problem(
                "packet.missing-field",
                f.line,
                format!("{} has no value", k.key),
            ));
            continue;
        }
        let lower = f.value.to_lowercase();
        if (lower.starts_with("not applicable:") || lower.starts_with("skipped:")) && !ctx.has_skips
        {
            out.push(problem(
                "packet.na-without-skips",
                f.line,
                format!(
                    "{} says not applicable or skipped, but nothing was skipped in this stage",
                    k.key
                ),
            ));
        }
        let placeholder = k.placeholder.trim();
        let is_placeholder = !placeholder.is_empty()
            && k.key != "APPROVED BY"
            && k.key != "OPEN ITEMS"
            && placeholder.starts_with('<')
            && f.value.trim() == placeholder;
        if is_placeholder {
            out.push(problem(
                "packet.placeholder",
                f.line,
                format!("{} still holds the placeholder {placeholder}", k.key),
            ));
        }
    }
    // (3) OPEN ITEMS exactly `none`.
    if let Some(f) = p.get("OPEN ITEMS")
        && f.value.trim() != "none"
    {
        out.push(problem(
            "packet.open-items",
            f.line,
            "OPEN ITEMS must be exactly `none`",
        ));
    }
    // (4) APPROVED BY.
    if let Some(f) = p.get("APPROVED BY") {
        match parse_signature(&f.value) {
            Signature::Invalid => out.push(problem(
                "packet.approved-by",
                f.line,
                "APPROVED BY must be `<blank>` or `<name> on <YYYY-MM-DD>`",
            )),
            Signature::Signed { note: Some(_), .. } => out.push(warning(
                "packet.signature-note",
                f.line,
                "the signature has a note after the date",
            )),
            _ => {}
        }
    }
    // (5) Stage 4's blueprint version.
    if ctx.stage == 4
        && let (Some(f), Some(lock)) = (p.get("BLUEPRINT VERSION"), &ctx.lock_blueprint_version)
        && f.value.trim() != lock
    {
        out.push(problem(
            "packet.blueprint-version",
            f.line,
            format!(
                "BLUEPRINT VERSION {} differs from the pinned blueprint {lock}",
                f.value.trim()
            ),
        ));
    }
    out
}

/// True when no problem (warnings allowed).
#[must_use]
pub fn is_valid(problems: &[Problem]) -> bool {
    problems.iter().all(|p| p.warning)
}
