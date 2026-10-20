//! Handoff packets (F-012, F-013; `docs/contracts/files.md` §5): normalization, splitting a
//! paste, parsing after normalization, the signature forms, validation and the fix request. Content never changes Foreman's behavior beyond the parsed
//! fields (AC-F013-03).

use serde::{Deserialize, Serialize};

pub mod adr_split;
pub mod normalize;
pub mod sign_fields;
pub mod split;
pub mod validate;

pub use normalize::{Normalized, Note, NoteKind, normalize_packet, normalize_paste};
pub use split::{Split, SplitError, split_paste};
pub use validate::{PacketCtx, Problem, fix_request_text, validate_packet};

/// A parsed field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    /// The key, exactly.
    pub key: String,
    /// The value: first line plus continuation lines, trailing blank lines trimmed.
    pub value: String,
    /// The key's line (1-based).
    pub line: u32,
}

/// A parsed packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Packet {
    /// `(N, M, line)` from `=== HANDOFF PACKET · Stage N → Stage M ===`.
    pub header: Option<(u8, u8, u32)>,
    /// Fields in order.
    pub fields: Vec<Field>,
}

impl Packet {
    /// A field by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.key == key)
    }
}

fn parse_header(l: &str) -> Option<(u8, u8)> {
    let inner = l
        .strip_prefix("=== HANDOFF PACKET · Stage ")?
        .strip_suffix(" ===")?;
    let (n, m) = inner.split_once(" → Stage ")?;
    Some((n.parse().ok()?, m.parse().ok()?))
}

/// An EXTRA key (`[A-Z][A-Z0-9 ()+/&.-]{0,59}`) or a known key at column 1, followed by `:`.
fn key_of<'a>(line: &'a str, known: &[String]) -> Option<&'a str> {
    if let Some(k) = known.iter().find(|k| {
        line.strip_prefix(k.as_str())
            .is_some_and(|r| r.starts_with(':'))
    }) {
        return Some(&line[..k.len()]);
    }
    let (k, _) = line.split_once(':')?;
    let ok = k.starts_with(|c: char| c.is_ascii_uppercase())
        && k.len() <= 60
        && k.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || " ()+/&.-".contains(c));
    ok.then_some(k)
}

/// Parses packet text (normalized, §5.1). Never panics.
#[must_use]
pub fn parse_packet(text: &str, known_keys: &[String]) -> Packet {
    let mut p = Packet {
        header: None,
        fields: Vec::new(),
    };
    for (i, line) in text.lines().enumerate() {
        let ln = u32::try_from(i + 1).unwrap_or(u32::MAX);
        if p.header.is_none() && p.fields.is_empty() {
            if let Some((n, m)) = parse_header(line.trim_end()) {
                p.header = Some((n, m, ln));
                continue;
            }
            if line.starts_with("=== HANDOFF PACKET") {
                p.header = Some((u8::MAX, u8::MAX, ln));
                continue;
            }
        }
        if !line.starts_with([' ', '\t'])
            && let Some(k) = key_of(line, known_keys)
        {
            let v = line[k.len() + 1..].trim();
            p.fields.push(Field {
                key: k.to_owned(),
                value: v.to_owned(),
                line: ln,
            });
            continue;
        }
        if let Some(f) = p.fields.last_mut() {
            f.value.push('\n');
            f.value.push_str(line.trim_end());
        }
    }
    for f in &mut p.fields {
        let trimmed = f.value.trim_end().to_owned();
        f.value = trimmed;
    }
    p
}

/// `APPROVED BY:` forms (DR-134).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "form")]
pub enum Signature {
    /// `<blank>`.
    Unsigned,
    /// `<name> on <YYYY-MM-DD>` with an optional parenthetical (warning).
    Signed {
        /// Name.
        name: String,
        /// Date.
        date: String,
        /// The parenthetical, if any.
        note: Option<String>,
    },
    /// Anything else.
    Invalid,
}

/// Parses an `APPROVED BY:` value. The signed **form** is not a signature (DR-135).
#[must_use]
pub fn parse_signature(v: &str) -> Signature {
    let v = v.trim();
    if v == "<blank>" {
        return Signature::Unsigned;
    }
    let (main, note) = match v.find(" (") {
        Some(i) if v.ends_with(')') => (&v[..i], Some(v[i + 2..v.len() - 1].to_owned())),
        _ => (v, None),
    };
    let Some((name, date)) = main.rsplit_once(" on ") else {
        return Signature::Invalid;
    };
    let name_ok =
        !name.is_empty() && name.chars().count() <= 80 && !name.contains(['<', '>', '\n']);
    let date_ok = date.len() == 10 && jiff::civil::Date::strptime("%Y-%m-%d", date).is_ok();
    let note_ok = note.as_ref().is_none_or(|n| n.chars().count() <= 200);
    if name_ok && date_ok && note_ok {
        Signature::Signed {
            name: name.to_owned(),
            date: date.to_owned(),
            note,
        }
    } else {
        Signature::Invalid
    }
}

/// The packet file of a stage (DR-130).
#[must_use]
pub fn file_name(stage: u8) -> String {
    format!("docs/handoff/stage-{stage:02}.md")
}

/// The stage of an exact packet file name, `None` for anything else (`stage-2.md`, `.bak` …).
#[must_use]
pub fn stage_of_file(path: &str) -> Option<u8> {
    let n = path
        .strip_prefix("docs/handoff/stage-")?
        .strip_suffix(".md")?;
    (n.len() == 2 && n.bytes().all(|b| b.is_ascii_digit()))
        .then(|| n.parse().ok())
        .flatten()
}
