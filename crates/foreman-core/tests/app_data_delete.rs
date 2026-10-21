//! M4-02 / M3-26: the store's app-data deletes (Clear run history, project removal, tail pruning)
//! and the user-export writer (`WritePurpose::UserExport`), all through the core's path policy.
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

use std::os::unix::fs::symlink;
use std::path::Path;

use foreman_core::pathpolicy::{self, PolicyViolation};
use foreman_core::store::app_dirs::{remove_file, remove_tree, tree_size};
use foreman_core::store::export::write_user_export;
use foreman_core::store::{Root, StoreError};

fn app() -> (tempfile::TempDir, Root) {
    let d = tempfile::tempdir().unwrap();
    let root = Root::open_app_data(&d.path().join("app")).unwrap();
    (d, root)
}

fn put(d: &tempfile::TempDir, rel: &str, text: &str) {
    let p = d.path().join("app").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn exists(d: &tempfile::TempDir, rel: &str) -> bool {
    d.path().join("app").join(rel).symlink_metadata().is_ok()
}

mod store_delete {
    use super::*;

    #[test]
    fn tree_goes_and_siblings_stay() {
        let (d, root) = app();
        put(&d, "runs/a/run.json", "1");
        put(&d, "runs/a/deep/x/output.log", "22");
        put(&d, "runs/index-2026-10.jsonl", "333");
        put(&d, "projects/keep/progress.json", "4444");
        assert_eq!(tree_size(&root, "runs").unwrap(), 6);
        assert!(remove_tree(&root, "runs").unwrap());
        assert!(!exists(&d, "runs"));
        assert!(exists(&d, "projects/keep/progress.json"));
        assert!(!remove_tree(&root, "runs").unwrap(), "already gone is fine");
        assert_eq!(tree_size(&root, "runs").unwrap(), 0);
    }

    #[test]
    fn symlinks_are_never_followed() {
        let (d, root) = app();
        let outside = d.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("precious.txt"), "keep me").unwrap();
        put(&d, "runs/a/run.json", "1");
        symlink(&outside, d.path().join("app/runs/a/link")).unwrap();
        symlink(
            outside.join("precious.txt"),
            d.path().join("app/runs/file-link"),
        )
        .unwrap();
        assert_eq!(tree_size(&root, "runs").unwrap(), 1, "links add no size");
        assert!(remove_tree(&root, "runs").unwrap());
        assert!(!exists(&d, "runs"));
        assert_eq!(
            std::fs::read_to_string(outside.join("precious.txt")).unwrap(),
            "keep me"
        );
    }

    #[test]
    fn a_symlinked_folder_is_unlinked_not_entered() {
        let (d, root) = app();
        let outside = d.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("x.txt"), "x").unwrap();
        symlink(&outside, d.path().join("app/runs")).unwrap();
        assert!(remove_tree(&root, "runs").unwrap());
        assert!(outside.join("x.txt").exists());
        assert!(!exists(&d, "runs"));
    }

    #[test]
    fn a_symlink_parent_is_refused() {
        let (d, root) = app();
        let outside = d.path().join("outside");
        std::fs::create_dir_all(outside.join("p")).unwrap();
        std::fs::write(outside.join("p/x.txt"), "x").unwrap();
        symlink(&outside, d.path().join("app/projects")).unwrap();
        assert!(matches!(
            remove_tree(&root, "projects/p"),
            Err(StoreError::Policy(PolicyViolation::Symlink(_)))
        ));
        assert!(matches!(
            remove_file(&root, "projects/p/x.txt"),
            Err(StoreError::Policy(PolicyViolation::Symlink(_)))
        ));
        assert!(outside.join("p/x.txt").exists());
    }

    #[test]
    fn files_and_the_policy() {
        let (d, root) = app();
        put(&d, "approvals/p.json", "{}");
        assert!(remove_file(&root, "approvals/p.json").unwrap());
        assert!(!remove_file(&root, "approvals/p.json").unwrap());
        assert!(!remove_file(&root, "nowhere/p.json").unwrap());
        put(&d, "approvals/p/lock.json", "{}");
        assert!(matches!(
            remove_file(&root, "approvals/p"),
            Err(StoreError::NotRegular(_))
        ));
        for bad in ["../x", "/etc", "a//b", "a/../b", ""] {
            assert!(remove_tree(&root, bad).is_err(), "{bad:?}");
            assert!(remove_file(&root, bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn only_app_data_roots() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("repo/runs")).unwrap();
        let repo = Root::open(
            &d.path().join("repo"),
            foreman_core::store::RootKind::Project,
        )
        .unwrap();
        assert!(remove_tree(&repo, "runs").is_err());
        assert!(d.path().join("repo/runs").exists());
    }
}

/// Mac check 6 (ADR-011, SR-53; `docs/progress.md` Mac checks): what the symlink-safe deletes do on
/// APFS, which is normalization-insensitive and (by default) case-insensitive, has its own
/// `unlinkat`/`utimensat` behavior, and litters folders with Finder files. Linux can't show any of
/// it, so the module exists on macOS only; run `cargo test -p foreman-core --test app_data_delete
/// apfs`.
#[cfg(target_os = "macos")]
mod apfs {
    use super::*;
    use foreman_core::store::app_dirs::{modified_secs, touch};

    /// The volume's case sensitivity, probed rather than assumed.
    fn case_insensitive(d: &tempfile::TempDir) -> bool {
        let a = d.path().join("CaseProbe");
        std::fs::write(&a, "x").unwrap();
        d.path().join("caseprobe").exists()
    }

    #[test]
    fn name_variants_follow_the_volume() {
        let (d, root) = app();
        let insensitive = case_insensitive(&d);
        put(&d, "approvals/p.json", "{}");
        // A case variant reaches the same entry on a case-insensitive volume and none otherwise;
        // in both cases the delete stays inside the app-data folder.
        let hit = remove_file(&root, "approvals/P.json").unwrap();
        assert_eq!(hit, insensitive, "case-insensitive volume: {insensitive}");
        assert_eq!(exists(&d, "approvals/p.json"), !insensitive);
        // A decomposed name (NFD) and its precomposed spelling (NFC) are one entry on APFS, which
        // preserves what was written and matches either spelling.
        let nfd = "approvals/e\u{301}.json";
        let nfc = "approvals/\u{e9}.json";
        put(&d, nfd, "{}");
        assert!(tree_size(&root, "approvals").unwrap() >= 2);
        assert!(remove_file(&root, nfc).unwrap() || remove_file(&root, nfd).unwrap());
        assert!(remove_tree(&root, "approvals").unwrap());
        assert!(!exists(&d, "approvals"));
    }

    #[test]
    fn finder_litter_and_open_files_go_with_the_tree() {
        let (d, root) = app();
        put(&d, "runs/a/run.json", "1");
        put(&d, "runs/.DS_Store", "finder");
        put(&d, "runs/a/._run.json", "appledouble");
        let open = std::fs::File::open(d.path().join("app/runs/a/run.json")).unwrap();
        assert!(remove_tree(&root, "runs").unwrap());
        assert!(!exists(&d, "runs"));
        // The unlinked file stays readable through the open handle (unlinkat semantics).
        drop(open);
    }

    #[test]
    fn sparse_files_count_their_logical_size() {
        let (d, root) = app();
        put(&d, "events/e/big", "");
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(d.path().join("app/events/e/big"))
            .unwrap();
        f.set_len(8 * 1024 * 1024).unwrap();
        assert_eq!(tree_size(&root, "events").unwrap(), 8 * 1024 * 1024);
        assert!(remove_tree(&root, "events").unwrap());
    }

    #[test]
    fn touch_sets_whole_seconds_without_following_a_link() {
        let (d, root) = app();
        put(&d, "templates/x.md", "text");
        assert!(touch(&root, "templates/x.md", 1_700_000_000).unwrap());
        assert_eq!(
            modified_secs(&root, "templates/x.md").unwrap(),
            Some(1_700_000_000)
        );
        assert_eq!(
            std::fs::read_to_string(d.path().join("app/templates/x.md")).unwrap(),
            "text"
        );
        let outside = d.path().join("outside.md");
        std::fs::write(&outside, "precious").unwrap();
        let before = std::fs::metadata(&outside).unwrap().modified().unwrap();
        symlink(&outside, d.path().join("app/templates/link.md")).unwrap();
        assert!(matches!(
            touch(&root, "templates/link.md", 1_600_000_000),
            Err(StoreError::NotRegular(_))
        ));
        assert_eq!(
            std::fs::metadata(&outside).unwrap().modified().unwrap(),
            before,
            "the link's target is never touched"
        );
    }
}

mod user_export {
    use super::*;

    fn dirs() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let out = d.path().join("Desktop");
        let app = d.path().join("app-data");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::create_dir_all(&app).unwrap();
        (d, out, app)
    }

    #[test]
    fn writes_a_md_file_and_replaces_one() {
        let (_d, out, app) = dirs();
        let t = out.join("metrics.md");
        write_user_export(&t, b"# one", &app).unwrap();
        assert_eq!(std::fs::read_to_string(&t).unwrap(), "# one");
        write_user_export(&t, b"# two", &app).unwrap();
        assert_eq!(std::fs::read_to_string(&t).unwrap(), "# two");
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(&t).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn md_only() {
        let (_d, out, app) = dirs();
        for name in [
            "metrics.txt",
            "metrics",
            ".md",
            "metrics.md.sh",
            "METRICS.MD.json",
        ] {
            let r = write_user_export(&out.join(name), b"x", &app);
            assert!(
                matches!(r, Err(StoreError::Policy(PolicyViolation::NotMarkdown))),
                "{name}: {r:?}"
            );
            assert!(!out.join(name).exists());
        }
        write_user_export(&out.join("Metrics.MD"), b"x", &app).unwrap();
    }

    #[test]
    fn protected_folders_are_refused() {
        let (d, _out, app) = dirs();
        for dir in [
            ".git",
            ".GIT",
            ".claude",
            "repo/.foreman",
            "repo/.claude/agents",
        ] {
            let p = d.path().join(dir);
            std::fs::create_dir_all(&p).unwrap();
            let r = write_user_export(&p.join("m.md"), b"x", &app);
            assert!(
                matches!(
                    r,
                    Err(StoreError::Policy(PolicyViolation::DeniedComponent(_)))
                ),
                "{dir}: {r:?}"
            );
            assert!(!p.join("m.md").exists());
        }
    }

    #[test]
    fn protected_names_are_refused() {
        let (_d, out, app) = dirs();
        for name in ["CLAUDE.md", "claude.md", "CLAUDE.local.md"] {
            assert!(
                write_user_export(&out.join(name), b"x", &app).is_err(),
                "{name}"
            );
            assert!(!out.join(name).exists());
        }
    }

    #[test]
    fn app_data_is_refused() {
        let (_d, _out, app) = dirs();
        std::fs::create_dir_all(app.join("templates")).unwrap();
        let r = write_user_export(&app.join("templates/abc.md"), b"x", &app);
        assert!(
            matches!(r, Err(StoreError::Policy(PolicyViolation::AppDataFolder))),
            "{r:?}"
        );
        assert!(!app.join("templates/abc.md").exists());
    }

    #[test]
    fn a_symlink_target_is_refused_and_left_alone() {
        let (d, out, app) = dirs();
        let secret = d.path().join("elsewhere.md");
        std::fs::write(&secret, "mine").unwrap();
        symlink(&secret, out.join("metrics.md")).unwrap();
        assert!(write_user_export(&out.join("metrics.md"), b"x", &app).is_err());
        assert_eq!(std::fs::read_to_string(&secret).unwrap(), "mine");
    }

    #[test]
    fn a_symlinked_folder_is_followed_because_billy_chose_it() {
        // ADR-011: the root's own ancestors may be symlinks, because Billy chose the path.
        let (d, out, app) = dirs();
        let link = d.path().join("link");
        symlink(&out, &link).unwrap();
        write_user_export(&link.join("metrics.md"), b"x", &app).unwrap();
        assert!(out.join("metrics.md").exists());
    }

    #[test]
    fn lexical_rules() {
        let app = Path::new("/app");
        let dir = Path::new("/home/b/Desktop");
        assert!(pathpolicy::check_user_export(dir, "m.md", app).is_ok());
        assert_eq!(
            pathpolicy::check_user_export(Path::new("rel"), "m.md", app),
            Err(PolicyViolation::Absolute)
        );
        assert!(pathpolicy::check_user_export(dir, "a/b.md", app).is_err());
        assert!(pathpolicy::check_user_export(dir, "..", app).is_err());
        assert!(
            pathpolicy::check_user_export(dir, &format!("{}.md", "a".repeat(300)), app).is_err()
        );
        assert!(pathpolicy::check_user_export(dir, "m\u{202e}.md", app).is_err());
        assert!(pathpolicy::check_user_export(dir, "m\n.md", app).is_err());
        let long = (0..5).fold(Path::new("/home").to_path_buf(), |p, _| {
            p.join("d".repeat(250))
        });
        assert!(matches!(
            pathpolicy::check_user_export(&long, "m.md", app),
            Err(PolicyViolation::TooLong(_))
        ));
    }
}
