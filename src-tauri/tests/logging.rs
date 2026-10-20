//! M4-01 (SR-54, SR-57, AC-XC-13, AC-XC-18, T-073): Foreman's own diagnostic log. A full run of
//! the P1 harness (chat copy, paste-back, a CLI session with typing and a paste, errors, a handler
//! panic and a thread panic) leaves `logs/foreman.log` with event names, ids, codes and timings
//! only: no prompt text, packet content, terminal output, clipboard or environment values, and no
//! panic payload. The file is `0600` and rotates by size, keeping five files.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::project::{ProjectApp, collect, git_identity, req};
use foreman_app::ipc::commands::terminal as term_cmd;
use foreman_app::ipc::commands::{chat, launch, projects, steps};
use foreman_app::ipc::dto::launch::PrepareOutcome;
use foreman_app::ipc::dto::requests::LaunchStartReq;
use foreman_app::ipc::run_caught;
use serde_json::{Value, json};

/// The log sink is process-wide: one test at a time attaches it.
static SERIAL: Mutex<()> = Mutex::new(());

const PACKET: &str = "=== HANDOFF PACKET · Stage 1 → Stage 2 ===
PROJECT: TradeLog
APPROVED BY: <blank>
APP: TradeLog — PACKET-CONTENT-31 journal that imports every fill
FOR: active options traders | also: coaches
PROBLEM: \"I lose track of why I took a trade\" (41 of 60 polled)
DOES:
  - imports fills from the broker
  - tags each trade with its setup
CORE LOOP: log after the close, review on Sunday
WOW MOMENT: the first import shows last month's P&L by setup
WITH (must integrate): broker CSV exports
WITH (constraints): evenings only, three months
STACK PREFERENCE: Svelte and Rust, decided in Stage 2
NOT DOING (v1): social features
MONETIZATION HYPOTHESIS: free tool for the trading room
SUCCESS METRICS: 30 weekly active members by week 6
KILL CRITERION: fewer than 10 weekly users after a month
RISKIEST ASSUMPTION: members log every week; v1 tests it with reminders
PRESS RELEASE + FAQ: attached below the packet
OPEN ITEMS: none
NEXT: Stage 2 · Architect's Design Studio · chat + Claude Code council";

fn wait_for(buf: &Arc<Mutex<Vec<u8>>>, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if String::from_utf8_lossy(&buf.lock().unwrap()).contains(needle) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for {needle:?}");
}

fn log_text(app_data: &Path) -> String {
    std::fs::read_to_string(app_data.join("logs/foreman.log")).unwrap_or_default()
}

/// A line without its timestamp and with ids and numbers generalized.
fn shape(line: &str) -> String {
    let rest = line.split_once(' ').map_or(line, |(_, r)| r);
    rest.split(' ')
        .map(|w| match w.split_once('=') {
            Some((k, v)) if v.len() == 36 && v.chars().filter(|c| *c == '-').count() == 4 => {
                format!("{k}=<id>")
            }
            Some((k, v)) if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()) => {
                format!("{k}=<n>")
            }
            Some(("at", _)) => "at=<file:line>".to_owned(),
            _ => w.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

mod logging {
    use super::*;

    /// SR-54, AC-XC-13, T-073: the log of a full harness run holds no content.
    #[test]
    fn redaction() {
        let _g = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        foreman_app::logging::install_subscriber();
        let p = ProjectApp::new(json!({}));
        let app = &p.t.app;
        let app_data = p.t.app_data();
        foreman_app::logging::attach(&app_data).unwrap();
        git_identity(&p);

        // Chat: run inputs reach the prompt, which goes to the clipboard; a packet comes back.
        for (name, value) in [
            ("MY_ADVANTAGES", "PROMPT-SECRET-42"),
            ("BUDGET_TIME", "Evenings, 3 months"),
            ("APP_NAME", "TradeLog"),
            ("PREFERRED_STACK", "Svelte and Rust"),
        ] {
            projects::variable_set(
                app,
                req(json!({ "projectId": p.id, "name": name, "value": value })),
            )
            .unwrap();
        }
        run_caught(
            app,
            req(json!({ "projectId": p.id, "step": "1.1", "entry": "stageView" })),
            chat::chat_copy_and_open,
        )
        .unwrap();
        let copied = p.t.os.clipboard().unwrap_or_default();
        assert!(
            copied.contains("PROMPT-SECRET-42"),
            "the fixture prompt has the marker"
        );
        p.t.os.set_clipboard(Some(PACKET));
        let pv = run_caught(
            app,
            req(json!({ "projectId": p.id, "step": "1.4", "kind": "auto", "source": "clipboard" })),
            chat::paste_back_preview,
        )
        .unwrap();
        let pv = serde_json::to_value(pv).unwrap();
        run_caught(
            app,
            req(json!({ "previewId": pv["previewId"] })),
            chat::paste_back_apply,
        )
        .unwrap();

        // CLI: a session, typing, a paste and its output.
        let PrepareOutcome::Ready(plan) = p.prepare("2.2") else {
            panic!("2.2 is blocked");
        };
        let (ch, buf) = collect();
        let start: LaunchStartReq = req(json!({ "planId": plan.plan_id }));
        let run = run_caught(app, (start, ch), |a, (r, c)| launch::launch_start(a, r, c)).unwrap();
        wait_for(&buf, "FAKE-CLAUDE READY");
        run_caught(
            app,
            req(json!({ "runId": run.run_id, "data": "TYPED-KEYS-91\r" })),
            term_cmd::pty_write,
        )
        .unwrap();
        p.t.os.set_clipboard(Some("CLIPBOARD-SECRET-7c"));
        run_caught(
            app,
            req(json!({ "runId": run.run_id })),
            term_cmd::pty_paste,
        )
        .unwrap();
        // Errors, with values in the requests that must not be echoed.
        let _ = run_caught(
            app,
            req::<foreman_app::ipc::dto::requests::StepViewReq>(
                json!({ "projectId": p.id, "step": "9.99" }),
            ),
            steps::step_view,
        );
        p.t.os.set_clipboard(Some(&"Q".repeat(1024 * 1024 + 1)));
        let _ = run_caught(
            app,
            req(json!({ "runId": run.run_id })),
            term_cmd::pty_paste,
        );
        let _: Result<(), _> = run_caught(app, (), |_, ()| panic!("PANIC-PAYLOAD-5d"));
        run_caught(
            app,
            req(json!({ "runId": run.run_id })),
            term_cmd::pty_terminate,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(500));
        // A panic outside a command: the hook logs its location only.
        foreman_app::logging::install_panic_hook();
        let crashed = std::thread::Builder::new()
            .name("fm-test-crash".into())
            .spawn(|| panic!("THREAD-PAYLOAD-8e"))
            .unwrap()
            .join();
        let _ = std::panic::take_hook();
        assert!(crashed.is_err());
        foreman_app::logging::detach();

        let log = log_text(&app_data);
        assert!(!log.is_empty(), "the run was logged");
        let path_env = std::env::var("PATH").unwrap_or_default();
        let mut forbidden = vec![
            "PROMPT-SECRET-42",
            "PACKET-CONTENT-31",
            "imports every fill",
            "FAKE-CLAUDE",
            "TYPED-KEYS-91",
            "CLIPBOARD-SECRET-7c",
            "QQQQ",
            "PANIC-PAYLOAD-5d",
            "THREAD-PAYLOAD-8e",
            "9.99",
        ];
        if path_env.len() > 8 {
            forbidden.push(path_env.as_str());
        }
        for f in &forbidden {
            assert!(!log.contains(f), "{f:?} reached the log:\n{log}");
        }
        // Every line is `<time> <LEVEL> <target>: <fixed message> name=value…`.
        for line in log.lines() {
            let mut w = line.split(' ');
            assert!(
                w.next().unwrap().parse::<jiff::Timestamp>().is_ok(),
                "{line}"
            );
            assert!(
                ["INFO", "WARN", "ERROR"].contains(&w.next().unwrap()),
                "{line}"
            );
            assert!(w.next().unwrap().ends_with(':'), "{line}");
        }
        // The snapshot: the event kinds a harness run produces.
        let shapes: BTreeSet<String> = log.lines().map(shape).collect();
        for want in [
            "INFO launch: run record written run_id=<id> step=2.2",
            "INFO launch: spawned run_id=<id> ms=<n>",
            "INFO ipc: command command=step_view code=notFound ms=<n>",
            "INFO ipc: command command=pty_paste code=clipboard ms=<n>",
            "ERROR panic: panic at=<file:line> thread=fm-test-crash",
        ] {
            assert!(shapes.contains(want), "{want:?} missing from {shapes:#?}");
        }
        assert!(
            shapes
                .iter()
                .any(|s| s.starts_with("INFO ipc: command command=") && s.contains("code=internal")),
            "{shapes:#?}"
        );
        assert!(
            shapes
                .iter()
                .any(|s| s.starts_with("INFO run: exited run_id=<id>")),
            "{shapes:#?}"
        );
        let mode = std::fs::metadata(app_data.join("logs/foreman.log"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(app_data.join("logs"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    /// SR-54: 10 MB × 5 by size (here with a small limit): the newest lines stay, five files at most.
    #[test]
    fn rotates_by_size() {
        let _g = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        foreman_app::logging::install_subscriber();
        let d = tempfile::tempdir().unwrap();
        foreman_app::logging::attach_with_limit(d.path(), 2_000).unwrap();
        for i in 0..200_u32 {
            tracing::info!(target: "test", i, "rotation line");
        }
        foreman_app::logging::detach();
        let logs = d.path().join("logs");
        let mut names: Vec<String> = std::fs::read_dir(&logs)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "foreman.log",
                "foreman.log.1",
                "foreman.log.2",
                "foreman.log.3",
                "foreman.log.4"
            ]
        );
        for n in &names {
            let len = std::fs::metadata(logs.join(n)).unwrap().len();
            assert!(len <= 2_000, "{n}: {len}");
        }
        assert!(log_text(d.path()).contains("i=199"));
        assert!(
            !std::fs::read_to_string(logs.join("foreman.log.4"))
                .unwrap()
                .contains("i=0 ")
        );
        // The constants are SR-54's.
        assert_eq!(foreman_app::logging::MAX_BYTES, 10 * 1024 * 1024);
        assert_eq!(foreman_app::logging::KEEP + 1, 5);
    }

    /// SR-54: content fields are redacted even when a caller names one by mistake.
    #[test]
    fn content_fields_redacted() {
        let _g = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        foreman_app::logging::install_subscriber();
        let d = tempfile::tempdir().unwrap();
        foreman_app::logging::attach(d.path()).unwrap();
        let secret = "SECRET-FIELD-VALUE";
        tracing::info!(target: "test", prompt = secret, packet_text = secret, env_value = secret, clipboard = secret, value = secret, output = secret, step = "2.3", "fields");
        tracing::warn!(target: "test", body = ?secret, "debug-formatted");
        foreman_app::logging::detach();
        let log = log_text(d.path());
        assert!(!log.contains(secret), "{log}");
        assert!(
            log.contains("prompt=‹redacted›") && log.contains("step=2.3"),
            "{log}"
        );
        assert!(log.contains("body=‹redacted›"), "{log}");
    }

    /// SR-54: every `tracing` call in the app has a literal message and no formatted arguments,
    /// so content can only appear as a named field (which the layer filters).
    #[test]
    fn messages_are_literals() {
        fn walk(dir: &Path, out: &mut Vec<(String, String)>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push((
                        p.display().to_string(),
                        std::fs::read_to_string(&p).unwrap(),
                    ));
                }
            }
        }
        let mut files = Vec::new();
        walk(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let mut calls = 0;
        for (path, text) in files {
            let mut rest = text.as_str();
            while let Some(i) = rest.find("tracing::") {
                rest = &rest[i + "tracing::".len()..];
                let Some(mac) = [
                    "info!(", "warn!(", "error!(", "debug!(", "trace!(", "event!(",
                ]
                .iter()
                .find(|m| rest.starts_with(**m)) else {
                    continue;
                };
                calls += 1;
                let body = &rest[mac.len()..];
                let end = body.find(");").unwrap();
                let args = &body[..end];
                assert!(
                    !mac.starts_with("debug") && !mac.starts_with("trace"),
                    "{path}: debug/trace events are not kept"
                );
                // The message is the last argument, a string literal without `{}` placeholders.
                let last = args.rsplit(',').next().unwrap().trim();
                assert!(
                    last.starts_with('"') && last.ends_with('"') && !last.contains('{'),
                    "{path}: message must be a plain literal: {args}"
                );
                assert!(!args.contains("format!"), "{path}: {args}");
            }
        }
        assert!(calls >= 6, "{calls}");
    }

    /// AC-XC-18: a crash leaves a log entry and the live runs `interrupted` at the next start;
    /// nothing is uploaded (no network client in the tree: `cargo deny` bans, ADR-016).
    #[test]
    fn crash_report_local() {
        let _g = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        foreman_app::logging::install_subscriber();
        let p = ProjectApp::new(json!({}));
        let app_data = p.t.app_data();
        let PrepareOutcome::Ready(plan) = p.prepare("2.2") else {
            panic!("2.2 is blocked");
        };
        let (ch, buf) = collect();
        let run =
            launch::launch_start(&p.t.app, req(json!({ "planId": plan.plan_id })), ch).unwrap();
        wait_for(&buf, "FAKE-CLAUDE READY");
        // The "crash": a second app over the same data, as after a restart, while the first
        // app's record still says running.
        let run_id = serde_json::to_value(&run.run_id)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let run_file = app_data.join("runs").join(&run_id).join("run.json");
        let before: Value =
            serde_json::from_str(&std::fs::read_to_string(&run_file).unwrap()).unwrap();
        assert_eq!(before["status"], "running");
        let restarted = foreman_app::state::App::new(
            Arc::new(foreman_app::os::ScriptedOs::default()),
            foreman_app::state::Paths {
                app_data: app_data.clone(),
                home: p.t.home(),
            },
        );
        foreman_app::logging::attach(&app_data).unwrap();
        let interrupted = foreman_app::runs::interrupted(&restarted).unwrap();
        foreman_app::logging::detach();
        assert!(
            interrupted
                .iter()
                .any(|r| serde_json::to_value(&r.run_id).unwrap() == run_id.as_str())
        );
        let after: Value =
            serde_json::from_str(&std::fs::read_to_string(&run_file).unwrap()).unwrap();
        assert_eq!(after["status"], "interrupted");
        let log = log_text(&app_data);
        assert!(
            log.lines()
                .any(|l| shape(l) == "WARN run: interrupted by a crash or forced quit run_id=<id>"),
            "{log}"
        );
        let _ = term_cmd::pty_terminate(&p.t.app, req(json!({ "runId": run.run_id })));
    }
}
