//! M4-24: an ADR-set paste split into `docs/adr/ADR-NNN.md` (AC-F012-15, SR-35).
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

use foreman_core::packet::split::{self, SplitError};
use foreman_core::store::{ReadStamp, Root, RootKind};

const PASTE: &str = "Here are the ADRs.\n\n# ADR-001: Desktop shell\nWe use Tauri.\n\n## Status\nAccepted\n\n# ADR-002: Frontend\nSvelteKit.\n# ADR-12: not a heading\n";

mod pasteback {
    use super::*;

    /// AC-F012-15, SR-35: the paste is split on `^# ADR-\d{3}:` and every target is listed, with
    /// what's there now for the diff, before anything is written; nothing is written by the plan.
    #[test]
    fn lists_targets_before_write() {
        let parts = split::split_adrs(PASTE).unwrap();
        let names: Vec<&str> = parts.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(names, ["docs/adr/ADR-001.md", "docs/adr/ADR-002.md"]);
        assert_eq!(parts[0].title, "Desktop shell");
        assert_eq!(
            parts[0].text,
            "# ADR-001: Desktop shell\nWe use Tauri.\n\n## Status\nAccepted\n"
        );
        assert_eq!(
            parts[1].text,
            "# ADR-002: Frontend\nSvelteKit.\n# ADR-12: not a heading\n"
        );
        assert_eq!(split::split_adrs("no adrs"), Err(SplitError::NoAdrs));
        assert_eq!(
            split::split_adrs("# ADR-001: a\n# ADR-001: b\n"),
            Err(SplitError::Duplicate("001".into()))
        );
        // The plan lists both, with ADR-001's current text for the diff; writes nothing.
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("docs/adr")).unwrap();
        std::fs::write(d.path().join("docs/adr/ADR-001.md"), "# ADR-001: old\n").unwrap();
        let root = Root::open(d.path(), RootKind::Project).unwrap();
        let plan = split::plan(&root, parts).unwrap();
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].current.as_deref(), Some("# ADR-001: old\n"));
        assert!(matches!(plan[0].stamp, ReadStamp::Present { .. }));
        assert_eq!(plan[1].current, None);
        assert!(plan.iter().all(|t| t.refused.is_none() && t.rel.is_some()));
        assert_eq!(
            std::fs::read_to_string(d.path().join("docs/adr/ADR-001.md")).unwrap(),
            "# ADR-001: old\n"
        );
        assert!(!d.path().join("docs/adr/ADR-002.md").exists());
    }
}
