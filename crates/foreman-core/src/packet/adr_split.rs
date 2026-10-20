//! Splitting an ADR-set paste (AC-F012-15, SR-35; M4-24): the text is cut on headings matching
//! `^# ADR-\d{3}:` into `docs/adr/ADR-NNN.md`, and every target is listed with its current content
//! (for the diff) before anything is written. Pure apart from reading the current files through
//! the store; the write is the paste-back apply's compare-and-swap.

use crate::pathpolicy::{self, ReadClass, RepoPath, WritePurpose};
use crate::store::{ReadStamp, Root, StoreError};

/// One ADR cut from the paste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdrPart {
    /// `NNN` (three or more digits as written).
    pub number: String,
    /// The heading's title after `ADR-NNN:`.
    pub title: String,
    /// `docs/adr/ADR-NNN.md`.
    pub path: String,
    /// The ADR's text, its heading line first, ending with one LF.
    pub text: String,
}

/// Why a paste couldn't be split.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SplitError {
    /// No `# ADR-NNN:` heading.
    #[error("The paste has no ADR heading (\"# ADR-001: …\").")]
    NoAdrs,
    /// The same ADR number twice.
    #[error("ADR-{0} appears twice in the paste. Paste each ADR once.")]
    Duplicate(String),
}

/// Parses a heading line `# ADR-\d{3}: title` (exactly three digits, as AC-F012-15 writes the
/// pattern). Returns the number and the title.
#[must_use]
pub fn adr_heading(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("# ADR-")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() != 3 {
        return None;
    }
    let title = rest[3..].strip_prefix(':')?;
    Some((digits, title.trim().to_owned()))
}

/// Splits the paste. Text before the first heading is dropped (it's chat commentary).
///
/// # Errors
/// [`SplitError`].
pub fn split_adrs(text: &str) -> Result<Vec<AdrPart>, SplitError> {
    let mut out: Vec<AdrPart> = Vec::new();
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if let Some((number, title)) = adr_heading(bare) {
            if out.iter().any(|a| a.number == number) {
                return Err(SplitError::Duplicate(number));
            }
            out.push(AdrPart {
                path: format!("docs/adr/ADR-{number}.md"),
                number,
                title,
                text: String::new(),
            });
        }
        if let Some(cur) = out.last_mut() {
            cur.text.push_str(bare);
            cur.text.push('\n');
        }
    }
    if out.is_empty() {
        return Err(SplitError::NoAdrs);
    }
    for a in &mut out {
        let trimmed = a.text.trim_end().to_owned();
        a.text = trimmed + "\n";
    }
    Ok(out)
}

/// One write target, listed before anything is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The ADR.
    pub part: AdrPart,
    /// The checked path (`None` when the path policy refuses it).
    pub rel: Option<RepoPath>,
    /// What's there now (`None`: a new file) and its stamp, for the diff and the CAS write.
    pub current: Option<String>,
    /// The stamp the write must match.
    pub stamp: ReadStamp,
    /// The refusal, when there is one.
    pub refused: Option<String>,
}

/// Every target with its current content (SR-35: listed and diffed before any write).
///
/// # Errors
/// A store error other than a missing file.
pub fn plan(project: &Root, parts: Vec<AdrPart>) -> Result<Vec<Target>, StoreError> {
    let mut out = Vec::new();
    for part in parts {
        match pathpolicy::check_write(&part.path, WritePurpose::Repo) {
            Ok(rel) => {
                let (bytes, stamp) = project.read_opt(&rel, ReadClass::Decision)?;
                out.push(Target {
                    current: bytes.map(|b| String::from_utf8_lossy(&b).into_owned()),
                    stamp,
                    rel: Some(rel),
                    refused: None,
                    part,
                });
            }
            Err(e) => out.push(Target {
                part,
                rel: None,
                current: None,
                stamp: ReadStamp::Absent,
                refused: Some(e.to_string()),
            }),
        }
    }
    Ok(out)
}
