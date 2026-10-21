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

/// SR-02 (M4-14): the single consented shell. Its argv is fixed (`-l -c 'printf %s "$PATH"'`),
/// it runs in app data with exactly the given environment, and the shell path gets SR-01's checks
/// (absolute, an executable regular file, not inside a project).
mod login_shell {
    use super::*;

    #[test]
    fn fixed_argv_in_app_data() {
        assert_eq!(sp::LOGIN_SHELL_ARGS, ["-l", "-c", "printf %s \"$PATH\""]);
        let d = tempfile::tempdir().unwrap();
        let app = d.path().join("app");
        let project = d.path().join("project");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        let ctx = SpawnCtx {
            home: d.path().into(),
            app_data: app.clone(),
            projects: vec![project.clone()],
            settings: vec![],
        };
        // A shell that reports its argv and cwd instead of PATH.
        let shell = d.path().join("bin/fakesh");
        std::fs::create_dir_all(shell.parent().unwrap()).unwrap();
        std::fs::write(
            &shell,
            "#!/bin/sh\nprintf '%s|' \"$@\"; pwd; printf '%s' \"$PATH\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();
        let req = |s: PathBuf| SpawnRequest::LoginShellPath {
            shell: s,
            consent: sp::LoginShellConsent::granted_in_native_dialog(),
        };
        let mut o = opts(vec![
            ("PATH".into(), "/usr/bin:/bin".into()),
            ("HOME".into(), d.path().display().to_string()),
        ]);
        o.cwd = Cwd::Dir(project.clone());
        // N-4 (security review 2026-10-20): a shell that /etc/shells doesn't list is refused by
        // the spawn module itself, whatever the caller checked.
        assert!(matches!(
            sp::run(&req(shell.clone()), &o, &ctx),
            Err(SpawnError::Unlisted(_))
        ));
        // A listed shell runs in app data (whatever cwd was asked) with the fixed argv: its login
        // start-up file records the folder it started in, and the output is `$PATH`.
        let listed = std::fs::read_to_string("/etc/shells")
            .unwrap_or_default()
            .lines()
            .any(|l| l.trim() == "/bin/sh");
        if listed {
            std::fs::write(d.path().join(".profile"), "pwd > \"$HOME/started-in\"\n").unwrap();
            let out = sp::run(&req(PathBuf::from("/bin/sh")), &o, &ctx).unwrap();
            assert_eq!(
                std::fs::read_to_string(d.path().join("started-in"))
                    .unwrap()
                    .trim_end(),
                sp::canonical(&app).display().to_string()
            );
            assert!(!String::from_utf8(out.stdout).unwrap().is_empty());
        } else {
            eprintln!("/bin/sh isn't in /etc/shells here; the listed-shell half needs it");
        }
        // SR-01 checks on the shell path.
        assert!(matches!(
            sp::run(&req(PathBuf::from("bin/sh")), &o, &ctx),
            Err(SpawnError::RelativePath(_))
        ));
        let inside = project.join("sh");
        std::fs::copy(&shell, &inside).unwrap();
        assert!(matches!(
            sp::run(&req(inside), &o, &ctx),
            Err(SpawnError::InsideProject(_))
        ));
        assert!(matches!(
            sp::run(&req(d.path().join("missing")), &o, &ctx),
            Err(SpawnError::NotExecutable(_))
        ));
    }

    /// N-4: `LoginShellConsent` must be public (the dialog is in foreman-app), so a source scan
    /// holds the rule that only `capture_login_path_with` makes one, right after the native
    /// dialog. Tests are the only other callers.
    #[test]
    fn consent_only_after_the_native_dialog() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut found = Vec::new();
        let mut stack = vec![root.join("crates"), root.join("src-tauri/src")];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    if p.file_name().is_some_and(|n| n != "tests" && n != "target") {
                        stack.push(p);
                    }
                } else if p.extension().is_some_and(|x| x == "rs")
                    && std::fs::read_to_string(&p)
                        .unwrap()
                        .contains("granted_in_native_dialog()")
                {
                    found.push(p.strip_prefix(&root).unwrap().display().to_string());
                }
            }
        }
        found.sort();
        assert_eq!(
            found,
            [
                "crates/foreman-sys/src/spawn.rs",
                "src-tauri/src/ipc/commands/doctor.rs"
            ]
        );
        let doctor =
            std::fs::read_to_string(root.join("src-tauri/src/ipc/commands/doctor.rs")).unwrap();
        let at = doctor.find("granted_in_native_dialog()").unwrap();
        let before = &doctor[..at];
        let dialog = before.rfind("app.os().confirm(").unwrap();
        let cancel = before.rfind("return Err(IpcError::cancelled());").unwrap();
        assert!(
            dialog < cancel,
            "the consent follows the dialog and its Cancel"
        );
    }
}

/// Security review 2026-10-20, N-2 (SR-05): "not inside a project" is checked on each PATH entry
/// as written and as resolved, with the project roots in both forms too.
mod env {
    use super::*;
    use foreman_core::launch::env::PathSources;

    #[test]
    fn path_symlink_into_project_is_dropped() {
        let d = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(d.path()).unwrap();
        let project = root.join("work/tradelog");
        std::fs::create_dir_all(project.join("node_modules/.bin")).unwrap();
        let usr = root.join("usr-bin");
        std::fs::create_dir_all(&usr).unwrap();
        // A folder outside the project that is a symlink into it.
        let link = root.join("tool-bin");
        symlink(project.join("node_modules/.bin"), &link).unwrap();
        // The project registered through a symlinked path.
        let alias = root.join("alias");
        symlink(&project, &alias).unwrap();
        let path_of = |projects: &[PathBuf], login: Vec<String>| {
            let sources = PathSources {
                etc_paths: vec![usr.display().to_string()],
                etc_paths_d: Vec::new(),
                home: root.join("home"),
                pnpm_home: None,
                login,
            };
            foreman_sys::launch::child_env(&[], &[], &sources, projects, uuid::Uuid::new_v4())
                .get("PATH")
                .unwrap()
                .to_owned()
        };
        let p = path_of(
            std::slice::from_ref(&project),
            vec![link.display().to_string()],
        );
        assert_eq!(
            p,
            usr.display().to_string(),
            "the symlink into the project is dropped"
        );
        let p = path_of(
            std::slice::from_ref(&alias),
            vec![project.join("node_modules/.bin").display().to_string()],
        );
        assert_eq!(
            p,
            usr.display().to_string(),
            "a project known by its alias still counts"
        );
        // Control: a symlink to a folder outside every project stays.
        let ok = root.join("ok-bin");
        symlink(&usr, &ok).unwrap();
        let p = path_of(
            std::slice::from_ref(&project),
            vec![ok.display().to_string()],
        );
        assert_eq!(p, format!("{}:{}", usr.display(), ok.display()));
    }
}
