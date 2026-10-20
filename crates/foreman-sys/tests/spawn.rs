//! M1-07: the spawn module core (ADR-007, SR-01, SR-02, SR-57, AC-XC-02).
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

use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::time::Duration;

use foreman_sys::spawn::{self as sp, Cwd, Program, RunOpts, SpawnCtx, SpawnError, SpawnRequest};

fn opts(env: Vec<(String, String)>) -> RunOpts {
    RunOpts {
        env,
        timeout: Duration::from_secs(10),
        output_cap: 1 << 20,
        cwd: Cwd::AppData,
    }
}

fn record(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

mod spawn {
    use super::*;

    #[test]
    fn rejects_relative_path() {
        let ctx = SpawnCtx {
            settings: vec![(Program::Claude, PathBuf::from("bin/claude"))],
            ..SpawnCtx::default()
        };
        assert_eq!(
            sp::resolve(Program::Claude, &ctx),
            Err(SpawnError::RelativePath("bin/claude".into()))
        );
        let r = sp::run(&SpawnRequest::ClaudeVersion, &opts(vec![]), &ctx);
        assert!(matches!(r, Err(SpawnError::RelativePath(_))));
        // A request with a relative path argument is refused too.
        let r = sp::run(
            &SpawnRequest::OpenReveal(PathBuf::from("docs/x.md")),
            &opts(vec![]),
            &ctx,
        );
        assert!(matches!(r, Err(SpawnError::InvalidRequest(_))));
    }

    #[test]
    fn rejects_unlisted_program() {
        let d = tempfile::tempdir().unwrap();
        // A real executable that isn't named like the program: refused.
        let sh = common::install_fake(&d.path().join("bin"), "sh", &serde_json::json!({}));
        let ctx = SpawnCtx {
            home: d.path().into(),
            app_data: d.path().into(),
            settings: vec![
                (Program::Claude, sh.clone()),
                (Program::Git, PathBuf::from("/bin/sh")),
            ],
            ..SpawnCtx::default()
        };
        assert_eq!(
            sp::resolve(Program::Claude, &ctx),
            Err(SpawnError::Unlisted(sh.display().to_string()))
        );
        assert!(matches!(
            sp::resolve(Program::Git, &ctx),
            Err(SpawnError::Unlisted(_))
        ));
        // `open` can't be pointed elsewhere by Settings, and --version is only for the tool probes.
        let ctx2 = SpawnCtx {
            settings: vec![(Program::Open, PathBuf::from("/tmp/open"))],
            home: d.path().into(),
            ..SpawnCtx::default()
        };
        let r = sp::resolve(Program::Open, &ctx2);
        assert!(r.is_err() || r == Ok(PathBuf::from("/usr/bin/open")));
        assert!(matches!(
            sp::run(
                &SpawnRequest::ToolVersion(Program::Git),
                &opts(vec![]),
                &ctx
            ),
            Err(SpawnError::InvalidRequest(_))
        ));
        // A missing or non-executable file is refused (the check runs before every spawn).
        let gone = d.path().join("bin2/claude");
        let ctx3 = SpawnCtx {
            settings: vec![(Program::Claude, gone)],
            ..ctx.clone()
        };
        assert!(matches!(
            sp::resolve(Program::Claude, &ctx3),
            Err(SpawnError::NotExecutable(_))
        ));
        // No candidate at all: unlisted (the fixed candidates under an empty home don't exist).
        let empty = SpawnCtx {
            home: d.path().join("nohome"),
            ..SpawnCtx::default()
        };
        assert!(matches!(
            sp::resolve(Program::Cargo, &empty),
            Err(SpawnError::Unlisted(_))
        ));
    }

    #[test]
    fn rejects_symlink_into_project() {
        let d = tempfile::tempdir().unwrap();
        let project = d.path().join("project");
        std::fs::create_dir_all(project.join("node_modules/.bin")).unwrap();
        let inside = common::install_fake(
            &project.join("node_modules/.bin"),
            "claude",
            &serde_json::json!({}),
        );
        let bin = d.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        symlink(&inside, bin.join("claude")).unwrap();
        let ctx = SpawnCtx {
            home: d.path().into(),
            app_data: d.path().into(),
            projects: vec![project.clone()],
            settings: vec![(Program::Claude, bin.join("claude"))],
        };
        assert!(matches!(
            sp::resolve(Program::Claude, &ctx),
            Err(SpawnError::InsideProject(_))
        ));
        let ctx2 = SpawnCtx {
            settings: vec![(Program::Claude, inside)],
            ..ctx.clone()
        };
        assert!(matches!(
            sp::resolve(Program::Claude, &ctx2),
            Err(SpawnError::InsideProject(_))
        ));
        // A world-writable (non-sticky) folder is refused too.
        let ww = d.path().join("ww");
        let p = common::install_fake(&ww, "claude", &serde_json::json!({}));
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&ww, std::fs::Permissions::from_mode(0o777)).unwrap();
        let ctx3 = SpawnCtx {
            settings: vec![(Program::Claude, p)],
            projects: vec![],
            ..ctx
        };
        assert!(matches!(
            sp::resolve(Program::Claude, &ctx3),
            Err(SpawnError::WorldWritable(_))
        ));
    }

    /// T-038 / AC-M1-07: every non-session spawn runs with its cwd in app data, whatever is asked.
    #[test]
    fn helper_cwd_is_app_data() {
        let d = tempfile::tempdir().unwrap();
        let app = d.path().join("app");
        let project = d.path().join("project");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        let rec = d.path().join("rec.json");
        let claude = common::install_fake(
            &d.path().join("bin"),
            "claude",
            &serde_json::json!({ "record": rec, "version": "2.1.304 (Claude Code)" }),
        );
        let ctx = SpawnCtx {
            home: d.path().into(),
            app_data: app.clone(),
            projects: vec![project.clone()],
            settings: vec![(Program::Claude, claude)],
        };
        let mut o = opts(vec![("HOME".into(), d.path().display().to_string())]);
        o.cwd = Cwd::Dir(project.clone());
        for req in [SpawnRequest::ClaudeVersion, SpawnRequest::ClaudeAuthStatus] {
            let out = sp::run(&req, &o, &ctx).unwrap();
            assert_eq!(out.code, Some(0));
            let r = record(&rec);
            assert_eq!(
                std::path::Path::new(r["cwd"].as_str().unwrap()),
                sp::canonical(&app),
                "{req:?}"
            );
        }
        let out = sp::run(&SpawnRequest::ClaudeVersion, &o, &ctx).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            "2.1.304 (Claude Code)"
        );
        // The environment is exactly what was given (env_clear first).
        let r = record(&rec);
        let keys: Vec<&String> = r["env"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["HOME"]);
    }

    /// SR-57 / T-034: the child inherits no fds above 2 from Foreman's own open files.
    #[test]
    fn no_inherited_fds() {
        let d = tempfile::tempdir().unwrap();
        let rec = d.path().join("rec.json");
        let claude = common::install_fake(
            &d.path().join("bin"),
            "claude",
            &serde_json::json!({ "record": rec }),
        );
        // Foreman holds files open (std and rustix both open with O_CLOEXEC).
        let _f1 = std::fs::File::create(d.path().join("held.txt")).unwrap();
        let _f2 = rustix::fs::open(
            d.path(),
            rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .unwrap();
        let ctx = SpawnCtx {
            home: d.path().into(),
            app_data: d.path().into(),
            projects: vec![],
            settings: vec![(Program::Claude, claude)],
        };
        sp::run(&SpawnRequest::ClaudeVersion, &opts(vec![]), &ctx).unwrap();
        let fds: Vec<u64> = record(&rec)["fds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        // 0, 1, 2 and the one fd the child used to list its own fd table.
        assert!(fds.iter().filter(|f| **f > 2).count() <= 1, "{fds:?}");
        assert!(fds.contains(&0) && fds.contains(&1) && fds.contains(&2));
    }

    /// Timeouts kill the process group; output is capped.
    #[test]
    fn timeout_and_cap() {
        let d = tempfile::tempdir().unwrap();
        let claude = common::install_fake(
            &d.path().join("bin"),
            "claude",
            &serde_json::json!({ "sleepMs": 5000 }),
        );
        let ctx = SpawnCtx {
            home: d.path().into(),
            app_data: d.path().into(),
            projects: vec![],
            settings: vec![(Program::Claude, claude)],
        };
        let mut o = opts(vec![]);
        o.timeout = Duration::from_millis(200);
        let t = std::time::Instant::now();
        assert_eq!(
            sp::run(&SpawnRequest::ClaudeVersion, &o, &ctx),
            Err(SpawnError::Timeout(Duration::from_millis(200)))
        );
        assert!(t.elapsed() < Duration::from_secs(3));
        let big = d.path().join("big");
        let claude = common::install_fake(
            &big,
            "claude",
            &serde_json::json!({ "version": "x".repeat(4096) }),
        );
        let ctx = SpawnCtx {
            settings: vec![(Program::Claude, claude)],
            ..ctx
        };
        let mut o = opts(vec![]);
        o.output_cap = 100;
        assert_eq!(
            sp::run(&SpawnRequest::ClaudeVersion, &o, &ctx),
            Err(SpawnError::OutputTooLarge(100))
        );
    }
}
