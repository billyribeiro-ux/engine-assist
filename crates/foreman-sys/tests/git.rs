//! M1-08: `git::run`, the single git entry point (ADR-012, SR-44, SR-45, DR-140, DR-141, AC-XC-04).
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

use std::path::{Path, PathBuf};
use std::time::Duration;

use foreman_core::git::{GitOp, ReadTreeFrom, RevParse, SUBCOMMANDS, ZERO_OID};
use foreman_core::pathpolicy::{self, WritePurpose};
use foreman_core::store::{ReadStamp, Root, RootKind, sha256_hex};
use foreman_sys::git::commit::{CommitOpts, CommitOutcome, Reviewed, commit_paths};
use foreman_sys::git::{self as fgit, GitError, GitOpts};
use foreman_sys::spawn::{Program, SpawnCtx};

fn all_ops() -> Vec<GitOp> {
    let oid = "a".repeat(40);
    vec![
        GitOp::Init,
        GitOp::RevParse(RevParse::ShowToplevel),
        GitOp::RevParse(RevParse::GitPath("info/exclude".into())),
        GitOp::RevParse(RevParse::GitCommonDir),
        GitOp::RevParse(RevParse::VerifyHead),
        GitOp::RevParse(RevParse::BlobAt {
            rev: "HEAD".into(),
            path: "docs/handoff/stage-02.md".into(),
        }),
        GitOp::SymbolicRefHead,
        GitOp::ConfigGet {
            key: "user.name".into(),
            show_origin_and_scope: false,
        },
        GitOp::ConfigGet {
            key: "commit.gpgSign".into(),
            show_origin_and_scope: true,
        },
        GitOp::CatFileBlob {
            rev: oid.clone(),
            path: "claude-build-blueprint.md".into(),
        },
        GitOp::LsTree {
            rev: oid.clone(),
            path: ".foreman/stages/stage-05.json".into(),
        },
        GitOp::ShowBlob {
            rev: oid.clone(),
            path: "claude-build-blueprint.md".into(),
        },
        GitOp::HashObjectNoFilters { write: false },
        GitOp::HashObjectNoFilters { write: true },
        GitOp::UpdateIndexCacheInfo {
            entries: vec![(oid.clone(), "docs/handoff/stage-02.md".into())],
        },
        GitOp::ReadTree(ReadTreeFrom::Tip(oid.clone())),
        GitOp::ReadTree(ReadTreeFrom::Empty),
        GitOp::WriteTree,
        GitOp::CommitTree {
            tree: oid.clone(),
            parent: Some("b".repeat(40)),
            msg: "handoff: stage 02".into(),
        },
        GitOp::CommitTree {
            tree: oid.clone(),
            parent: None,
            msg: "foreman: project.json".into(),
        },
        GitOp::UpdateRef {
            refname: "HEAD".into(),
            new: oid,
            old: ZERO_OID.into(),
        },
    ]
}

fn rp(s: &str) -> pathpolicy::RepoPath {
    pathpolicy::check_write(s, WritePurpose::Repo).unwrap()
}

mod git {
    use super::*;

    /// AC-XC-04: `Command::new` exists only in foreman-sys::spawn, and git is spawned only by
    /// git::run (clippy `disallowed-methods` enforces the first; this scan pins both).
    #[test]
    fn single_entry_point() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut hits = Vec::new();
        let mut git_uses = Vec::new();
        let mut stack: Vec<PathBuf> = [
            "crates/foreman-core/src",
            "crates/foreman-sys/src",
            "crates/foreman-sensor/src",
            "src-tauri/src",
        ]
        .iter()
        .map(|d| root.join(d))
        .collect();
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let text = std::fs::read_to_string(&p).unwrap();
                    let rel = p.strip_prefix(&root).unwrap().display().to_string();
                    if text.contains("Command::new") || text.contains("CommandBuilder::new") {
                        hits.push(rel.clone());
                    }
                    if text.contains("Program::Git") {
                        git_uses.push(rel);
                    }
                }
            }
        }
        assert_eq!(hits, ["crates/foreman-sys/src/spawn.rs"]);
        git_uses.sort();
        assert_eq!(
            git_uses,
            [
                "crates/foreman-sys/src/git/mod.rs",
                "crates/foreman-sys/src/spawn.rs"
            ]
        );
        let clippy = std::fs::read_to_string(root.join("clippy.toml")).unwrap();
        assert!(clippy.contains("std::process::Command::new"));
    }

    /// The exact argv of every operation (SR-45 overrides, closed enum).
    #[test]
    fn invocation_snapshot() {
        let lines: Vec<String> = all_ops()
            .iter()
            .map(|op| {
                fgit::argv(Path::new("/Users/billy/dev/tradelog"), op)
                    .unwrap()
                    .join(" ")
            })
            .collect();
        insta::assert_snapshot!(lines.join("\n"));
        // The temporary-index rule and stdin flags.
        let a = foreman_core::git::args(&GitOp::ReadTree(ReadTreeFrom::Empty)).unwrap();
        assert!(a.needs_temp_index);
        assert!(
            foreman_core::git::args(&GitOp::HashObjectNoFilters { write: true })
                .unwrap()
                .stdin
        );
        for op in all_ops() {
            if let GitOp::CommitTree { .. } = op {
                assert!(
                    fgit::argv(Path::new("/r"), &op)
                        .unwrap()
                        .contains(&"--no-gpg-sign".to_owned())
                );
            }
        }
    }

    /// AC-XC-04: the environment is exactly HOME, a fixed PATH, LANG, GIT_TERMINAL_PROMPT,
    /// GIT_OPTIONAL_LOCKS (plus GIT_INDEX_FILE only for the temporary index).
    #[test]
    fn env_cleared() {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("bin");
        let fake = common::install_fake(&bin, "git", &serde_json::json!({}));
        let ctx = SpawnCtx {
            home: d.path().into(),
            app_data: d.path().into(),
            projects: vec![],
            settings: vec![(Program::Git, fake)],
        };
        let out = fgit::run(d.path(), &GitOp::Init, &GitOpts::default(), &ctx).unwrap();
        let rec: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let env = rec["env"].as_object().unwrap();
        let mut keys: Vec<&str> = env.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "GIT_OPTIONAL_LOCKS",
                "GIT_TERMINAL_PROMPT",
                "HOME",
                "LANG",
                "PATH"
            ]
        );
        assert_eq!(
            env["PATH"],
            format!("/usr/bin:/bin:/usr/sbin:/sbin:{}", bin.display())
        );
        assert_eq!(env["LANG"], "C");
        assert_eq!(env["GIT_TERMINAL_PROMPT"], "0");
        let argv: Vec<&str> = rec["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(&argv[1..3], ["-C", d.path().to_str().unwrap()]);
        assert_eq!(argv.iter().filter(|a| **a == "-c").count(), 10);
        // GIT_INDEX_FILE appears only with a temporary index.
        let idx = d.path().join("tmp/git-index-x");
        let out = fgit::run(
            d.path(),
            &GitOp::Init,
            &GitOpts {
                index_file: Some(idx.clone()),
                ..GitOpts::default()
            },
            &ctx,
        )
        .unwrap();
        let rec: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(rec["env"]["GIT_INDEX_FILE"], idx.display().to_string());
        // read-tree without it is refused before anything runs.
        let r = fgit::run(
            d.path(),
            &GitOp::ReadTree(ReadTreeFrom::Empty),
            &GitOpts::default(),
            &ctx,
        );
        assert!(matches!(
            r,
            Err(GitError::NeedsTempIndex) | Err(GitError::NotOwnRepo { .. })
        ));
        let a = common::ctx(d.path());
        common::init_repo(&d.path().join("r"));
        assert_eq!(
            fgit::run(
                &d.path().join("r"),
                &GitOp::ReadTree(ReadTreeFrom::Empty),
                &GitOpts::default(),
                &a
            ),
            Err(GitError::NeedsTempIndex)
        );
    }

    /// T-043: a hung git is killed after the timeout (60 s by default).
    #[test]
    fn timeout() {
        assert_eq!(fgit::TIMEOUT, Duration::from_secs(60));
        let d = tempfile::tempdir().unwrap();
        let fake = common::install_fake(
            &d.path().join("bin"),
            "git",
            &serde_json::json!({ "sleepMs": 10000 }),
        );
        let ctx = SpawnCtx {
            home: d.path().into(),
            app_data: d.path().into(),
            projects: vec![],
            settings: vec![(Program::Git, fake)],
        };
        let t = std::time::Instant::now();
        let r = fgit::run(
            d.path(),
            &GitOp::Init,
            &GitOpts {
                timeout: Duration::from_millis(300),
                ..GitOpts::default()
            },
            &ctx,
        );
        assert!(
            matches!(
                r,
                Err(GitError::Spawn(foreman_sys::spawn::SpawnError::Timeout(_)))
            ),
            "{r:?}"
        );
        assert!(t.elapsed() < Duration::from_secs(5));
    }

    fn exercise_everything(repo: &Path, app: &Path, ctx: &SpawnCtx) {
        let g = GitOpts::default();
        for op in [
            GitOp::RevParse(RevParse::ShowToplevel),
            GitOp::RevParse(RevParse::GitPath("info/exclude".into())),
            GitOp::RevParse(RevParse::GitCommonDir),
            GitOp::RevParse(RevParse::VerifyHead),
            GitOp::RevParse(RevParse::BlobAt {
                rev: "HEAD".into(),
                path: "README.md".into(),
            }),
            GitOp::SymbolicRefHead,
            GitOp::ConfigGet {
                key: "user.name".into(),
                show_origin_and_scope: true,
            },
            GitOp::CatFileBlob {
                rev: "HEAD".into(),
                path: "README.md".into(),
            },
            GitOp::LsTree {
                rev: "HEAD".into(),
                path: "README.md".into(),
            },
            GitOp::ShowBlob {
                rev: "HEAD".into(),
                path: "README.md".into(),
            },
        ] {
            let out = fgit::run(repo, &op, &g, ctx).unwrap();
            assert!(out.ok(), "{op:?}: {}", String::from_utf8_lossy(&out.stderr));
        }
        let o = fgit::run(
            repo,
            &GitOp::HashObjectNoFilters { write: true },
            &GitOpts {
                stdin: Some(b"x\n".to_vec()),
                ..g.clone()
            },
            ctx,
        )
        .unwrap();
        assert!(o.ok());
        // A full plumbing commit (every remaining op) of a file that the filter would rewrite.
        std::fs::create_dir_all(repo.join("docs/handoff")).unwrap();
        let root = Root::open(repo, RootKind::Project).unwrap();
        let stamp = root
            .write(
                &rp("docs/handoff/stage-01.md"),
                b"=== HANDOFF PACKET\n",
                &ReadStamp::Absent,
            )
            .unwrap();
        let out = commit_paths(
            &root,
            &[Reviewed {
                path: rp("docs/handoff/stage-01.md"),
                sha256: stamp.sha256().unwrap().into(),
            }],
            "handoff: stage 01",
            &CommitOpts {
                app_data: app.into(),
                ..CommitOpts::default()
            },
            ctx,
        )
        .unwrap();
        assert!(matches!(out, CommitOutcome::Committed { .. }));
        let init_dir = tempfile::tempdir().unwrap();
        assert!(
            fgit::run(init_dir.path(), &GitOp::Init, &g, ctx)
                .unwrap()
                .ok()
        );
    }

    /// AB-06: no repo hook runs on any command (local core.hooksPath, default hooks, the
    /// reference-transaction hook on update-ref).
    #[test]
    fn no_hooks_on_any_command() {
        let d = tempfile::tempdir().unwrap();
        let (repo, markers) = (d.path().join("repo"), d.path().join("markers"));
        common::hostile_repo(&repo, &markers);
        let ctx = common::ctx(d.path());
        exercise_everything(&repo, d.path(), &ctx);
        let fired: Vec<String> = common::markers_fired(&markers)
            .into_iter()
            .filter(|m| m.contains("hook"))
            .collect();
        assert!(fired.is_empty(), "{fired:?}");
    }

    /// T-039: a repo filter driver (clean/smudge) and textconv never run.
    #[test]
    fn filter_driver_never_runs() {
        let d = tempfile::tempdir().unwrap();
        let (repo, markers) = (d.path().join("repo"), d.path().join("markers"));
        common::hostile_repo(&repo, &markers);
        let ctx = common::ctx(d.path());
        exercise_everything(&repo, d.path(), &ctx);
        let fired = common::markers_fired(&markers);
        assert!(
            !fired.iter().any(|m| m.starts_with("filter")
                || m == "textconv"
                || m == "fsmonitor"
                || m == "pager"
                || m == "editor"),
            "{fired:?}"
        );
        // The committed blob is exactly the bytes Foreman wrote (no clean filter applied).
        let blob = common::git(
            &repo,
            &[
                "-c",
                "filter.evil.smudge=cat",
                "cat-file",
                "blob",
                "HEAD:docs/handoff/stage-01.md",
            ],
        );
        assert_eq!(blob, "=== HANDOFF PACKET");
    }

    /// T-040: Foreman never runs `status`, `diff`, `add`, `commit` or `ls-files`: the operation enum
    /// is exactly DR-140's list (static check) and no filter runs on any of them (live check).
    #[test]
    fn status_and_diff_never_run_filters() {
        let mut subs: Vec<String> = all_ops()
            .iter()
            .map(|op| foreman_core::git::args(op).unwrap().args[0].clone())
            .collect();
        subs.sort();
        subs.dedup();
        let mut expected: Vec<String> = SUBCOMMANDS.iter().map(|s| (*s).to_owned()).collect();
        expected.sort();
        assert_eq!(subs, expected, "the enum covers exactly DR-140");
        for forbidden in [
            "status", "diff", "add", "commit", "ls-files", "log", "checkout", "fetch", "push", "gc",
        ] {
            assert!(!SUBCOMMANDS.contains(&forbidden), "{forbidden}");
        }
        let d = tempfile::tempdir().unwrap();
        let (repo, markers) = (d.path().join("repo"), d.path().join("markers"));
        common::hostile_repo(&repo, &markers);
        // Make the index racily clean: README.md is rewritten with the same size within the index's
        // timestamp granularity, so writing the real index hashes it "as it would be added".
        std::fs::write(repo.join("README.md"), "HI\n").unwrap();
        exercise_everything(&repo, d.path(), &common::ctx(d.path()));
        assert!(
            !common::markers_fired(&markers)
                .iter()
                .any(|m| m.starts_with("filter")),
            "{:?}",
            common::markers_fired(&markers)
        );
        // A filter wired through $GIT_DIR/info/attributes can't be neutralized: the refresh is left
        // pending instead of running it.
        let d2 = tempfile::tempdir().unwrap();
        let (repo2, markers2) = (d2.path().join("repo"), d2.path().join("markers"));
        common::hostile_repo(&repo2, &markers2);
        std::fs::write(repo2.join(".git/info/attributes"), "* filter=evil\n").unwrap();
        std::fs::write(repo2.join("README.md"), "HI\n").unwrap();
        std::fs::create_dir_all(repo2.join("docs")).unwrap();
        let root = Root::open(&repo2, RootKind::Project).unwrap();
        let st = root
            .write(&rp("docs/x.md"), b"x", &ReadStamp::Absent)
            .unwrap();
        let out = commit_paths(
            &root,
            &[Reviewed {
                path: rp("docs/x.md"),
                sha256: st.sha256().unwrap().into(),
            }],
            "foreman: x",
            &CommitOpts {
                app_data: d2.path().into(),
                ..CommitOpts::default()
            },
            &common::ctx(d2.path()),
        )
        .unwrap();
        assert!(
            matches!(
                out,
                CommitOutcome::Committed {
                    index_refresh_pending: true,
                    ..
                }
            ),
            "{out:?}"
        );
        assert!(
            !common::markers_fired(&markers2)
                .iter()
                .any(|m| m.starts_with("filter"))
        );
    }

    /// DR-144: a repo `gpg.program` with `commit.gpgSign=true` never runs; commits aren't signed.
    #[test]
    fn repo_gpg_program_never_runs() {
        let d = tempfile::tempdir().unwrap();
        let (repo, markers) = (d.path().join("repo"), d.path().join("markers"));
        common::hostile_repo(&repo, &markers);
        exercise_everything(&repo, d.path(), &common::ctx(d.path()));
        assert!(!common::markers_fired(&markers).contains(&"gpg".to_owned()));
        let raw = common::git(&repo, &["cat-file", "commit", "HEAD"]);
        assert!(!raw.contains("gpgsig"), "{raw}");
    }

    /// DR-141: a folder inside an enclosing repo (and a folder with no repo) is refused.
    #[test]
    fn refuses_enclosing_repo() {
        let d = tempfile::tempdir().unwrap();
        let outer = d.path().join("monorepo");
        common::init_repo(&outer);
        common::commit_file(&outer, "x.txt", "x");
        let inner = outer.join("apps/tradelog");
        std::fs::create_dir_all(&inner).unwrap();
        let ctx = common::ctx(d.path());
        for op in [
            GitOp::RevParse(RevParse::VerifyHead),
            GitOp::SymbolicRefHead,
            GitOp::HashObjectNoFilters { write: true },
        ] {
            let e = fgit::run(&inner, &op, &GitOpts::default(), &ctx).unwrap_err();
            let GitError::NotOwnRepo { found, .. } = e else {
                panic!("{e:?}")
            };
            assert_eq!(
                found
                    .map(PathBuf::from)
                    .map(|p| foreman_sys::spawn::canonical(&p)),
                Some(foreman_sys::spawn::canonical(&outer))
            );
        }
        let lone = d.path().join("lone");
        std::fs::create_dir_all(&lone).unwrap();
        assert!(matches!(
            fgit::run(&lone, &GitOp::SymbolicRefHead, &GitOpts::default(), &ctx),
            Err(GitError::NotOwnRepo { found: None, .. })
        ));
        // `init` creates the project's own repo, after which ops run.
        assert!(
            fgit::run(&inner, &GitOp::Init, &GitOpts::default(), &ctx)
                .unwrap()
                .ok()
        );
        assert!(
            fgit::run(&inner, &GitOp::SymbolicRefHead, &GitOpts::default(), &ctx)
                .unwrap()
                .ok()
        );
        let _ = sha256_hex(b"");
    }

    /// SR-32 through `git::run`: the exclude writer gets its paths from git.
    #[test]
    fn exclude_through_git_run() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        common::init_repo(&repo);
        let root = Root::open(&repo, RootKind::Project).unwrap();
        let ctx = common::ctx(d.path());
        assert_eq!(
            fgit::ensure_exclude(&root, &ctx).unwrap(),
            foreman_core::store::git_exclude::Ensured::Appended
        );
        assert_eq!(
            fgit::ensure_exclude(&root, &ctx).unwrap(),
            foreman_core::store::git_exclude::Ensured::AlreadyPresent
        );
        let inner = repo.join("sub");
        std::fs::create_dir_all(&inner).unwrap();
        let r2 = Root::open(&inner, RootKind::Project).unwrap();
        assert!(
            fgit::ensure_exclude(&r2, &ctx).is_err(),
            "an enclosing repo is refused"
        );
    }
}
