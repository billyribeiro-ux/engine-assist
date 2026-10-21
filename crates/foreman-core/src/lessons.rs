//! `docs/lessons.md` (F-016, DR-074, DR-138): entries, the stage requirement, the next free ID,
//! the entry format from the file's header (the F.2 prompt's suffix), and the append with
//! renumbering. Pure apart from [`append`], which writes through the store by compare-and-swap.

use std::fmt::Write as _;

use crate::pathpolicy::{self, ReadClass, RepoPath, WritePurpose};
use crate::store::{ReadStamp, Root, StoreError};

/// The lessons file's repo path.
pub const PATH: &str = "docs/lessons.md";

/// One entry heading: `## L-NNN · Stage N · <rest>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The number (`7` for `L-007`).
    pub number: u32,
    /// The ID as written (`L-007`).
    pub id: String,
    /// The stage the heading names.
    pub stage: u8,
    /// The rest of the heading after `Stage N · ` (date and title).
    pub title: String,
    /// 1-based line.
    pub line: u32,
}

const SEP: &str = " · ";

/// Parses one heading line (DR-074: `^## L-(\d{3,}) · Stage (\d{1,2}) · `).
#[must_use]
pub fn parse_heading(line: &str) -> Option<(u32, String, u8, String)> {
    let rest = line.strip_prefix("## L-")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() < 3 {
        return None;
    }
    let number: u32 = digits.parse().ok()?;
    let rest = rest[digits.len()..].strip_prefix(SEP)?;
    let rest = rest.strip_prefix("Stage ")?;
    let sd: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if sd.is_empty() || sd.len() > 2 {
        return None;
    }
    let stage: u8 = sd.parse().ok()?;
    let title = rest[sd.len()..].strip_prefix(SEP)?;
    Some((
        number,
        format!("L-{digits}"),
        stage,
        title.trim_end().to_owned(),
    ))
}

/// Every entry heading in the file, in order.
#[must_use]
pub fn entries(text: &str) -> Vec<Entry> {
    text.lines()
        .enumerate()
        .filter_map(|(i, l)| {
            parse_heading(l).map(|(number, id, stage, title)| Entry {
                number,
                id,
                stage,
                title,
                line: u32::try_from(i + 1).unwrap_or(u32::MAX),
            })
        })
        .collect()
}

/// The next free ID: `L-` + (highest number + 1), zero-padded to 3 (DR-074).
#[must_use]
pub fn next_id(text: &str) -> String {
    let n = entries(text).iter().map(|e| e.number).max().unwrap_or(0);
    format!("L-{:03}", n.saturating_add(1))
}

/// A stage's lessons requirement (DR-074, AC-F016-06).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// An entry for this stage exists ("clean stage" in the same form counts).
    pub met: bool,
    /// This stage's entries.
    pub entries: Vec<Entry>,
    /// The next free ID.
    pub next_id: String,
}

/// The requirement for `stage`: only `## L-NNN · Stage <stage> · …` counts; another stage's entry
/// doesn't.
#[must_use]
pub fn status(text: &str, stage: u8) -> Status {
    let mine: Vec<Entry> = entries(text)
        .into_iter()
        .filter(|e| e.stage == stage)
        .collect();
    Status {
        met: !mine.is_empty(),
        entries: mine,
        next_id: next_id(text),
    }
}

/// The header: everything before the first entry heading (kept unchanged by an append).
#[must_use]
pub fn header(text: &str) -> &str {
    let mut off = 0;
    for l in text.split_inclusive('\n') {
        if parse_heading(l.trim_end_matches('\n')).is_some() {
            return &text[..off];
        }
        off += l.len();
    }
    text
}

/// The entry format in the header: from its `## L-` template line up to the `---` rule (or the
/// header's end). `None` when the header has no template line.
#[must_use]
pub fn entry_format(text: &str) -> Option<String> {
    let h = header(text);
    let mut out = Vec::new();
    let mut on = false;
    for l in h.lines() {
        if !on && l.starts_with("## L-") {
            on = true;
        }
        if on {
            if l.trim() == "---" {
                break;
            }
            out.push(l);
        }
    }
    let s = out.join("\n").trim().to_owned();
    (!s.is_empty()).then_some(s)
}

/// AC-F016-01: what the F.2 prompt gets appended (the format from the file's header).
#[must_use]
pub fn f2_suffix(lessons_text: &str) -> Option<String> {
    entry_format(lessons_text).map(|f| {
        format!(
            "\n\nWrite each lesson in the format from docs/lessons.md's header, exactly:\n\n{f}"
        )
    })
}

/// Why an append was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AppendError {
    /// `docs/lessons.md` doesn't exist (AC-F016-04): nothing is written.
    #[error(
        "docs/lessons.md is missing, so nothing was appended. Create it with its header first."
    )]
    Missing,
    /// The paste holds no `## L-… · Stage N · …` entry.
    #[error("The paste has no lessons entry headed \"## L-NNN · Stage N · …\".")]
    NoEntries,
    /// The file changed since Foreman read it (AC-F016-03): re-read and review the diff again.
    #[error("docs/lessons.md changed on disk since Foreman read it. Review the new diff.")]
    Changed,
    /// The store refused.
    #[error(transparent)]
    Store(StoreError),
}

/// The text after appending `pasted`'s entries to `existing`: each `## L-NNN` heading is
/// renumbered to the next free IDs in order, the header and every existing entry are kept
/// byte-for-byte, and anything in the paste before its first entry heading is dropped.
///
/// # Errors
/// [`AppendError::NoEntries`].
pub fn appended(existing: &str, pasted: &str) -> Result<String, AppendError> {
    let start = header(pasted).len();
    let body = &pasted[start..];
    if body.is_empty() {
        return Err(AppendError::NoEntries);
    }
    let mut next = entries(existing)
        .iter()
        .map(|e| e.number)
        .max()
        .unwrap_or(0);
    let mut out = existing.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
    for l in body.split_inclusive('\n') {
        let bare = l.trim_end_matches('\n');
        if let Some((_, id, _, _)) = parse_heading(bare) {
            next = next.saturating_add(1);
            // `id` is `L-` plus the digits; everything after it (` · Stage N · …`) is kept.
            let after = &bare["## ".len() + id.len()..];
            let _ = write!(out, "## L-{next:03}{after}");
            if l.ends_with('\n') {
                out.push('\n');
            }
        } else {
            out.push_str(l);
        }
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn rel() -> Result<RepoPath, AppendError> {
    pathpolicy::check_write(PATH, WritePurpose::Repo)
        .map_err(|e| AppendError::Store(StoreError::from(e)))
}

/// Reads `docs/lessons.md` through the store (`None` when missing).
///
/// # Errors
/// A store refusal.
pub fn read(project: &Root) -> Result<Option<(String, ReadStamp)>, StoreError> {
    let r = pathpolicy::check_read(PATH, ReadClass::Decision)?;
    match project.read_opt(&r, ReadClass::Decision)? {
        (Some(b), s) => Ok(Some((String::from_utf8_lossy(&b).into_owned(), s))),
        (None, _) => Ok(None),
    }
}

/// DR-138: appends by a full compare-and-swap write against the stamp Foreman read when it showed
/// the diff. A missing file refuses; a changed file refuses (the caller re-reads and shows the
/// diff again).
///
/// # Errors
/// See [`AppendError`].
pub fn append(project: &Root, seen: &ReadStamp, pasted: &str) -> Result<ReadStamp, AppendError> {
    if matches!(seen, ReadStamp::Absent) {
        return Err(AppendError::Missing);
    }
    let Some((now, stamp)) = read(project).map_err(AppendError::Store)? else {
        return Err(AppendError::Missing);
    };
    if stamp.sha256() != seen.sha256() {
        return Err(AppendError::Changed);
    }
    let text = appended(&now, pasted)?;
    project
        .write(&rel()?, text.as_bytes(), seen)
        .map_err(|e| match e {
            StoreError::Conflict { .. } => AppendError::Changed,
            StoreError::NotFound(_) => AppendError::Missing,
            other => AppendError::Store(other),
        })
}
