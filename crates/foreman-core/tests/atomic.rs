//! M1-05, M1-09: atomic durable writes that never clobber a newer file (ADR-010, SR-32, SR-33),
//! and the `info/exclude` writer.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "test code may use std::fs and run git (design doc §3.2)"
)]

mod common;

use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::Path;
use std::time::{Duration, SystemTime};

use foreman_core::pathpolicy::{self, PolicyViolation, ReadClass, WritePurpose, check_write};
use foreman_core::store::{
    FaultPoint, ReadStamp, Root, RootKind, StoreError, WriteOptions, is_temp_name,
};

fn project() -> (tempfile::TempDir, Root) {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("docs/handoff")).unwrap();
    let r = Root::open(d.path(), RootKind::Project).unwrap();
    (d, r)
}

fn p(s: &str) -> pathpolicy::RepoPath {
    check_write(s, WritePurpose::Repo).unwrap()
}

fn temps(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| is_temp_name(n))
        .collect()
}

mod atomic {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]
        /// AC-F020-01 / NFR-10: a write killed at every step leaves the complete old or complete
        /// new content, never a torn file and never a temp in the target's place; a later write
        /// ("after restart") succeeds.
        #[test]
        fn crash_injection(old in proptest::collection::vec(any::<u8>(), 0..2048),
                           new in proptest::collection::vec(any::<u8>(), 0..2048),
                           at in 0usize..7, existing in any::<bool>(), no_swap in any::<bool>()) {
            let (d, root) = project();
            let path = p("docs/handoff/stage-02.md");
            let file = d.path().join("docs/handoff/stage-02.md");
            let stamp = if existing {
                std::fs::write(&file, &old).unwrap();
                root.read(&path, ReadClass::Decision).unwrap().1
            } else {
                ReadStamp::Absent
            };
            let opts = WriteOptions { fault: Some(FaultPoint::ALL[at]), force_no_swap: no_swap, ..WriteOptions::default() };
            let r = root.write_with(&path, &new, &stamp, &opts);
            prop_assert!(matches!(r, Err(StoreError::Injected(_))));
            let now = std::fs::read(&file).ok();
            let ok = match &now {
                None => !existing,
                Some(b) => *b == new || (existing && *b == old),
            };
            prop_assert!(ok, "torn or lost: {:?}", now.map(|b| b.len()));
            // Restart: the next write cleans up and lands (stale temps from the "crash" are older
            // than the threshold in this simulation).
            let app = tempfile::tempdir().unwrap();
            let root = Root::open(d.path(), RootKind::Project).unwrap().with_recovery(app.path(), "p");
            let (_, stamp) = root.read_opt(&path, ReadClass::Decision).unwrap();
            let opts = WriteOptions { stale_after: Duration::ZERO, ..WriteOptions::default() };
            let r = root.write_with(&path, b"after", &stamp, &opts).unwrap();
            prop_assert_eq!(std::fs::read(&file).unwrap(), b"after".to_vec());
            // A leftover that differed from the target was kept in recovered/, never deleted.
            for rec in &r.recovered {
                let kept = std::fs::read(app.path().join(rec)).unwrap();
                prop_assert!(new.starts_with(&kept) || kept == old, "a torn temp of ours or the swapped-out old file");
            }
            prop_assert!(temps(&d.path().join("docs/handoff")).is_empty());
        }

        /// AC-F020-03 / SR-33: with interleaved external writes, Foreman never overwrites a file
        /// changed after its last read.
        #[test]
        fn refuses_newer_file(ops in proptest::collection::vec((any::<bool>(), proptest::collection::vec(any::<u8>(), 1..64)), 1..12)) {
            let (d, root) = project();
            let path = p("docs/lessons.md");
            let file = d.path().join("docs/lessons.md");
            std::fs::write(&file, b"v0").unwrap();
            let (_, mut stamp) = root.read(&path, ReadClass::Decision).unwrap();
            let mut expected = b"v0".to_vec();
            for (external, bytes) in ops {
                if external {
                    // Someone else edits after Foreman's read.
                    std::fs::write(&file, &bytes).unwrap();
                    expected.clone_from(&bytes);
                    let r = root.write(&path, b"foreman", &stamp);
                    let changed = Some(foreman_core::store::sha256_hex(&bytes)) != stamp.sha256().map(str::to_owned);
                    if changed {
                        prop_assert!(matches!(r, Err(StoreError::Conflict { .. })), "{:?}", r);
                        prop_assert_eq!(std::fs::read(&file).unwrap(), expected.clone());
                    } else if r.is_ok() {
                        expected = b"foreman".to_vec();
                    }
                    let (_, s) = root.read(&path, ReadClass::Decision).unwrap();
                    stamp = s;
                } else {
                    stamp = root.write(&path, &bytes, &stamp).unwrap();
                    expected = bytes;
                }
                prop_assert_eq!(std::fs::read(&file).unwrap(), expected.clone());
            }
        }
    }

    /// The swap race itself: an edit that lands between Foreman's check and the rename is swapped
    /// back and reported (a fault-free simulation: the stamp is stale by one external write).
    #[test]
    fn refuses_newer_file_between_check_and_swap() {
        let (d, root) = project();
        let path = p("docs/lessons.md");
        std::fs::write(d.path().join("docs/lessons.md"), "mine").unwrap();
        let (_, stamp) = root.read(&path, ReadClass::Decision).unwrap();
        std::fs::write(d.path().join("docs/lessons.md"), "theirs").unwrap();
        for no_swap in [false, true] {
            let opts = WriteOptions {
                force_no_swap: no_swap,
                ..WriteOptions::default()
            };
            let e = root
                .write_with(&path, b"foreman", &stamp, &opts)
                .unwrap_err();
            let StoreError::Conflict { name, current } = e else {
                panic!()
            };
            assert_eq!(name, "docs/lessons.md");
            assert_eq!(
                current.sha256(),
                Some(foreman_core::store::sha256_hex(b"theirs").as_str())
            );
            assert_eq!(
                std::fs::read_to_string(d.path().join("docs/lessons.md")).unwrap(),
                "theirs"
            );
        }
        assert!(temps(&d.path().join("docs")).is_empty());
    }

    /// AC-F020-02: the temp is `.<name>.<16 hex>.foreman-tmp` created `O_EXCL|O_NOFOLLOW`; a planted
    /// symlink at the temp path is refused and its target untouched.
    #[test]
    fn refuses_planted_tmp_symlink() {
        let (d, root) = project();
        let victim = tempfile::tempdir().unwrap();
        let zshrc = victim.path().join(".zshrc");
        std::fs::write(&zshrc, "safe").unwrap();
        let suffix = "0123456789abcdef";
        let tmp = d
            .path()
            .join(format!("docs/handoff/.stage-02.md.{suffix}.foreman-tmp"));
        symlink(&zshrc, &tmp).unwrap();
        let opts = WriteOptions {
            temp_suffix: Some(suffix.into()),
            ..WriteOptions::default()
        };
        let e = root
            .write_with(
                &p("docs/handoff/stage-02.md"),
                b"pwn",
                &ReadStamp::Absent,
                &opts,
            )
            .unwrap_err();
        assert!(matches!(e, StoreError::Io { .. }), "{e:?}");
        assert_eq!(std::fs::read_to_string(&zshrc).unwrap(), "safe");
        assert!(!d.path().join("docs/handoff/stage-02.md").exists());
        // A normal write: random temp name, gone afterwards, mode 0644 for a new repo file.
        root.write(&p("docs/handoff/stage-02.md"), b"ok", &ReadStamp::Absent)
            .unwrap();
        let md = std::fs::metadata(d.path().join("docs/handoff/stage-02.md")).unwrap();
        assert_eq!(md.permissions().mode() & 0o777, 0o644);
        assert_eq!(
            temps(&d.path().join("docs/handoff")).len(),
            1,
            "only the planted symlink remains"
        );
    }

    /// `NOREPLACE` for new files: someone creating the file meanwhile is a conflict.
    #[test]
    fn noreplace_conflict() {
        let (d, root) = project();
        let path = p("docs/handoff/stage-01.md");
        std::fs::write(
            d.path().join("docs/handoff/stage-01.md"),
            "claude wrote this",
        )
        .unwrap();
        let e = root
            .write(&path, b"foreman", &ReadStamp::Absent)
            .unwrap_err();
        assert!(matches!(e, StoreError::Conflict { .. }));
        assert_eq!(
            std::fs::read_to_string(d.path().join("docs/handoff/stage-01.md")).unwrap(),
            "claude wrote this"
        );
        // A file that vanished after Foreman read it is a conflict too.
        let (_, stamp) = root.read(&path, ReadClass::Decision).unwrap();
        std::fs::remove_file(d.path().join("docs/handoff/stage-01.md")).unwrap();
        assert!(matches!(
            root.write(&path, b"x", &stamp),
            Err(StoreError::Conflict { .. })
        ));
        // AC-F020-04: an OS error names the file and leaves the original intact.
        std::fs::write(d.path().join("docs/blocker"), "a file, not a folder").unwrap();
        let e = root
            .write(&p("docs/blocker/x.md"), b"x", &ReadStamp::Absent)
            .unwrap_err();
        assert!(e.to_string().contains("blocker"), "{e}");
        assert_eq!(
            std::fs::read_to_string(d.path().join("docs/blocker")).unwrap(),
            "a file, not a folder"
        );
        assert!(temps(&d.path().join("docs/handoff")).is_empty());
    }

    /// ADR-010 §2.7: without swap support, re-check then plain rename (`degraded-cas`).
    #[test]
    fn swap_unsupported_fallback() {
        let (d, root) = project();
        let path = p("docs/lessons.md");
        std::fs::write(d.path().join("docs/lessons.md"), "v1").unwrap();
        let (_, stamp) = root.read(&path, ReadClass::Decision).unwrap();
        let opts = WriteOptions {
            force_no_swap: true,
            ..WriteOptions::default()
        };
        let out = root.write_with(&path, b"v2", &stamp, &opts).unwrap();
        assert!(out.degraded_cas);
        assert_eq!(
            std::fs::read_to_string(d.path().join("docs/lessons.md")).unwrap(),
            "v2"
        );
        // Stale stamp is still refused in the fallback.
        let e = root.write_with(&path, b"v3", &stamp, &opts).unwrap_err();
        assert!(matches!(e, StoreError::Conflict { .. }));
        // With swap support (this volume), the normal path isn't degraded.
        let (_, s) = root.read(&path, ReadClass::Decision).unwrap();
        let out = root
            .write_with(&path, b"v4", &s, &WriteOptions::default())
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(d.path().join("docs/lessons.md")).unwrap(),
            "v4"
        );
        let _ = out.degraded_cas; // false on APFS/ext4/tmpfs; overlayfs may lack EXCHANGE.
    }

    /// ADR-010 §2.6: if the directory full-sync is unsupported, plain fsync is used and logged.
    #[test]
    fn dir_sync_fallback() {
        let (d, root) = project();
        let opts = WriteOptions {
            force_no_dir_sync: true,
            ..WriteOptions::default()
        };
        let out = root
            .write_with(&p("docs/a.md"), b"x", &ReadStamp::Absent, &opts)
            .unwrap();
        assert!(out.dir_sync_fallback);
        let out = root
            .write_with(
                &p("docs/b.md"),
                b"x",
                &ReadStamp::Absent,
                &WriteOptions::default(),
            )
            .unwrap();
        assert!(!out.dir_sync_fallback);
        assert_eq!(
            std::fs::read_to_string(d.path().join("docs/a.md")).unwrap(),
            "x"
        );
    }

    /// ADR-010: a stale `.foreman-tmp` that differs from its target goes to `<app-data>/recovered/`
    /// and is shown, never deleted; one equal to its target is removed; fresh temps are left alone.
    #[test]
    fn leftover_temp_differs_moved_to_recovered() {
        let (d, _) = project();
        let app = tempfile::tempdir().unwrap();
        let root = Root::open(d.path(), RootKind::Project)
            .unwrap()
            .with_recovery(app.path(), "3f6a1c2e-8b4d-4e7f-9a1b-2c3d4e5f6a7b");
        let dir = d.path().join("docs/handoff");
        std::fs::write(dir.join("stage-02.md"), "current").unwrap();
        let old = SystemTime::now() - Duration::from_secs(7200);
        let differ = dir.join(".stage-02.md.aaaaaaaaaaaaaaaa.foreman-tmp");
        std::fs::write(&differ, "newer external edit").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&differ)
            .unwrap()
            .set_modified(old)
            .unwrap();
        let same = dir.join(".stage-02.md.bbbbbbbbbbbbbbbb.foreman-tmp");
        std::fs::write(&same, "current").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&same)
            .unwrap()
            .set_modified(old)
            .unwrap();
        let fresh = dir.join(".stage-02.md.cccccccccccccccc.foreman-tmp");
        std::fs::write(&fresh, "being written right now").unwrap();

        let path = p("docs/handoff/stage-02.md");
        let (_, stamp) = root.read(&path, ReadClass::Decision).unwrap();
        let out = root
            .write_with(&path, b"new", &stamp, &WriteOptions::default())
            .unwrap();
        assert_eq!(out.recovered.len(), 1);
        assert_eq!(out.removed_stale.len(), 1);
        assert!(
            out.recovered[0].starts_with("recovered/")
                && out.recovered[0].ends_with("-3f6a1c2e-8b4d-4e7f-9a1b-2c3d4e5f6a7b-stage-02.md")
        );
        let rec = app.path().join(&out.recovered[0]);
        assert_eq!(
            std::fs::read_to_string(&rec).unwrap(),
            "newer external edit"
        );
        assert_eq!(
            std::fs::metadata(&rec).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(!differ.exists() && !same.exists() && fresh.exists());
        // Without a recovery target, a differing temp is left in place (never discarded).
        let root2 = Root::open(d.path(), RootKind::Project).unwrap();
        let lone = dir.join(".x.md.dddddddddddddddd.foreman-tmp");
        std::fs::write(&lone, "keep").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&lone)
            .unwrap()
            .set_modified(old)
            .unwrap();
        root2
            .write(&p("docs/handoff/x.md"), b"y", &ReadStamp::Absent)
            .unwrap();
        assert!(lone.exists());
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim_end().to_owned()
    }

    fn init(dir: &Path) {
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["config", "user.email", "t@example.com"]);
        git(dir, &["config", "user.name", "T"]);
    }

    fn ensure(
        dir: &Path,
    ) -> Result<
        foreman_core::store::git_exclude::Ensured,
        foreman_core::store::git_exclude::ExcludeError,
    > {
        let root = Root::open(dir, RootKind::Project).unwrap();
        let ex = git(dir, &["rev-parse", "--git-path", "info/exclude"]);
        let common = git(dir, &["rev-parse", "--git-common-dir"]);
        foreman_core::store::git_exclude::ensure(&root, Path::new(&ex), Path::new(&common))
    }

    /// AC-F020-13: normal repo, linked worktree, a `.git` file pointing outside; idempotent; symlinks
    /// refused; only `info/exclude` under `.git`.
    #[test]
    fn exclude_writer_scope() {
        use foreman_core::store::git_exclude::{Ensured, ExcludeError};
        // Normal repo, twice.
        let d = tempfile::tempdir().unwrap();
        init(d.path());
        assert_eq!(ensure(d.path()).unwrap(), Ensured::Appended);
        assert_eq!(ensure(d.path()).unwrap(), Ensured::AlreadyPresent);
        let text = std::fs::read_to_string(d.path().join(".git/info/exclude")).unwrap();
        assert_eq!(text.matches(".*.foreman-tmp").count(), 1);
        assert!(text.ends_with(".*.foreman-tmp\n"));

        // Linked worktree: info/exclude lives in the common dir.
        std::fs::write(d.path().join("a.txt"), "a").unwrap();
        git(d.path(), &["add", "a.txt"]);
        git(d.path(), &["commit", "-qm", "init"]);
        let wt_parent = tempfile::tempdir().unwrap();
        let wt = wt_parent.path().join("wt");
        git(d.path(), &["worktree", "add", "-q", wt.to_str().unwrap()]);
        std::fs::write(d.path().join(".git/info/exclude"), "# fresh\n").unwrap();
        assert_eq!(ensure(&wt).unwrap(), Ensured::Appended);
        assert_eq!(
            std::fs::read_to_string(d.path().join(".git/info/exclude")).unwrap(),
            "# fresh\n.*.foreman-tmp\n"
        );

        // A path git reports outside the project and its common dir is refused.
        let outside = tempfile::tempdir().unwrap();
        let root = Root::open(d.path(), RootKind::Project).unwrap();
        let e = foreman_core::store::git_exclude::ensure(
            &root,
            &outside.path().join("info/exclude"),
            &d.path().join(".git"),
        )
        .unwrap_err();
        assert!(matches!(e, ExcludeError::OutsideScope(_)), "{e:?}");
        // A `.git` file pointing outside the project: the common dir is elsewhere, and the writer
        // accepts only that common dir's info/exclude (never a third location).
        let gitfile = tempfile::tempdir().unwrap();
        let real_git = outside.path().join("real.git");
        let o = std::process::Command::new("git")
            .args(["init", "-q", "--separate-git-dir"])
            .arg(&real_git)
            .arg(gitfile.path())
            .output()
            .unwrap();
        assert!(o.status.success());
        assert_eq!(ensure(gitfile.path()).unwrap(), Ensured::Appended);
        let root = Root::open(gitfile.path(), RootKind::Project).unwrap();
        let e = foreman_core::store::git_exclude::ensure(
            &root,
            &outside.path().join("elsewhere/info/exclude"),
            &real_git,
        )
        .unwrap_err();
        assert!(matches!(e, ExcludeError::OutsideScope(_)));
        // Not info/exclude → refused even inside the git dir (it's the only write under .git).
        let e = foreman_core::store::git_exclude::ensure(
            &root,
            &real_git.join("hooks/pre-commit"),
            &real_git,
        )
        .unwrap_err();
        assert!(matches!(e, ExcludeError::OutsideScope(_)));

        // A symlinked info/exclude or info/ is refused.
        let s = tempfile::tempdir().unwrap();
        init(s.path());
        let target = outside.path().join("victim");
        std::fs::write(&target, "x").unwrap();
        std::fs::remove_file(s.path().join(".git/info/exclude")).ok();
        symlink(&target, s.path().join(".git/info/exclude")).unwrap();
        let e = ensure(s.path()).unwrap_err();
        assert!(
            matches!(
                e,
                ExcludeError::Store(StoreError::Policy(PolicyViolation::Symlink(_)))
            ),
            "{e:?}"
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "x");
        // Relative `..` from git is refused as not plain.
        let root = Root::open(s.path(), RootKind::Project).unwrap();
        assert!(matches!(
            foreman_core::store::git_exclude::ensure(
                &root,
                Path::new(".git/../../x/info/exclude"),
                Path::new(".git")
            ),
            Err(ExcludeError::NotPlain(_))
        ));
    }

    /// AC-F020-02 / T-061: Foreman's temp files don't show up in git.
    #[test]
    fn temp_excluded_from_git() {
        let d = tempfile::tempdir().unwrap();
        init(d.path());
        ensure(d.path()).unwrap();
        std::fs::create_dir_all(d.path().join("docs")).unwrap();
        std::fs::write(
            d.path()
                .join("docs/.lessons.md.0123456789abcdef.foreman-tmp"),
            "x",
        )
        .unwrap();
        std::fs::write(d.path().join("docs/lessons.md"), "y").unwrap();
        let st = git(d.path(), &["ls-files", "--others", "--exclude-standard"]);
        assert!(st.contains("docs/lessons.md"), "{st}");
        assert!(!st.contains("foreman-tmp"), "{st}");
    }
}
