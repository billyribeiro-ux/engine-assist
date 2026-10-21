//! M4-02 (ADR-018 §5, files.md rows 20 and 21): the startup pruner's template pass. A template no
//! project pins goes 90 days after it was last used, with its parse caches; a pinned one is kept
//! and its "last used" time refreshed; nothing else in `templates/` is touched and no link is
//! followed.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

use std::collections::BTreeSet;
use std::os::unix::fs::symlink;
use std::time::{Duration, SystemTime};

use foreman_core::retention::{self, TEMPLATE_DAYS};
use foreman_core::store::{Root, RootKind};

const DAY: u64 = 86_400;
const NOW: i64 = 1_792_400_400;

fn sha(c: char) -> String {
    c.to_string().repeat(64)
}

fn app() -> (tempfile::TempDir, Root) {
    let d = tempfile::tempdir().unwrap();
    let root = Root::open_app_data(&d.path().join("app")).unwrap();
    (d, root)
}

fn put(d: &tempfile::TempDir, rel: &str, text: &str, days_old: u64) {
    let p = d.path().join("app").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, text).unwrap();
    let t = SystemTime::UNIX_EPOCH + Duration::from_secs(NOW as u64 - days_old * DAY);
    std::fs::File::options()
        .write(true)
        .open(&p)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

fn exists(d: &tempfile::TempDir, rel: &str) -> bool {
    d.path().join("app").join(rel).symlink_metadata().is_ok()
}

fn mtime(d: &tempfile::TempDir, rel: &str) -> u64 {
    std::fs::metadata(d.path().join("app").join(rel))
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn now() -> jiff::Timestamp {
    jiff::Timestamp::from_second(NOW).unwrap()
}

fn nothing() -> BTreeSet<String> {
    BTreeSet::new()
}

mod templates {
    use super::*;

    /// The boundary: 90 days exactly stays, one second more goes, with the text and every cache.
    #[test]
    fn unused_template_goes_after_90_days_with_its_caches() {
        let (d, root) = app();
        let old = sha('a');
        let edge = sha('b');
        put(&d, &format!("templates/{old}.md"), "text", 91);
        put(&d, &format!("templates/{old}.p3.json"), "{}", 91);
        put(&d, &format!("templates/{old}.p2.json"), "{}", 120);
        put(
            &d,
            &format!("templates/{edge}.md"),
            "text",
            TEMPLATE_DAYS as u64,
        );
        let pass = retention::prune_templates(&root, &nothing(), now()).unwrap();
        assert_eq!(pass.removed, 3);
        assert_eq!(pass.refreshed, 0);
        for f in [".md", ".p3.json", ".p2.json"] {
            assert!(!exists(&d, &format!("templates/{old}{f}")), "{f}");
        }
        assert!(
            exists(&d, &format!("templates/{edge}.md")),
            "exactly 90 days"
        );
        // Nothing more to do on a second pass.
        assert_eq!(
            retention::prune_templates(&root, &nothing(), now())
                .unwrap()
                .removed,
            0
        );
    }

    /// A fresh cache keeps the text of an unused template: the newest file of the template counts.
    #[test]
    fn the_newest_file_of_a_template_decides() {
        let (d, root) = app();
        let s = sha('c');
        put(&d, &format!("templates/{s}.md"), "text", 200);
        put(&d, &format!("templates/{s}.p3.json"), "{}", 10);
        let pass = retention::prune_templates(&root, &nothing(), now()).unwrap();
        assert_eq!(pass.removed, 0);
        assert!(exists(&d, &format!("templates/{s}.md")));
    }

    /// A template a project pins is never deleted, however old; its files are refreshed, so the
    /// 90 days count from the last time it was in use.
    #[test]
    fn pinned_template_is_kept_and_refreshed() {
        let (d, root) = app();
        let s = sha('d');
        let fresh = sha('e');
        put(&d, &format!("templates/{s}.md"), "text", 400);
        put(&d, &format!("templates/{s}.p3.json"), "{}", 400);
        put(&d, &format!("templates/{fresh}.md"), "text", 0);
        let used: BTreeSet<String> = [s.clone(), fresh.clone()].into();
        let pass = retention::prune_templates(&root, &used, now()).unwrap();
        assert_eq!((pass.removed, pass.refreshed), (0, 2), "{pass:?}");
        assert_eq!(mtime(&d, &format!("templates/{s}.md")), NOW as u64);
        assert_eq!(
            std::fs::read_to_string(d.path().join(format!("app/templates/{s}.md"))).unwrap(),
            "text",
            "only the time changes"
        );
        assert_eq!(
            mtime(&d, &format!("templates/{fresh}.md")),
            NOW as u64,
            "touched within a day: left as it was"
        );
        // Unpinned later, it stays for another 90 days from the refresh.
        let later = jiff::Timestamp::from_second(NOW + 89 * DAY as i64).unwrap();
        assert_eq!(
            retention::prune_templates(&root, &nothing(), later)
                .unwrap()
                .removed,
            0
        );
        let after = jiff::Timestamp::from_second(NOW + 91 * DAY as i64).unwrap();
        assert_eq!(
            retention::prune_templates(&root, &nothing(), after)
                .unwrap()
                .removed,
            3
        );
    }

    /// Only Foreman's template names are candidates; a link is unlinked or left, never followed.
    #[test]
    fn other_names_and_links_are_left_alone() {
        let (d, root) = app();
        put(&d, "templates/notes.md", "mine", 500);
        put(&d, &format!("templates/{}.md.bak", sha('f')), "bak", 500);
        put(&d, "templates/sub/inner.md", "x", 500);
        let outside = d.path().join("outside.md");
        std::fs::write(&outside, "precious").unwrap();
        let link = sha('1');
        symlink(&outside, d.path().join(format!("app/templates/{link}.md"))).unwrap();
        // A link named like a template is skipped, not followed; the rest of the pass still runs.
        put(&d, &format!("templates/{}.md", sha('2')), "old", 500);
        let pass = retention::prune_templates(&root, &nothing(), now()).unwrap();
        assert_eq!(pass.removed, 1);
        assert!(!exists(&d, &format!("templates/{}.md", sha('2'))));
        assert!(
            exists(&d, &format!("templates/{link}.md")),
            "the link stays"
        );
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "precious");
        for f in ["notes.md", "sub/inner.md"] {
            assert!(exists(&d, &format!("templates/{f}")), "{f}");
        }
        assert!(exists(&d, &format!("templates/{}.md.bak", sha('f'))));
    }

    /// No templates folder is a clean no-op.
    #[test]
    fn no_templates_folder() {
        let (_d, root) = app();
        let pass = retention::prune_templates(&root, &nothing(), now()).unwrap();
        assert_eq!((pass.removed, pass.refreshed), (0, 0));
    }
}

mod pinned {
    use super::*;

    fn lock(sha: &str) -> String {
        serde_json::json!({
            "schemaVersion": 1,
            "blueprintRepoCommit": "0".repeat(40),
            "blueprintPath": "claude-build-blueprint.md",
            "sourceSha256": sha,
            "blueprintVersion": "2.2",
            "grammarVersion": 1,
            "pinnedAt": "2026-10-01T00:00:00Z",
            "items": {}
        })
        .to_string()
    }

    const ID: &str = "0b1c2d3e-0000-4000-8000-000000000001";

    /// A project pins what its approved copy says and what its repo lock says (they differ while
    /// an outside edit of the lock waits for review).
    #[test]
    fn both_locks_count() {
        let (d, root) = app();
        put(
            &d,
            &format!("approvals/{ID}/blueprint.lock.json"),
            &lock(&sha('a')),
            0,
        );
        let repo = d.path().join("repo");
        std::fs::create_dir_all(repo.join(".foreman")).unwrap();
        std::fs::write(repo.join(".foreman/blueprint.lock.json"), lock(&sha('b'))).unwrap();
        let project = Root::open(&repo, RootKind::Project).unwrap();
        let mut got = retention::pinned_templates(&root, Some(&project), ID).unwrap();
        got.sort();
        assert_eq!(got, vec![sha('a'), sha('b')]);
        // The approved copy alone (the folder is unavailable).
        assert_eq!(
            retention::pinned_templates(&root, None, ID).unwrap(),
            vec![sha('a')]
        );
        // Never pinned: nothing.
        assert!(
            retention::pinned_templates(&root, None, "0b1c2d3e-0000-4000-8000-000000000002")
                .unwrap()
                .is_empty()
        );
    }

    /// A lock that can't be read stops the pass: with one unknown, nothing may be called unused.
    #[test]
    fn an_unreadable_lock_is_an_error() {
        let (d, root) = app();
        put(
            &d,
            &format!("approvals/{ID}/blueprint.lock.json"),
            "{ not json",
            0,
        );
        assert!(retention::pinned_templates(&root, None, ID).is_err());
    }
}
