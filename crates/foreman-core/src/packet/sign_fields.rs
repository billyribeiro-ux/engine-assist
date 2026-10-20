//! The fields Foreman adds at signing (DR-139, `docs/contracts/files.md` §5.4): `SKIPPED:`,
//! `SUBSTITUTIONS:` and `ATTESTED:` rendered deterministically as the last fields before
//! `OPEN ITEMS:` (replacing same-named fields), and `APPROVED BY:` replaced in place. Pure.

use super::parse_packet;

/// The keys Foreman writes.
pub const ADDED_KEYS: [&str; 3] = ["SKIPPED", "SUBSTITUTIONS", "ATTESTED"];

/// What signing adds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignFields {
    /// DRI name (validated by the caller: 1–80 characters, no `<`, `>` or line breaks).
    pub name: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// `SKIPPED:` lines (DR-055), in stage-step order.
    pub skipped: Vec<String>,
    /// `SUBSTITUTIONS:` lines (DR-104), in stage-step order.
    pub substitutions: Vec<String>,
    /// `ATTESTED:` lines (`<item>: done: <note>` …), in item order.
    pub attested: Vec<String>,
}

/// One line of a list value: control characters and line breaks become spaces, so a note can't
/// add a field or a line.
#[must_use]
pub fn one_line(s: &str) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    flat.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Renders one added field: `KEY: none` for an empty list, otherwise `KEY:` and `  - item` lines.
#[must_use]
pub fn render_field(key: &str, items: &[String]) -> Vec<String> {
    if items.is_empty() {
        return vec![format!("{key}: none")];
    }
    let mut v = vec![format!("{key}:")];
    v.extend(items.iter().map(|i| format!("  - {}", one_line(i))));
    v
}

/// The three added fields, in order.
#[must_use]
pub fn render(f: &SignFields) -> Vec<String> {
    let mut v = render_field("SKIPPED", &f.skipped);
    v.extend(render_field("SUBSTITUTIONS", &f.substitutions));
    v.extend(render_field("ATTESTED", &f.attested));
    v
}

/// Why the fields couldn't be applied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignFieldsError {
    /// The packet has no `APPROVED BY:` line to replace.
    #[error("The packet has no APPROVED BY line.")]
    NoApprovedBy,
}

/// Applies the signing fields to packet text (normalized, LF). `known_keys` is the stage
/// format's keys, so continuation lines are told from keys exactly as validation does.
///
/// # Errors
/// [`SignFieldsError::NoApprovedBy`].
pub fn apply(text: &str, known_keys: &[String], f: &SignFields) -> Result<String, SignFieldsError> {
    let mut keys: Vec<String> = known_keys.to_vec();
    for k in ADDED_KEYS
        .iter()
        .chain(["APPROVED BY", "OPEN ITEMS"].iter())
    {
        if !keys.iter().any(|x| x == k) {
            keys.push((*k).to_owned());
        }
    }
    let parsed = parse_packet(text, &keys);
    let lines: Vec<&str> = text.lines().collect();
    let n = lines.len();
    // Each field's line range [start, end).
    let starts: Vec<(String, usize)> = parsed
        .fields
        .iter()
        .map(|fl| {
            (
                fl.key.clone(),
                usize::try_from(fl.line).unwrap_or(1).saturating_sub(1),
            )
        })
        .collect();
    let range_of = |i: usize| -> (usize, usize) {
        let s = starts[i].1;
        let e = starts.get(i + 1).map_or(n, |x| x.1);
        (s, e)
    };
    let approved = starts
        .iter()
        .position(|(k, _)| k == "APPROVED BY")
        .ok_or(SignFieldsError::NoApprovedBy)?;
    let mut drop = vec![false; n];
    for (i, (k, _)) in starts.iter().enumerate() {
        if ADDED_KEYS.contains(&k.as_str()) {
            let (s, e) = range_of(i);
            for d in drop.iter_mut().take(e).skip(s) {
                *d = true;
            }
        }
    }
    // APPROVED BY keeps its line; its continuation lines (if any) go, trailing blanks stay.
    let (a_start, a_end) = range_of(approved);
    for i in a_start + 1..a_end {
        if !lines[i].trim().is_empty() {
            drop[i] = true;
        }
    }
    let open = starts.iter().find(|(k, _)| k == "OPEN ITEMS").map(|x| x.1);
    let added = render(f);
    let mut out: Vec<String> = Vec::with_capacity(n + added.len() + 1);
    let mut inserted = false;
    for (i, l) in lines.iter().enumerate() {
        if Some(i) == open && !inserted {
            out.extend(added.iter().cloned());
            if out.len() > added.len() && out[out.len() - added.len() - 1].trim().is_empty() {
                // The packet separates fields with blank lines: keep that before OPEN ITEMS.
                out.push(String::new());
            }
            inserted = true;
        }
        if drop[i] {
            continue;
        }
        if i == a_start {
            out.push(format!("APPROVED BY: {} on {}", one_line(&f.name), f.date));
        } else {
            out.push((*l).to_owned());
        }
    }
    if !inserted {
        while out.last().is_some_and(|l| l.trim().is_empty()) {
            out.pop();
        }
        out.extend(added);
    }
    // Collapse runs of blank lines a removed field may leave behind.
    let mut tidy: Vec<String> = Vec::with_capacity(out.len());
    for l in out {
        if l.trim().is_empty() && tidy.last().is_some_and(|p: &String| p.trim().is_empty()) {
            continue;
        }
        tidy.push(l);
    }
    let mut s = tidy.join("\n");
    s.push('\n');
    Ok(s)
}
