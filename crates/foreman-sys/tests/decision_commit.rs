//! M1-28: Foreman's own decision commits with the real git (AC-F020-08, -09; DR-153).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may spawn and use std::fs (design doc §3.2)"
)]

mod common;

use std::path::{Path, PathBuf};

use common::{commit_file, ctx, git, init_repo};
use foreman_core::decisions::{Decisions, OwnChange, Processed};
use foreman_core::schema::progress::DecisionFileKind as K;
use foreman_core::store::{Root, RootKind, sha256_hex};
use foreman_sys::decisions::commit_own;
use foreman_sys::git::commit::{CommitError, CommitOpts, CommitOutcome};

const ID: &str = "3f6a1c2e-8b4d-4e7f-9a1b-2c3d4e5f6a7b";

fn project_json() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../foreman-core/tests/schema/project/v1.json"),
    )
    .unwrap()
}

struct Env {
    _tmp: tempfile::TempDir,
    repo: PathBuf,
    app: PathBuf,
}

impl Env {
    /// `init` only (setup's `git init`; nothing committed, AC-F018-03).
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        let app = tmp.path().join("app");
        std::fs::create_dir_all(&app).unwrap();
        init_repo(&repo);
        Self {
            _tmp: tmp,
            repo,
            app,
        }
    }
    fn roots(&self) -> (Root, Root) {
        (
            Root::open(&self.repo, RootKind::Project).unwrap(),
            Root::open_app_data(&self.app).unwrap(),
        )
    }
    fn opts(&self) -> CommitOpts<'static> {
        CommitOpts {
            app_data: self.app.clone(),
            ..CommitOpts::default()
        }
    }
    /// Setup: the project file written and approved (not committed).
    fn setup(&self) {
        let p = self.repo.join(".foreman/project.json");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, project_json()).unwrap();
        let (project, app) = self.roots();
        let d = Decisions {
            project: &project,
            app: &app,
            project_id: ID,
        };
        d.approve(K::Project, project_json().as_bytes(), false)
            .unwrap();
    }
    /// Foreman's first `condition_set` (payments → yes) before Stage 5: commit at once.
    fn condition_set(
        &self,
        from: &str,
        to: &str,
    ) -> (OwnChange, Result<Option<CommitOutcome>, CommitError>) {
        let (project, app) = self.roots();
        let d = Decisions {
            project: &project,
            app: &app,
            project_id: ID,
        };
        let current = String::from_utf8(d.approved(K::Project).unwrap().0).unwrap();
        let next = current.replacen(from, to, 1);
        assert_ne!(next, current);
        let change = d
            .own_change(K::Project, next.as_bytes(), false, "t")
            .unwrap();
        let out = commit_own(&project, &change, &self.opts(), &ctx(&self.app));
        if let (OwnChange::CommitNow { sha256, .. }, Ok(Some(CommitOutcome::Committed { .. }))) =
            (&change, &out)
        {
            d.mark_committed(K::Project, sha256).unwrap();
        }
        (change, out)
    }
}

mod sign {
    use super::*;

    /// AC-F020-08: setup's `git init`, then the first `condition_set` → the repo's first commit.
    #[test]
    fn unborn_head_first_commit_decision() {
        let e = Env::new();
        e.setup();
        let (_, out) = e.condition_set("\"payments\": \"no\"", "\"payments\": \"yes\"");
        let Some(CommitOutcome::Committed {
            commit,
            first,
            paths,
            ..
        }) = out.unwrap()
        else {
            panic!()
        };
        assert!(first);
        assert_eq!(paths, [".foreman/project.json"]);
        assert_eq!(git(&e.repo, &["rev-list", "--count", "HEAD"]), "1");
        assert_eq!(
            git(&e.repo, &["log", "-1", "--format=%s"]),
            "foreman: .foreman/project.json"
        );
        assert_eq!(
            git(&e.repo, &["rev-list", "--parents", "-1", &commit]),
            commit,
            "no parent"
        );
    }
}

mod decisions {
    use super::*;

    /// AC-F020-08 (unrelated staged changes untouched) and the detached-HEAD refusal, where the
    /// change stays pending; AC-F020-09 (from Stage 5 on nothing is committed).
    #[test]
    fn own_change_commit_and_pending() {
        let e = Env::new();
        commit_file(&e.repo, "src/app.rs", "fn main() {}\n");
        e.setup();
        std::fs::write(e.repo.join("src/app.rs"), "fn main() { staged(); }\n").unwrap();
        git(&e.repo, &["add", "src/app.rs"]);
        let staged = git(&e.repo, &["diff", "--cached", "--name-only"]);
        let (change, out) = e.condition_set("\"payments\": \"no\"", "\"payments\": \"yes\"");
        let Some(CommitOutcome::Committed { commit, .. }) = out.unwrap() else {
            panic!()
        };
        assert_eq!(
            git(
                &e.repo,
                &["diff-tree", "--no-commit-id", "--name-only", "-r", &commit]
            ),
            ".foreman/project.json"
        );
        assert_eq!(git(&e.repo, &["diff", "--cached", "--name-only"]), staged);
        let OwnChange::CommitNow { sha256, .. } = change else {
            panic!()
        };
        let (project, app) = e.roots();
        let d = Decisions {
            project: &project,
            app: &app,
            project_id: ID,
        };
        let (a, _) = d.approvals().unwrap();
        assert_eq!(
            a.files.project.committed_sha256.as_deref(),
            Some(sha256.as_str())
        );
        // Detached HEAD: refused with "Check out a branch first"; the change stays pending.
        let head = git(&e.repo, &["rev-parse", "HEAD"]);
        git(&e.repo, &["checkout", "-q", "--detach", &head]);
        let (change, out) = e.condition_set("\"public\": \"no\"", "\"public\": \"yes\"");
        let err = out.unwrap_err();
        assert!(matches!(err, CommitError::DetachedHead));
        assert_eq!(err.to_string(), "Check out a branch first");
        assert_eq!(git(&e.repo, &["rev-parse", "HEAD"]), head);
        let (a, _) = d.approvals().unwrap();
        assert_eq!(
            a.files.project.committed_sha256.as_deref(),
            Some(sha256.as_str()),
            "still the old commit"
        );
        let OwnChange::CommitNow {
            sha256: pending, ..
        } = change
        else {
            panic!()
        };
        assert_eq!(
            a.files.project.approved_sha256.as_deref(),
            Some(pending.as_str())
        );
        assert_eq!(
            d.process(K::Project, "c", "t").unwrap(),
            Processed::Unchanged,
            "not an outside change"
        );
        // From Stage 5 on (AC-F020-09): pending, nothing committed, the repo file untouched.
        git(&e.repo, &["checkout", "-q", "main"]);
        let before = std::fs::read(e.repo.join(".foreman/project.json")).unwrap();
        let next = String::from_utf8(before.clone()).unwrap().replacen(
            "\"server\": \"no\"",
            "\"server\": \"yes\"",
            1,
        );
        let change = d
            .own_change(K::Project, next.as_bytes(), true, "t")
            .unwrap();
        assert!(
            matches!(change, OwnChange::Pending(ref p) if p.sha256 == sha256_hex(next.as_bytes()))
        );
        assert_eq!(
            commit_own(&project, &change, &e.opts(), &ctx(&e.app)).unwrap(),
            None
        );
        assert_eq!(git(&e.repo, &["rev-parse", "HEAD"]), head);
        assert_eq!(
            std::fs::read(e.repo.join(".foreman/project.json")).unwrap(),
            before
        );
    }
}
