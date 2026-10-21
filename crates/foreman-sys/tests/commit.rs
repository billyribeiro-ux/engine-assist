//! M1-10 (and M1-28's decision-file cases): the plumbing commit with read-back (SR-42, DR-142–DR-145).
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

use std::path::Path;

use foreman_core::pathpolicy::{self, DecisionPath, ReadClass, WritePurpose};
use foreman_core::store::{ReadStamp, Root, RootKind, sha256_hex};
use foreman_sys::git::commit::{
    CommitError, CommitOpts, CommitOutcome, Reviewed, commit_paths, verify_readback,
};

fn rp(s: &str) -> pathpolicy::RepoPath {
    pathpolicy::check_write(s, WritePurpose::Repo)
        .or_else(|_| pathpolicy::check_write(s, WritePurpose::DecisionFile(DecisionPath::Project)))
        .unwrap()
}

fn write(root: &Root, path: &str, body: &str) -> Reviewed {
    let p = rp(path);
    let (_, stamp) = root.read_opt(&p, ReadClass::Decision).unwrap();
    let st = root.write(&p, body.as_bytes(), &stamp).unwrap();
    Reviewed {
        path: p,
        sha256: st.sha256().unwrap().to_owned(),
    }
}

fn opts(app: &Path) -> CommitOpts<'static> {
    CommitOpts {
        app_data: app.to_path_buf(),
        ..CommitOpts::default()
    }
}

/// A repo with unrelated staged and unstaged changes (the staged-changes fixture).
fn staged_changes_repo(dir: &Path) {
    common::init_repo(dir);
    common::commit_file(dir, "src/app.rs", "fn main() {}\n");
    common::commit_file(dir, "docs/notes.md", "notes\n");
    std::fs::write(dir.join("src/app.rs"), "fn main() { staged(); }\n").unwrap();
    common::git(dir, &["add", "src/app.rs"]);
    std::fs::write(
        dir.join("src/app.rs"),
        "fn main() { staged(); unstaged(); }\n",
    )
    .unwrap();
    std::fs::write(dir.join("src/new.rs"), "// staged new\n").unwrap();
    common::git(dir, &["add", "src/new.rs"]);
    std::fs::write(dir.join("docs/notes.md"), "unstaged notes\n").unwrap();
}

mod sign {
    use super::*;

    #[test]
    fn pathspec_only() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        staged_changes_repo(&repo);
        let staged_before = common::git(&repo, &["diff", "--cached", "--name-only"]);
        let root = Root::open(&repo, RootKind::Project).unwrap();
        let packet = write(
            &root,
            "docs/handoff/stage-02.md",
            "=== HANDOFF PACKET · Stage 2 → Stage 3 ===\n",
        );
        let lessons = write(&root, "docs/notes.md", "unstaged notes\n"); // equals the working file: still unstaged content
        let out = commit_paths(
            &root,
            std::slice::from_ref(&packet),
            "handoff: stage 02",
            &opts(d.path()),
            &common::ctx(d.path()),
        )
        .unwrap();
        let CommitOutcome::Committed {
            commit,
            paths,
            first,
            branch,
            index_refresh_pending,
        } = out
        else {
            panic!()
        };
        assert!(!first && !index_refresh_pending);
        assert_eq!(branch, "refs/heads/main");
        assert_eq!(paths, ["docs/handoff/stage-02.md"]);
        // The commit holds exactly that path on top of HEAD~1's tree.
        let changed = common::git(
            &repo,
            &["diff-tree", "--no-commit-id", "--name-only", "-r", &commit],
        );
        assert_eq!(changed, "docs/handoff/stage-02.md");
        assert_eq!(common::git(&repo, &["rev-parse", "HEAD"]), commit);
        assert_eq!(
            common::git(&repo, &["log", "-1", "--format=%s"]),
            "handoff: stage 02"
        );
        // Unrelated staged and unstaged changes stay as they were; the packet shows clean.
        assert_eq!(
            common::git(&repo, &["diff", "--cached", "--name-only"]),
            staged_before
        );
        assert_eq!(
            std::fs::read_to_string(repo.join("src/app.rs")).unwrap(),
            "fn main() { staged(); unstaged(); }\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.join("docs/notes.md")).unwrap(),
            "unstaged notes\n"
        );
        let porcelain = common::git(&repo, &["status", "--porcelain"]);
        assert!(!porcelain.contains("stage-02.md"), "{porcelain}");
        // A path unchanged since HEAD isn't committed (DR-142).
        let out = commit_paths(
            &root,
            &[packet],
            "handoff: stage 02",
            &opts(d.path()),
            &common::ctx(d.path()),
        )
        .unwrap();
        assert_eq!(out, CommitOutcome::NothingToCommit);
        let _ = lessons;
        // Decision-file case (M1-28): a Foreman decision change commits only `.foreman/project.json`.
        let proj = rp(".foreman/project.json");
        let st = root
            .write(&proj, b"{\n  \"schemaVersion\": 1\n}\n", &ReadStamp::Absent)
            .unwrap();
        let out = commit_paths(
            &root,
            &[Reviewed {
                path: proj,
                sha256: st.sha256().unwrap().into(),
            }],
            "foreman: project.json",
            &opts(d.path()),
            &common::ctx(d.path()),
        )
        .unwrap();
        let CommitOutcome::Committed { commit, .. } = out else {
            panic!()
        };
        assert_eq!(
            common::git(
                &repo,
                &["diff-tree", "--no-commit-id", "--name-only", "-r", &commit]
            ),
            ".foreman/project.json"
        );
        assert_eq!(
            common::git(&repo, &["diff", "--cached", "--name-only"]),
            staged_before
        );
    }

    #[test]
    fn readback_detects_mismatch() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        common::init_repo(&repo);
        common::commit_file(&repo, "docs/handoff/stage-01.md", "what git has\n");
        let commit = common::git(&repo, &["rev-parse", "HEAD"]);
        let ctx = common::ctx(d.path());
        let good = vec![(
            "docs/handoff/stage-01.md".to_owned(),
            sha256_hex(b"what git has\n"),
        )];
        verify_readback(&repo, &commit, &good, &ctx).unwrap();
        let bad = vec![(
            "docs/handoff/stage-01.md".to_owned(),
            sha256_hex(b"what Foreman wrote\n"),
        )];
        let e = verify_readback(&repo, &commit, &bad, &ctx).unwrap_err();
        assert_eq!(
            e,
            CommitError::ReadbackMismatch {
                commit: commit.clone(),
                path: "docs/handoff/stage-01.md".into()
            }
        );
        assert_eq!(
            e.to_string(),
            "Your repo's commit hooks changed docs/handoff/stage-01.md"
        );
        // A path missing from the commit is a mismatch too.
        let gone = vec![("docs/handoff/stage-09.md".to_owned(), sha256_hex(b"x"))];
        assert!(verify_readback(&repo, &commit, &gone, &ctx).is_err());
        // A file changed after review is refused before anything is committed.
        let root = Root::open(&repo, RootKind::Project).unwrap();
        let r = write(&root, "docs/handoff/stage-02.md", "reviewed\n");
        std::fs::write(
            repo.join("docs/handoff/stage-02.md"),
            "edited after review\n",
        )
        .unwrap();
        let e = commit_paths(&root, &[r], "handoff: stage 02", &opts(d.path()), &ctx).unwrap_err();
        assert!(matches!(e, CommitError::ChangedSinceReview { .. }));
        assert_eq!(common::git(&repo, &["rev-parse", "HEAD"]), commit);
    }

    #[test]
    fn untracked_new_packet() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        common::init_repo(&repo);
        common::commit_file(&repo, "README.md", "x\n");
        let root = Root::open(&repo, RootKind::Project).unwrap();
        let r = write(&root, "docs/handoff/stage-03.md", "new packet\n");
        let a = write(
            &root,
            "docs/handoff/stage-03-attachments.md",
            "attachments\n",
        );
        let out = commit_paths(
            &root,
            &[r, a],
            "handoff: stage 03",
            &opts(d.path()),
            &common::ctx(d.path()),
        )
        .unwrap();
        let CommitOutcome::Committed { commit, paths, .. } = out else {
            panic!()
        };
        assert_eq!(paths.len(), 2);
        assert_eq!(
            common::git(
                &repo,
                &[
                    "cat-file",
                    "blob",
                    &format!("{commit}:docs/handoff/stage-03.md")
                ]
            ),
            "new packet"
        );
        assert_eq!(
            common::git(&repo, &["ls-files", "docs/handoff"]),
            "docs/handoff/stage-03-attachments.md\ndocs/handoff/stage-03.md"
        );
    }

    #[test]
    fn concurrent_ref_move_fails() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        common::init_repo(&repo);
        common::commit_file(&repo, "README.md", "x\n");
        let root = Root::open(&repo, RootKind::Project).unwrap();
        let r = write(&root, "docs/handoff/stage-02.md", "packet\n");
        let repo2 = repo.clone();
        let mover =
            move || common::commit_file(&repo2, "src/claude.rs", "// Claude committed meanwhile\n");
        let o = CommitOpts {
            app_data: d.path().into(),
            before_update_ref: Some(&mover),
            ..CommitOpts::default()
        };
        let e =
            commit_paths(&root, &[r], "handoff: stage 02", &o, &common::ctx(d.path())).unwrap_err();
        assert_eq!(e, CommitError::ConcurrentRefMove);
        assert_eq!(
            common::git(&repo, &["log", "-1", "--format=%s"]),
            "fixture src/claude.rs"
        );
        assert!(
            common::git(&repo, &["ls-tree", "-r", "--name-only", "HEAD"]).contains("src/claude.rs")
        );
        // The temporary index was removed.
        assert_eq!(std::fs::read_dir(d.path().join("tmp")).unwrap().count(), 0);
    }

    /// DR-143: an unborn HEAD gets the repo's first, parentless commit (setup's `git init`, then
    /// the first `condition_set`, M1-28).
    #[test]
    fn unborn_head_first_commit() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        common::init_repo(&repo);
        let root = Root::open(&repo, RootKind::Project).unwrap();
        let proj = pathpolicy::check_write(
            ".foreman/project.json",
            WritePurpose::DecisionFile(DecisionPath::Project),
        )
        .unwrap();
        let st = root
            .write(&proj, b"{\n  \"schemaVersion\": 1\n}\n", &ReadStamp::Absent)
            .unwrap();
        let out = commit_paths(
            &root,
            &[Reviewed {
                path: proj,
                sha256: st.sha256().unwrap().into(),
            }],
            "foreman: project.json",
            &opts(d.path()),
            &common::ctx(d.path()),
        )
        .unwrap();
        let CommitOutcome::Committed {
            commit,
            first,
            branch,
            ..
        } = out
        else {
            panic!()
        };
        assert!(first);
        assert_eq!(branch, "refs/heads/main");
        assert_eq!(common::git(&repo, &["rev-list", "--count", "HEAD"]), "1");
        let raw = common::git(&repo, &["cat-file", "commit", &commit]);
        assert!(!raw.contains("\nparent "), "{raw}");
        assert_eq!(common::git(&repo, &["ls-files"]), ".foreman/project.json");
    }

    /// DR-143: a detached HEAD is refused with "Check out a branch first"; nothing is written, and
    /// the decision change stays pending (the file is still uncommitted).
    #[test]
    fn detached_head_refused() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        common::init_repo(&repo);
        common::commit_file(&repo, "README.md", "x\n");
        let head = common::git(&repo, &["rev-parse", "HEAD"]);
        common::git(&repo, &["checkout", "-q", "--detach", &head]);
        let root = Root::open(&repo, RootKind::Project).unwrap();
        let proj = pathpolicy::check_write(
            ".foreman/project.json",
            WritePurpose::DecisionFile(DecisionPath::Project),
        )
        .unwrap();
        let st = root.write(&proj, b"{}\n", &ReadStamp::Absent).unwrap();
        let e = commit_paths(
            &root,
            &[Reviewed {
                path: proj,
                sha256: st.sha256().unwrap().into(),
            }],
            "foreman: project.json",
            &opts(d.path()),
            &common::ctx(d.path()),
        )
        .unwrap_err();
        assert_eq!(e, CommitError::DetachedHead);
        assert_eq!(e.to_string(), "Check out a branch first");
        assert_eq!(common::git(&repo, &["rev-parse", "HEAD"]), head);
        assert_eq!(
            common::git(&repo, &["ls-files", "--others", "--exclude-standard"]),
            ".foreman/project.json"
        );
    }
}

/// Security review 2026-10-20, N-6 and N-7.
mod git_temp {
    use super::*;

    fn one_commit(repo: &Path, app: &Path) -> Result<CommitOutcome, CommitError> {
        let root = Root::open(repo, RootKind::Project).unwrap();
        std::fs::create_dir_all(repo.join("docs")).unwrap();
        let r = write(&root, "docs/handoff/stage-02.md", "packet\n");
        commit_paths(
            &root,
            std::slice::from_ref(&r),
            "handoff: stage 02",
            &opts(app),
            &common::ctx(app),
        )
    }

    /// N-6: the temporary index lives in `<app-data>/tmp`, made through the store; a symlinked
    /// `tmp` is refused (nothing is written through it) and the index file is removed after use.
    #[test]
    fn temp_index_never_follows_a_symlinked_tmp() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        common::init_repo(&repo);
        common::commit_file(&repo, "README.md", "hi\n");
        let app = d.path().join("app");
        std::fs::create_dir_all(&app).unwrap();
        let elsewhere = d.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        symlink(&elsewhere, app.join("tmp"));
        let e = one_commit(&repo, &app).unwrap_err();
        assert!(matches!(e, CommitError::Store(_)), "{e:?}");
        assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);
        // A real folder works, and keeps no index file afterwards.
        std::fs::remove_file(app.join("tmp")).unwrap();
        assert!(matches!(
            one_commit(&repo, &app).unwrap(),
            CommitOutcome::Committed { .. }
        ));
        assert_eq!(std::fs::read_dir(app.join("tmp")).unwrap().count(), 0);
    }

    /// N-7: `info/attributes` that isn't a regular file (a FIFO reports size 0, a folder) can't
    /// be read as "no rules": the real-index refresh stays pending (fails closed, CR-002).
    #[test]
    fn info_attributes_not_regular_keeps_refresh_pending() {
        for kind in ["fifo", "dir"] {
            let d = tempfile::tempdir().unwrap();
            let repo = d.path().join("repo");
            common::init_repo(&repo);
            common::commit_file(&repo, "README.md", "hi\n");
            let attrs = repo.join(".git/info/attributes");
            std::fs::create_dir_all(attrs.parent().unwrap()).unwrap();
            if kind == "fifo" {
                rustix::fs::mknodat(
                    rustix::fs::CWD,
                    &attrs,
                    rustix::fs::FileType::Fifo,
                    rustix::fs::Mode::from_raw_mode(0o600),
                    0,
                )
                .unwrap();
            } else {
                std::fs::create_dir(&attrs).unwrap();
            }
            let out = one_commit(&repo, d.path()).unwrap();
            assert!(
                matches!(
                    out,
                    CommitOutcome::Committed {
                        index_refresh_pending: true,
                        ..
                    }
                ),
                "{kind}: {out:?}"
            );
        }
        // Control: an empty regular file means no rules, and the refresh runs.
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        common::init_repo(&repo);
        common::commit_file(&repo, "README.md", "hi\n");
        std::fs::write(repo.join(".git/info/attributes"), "").unwrap();
        assert!(matches!(
            one_commit(&repo, d.path()).unwrap(),
            CommitOutcome::Committed {
                index_refresh_pending: false,
                ..
            }
        ));
    }

    fn symlink(target: &Path, link: impl AsRef<Path>) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }
}
