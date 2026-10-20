//! foreman-sensor as Claude Code runs it (M4-07; SR-15, SR-53; AC-F011-03, AC-F011-04; T-045, T-048).
//! The built binary is hard-linked into `<tmp>/bin/foreman-sensor-<12 hex>` so it finds its app-data
//! folder exactly as it does when installed (ADR-009).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "integration-test code (tests/): unwrap/expect are denied only outside tests (SR-62)"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "integration tests run the sensor binary and inspect app data (test code, ADR-003)"
)]

mod sensor {
    use std::io::Write;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output, Stdio};
    use std::time::{Duration, Instant};

    use serde_json::Value;

    const RUN: &str = "0b5c9d8e-1f2a-4b3c-9d4e-5f6a7b8c9d0e";
    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const CAP: usize = 256 * 1024;
    const STOP: &str = r#"{"session_id":"s1","hook_event_name":"Stop","last_assistant_message":"done","stop_hook_active":false}"#;

    /// A fake `<app-data>` with the sensor installed under `bin/` and a private `events/<RUN>/`.
    struct AppData {
        _tmp: tempfile::TempDir,
        root: PathBuf,
        exe: PathBuf,
    }

    impl AppData {
        fn new() -> Self {
            // Same filesystem as the build output, so the binary can be hard-linked (a copy can
            // hit ETXTBSY when another test thread forks while the copy is open for writing).
            let tmp = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
            let root = tmp.path().join("Application Support");
            let bin = root.join("bin");
            std::fs::create_dir_all(&bin).unwrap();
            let exe = bin.join("foreman-sensor-0123456789ab");
            std::fs::hard_link(env!("CARGO_BIN_EXE_foreman-sensor"), &exe).unwrap();
            let me = Self {
                _tmp: tmp,
                root,
                exe,
            };
            private_dir(&me.root.join("events"));
            private_dir(&me.run_dir());
            me
        }

        fn run_dir(&self) -> PathBuf {
            self.root.join("events").join(RUN)
        }

        fn run(&self, args: &[&str], stdin: &[u8]) -> Output {
            run_exe(&self.exe, args, stdin)
        }

        fn send(&self, event: &str, stdin: &[u8]) -> Output {
            self.run(&args(RUN, TOKEN, event), stdin)
        }

        fn events(&self) -> Vec<String> {
            list(&self.run_dir())
        }
    }

    fn private_dir(p: &Path) {
        std::fs::create_dir_all(p).unwrap();
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn list(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        v.sort();
        v
    }

    fn args<'a>(run: &'a str, token: &'a str, event: &'a str) -> Vec<&'a str> {
        vec!["--run", run, "--token", token, "--event", event, "--v", "1"]
    }

    fn run_exe(exe: &Path, args: &[&str], stdin: &[u8]) -> Output {
        let mut child = Command::new(exe)
            .args(args)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut pipe = child.stdin.take().unwrap();
        let input = stdin.to_vec();
        // The sensor stops reading at the cap, so this write may fail with EPIPE; that's expected.
        let writer = std::thread::spawn(move || {
            let _ = pipe.write_all(&input);
        });
        let out = child.wait_with_output().unwrap();
        writer.join().unwrap();
        out
    }

    /// Exit 0, nothing on stdout or stderr (SR-15, AC-F011-04).
    fn assert_silent(out: &Output) {
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        assert!(out.stdout.is_empty(), "stdout: {:?}", out.stdout);
        assert!(
            out.stderr.is_empty(),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn read_event(dir: &Path, name: &str) -> Value {
        serde_json::from_slice(&std::fs::read(dir.join(name)).unwrap()).unwrap()
    }

    /// AC-F011-03, T-045, AB-12 step 1: an argument vector whose token isn't the per-run token's
    /// shape (64 lowercase hex) is refused and nothing is written. A well-formed but wrong token is
    /// discarded on ingest, which holds the run's token (ADR-009; `app::sensor_ingest`, M4-09).
    #[test]
    fn rejects_wrong_token() {
        let app = AppData::new();
        let upper = TOKEN.to_uppercase();
        let long = format!("{TOKEN}0");
        let nonhex = format!("{}g", &TOKEN[..63]);
        for token in ["", &TOKEN[..63], &long, &upper, &nonhex, "--v"] {
            let out = app.run(&args(RUN, token, "Stop"), STOP.as_bytes());
            assert_silent(&out);
            assert!(app.events().is_empty(), "token {token:?} was accepted");
        }
        let mut missing = args(RUN, TOKEN, "Stop");
        missing.drain(2..4);
        assert_silent(&app.run(&missing, STOP.as_bytes()));
        assert!(app.events().is_empty());
        // The real token goes through.
        assert_silent(&app.send("Stop", STOP.as_bytes()));
        assert_eq!(app.events().len(), 1);
    }

    /// AC-F011-04: one file `<20-digit ns>-<10-digit pid>-<Event>.json`, mode 0600, holding exactly
    /// the schema's fields (files.md §4.8) in canonical form, no temp file left, nothing printed.
    #[test]
    fn writes_one_event_file_0600() {
        let app = AppData::new();
        let out = app.send("Stop", STOP.as_bytes());
        assert_silent(&out);
        let names = app.events();
        assert_eq!(names.len(), 1, "{names:?}");
        let name = &names[0];
        let (seq, rest) = name.split_at(31);
        assert!(
            seq[..20].bytes().all(|b| b.is_ascii_digit()) && &seq[20..21] == "-",
            "{name}"
        );
        assert!(seq[21..].bytes().all(|b| b.is_ascii_digit()), "{name}");
        assert_eq!(rest, "-Stop.json");
        let meta = std::fs::symlink_metadata(app.run_dir().join(name)).unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.mode() & 0o7777, 0o600);
        assert_eq!(meta.nlink(), 1);

        let raw = std::fs::read_to_string(app.run_dir().join(name)).unwrap();
        assert!(raw.ends_with("}\n") && !raw.ends_with("\n\n"));
        let doc: Value = serde_json::from_str(&raw).unwrap();
        let keys: Vec<&str> = doc
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "event",
                "payload",
                "receivedAt",
                "schemaVersion",
                "token",
                "truncated"
            ]
        );
        assert_eq!(doc["schemaVersion"], 1);
        assert_eq!(doc["token"], TOKEN);
        assert_eq!(doc["event"], "Stop");
        assert_eq!(doc["truncated"], false);
        assert_eq!(doc["payload"], serde_json::from_str::<Value>(STOP).unwrap());
        let at = doc["receivedAt"].as_str().unwrap();
        assert_eq!(at.len(), 24, "{at}");
        assert!(
            at.starts_with("20") && at.ends_with('Z') && &at[10..11] == "T",
            "{at}"
        );
        // Canonical form (FILE-R1): two-space indent, keys sorted at every depth.
        assert!(
            raw.starts_with("{\n  \"event\": \"Stop\",\n  \"payload\": {\n    \"hook_event_name\"")
        );
    }

    /// Every event the run settings register is accepted and lands under its own name.
    #[test]
    fn writes_every_registered_event() {
        let app = AppData::new();
        let events = [
            "SessionStart",
            "Stop",
            "StopFailure",
            "Notification",
            "PermissionRequest",
            "SessionEnd",
            "SubagentStart",
            "SubagentStop",
            "TaskCreated",
            "TaskCompleted",
            "PostToolUse",
        ];
        for e in events {
            assert_silent(&app.send(e, br#"{"hook_event_name":"x"}"#));
        }
        let names = app.events();
        assert_eq!(names.len(), events.len());
        for e in events {
            let suffix = format!("-{e}.json");
            assert!(names.iter().any(|n| n.ends_with(&suffix)), "{e}");
        }
    }

    /// SR-15, T-048: at most 256 KB of stdin is read. Over the cap the event is `truncated` with a
    /// `null` payload (the cut JSON no longer parses); at the cap it's kept whole.
    #[test]
    fn stdin_cap() {
        let app = AppData::new();
        // Exactly at the cap, valid JSON: kept whole.
        let pad = CAP - r#"{"p":""}"#.len();
        let at_cap = format!(r#"{{"p":"{}"}}"#, "a".repeat(pad));
        assert_eq!(at_cap.len(), CAP);
        assert_silent(&app.send("PostToolUse", at_cap.as_bytes()));
        // 4 MB offered: the sensor stops at the cap and exits promptly anyway.
        let huge = format!(r#"{{"p":"{}"}}"#, "b".repeat(4 * 1024 * 1024));
        let start = Instant::now();
        assert_silent(&app.send("PostToolUse", huge.as_bytes()));
        assert!(start.elapsed() < Duration::from_secs(1));

        let names = app.events();
        assert_eq!(names.len(), 2);
        let first = read_event(&app.run_dir(), &names[0]);
        assert_eq!(first["truncated"], false);
        assert_eq!(first["payload"]["p"].as_str().unwrap().len(), pad);
        let second = read_event(&app.run_dir(), &names[1]);
        assert_eq!(second["truncated"], true);
        assert_eq!(second["payload"], Value::Null);
        let size = std::fs::metadata(app.run_dir().join(&names[1]))
            .unwrap()
            .len();
        assert!(size < 1024, "a cut payload isn't stored: {size} bytes");
    }

    /// AC-F011-04 with AB-12 step 5: a symlinked `events/<runId>` (or `events/`) is refused and the
    /// folder it points to stays empty.
    #[test]
    fn refuses_symlinked_run_dir() {
        let app = AppData::new();
        let elsewhere = tempfile::tempdir().unwrap();
        std::fs::set_permissions(elsewhere.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::remove_dir(app.run_dir()).unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), app.run_dir()).unwrap();
        assert_silent(&app.send("Stop", STOP.as_bytes()));
        assert!(list(elsewhere.path()).is_empty());

        let app = AppData::new();
        let events = app.root.join("events");
        let moved = app.root.join("real-events");
        std::fs::rename(&events, &moved).unwrap();
        std::os::unix::fs::symlink(&moved, &events).unwrap();
        assert_silent(&app.send("Stop", STOP.as_bytes()));
        assert!(list(&moved.join(RUN)).is_empty());
    }

    /// The sensor never creates `events/<runId>/`; a missing folder means nothing is written.
    #[test]
    fn refuses_missing_run_dir() {
        let app = AppData::new();
        std::fs::remove_dir(app.run_dir()).unwrap();
        assert_silent(&app.send("Stop", STOP.as_bytes()));
        assert!(!app.run_dir().exists());
        assert!(list(&app.root.join("events")).is_empty());
    }

    /// A group- or world-writable run folder isn't Foreman's (SR-53 makes it 0700): refused.
    #[test]
    fn refuses_shared_writable_run_dir() {
        let app = AppData::new();
        std::fs::set_permissions(app.run_dir(), std::fs::Permissions::from_mode(0o777)).unwrap();
        assert_silent(&app.send("Stop", STOP.as_bytes()));
        assert!(app.events().is_empty());
    }

    /// SR-53, AC-F011-07: only the newest 20 events of a run are kept.
    #[test]
    fn caps_events_at_20() {
        let app = AppData::new();
        let mut written = Vec::new();
        for _ in 0..25 {
            let before = app.events();
            assert_silent(&app.send("PostToolUse", STOP.as_bytes()));
            let new: Vec<String> = app
                .events()
                .into_iter()
                .filter(|n| !before.contains(n))
                .collect();
            assert_eq!(new.len(), 1);
            written.push(new[0].clone());
        }
        let names = app.events();
        assert_eq!(names.len(), 20);
        written.sort();
        assert_eq!(names, written[5..]);
    }

    /// Garbage on stdin or in argv: exit 0, print nothing, write nothing.
    #[test]
    fn garbage_input_exits_0_writes_nothing() {
        let app = AppData::new();
        for stdin in [
            &b""[..],
            b"not json",
            b"[1,2,3]",
            b"\"Stop\"",
            b"{\"unterminated\":",
            b"\xff\xfe\x00{}",
            b"null",
        ] {
            assert_silent(&app.send("Stop", stdin));
        }
        let traversal = "../../../tmp/x";
        for argv in [
            vec![],
            vec!["--help"],
            args(traversal, TOKEN, "Stop"),
            args("0B5C9D8E-1F2A-4B3C-9D4E-5F6A7B8C9D0E", TOKEN, "Stop"),
            args(RUN, TOKEN, "PreToolUse"),
            args(RUN, TOKEN, "Stop/../x"),
            {
                let mut v = args(RUN, TOKEN, "Stop");
                v[7] = "2";
                v
            },
            {
                let mut v = args(RUN, TOKEN, "Stop");
                v.push("--extra");
                v
            },
        ] {
            assert_silent(&app.run(&argv, STOP.as_bytes()));
        }
        assert!(app.events().is_empty());
        assert_eq!(list(&app.root.join("events")), [RUN]);
    }

    /// Only the installed layout (`<app-data>/bin/foreman-sensor-<12 hex>`) writes: the build-tree
    /// binary, run directly, writes nothing anywhere.
    #[test]
    fn writes_only_from_installed_path() {
        let tmp = tempfile::tempdir().unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_foreman-sensor"))
            .args(args(RUN, TOKEN, "Stop"))
            .current_dir(tmp.path())
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_silent(&out);
        assert!(list(tmp.path()).is_empty());
    }

    /// SR-15, T-048, AC-F011-04: the sensor exits within 10 ms of receiving its input. Median of
    /// 30 runs, spawn to exit (process start included), so a single scheduler hiccup doesn't fail
    /// it; `benches/sensor.rs` measures the same path with criterion.
    #[test]
    fn exits_under_10ms() {
        let app = AppData::new();
        let mut times: Vec<Duration> = (0..30)
            .map(|_| {
                let start = Instant::now();
                let out = app.send("Stop", STOP.as_bytes());
                let t = start.elapsed();
                assert_silent(&out);
                t
            })
            .collect();
        times.sort();
        let median = times[times.len() / 2];
        assert!(
            median < Duration::from_millis(10),
            "median {median:?}, all {times:?}"
        );
        assert_eq!(app.events().len(), 20);
    }
}
