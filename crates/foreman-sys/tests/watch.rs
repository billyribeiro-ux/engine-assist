//! M3-19: the watcher on a real file system (ADR-013, DR-162, SR-18, SR-31). Hints are
//! filtered by the exact interest set, re-read without following symlinks, and compared by stamp.
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

mod common;

use std::collections::HashMap;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::time::Duration;

use foreman_core::interest::Interest;
use foreman_core::store::{Root, RootKind};
use foreman_sys::watch::{self, Change, ProjectWatcher, WatchError};

fn repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let root = common::canon(d.path()).join("tradelog");
    std::fs::create_dir_all(root.join("docs/handoff")).unwrap();
    std::fs::create_dir_all(root.join(".foreman")).unwrap();
    (d, root)
}

fn changes(w: &ProjectWatcher, secs: u64) -> Vec<Change> {
    let mut v = Vec::new();
    let end = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < end {
        if let Some(c) = w.next(Duration::from_millis(100)) {
            v.push(c);
        }
    }
    v
}

fn write(root: &Path, rel: &str, body: &str) {
    std::fs::write(root.join(rel), body).unwrap();
}

/// Every file under `dir` with its bytes and modification time.
fn tree(dir: &Path) -> std::collections::BTreeMap<String, (Vec<u8>, std::time::SystemTime)> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            let m = std::fs::symlink_metadata(&p).unwrap();
            if m.is_dir() {
                stack.push(p);
            } else {
                out.insert(
                    p.display().to_string(),
                    (std::fs::read(&p).unwrap_or_default(), m.modified().unwrap()),
                );
            }
        }
    }
    out
}

/// This test process's child processes (Linux `/proc`; empty elsewhere).
fn children() -> Vec<String> {
    let Ok(tasks) = std::fs::read_dir("/proc/self/task") else {
        return Vec::new();
    };
    tasks
        .filter_map(|t| std::fs::read_to_string(t.ok()?.path().join("children")).ok())
        .flat_map(|s| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
        .collect()
}

mod watcher {
    use super::*;

    #[test]
    fn symlinked_root_refused() {
        let (d, root) = repo();
        let link = common::canon(d.path()).join("link");
        symlink(&root, &link).unwrap();
        assert!(matches!(
            ProjectWatcher::start(&link, vec![]),
            Err(WatchError::SymlinkedRoot(_))
        ));
        // A watch root below a symlinked component inside the project is refused too.
        symlink(root.join("docs"), root.join("docs-link")).unwrap();
        assert!(matches!(
            watch::check_root(&root.join("docs-link/handoff"), &root),
            Err(WatchError::SymlinkedRoot(_))
        ));
        assert!(watch::check_root(&root.join("docs/handoff"), &root).is_ok());
        assert!(watch::check_root(&root, &root).is_ok());
    }

    #[test]
    fn no_symlink_follow() {
        let (d, root) = repo();
        let outside = common::canon(d.path()).join("secret.md");
        std::fs::write(&outside, "=== HANDOFF PACKET\nfrom outside").unwrap();
        let w = ProjectWatcher::start(&root, vec![]).unwrap();
        symlink(&outside, root.join("docs/handoff/stage-02.md")).unwrap();
        let got = changes(&w, 2);
        assert!(
            got.iter().any(|c| matches!(
                c,
                Change::Refused {
                    interest: Interest::Packet { stage: 2 },
                    ..
                }
            )),
            "{got:?}"
        );
        assert!(
            !got.iter().any(|c| matches!(c, Change::Changed { .. })),
            "{got:?}"
        );
    }

    #[test]
    fn ignores_tmp_and_secrets() {
        let (_d, root) = repo();
        let w = ProjectWatcher::start(&root, vec!["docs/.env".into()]).unwrap();
        write(
            &root,
            "docs/handoff/.stage-02.md.0123456789abcdef.foreman-tmp",
            "x",
        );
        write(&root, "docs/.env", "TOKEN=1");
        write(&root, "docs/handoff/stage-02.md.bak", "x");
        write(&root, "docs/handoff/stage-2.md", "x");
        assert!(changes(&w, 1).is_empty());
        write(&root, "docs/handoff/stage-02.md", "=== HANDOFF PACKET");
        let got = changes(&w, 2);
        assert!(
            matches!(
                got.as_slice(),
                [Change::Changed {
                    interest: Interest::Packet { stage: 2 },
                    ..
                }]
            ),
            "{got:?}"
        );
    }

    /// Editors save by rename; the content change is seen once; an identical rewrite isn't.
    #[test]
    fn atomic_save_detected() {
        let (_d, root) = repo();
        write(&root, "docs/lessons.md", "one");
        let w = ProjectWatcher::start(&root, vec![]).unwrap();
        write(&root, "docs/.lessons.md.swp", "two");
        std::fs::rename(
            root.join("docs/.lessons.md.swp"),
            root.join("docs/lessons.md"),
        )
        .unwrap();
        let got = changes(&w, 2);
        assert!(
            got.iter().any(|c| matches!(
                c,
                Change::Changed {
                    interest: Interest::Lessons,
                    ..
                }
            )),
            "{got:?}"
        );
        write(&root, ".foreman/project.json", "{}");
        let got = changes(&w, 2);
        assert!(
            got.iter().any(|c| matches!(
                c,
                Change::Changed {
                    interest: Interest::Decision { .. },
                    ..
                }
            )),
            "{got:?}"
        );
    }

    /// ADR-013: a missed event can't hide a change past the next re-stat (focus, open, launch).
    #[test]
    fn refocus_rescan_catches_missed_event() {
        let (_d, root) = repo();
        let r = Root::open(&root, RootKind::Project).unwrap();
        let mut known = HashMap::new();
        let first = watch::restat(&r, &[1, 2], &[], &mut known);
        assert!(first.iter().all(|c| matches!(c, Change::Changed { .. })));
        assert!(watch::restat(&r, &[1, 2], &[], &mut known).is_empty());
        write(&root, "docs/handoff/stage-02.md", "packet");
        let again = watch::restat(&r, &[1, 2], &[], &mut known);
        assert!(
            matches!(
                again.as_slice(),
                [Change::Changed {
                    interest: Interest::Packet { stage: 2 },
                    ..
                }]
            ),
            "{again:?}"
        );
    }

    /// SR-18 (code review W8: a runtime proof, not a text search): the watcher turns every kind
    /// of change into data and does nothing else. While it reports a packet, a decision file, an
    /// `out` file and the lessons file, the project tree (`.git` included) stays exactly as the
    /// test left it and no child process exists.
    #[test]
    fn events_never_act() {
        let (_d, root) = repo();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("spec")).unwrap();
        write(&root, ".git/HEAD", "ref: refs/heads/main\n");
        let w = ProjectWatcher::start(&root, vec!["spec/out.md".into()]).unwrap();
        write(
            &root,
            "docs/handoff/stage-02.md",
            "=== HANDOFF PACKET\nbody\n",
        );
        write(&root, ".foreman/project.json", "{}\n");
        write(&root, "spec/out.md", "# out\n");
        write(&root, "docs/lessons.md", "# lessons\n");
        let before = tree(&root);
        let mut seen = std::collections::BTreeSet::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while seen.len() < 4 {
            assert!(std::time::Instant::now() < deadline, "only saw {seen:?}");
            if let Some(Change::Changed { interest, .. }) = w.next(Duration::from_millis(100)) {
                seen.insert(match interest {
                    Interest::Packet { .. } => "packet",
                    Interest::Decision { .. } => "decision",
                    Interest::Out { .. } => "out",
                    Interest::Lessons => "lessons",
                    _ => "other",
                });
            }
        }
        assert_eq!(tree(&root), before, "the watcher changed the project");
        assert!(
            children().is_empty(),
            "the watcher spawned {:?}",
            children()
        );
    }
}
