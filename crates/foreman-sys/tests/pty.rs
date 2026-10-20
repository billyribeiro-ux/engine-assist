//! M3-09, M3-10, M3-12, M2-08: `PtySession` against fake-claude's TUI emulation in a real PTY
//! (ADR-006, SR-24, SR-28, SR-57, NFR-08).
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

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use common::Collect;
use foreman_sys::pty::{self as fpty, PtyError, PtySession, PtyStatus};
use foreman_sys::spawn::{Program, SpawnCtx, SpawnError};
use uuid::Uuid;

struct Rig {
    _dir: tempfile::TempDir,
    root: PathBuf,
    record: PathBuf,
    ctx: SpawnCtx,
    run: Uuid,
}

fn rig(script: serde_json::Value) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let root = common::canon(dir.path());
    let record = root.join("rec.json");
    let mut cfg = script;
    cfg["record"] = serde_json::json!(record);
    let claude = common::install_fake(&root.join("bin"), "claude", &cfg);
    std::fs::create_dir_all(root.join("home")).unwrap();
    std::fs::create_dir_all(root.join("repo")).unwrap();
    Rig {
        ctx: SpawnCtx {
            home: root.join("home"),
            app_data: root.join("app"),
            projects: vec![root.join("repo")],
            settings: vec![(Program::Claude, claude)],
        },
        _dir: dir,
        record,
        root,
        run: Uuid::new_v4(),
    }
}

fn spawn(r: &Rig, args: &[&str], sink: &Arc<Collect>) -> PtySession {
    let env = common::child_env(&r.root.join("home"), r.run);
    let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let s = PtySession::spawn(
        &r.ctx,
        &args,
        &env.0,
        &r.root.join("repo"),
        fpty::size(100, 30).unwrap(),
        Arc::clone(sink) as Arc<dyn fpty::Sink>,
    )
    .unwrap();
    sink.wait_for("FAKE-CLAUDE READY", 10);
    s
}

fn record(p: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

const SID: &str = "6f1c2a3b-1d2e-4f50-8a9b-0c1d2e3f4a5b";

mod pty {
    use super::*;

    /// SR-05 / SR-57: the child sees exactly the built environment (portable-pty's `SHELL` is
    /// the one from the allowlist), the argv and the cwd.
    #[test]
    fn child_env_exact() {
        let r = rig(serde_json::json!({}));
        let sink = Arc::new(Collect::default());
        let s = spawn(&r, &["--session-id", SID, "--model", "opus"], &sink);
        let rec = record(&r.record);
        let env = common::child_env(&r.root.join("home"), r.run);
        let got: std::collections::BTreeMap<String, String> =
            serde_json::from_value(rec["env"].clone()).unwrap();
        let want: std::collections::BTreeMap<String, String> = env.0.iter().cloned().collect();
        assert_eq!(got, want);
        assert!(!got.contains_key("ANTHROPIC_API_KEY"));
        assert_eq!(rec["cwd"], r.root.join("repo").display().to_string());
        assert_eq!(rec["argv"][1], "--session-id");
        s.hangup();
        assert!(s.wait_exit(Duration::from_secs(5)).is_some());
    }

    /// SR-57: no descriptor above 2 reaches the PTY child.
    #[test]
    fn no_inherited_fds() {
        let r = rig(serde_json::json!({}));
        let _held = std::fs::File::create(r.root.join("held.txt")).unwrap();
        let sink = Arc::new(Collect::default());
        let s = spawn(&r, &["--session-id", SID], &sink);
        let fds: Vec<u64> = record(&r.record)["fds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert!(fds.iter().filter(|f| **f > 2).count() <= 1, "{fds:?}");
        s.hangup();
    }

    /// NFR-08: with the flow paused, a 1 MB burst stalls in the kernel PTY buffer (the child
    /// blocks, nothing is lost, memory stays bounded); resuming delivers everything.
    #[test]
    fn burst_1mb_backpressure() {
        let r = rig(serde_json::json!({ "tui": { "burstBytes": 1_048_576 } }));
        let sink = Arc::new(Collect::default());
        let env = common::child_env(&r.root.join("home"), r.run);
        let s = PtySession::spawn(
            &r.ctx,
            &["--session-id".to_owned(), SID.to_owned()],
            &env.0,
            &r.root.join("repo"),
            fpty::size(100, 30).unwrap(),
            Arc::clone(&sink) as Arc<dyn fpty::Sink>,
        )
        .unwrap();
        s.set_flow(true);
        std::thread::sleep(Duration::from_millis(400));
        let a = sink.len();
        std::thread::sleep(Duration::from_millis(400));
        let b = sink.len();
        assert!(
            a < 1_048_576,
            "paused reader still delivered everything ({a})"
        );
        assert!(
            b <= a + 64 * 1024,
            "reader kept reading while paused ({a} → {b})"
        );
        assert!(s.is_live(), "the child is blocked, not gone");
        s.set_flow(false);
        sink.wait_for("BURST-END", 20);
        let xs = sink
            .buf
            .lock()
            .unwrap()
            .iter()
            .filter(|c| **c == b'x')
            .count();
        assert!(xs >= 1_000_000, "{xs}");
        s.hangup();
    }

    /// SR-24: writes only while live; > 64 KB refused.
    #[test]
    fn write_requires_live_run() {
        let r = rig(serde_json::json!({}));
        let sink = Arc::new(Collect::default());
        let s = spawn(&r, &["--session-id", SID], &sink);
        assert_eq!(s.write(&vec![b'a'; 64 * 1024 + 1]), Err(PtyError::TooLarge));
        s.write(b"hello\r").unwrap();
        sink.wait_for("ECHO: hello", 10);
        s.write(b"/exit\r").unwrap();
        let info = s.wait_exit(Duration::from_secs(10)).unwrap();
        assert_eq!(info.code, Some(0));
        assert!(matches!(s.status(), PtyStatus::Exited(_)));
        assert_eq!(s.write(b"x"), Err(PtyError::NotLive));
        assert_eq!(s.interrupt(), Err(PtyError::NotLive));
        assert!(sink.exit.lock().unwrap().is_some());
    }

    /// SR-28: the fixture `ESC[201~ ^C !touch /tmp/pwn CR`, sanitized in Rust and pasted the way
    /// xterm does, arrives as one bracketed block with no ESC inside; nothing is typed outside it.
    #[test]
    fn paste_strips_escape_sequences() {
        let r = rig(serde_json::json!({}));
        let sink = Arc::new(Collect::default());
        let s = spawn(&r, &["--session-id", SID], &sink);
        let fixture = "\u{1b}[201~\u{3}!touch /tmp/pwn\r";
        assert_eq!(fixture.len(), 23);
        let p = fpty::sanitize_paste(fixture);
        assert_eq!(p.removed, 2);
        assert_eq!(p.notice.as_deref(), Some("removed 2 control characters"));
        assert!(!p.text.contains('\u{1b}') && !p.text.contains('\u{3}'));
        s.write(&fpty::bracketed(&p.text)).unwrap();
        sink.wait_for("PASTE", 10);
        std::thread::sleep(Duration::from_millis(200));
        let log = common::tui_log(&r.record);
        let pastes: Vec<_> = log.iter().filter(|e| e["type"] == "paste").collect();
        assert_eq!(pastes.len(), 1, "{log:?}");
        assert_eq!(pastes[0]["hasEsc"], false);
        assert_eq!(pastes[0]["text"], "[201~!touch /tmp/pwn\r");
        assert!(
            !log.iter()
                .any(|e| e["type"] == "ctrlC" || e["type"] == "submit"),
            "keystrokes escaped the paste: {log:?}"
        );
        // Control: the unsanitized fixture would end the paste early and type the rest.
        s.write(&fpty::bracketed(fixture)).unwrap();
        std::thread::sleep(Duration::from_millis(400));
        let log = common::tui_log(&r.record);
        assert!(
            log.iter().any(|e| e["type"] == "ctrlC"),
            "control case: {log:?}"
        );
        s.hangup();
    }

    /// Output, input and resize through the session; the tail replays (`pty_attach`).
    #[test]
    fn spawn_output_input_resize() {
        let r = rig(serde_json::json!({}));
        let sink = Arc::new(Collect::default());
        let s = spawn(&r, &["--session-id", SID, "do the thing"], &sink);
        sink.wait_for("ECHO: do the thing", 10);
        s.resize(fpty::size(132, 40).unwrap()).unwrap();
        sink.wait_for("RESIZED 132x40", 10);
        assert_eq!(fpty::size(1, 40), Err(PtyError::BadSize));
        assert_eq!(fpty::size(80, 501), Err(PtyError::BadSize));
        let (tail, lines) = s.tail();
        assert!(String::from_utf8_lossy(&tail).contains("ECHO: do the thing"));
        assert!(lines >= 2);
        s.interrupt().unwrap();
        s.write(b"/exit\r").unwrap();
        assert_eq!(s.wait_exit(Duration::from_secs(10)).unwrap().code, Some(0));
        let log = common::tui_log(&r.record);
        let kinds: Vec<&str> = log.iter().filter_map(|e| e["type"].as_str()).collect();
        assert!(
            kinds.contains(&"resize") && kinds.contains(&"ctrlC"),
            "{kinds:?}"
        );
    }

    /// ADR-006: a cwd that isn't a canonical directory is refused (no `$HOME` fallback).
    #[test]
    fn refuses_missing_cwd() {
        let r = rig(serde_json::json!({}));
        let env = common::child_env(&r.root.join("home"), r.run);
        let sink: Arc<dyn fpty::Sink> = Arc::new(Collect::default());
        std::os::unix::fs::symlink(r.root.join("repo"), r.root.join("link")).unwrap();
        for cwd in [
            r.root.join("missing"),
            r.root.join("link"),
            PathBuf::from("rel"),
        ] {
            let e = PtySession::spawn(
                &r.ctx,
                &[],
                &env.0,
                &cwd,
                fpty::size(80, 24).unwrap(),
                Arc::clone(&sink),
            )
            .unwrap_err();
            assert!(
                matches!(e, PtyError::Spawn(SpawnError::InvalidRequest(_))),
                "{cwd:?}: {e:?}"
            );
        }
        // Forbidden or unknown flags never reach a PTY.
        for bad in [["--print"], ["--dangerously-skip-permissions"], ["--bogus"]] {
            let args: Vec<String> = bad.iter().map(|s| (*s).to_owned()).collect();
            assert!(
                PtySession::spawn(
                    &r.ctx,
                    &args,
                    &env.0,
                    &r.root.join("repo"),
                    fpty::size(80, 24).unwrap(),
                    Arc::clone(&sink)
                )
                .is_err()
            );
        }
    }

    /// Exit codes and Terminate (SIGTERM to the group); the quit path's SIGHUP.
    #[test]
    fn exit_codes() {
        let r = rig(serde_json::json!({ "tui": { "exitCode": 3 } }));
        let sink = Arc::new(Collect::default());
        let s = spawn(&r, &["--session-id", SID], &sink);
        s.write(b"/exit\r").unwrap();
        assert_eq!(s.wait_exit(Duration::from_secs(10)).unwrap().code, Some(3));
        let sink = Arc::new(Collect::default());
        let s = spawn(&r, &["--session-id", SID], &sink);
        let info = s.terminate(Duration::from_secs(5)).unwrap();
        assert!(info.code.is_none() || info.signaled, "{info:?}");
        let sink = Arc::new(Collect::default());
        let s = spawn(&r, &["--session-id", SID], &sink);
        s.hangup();
        assert!(s.wait_exit(Duration::from_secs(5)).is_some());
    }
}
