//! M3-06 (SR-56, AC-XC-29, T-099): the clipboard is read only in Rust, only by the three commands
//! that are Billy's explicit actions (`pty_paste`, `paste_back_preview { source: "clipboard" }`,
//! `chat_link_set { source: clipboard }`), as text, capped at 1 MB, never polled.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::project::{ProjectApp, collect, req};
use foreman_app::ipc::commands::terminal as term_cmd;
use foreman_app::ipc::commands::{app as app_cmd, chat, launch, projects, runs, settings, steps};
use foreman_app::ipc::dto::launch::{PrepareOutcome, RunStarted};
use foreman_app::ipc::error::{ClipboardProblem, IpcErrorCode, IpcErrorDetail};
use foreman_app::os::{CLIPBOARD_MAX, check_clipboard};
use serde_json::json;

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

fn start(p: &ProjectApp, step: &str) -> (RunStarted, Arc<Mutex<Vec<u8>>>) {
    let PrepareOutcome::Ready(pv) = p.prepare(step) else {
        panic!("{step} is blocked");
    };
    let (ch, buf) = collect();
    let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
    wait_for(&buf, "FAKE-CLAUDE READY");
    (r, buf)
}

fn src(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
}

/// Every `.rs` file under `src-tauri/src`, with its path relative to `src`.
fn app_sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, String)>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, base, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let rel = p.strip_prefix(base).unwrap().display().to_string();
                out.push((rel, std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    walk(&base, &base, &mut out);
    out
}

fn clip_problem(e: &foreman_app::ipc::IpcError) -> Option<ClipboardProblem> {
    match &e.detail {
        Some(IpcErrorDetail::Clipboard { why }) => Some(*why),
        _ => None,
    }
}

mod clipboard {
    use super::*;

    /// SR-56, AC-XC-29: nothing reads the clipboard except Billy's three explicit actions.
    #[test]
    fn read_only_on_user_action() {
        // Source: the clipboard is read in exactly three places, each the clipboard branch of an
        // explicit command, and only `Os` touches the OS clipboard API (text only).
        let mut sites = Vec::new();
        for (rel, text) in app_sources() {
            for (n, line) in text.lines().enumerate() {
                if line.contains(".clipboard_read()") {
                    sites.push(format!("{rel}:{}", n + 1));
                }
                if line.contains(".read_text()") || line.contains("read_image") {
                    assert_eq!(rel, "os.rs", "only app::os reads the OS clipboard: {rel}");
                }
            }
        }
        let files: Vec<&str> = sites.iter().map(|s| s.split(':').next().unwrap()).collect();
        assert_eq!(
            files,
            [
                "ipc/commands/chat.rs",
                "ipc/commands/chat.rs",
                "ipc/commands/terminal.rs"
            ],
            "{sites:?}"
        );
        let chat_src = src("src/ipc/commands/chat.rs");
        assert!(chat_src.contains("LinkSource::Clipboard(_) => app.os().clipboard_read()"));
        assert!(chat_src.contains("PasteSource::Clipboard => app.os().clipboard_read()"));
        let os_src = src("src/os.rs");
        assert!(os_src.contains(".read_text()"));
        assert!(!os_src.contains("read_image"), "text only");
        // The webview has no clipboard permission (SR-22) and no clipboard plugin of its own.
        let cap = src("capabilities/main-window.json");
        assert!(!cap.contains("clipboard-manager"), "{cap}");
        let pkg = src("../package.json");
        assert!(!pkg.contains("plugin-clipboard"), "no JS clipboard plugin");

        // Behavior: with a secret on the clipboard, everything Foreman does on its own (start,
        // Today, projects, step views, launches, copies, focus re-stat, the inbox) reads nothing.
        let p = ProjectApp::new(json!({}));
        let os = &p.t.os;
        os.set_clipboard(Some("clipboard-secret-7c1e"));
        let app = &p.t.app;
        app_cmd::app_bootstrap(app, req(json!({}))).unwrap();
        app_cmd::today_get(app, req(json!({}))).unwrap();
        projects::projects_list(app, req(json!({}))).unwrap();
        projects::project_open(app, req(json!({ "projectId": p.id }))).unwrap();
        settings::settings_get(app, req(json!({}))).unwrap();
        for step in ["1.1", "1.4", "2.2", "2.3"] {
            steps::step_view(app, req(json!({ "projectId": p.id, "step": step }))).unwrap();
        }
        runs::runs_search(
            app,
            req(json!({ "query": "", "projectId": null, "cursor": null, "limit": 50 })),
        )
        .unwrap();
        let (run, _buf) = start(&p, "2.2");
        let _ = steps::clipboard_copy(
            app,
            req(
                json!({ "projectId": p.id, "target": "command", "step": "2.3", "chain": null, "path": null, "stage": null }),
            ),
        );
        foreman_app::watch::on_focus(app);
        foreman_app::inbox::schedule(app);
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            os.clipboard_reads(),
            0,
            "nothing read without Billy's action"
        );
        os.set_clipboard(Some("clipboard-secret-7c1e"));

        // Each explicit action reads it exactly once.
        term_cmd::pty_paste(app, req(json!({ "runId": run.run_id }))).unwrap();
        assert_eq!(os.clipboard_reads(), 1);
        let _ = chat::paste_back_preview(
            app,
            req(json!({ "projectId": p.id, "step": "1.4", "kind": "auto", "source": "clipboard" })),
        );
        assert_eq!(os.clipboard_reads(), 2);
        let _ = chat::chat_link_set(
            app,
            req(
                json!({ "projectId": p.id, "chainStart": "1.1", "source": { "kind": "clipboard" } }),
            ),
        );
        assert_eq!(os.clipboard_reads(), 3);
        // The sensor source and a typed link never touch the clipboard.
        let _ = chat::paste_back_preview(
            app,
            req(json!({ "projectId": p.id, "step": "1.4", "kind": "auto", "source": "sensor" })),
        );
        let _ = chat::chat_link_set(
            app,
            req(
                json!({ "projectId": p.id, "chainStart": "1.1", "source": { "kind": "text", "url": "https://claude.ai/project/0198a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b" } }),
            ),
        );
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(os.clipboard_reads(), 3, "never polled");
        let _ = term_cmd::pty_terminate(app, req(json!({ "runId": run.run_id })));
    }

    /// SR-56, AC-XC-29: at most 1 MB of text; an empty or larger clipboard is a typed error and
    /// nothing reaches the session.
    #[test]
    fn size_cap() {
        assert_eq!(CLIPBOARD_MAX, 1024 * 1024);
        let at_cap = "x".repeat(CLIPBOARD_MAX);
        assert_eq!(
            check_clipboard(at_cap.clone()).unwrap().len(),
            CLIPBOARD_MAX
        );
        assert_eq!(
            check_clipboard("x".repeat(CLIPBOARD_MAX + 1)),
            Err(ClipboardProblem::TooLarge)
        );
        // Multi-byte text counts bytes, not characters.
        assert_eq!(
            check_clipboard("é".repeat(CLIPBOARD_MAX / 2 + 1)),
            Err(ClipboardProblem::TooLarge)
        );
        assert_eq!(check_clipboard(String::new()), Err(ClipboardProblem::Empty));
        // The real OS path goes through the same check, text only.
        let os_src = src("src/os.rs");
        let at = os_src.find(".read_text()").unwrap();
        let real = &os_src[at..at + 200];
        assert!(real.contains("check_clipboard(text)"), "{real}");
        assert_eq!(os_src.matches(".read_text()").count(), 1);

        let p = ProjectApp::new(json!({}));
        let app = &p.t.app;
        let (run, buf) = start(&p, "2.2");
        let over = "y".repeat(CLIPBOARD_MAX + 1);
        p.t.os.set_clipboard(Some(&over));
        let e = term_cmd::pty_paste(app, req(json!({ "runId": run.run_id }))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Clipboard);
        assert_eq!(clip_problem(&e), Some(ClipboardProblem::TooLarge));
        assert!(!e.message.contains("yyyy"), "the text is never echoed");
        let e = chat::paste_back_preview(
            app,
            req(json!({ "projectId": p.id, "step": "1.4", "kind": "auto", "source": "clipboard" })),
        )
        .unwrap_err();
        assert_eq!(clip_problem(&e), Some(ClipboardProblem::TooLarge));
        let e = chat::chat_link_set(
            app,
            req(json!({ "projectId": p.id, "chainStart": "1.1", "source": { "kind": "clipboard" } })),
        )
        .unwrap_err();
        assert_eq!(clip_problem(&e), Some(ClipboardProblem::TooLarge));
        p.t.os.set_clipboard(None);
        let e = term_cmd::pty_paste(app, req(json!({ "runId": run.run_id }))).unwrap_err();
        assert_eq!(clip_problem(&e), Some(ClipboardProblem::Empty));
        assert!(
            !String::from_utf8_lossy(&buf.lock().unwrap()).contains("yyyy"),
            "nothing reached the session"
        );
        // Exactly 1 MB is accepted.
        p.t.os.set_clipboard(Some(&at_cap));
        let ok = term_cmd::pty_paste(app, req(json!({ "runId": run.run_id }))).unwrap();
        assert_eq!(ok.text.len(), CLIPBOARD_MAX);
        let _ = term_cmd::pty_terminate(app, req(json!({ "runId": run.run_id })));
    }
}
