//! M1-17: the lock against a real blueprint repo (DR-142, DR-155, DR-156).
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
use foreman_core::lock::{self as lk, Fetched, LockError, ReadOnly, Restored};
use foreman_core::store::{Root, RootKind};
use foreman_sys::lock::{blueprint_state, fetch};
use foreman_sys::spawn::SpawnCtx;

const PATH: &str = "claude-build-blueprint.md";

fn blueprint() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../foreman-core/tests/fixtures/blueprint/claude-build-blueprint-v2.2.md"),
    )
    .unwrap()
}

struct Env {
    _tmp: tempfile::TempDir,
    repo: PathBuf,
    project: PathBuf,
    app: PathBuf,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("claude-blueprint");
    let project = tmp.path().join("project");
    let app = tmp.path().join("app");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&app).unwrap();
    init_repo(&repo);
    commit_file(&repo, PATH, &blueprint());
    Env {
        _tmp: tmp,
        repo,
        project,
        app,
    }
}

impl Env {
    fn roots(&self) -> (Root, Root) {
        (
            Root::open(&self.project, RootKind::Project).unwrap(),
            Root::open_app_data(&self.app).unwrap(),
        )
    }
    fn ctx(&self) -> SpawnCtx {
        ctx(&self.app)
    }
}

mod lock {
    use super::*;

    /// AC-F002-04, -06, -07 / DR-156.
    #[test]
    fn restore_from_git() {
        let e = env();
        let (project, app) = e.roots();
        let st = blueprint_state(&e.repo, PATH, &e.ctx()).unwrap();
        assert_eq!(st.commit, git(&e.repo, &["rev-parse", "HEAD"]));
        let lock = lk::pin(&project, &app, PATH, &st, false, "2026-10-20T10:00:00Z").unwrap();
        // The app-data copy is gone: restored from `cat-file blob <commit>:<path>`, re-hashed, stored.
        let copy = e.app.join(lk::text_path(&lock.source_sha256));
        std::fs::remove_file(&copy).unwrap();
        let (text, from) = lk::restore(&app, &lock, || fetch(&e.repo, &lock, &e.ctx())).unwrap();
        assert_eq!((text, from), (blueprint(), Restored::Git));
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), blueprint());
        // The blueprint moved on since the pin: the pinned commit still restores.
        commit_file(&e.repo, PATH, "# something else\n");
        std::fs::remove_file(&copy).unwrap();
        assert_eq!(
            lk::restore(&app, &lock, || fetch(&e.repo, &lock, &e.ctx()))
                .unwrap()
                .1,
            Restored::Git
        );
        // A commit that doesn't exist, or no repo: "not found" (read-only).
        std::fs::remove_file(&copy).unwrap();
        let mut gone = lock.clone();
        gone.blueprint_repo_commit = "0123456789abcdef0123456789abcdef01234567".into();
        let r = lk::restore(&app, &gone, || fetch(&e.repo, &gone, &e.ctx())).unwrap_err();
        assert!(matches!(r, ReadOnly::NotFound { .. }), "{r:?}");
        let missing = e.repo.with_file_name("moved");
        assert_eq!(fetch(&missing, &lock, &e.ctx()), Fetched::NotFound);
        // The path moved in the repo at that commit: not found as well.
        let mut moved = lock.clone();
        moved.blueprint_path = "other.md".into();
        assert_eq!(fetch(&e.repo, &moved, &e.ctx()), Fetched::NotFound);
        // git missing: the read-only reason names git.
        let mut no_git = e.ctx();
        no_git.settings.clear();
        let missing_git = fetch(&e.repo, &lock, &no_git);
        if missing_git == Fetched::GitMissing {
            let r = lk::restore(&app, &lock, || missing_git).unwrap_err();
            assert!(r.to_string().contains("git"));
        } else {
            // A git on a standard candidate path was found instead; the setting is the only source
            // of a missing git on such machines, so check the mapping directly.
            let mut bad = e.ctx();
            bad.settings = vec![(foreman_sys::spawn::Program::Git, e.app.join("no-git"))];
            assert_eq!(fetch(&e.repo, &lock, &bad), Fetched::GitMissing);
        }
    }

    /// AC-F002-02 / DR-142 with real hashes: `hash-object --no-filters` vs `rev-parse HEAD:<path>`.
    #[test]
    fn dirty_blueprint_hashes() {
        let e = env();
        let (project, app) = e.roots();
        std::fs::write(e.repo.join(PATH), format!("{}\nlocal edit\n", blueprint())).unwrap();
        let st = blueprint_state(&e.repo, PATH, &e.ctx()).unwrap();
        let head = git(&e.repo, &["rev-parse", &format!("HEAD:{PATH}")]);
        assert_eq!(st.head_oid.as_deref(), Some(head.as_str()));
        assert_ne!(st.working_oid, head);
        let err = lk::pin(&project, &app, PATH, &st, false, "t").unwrap_err();
        assert!(matches!(err, LockError::Dirty { .. }));
        assert!(err.to_string().contains(&head) && err.to_string().contains(&st.working_oid));
        assert!(!e.project.join(".foreman").exists());
    }
}
