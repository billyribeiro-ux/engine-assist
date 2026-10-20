//! The build-package assembler (F-023, AC-F023-03…-05, AC-F021-06): the Stage 4 packet's
//! REPO FILES list, each entry checked by the write policy before anything else happens. Only
//! root `CLAUDE.md` takes the typed [`WritePurpose::AssemblerClaudeMd`] exception (written after
//! the native confirmation the app raises); every other entry is an ordinary repo write
//! (`docs/` and `spec/` only), so an entry the policy refuses stays a refused row that is listed
//! and never written (T-051, T-052, T-090).

use crate::packet::parse_packet;
use crate::pathpolicy::{
    PolicyViolation, ReadClass, RepoPath, WritePurpose, check_read, check_write,
};
use crate::store::dirs::EntryKind;

/// The packet keys that hold the list (`REPO FILES (create exactly these):` in v2.2).
pub const REPO_FILES_KEYS: [&str; 2] = ["REPO FILES (create exactly these)", "REPO FILES"];

/// The note on root `CLAUDE.md` (AC-F021-06; components.md "Build-package assembler").
pub const CLAUDE_MD_NOTE: &str = "Claude Code loads this file in every session. Writing it asks you again in a macOS dialog.";

/// The most entries read from one list.
pub const MAX_ENTRIES: usize = 200;

/// One REPO FILES entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFile {
    /// The path as listed (first word of the line), a trailing `/` kept for folders.
    pub path: String,
    /// Listed as a folder (`docs/migrations/`).
    pub dir: bool,
}

/// Parses the REPO FILES list of a Stage 4 packet: one entry per non-empty line of the field
/// (the first word; trailing remarks such as `(SR/PR/LC lists)` or `…` are dropped), in packet
/// order, duplicates removed, at most [`MAX_ENTRIES`]. `known_keys` are the stage's format keys
/// (the REPO FILES keys are always known). `None` when the packet has no REPO FILES field.
#[must_use]
pub fn repo_files(packet_text: &str, known_keys: &[String]) -> Option<Vec<RepoFile>> {
    let mut keys: Vec<String> = known_keys.to_vec();
    keys.extend(REPO_FILES_KEYS.iter().map(|k| (*k).to_owned()));
    let p = parse_packet(packet_text, &keys);
    let field = REPO_FILES_KEYS.iter().find_map(|k| p.get(k))?;
    let mut out: Vec<RepoFile> = Vec::new();
    for line in field.value.lines() {
        let Some(word) = line.split_whitespace().next() else {
            continue;
        };
        if matches!(word, "…" | "..." | "none" | "-") {
            continue;
        }
        let word = word.trim_end_matches('…');
        if word.is_empty() || out.iter().any(|f| f.path == word) {
            continue;
        }
        out.push(RepoFile {
            path: word.to_owned(),
            dir: word.ends_with('/'),
        });
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    Some(out)
}

/// The write check of one entry (SR-30): root `CLAUDE.md` with the assembler's purpose, every
/// other path (a folder without its trailing `/`) as an ordinary repo write; a secret-file name
/// (SR-50) is refused too.
///
/// # Errors
/// The rule that refuses it.
pub fn target(path: &str) -> Result<(RepoPath, WritePurpose), PolicyViolation> {
    let rel = path.strip_suffix('/').unwrap_or(path);
    let purpose = if rel == "CLAUDE.md" {
        WritePurpose::AssemblerClaudeMd
    } else {
        WritePurpose::Repo
    };
    let checked = check_write(rel, purpose)?;
    // Secret-file names (SR-50) are never read, so they are never pasted into either.
    check_read(rel, ReadClass::Decision)?;
    Ok((checked, purpose))
}

/// A row's state (AC-F023-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowState {
    /// A non-empty file, or a folder with something in it.
    Exists,
    /// Nothing there.
    Missing,
    /// An empty file or folder.
    Empty,
    /// The policy refuses it (or what is there isn't a regular file or folder).
    Refused(String),
}

/// The state of an entry from what is on disk.
#[must_use]
pub fn row_state(entry: &RepoFile, on_disk: EntryKind) -> RowState {
    match (entry.dir, on_disk) {
        (_, EntryKind::Missing) => RowState::Missing,
        (false, EntryKind::File(0)) | (true, EntryKind::Dir(0)) => RowState::Empty,
        (false, EntryKind::File(_)) | (true, EntryKind::Dir(_)) => RowState::Exists,
        (false, EntryKind::Dir(_)) => RowState::Refused("path.not-a-file".to_owned()),
        (true, EntryKind::File(_)) => RowState::Refused("path.not-a-folder".to_owned()),
        (_, EntryKind::Other) => RowState::Refused("path.not-regular".to_owned()),
    }
}

/// AC-F023-05: the package is complete when every row Foreman may write exists; a refused row
/// never blocks it (Foreman can never write it; Billy creates it himself). An empty list isn't
/// complete.
#[must_use]
pub fn complete(states: &[RowState]) -> bool {
    !states.is_empty()
        && states
            .iter()
            .all(|s| matches!(s, RowState::Exists | RowState::Refused(_)))
}

/// The sentence shown with a refused row.
#[must_use]
pub fn refused_note(why: &PolicyViolation) -> String {
    let base = match why {
        PolicyViolation::DeniedComponent(c) => {
            format!("Foreman doesn't write under {c}/ (path policy).")
        }
        PolicyViolation::DeniedName(n) => format!("Foreman never writes {n} (path policy)."),
        PolicyViolation::ClaudeMd => {
            "CLAUDE.md is written only at the repo root (path policy).".to_owned()
        }
        PolicyViolation::NotAllowlisted => {
            "Foreman writes only under docs/ and spec/ (path policy).".to_owned()
        }
        other => format!("Refused by the path policy: {other}."),
    };
    format!("{base} Create this file yourself or in a Stage 5 session.")
}
