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
    /// The format key the problem is about, when it is about one (the fix request names it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

fn problem(rule: &str, line: u32, message: impl Into<String>) -> Problem {
    Problem {
        rule: rule.into(),
        line: line.max(1),
        message: message.into(),
        warning: false,
        field: None,
    }
}

fn about(p: Problem, key: &str) -> Problem {
    Problem {
        field: Some(key.to_owned()),
        ..p
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
            out.push(about(
                problem(
                    "packet.missing-field",
                    header_line,
                    format!("missing {}", k.key),
                ),
                &k.key,
            ));
            continue;
        };
        if f.value.trim().is_empty() && k.key != "APPROVED BY" {
            out.push(about(
                problem(
                    "packet.missing-field",
                    f.line,
                    format!("{} has no value", k.key),
                ),
                &k.key,
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
            out.push(about(
                problem(
                    "packet.placeholder",
                    f.line,
                    format!("{} still holds the placeholder {placeholder}", k.key),
                ),
                &k.key,
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

/// The fix request for a packet's problems (DR-137, AC-F012-11): "Your packet is missing: X, Y.
/// OPEN ITEMS must be exactly `none`. Re-output the complete packet in one code block." with the
/// actual problems. Missing keys (absent, empty or still the placeholder) are named; the OPEN ITEMS sentence appears only
/// when rule 3 failed; any other problem is listed by its message. Warnings are left out.
#[must_use]
pub fn fix_request_text(problems: &[Problem]) -> String {
    let errors: Vec<&Problem> = problems.iter().filter(|p| !p.warning).collect();
    let mut missing: Vec<&str> = Vec::new();
    for p in &errors {
        if (p.rule == "packet.missing-field" || p.rule == "packet.placeholder")
            && let Some(k) = p.field.as_deref()
            && !missing.contains(&k)
        {
            missing.push(k);
        }
    }
    let open_items = errors.iter().any(|p| p.rule == "packet.open-items");
    let other: Vec<&str> = errors
        .iter()
        .filter(|p| {
            p.rule != "packet.open-items"
                && !((p.rule == "packet.missing-field" || p.rule == "packet.placeholder")
                    && p.field.is_some())
        })
        .map(|p| p.message.trim_end_matches('.'))
        .collect();
    let mut parts: Vec<String> = Vec::new();
    if !missing.is_empty() {
        parts.push(format!("Your packet is missing: {}.", missing.join(", ")));
    }
    if !other.is_empty() {
        let lead = if missing.is_empty() {
            "Your packet has problems:"
        } else {
            "Also fix:"
        };
        parts.push(format!("{lead} {}.", other.join("; ")));
    }
    if open_items {
        parts.push("OPEN ITEMS must be exactly `none`.".to_owned());
    }
    parts.push("Re-output the complete packet in one code block.".to_owned());
    parts.join(" ")
}
