//! M4-18: appending F.2 lessons to `docs/lessons.md` (DR-138, AC-F016-01…-04).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

use foreman_core::lessons::{self as l, AppendError};
use foreman_core::store::{ReadStamp, Root, RootKind};

const HEADER: &str = "# Lessons — TradeLog\n\nFormat for every entry:\n\n## L-NNN · Stage [N] · <date> · <short title>\nWHAT HAPPENED: what went wrong\nSTATUS: open | applied | rejected\n\n---\n\n";

fn project(text: Option<&str>) -> (tempfile::TempDir, Root) {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("docs")).unwrap();
    if let Some(t) = text {
        std::fs::write(d.path().join("docs/lessons.md"), t).unwrap();
    }
    let r = Root::open(d.path(), RootKind::Project).unwrap();
    (d, r)
}

mod lessons {
    use super::*;

    /// DR-138, AC-F016-02/-03/-04: entries get the next free IDs in order, the header and every
    /// existing entry are kept byte for byte; a file changed since it was read refuses (re-read,
    /// diff again); a missing file refuses and writes nothing.
    #[test]
    fn append_numbering() {
        let existing = format!(
            "{HEADER}## L-001 · Stage 1 · 2026-10-01 · First\nWHAT HAPPENED: a\nSTATUS: open\n\n## L-004 · Stage 2 · 2026-10-02 · Fourth\nWHAT HAPPENED: b\nSTATUS: open\n"
        );
        let pasted = "Here are the lessons:\n\n## L-001 · Stage 3 · 2026-10-05 · New one\nWHAT HAPPENED: c\nSTATUS: open\n\n## L-002 · Stage 3 · 2026-10-05 · clean stage\nSTATUS: applied\n";
        let out = l::appended(&existing, pasted).unwrap();
        assert!(out.starts_with(&existing), "header and entries kept");
        let tail = &out[existing.len()..];
        assert_eq!(
            tail,
            "\n## L-005 · Stage 3 · 2026-10-05 · New one\nWHAT HAPPENED: c\nSTATUS: open\n\n## L-006 · Stage 3 · 2026-10-05 · clean stage\nSTATUS: applied\n"
        );
        assert!(l::status(&out, 3).met);
        assert_eq!(l::next_id(&out), "L-007");
        assert_eq!(l::header(&out), HEADER);
        // Nothing to append.
        assert_eq!(
            l::appended(&existing, "no entries here\n"),
            Err(AppendError::NoEntries)
        );

        // Through the store, by compare-and-swap.
        let (d, root) = project(Some(&existing));
        let (_, seen) = l::read(&root).unwrap().unwrap();
        l::append(&root, &seen, pasted).unwrap();
        assert_eq!(
            std::fs::read_to_string(d.path().join("docs/lessons.md")).unwrap(),
            out
        );
        // AC-F016-03: changed on disk since Foreman read it → refused, nothing written.
        let (d, root) = project(Some(&existing));
        let (_, seen) = l::read(&root).unwrap().unwrap();
        let edited = format!("{existing}\nedited by hand\n");
        std::fs::write(d.path().join("docs/lessons.md"), &edited).unwrap();
        assert_eq!(l::append(&root, &seen, pasted), Err(AppendError::Changed));
        assert_eq!(
            std::fs::read_to_string(d.path().join("docs/lessons.md")).unwrap(),
            edited
        );
        // Re-read and append again: works on the new text.
        let (_, seen) = l::read(&root).unwrap().unwrap();
        l::append(&root, &seen, pasted).unwrap();
        assert!(
            std::fs::read_to_string(d.path().join("docs/lessons.md"))
                .unwrap()
                .contains("edited by hand\n\n## L-005 · Stage 3")
        );
        // AC-F016-04: missing → nothing written, the message says the file is missing.
        let (d, root) = project(None);
        assert!(l::read(&root).unwrap().is_none());
        let e = l::append(&root, &ReadStamp::Absent, pasted).unwrap_err();
        assert_eq!(e, AppendError::Missing);
        assert!(e.to_string().contains("missing"));
        assert!(!d.path().join("docs/lessons.md").exists());
    }

    /// AC-F016-01: the F.2 prompt gets the entry format from the file's header.
    #[test]
    fn f2_prompt_gets_header_format() {
        let suffix = l::f2_suffix(HEADER).unwrap();
        assert!(suffix.contains("## L-NNN · Stage [N] · <date> · <short title>"));
        assert!(suffix.contains("STATUS: open | applied | rejected"));
        assert!(!suffix.contains("---"));
        assert!(!suffix.contains("# Lessons — TradeLog"));
        assert_eq!(l::f2_suffix("# Lessons\n"), None);
    }
}
