//! M3-23, M3-12: chat chips and links, Copy and open, and terminal links, against a real project
//! (fake-claude for the terminal) with the scripted OS: what was copied, what was opened, and every
//! native dialog.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::project::{ProjectApp, collect, git_identity, req};
use foreman_app::ipc::commands::terminal as term_cmd;
use foreman_app::ipc::commands::{chat, launch, projects, steps};
use foreman_app::ipc::dto::launch::{LinkOutcome, PrepareOutcome, RunStarted};
use foreman_app::ipc::dto::steps::MissingLink;
use foreman_app::ipc::error::{IpcErrorCode, IpcErrorDetail};
use serde_json::{Value, json};

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

/// Fills the variables 1.1's prompt needs.
fn fill_variables(p: &ProjectApp) {
    git_identity(p);
    for (name, value) in [
        ("MY_ADVANTAGES", "Ten years of trading"),
        ("BUDGET_TIME", "Evenings, 3 months"),
        ("APP_NAME", "TradeLog"),
        ("PREFERRED_STACK", "Svelte and Rust"),
    ] {
        projects::variable_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "name": name, "value": value })),
        )
        .unwrap();
    }
}

fn copy_and_open(p: &ProjectApp, step: &str) -> Result<Value, foreman_app::ipc::IpcError> {
    chat::chat_copy_and_open(
        &p.t.app,
        req(json!({ "projectId": p.id, "step": step, "entry": "stageView" })),
    )
    .map(|r| serde_json::to_value(r).unwrap())
}

fn view(p: &ProjectApp, step: &str) -> Value {
    serde_json::to_value(
        steps::step_view(&p.t.app, req(json!({ "projectId": p.id, "step": step }))).unwrap(),
    )
    .unwrap()
}

mod chain {
    use super::*;

    /// AC-F012-01…-04: chips, Copy and open to the Project link (new conversation) or the chain's
    /// conversation link (continuing step), Link this chat, and the missing-link answer.
    #[test]
    fn chat_links() {
        let p = ProjectApp::new(json!({}));
        fill_variables(&p);

        // AC-F012-01: 1.1 starts a conversation; its chips are reminders from the RUN WITH line.
        let v = view(&p, "1.1");
        let chips: Vec<(String, bool)> = v["chips"]["chips"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                (
                    c["label"].as_str().unwrap().to_owned(),
                    c["strong"].as_bool().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            chips[0],
            ("New conversation in Project TradeLog".to_owned(), true)
        );
        assert!(chips.iter().any(|(l, _)| l == "Web search ON"), "{chips:?}");
        assert!(chips.iter().any(|(l, _)| l.contains("Opus")), "{chips:?}");
        assert!(v["chips"]["link"].is_null());
        let v12 = view(&p, "1.2");
        assert_eq!(v12["chips"]["chips"][0]["strong"], true);
        assert!(
            v12["chips"]["chips"][0]["label"]
                .as_str()
                .unwrap()
                .starts_with("Same chat as 1.1"),
            "{}",
            v12["chips"]["chips"][0]
        );

        // AC-F012-04: no Project link yet: the prompt is copied, nothing opens, and the step says
        // which link to set.
        let r = copy_and_open(&p, "1.1").unwrap();
        assert_eq!(r["opened"], false);
        assert_eq!(r["missingLink"], "project");
        let clip = p.t.os.clipboard().unwrap();
        assert!(clip.starts_with("You are my product architect"), "{clip}");
        assert!(clip.contains("Ten years of trading"));
        assert!(p.t.os.opened().is_empty());
        assert_eq!(view(&p, "1.1")["summary"]["state"], "copied");
        // The run record (AC-F026-02 input): chat copy, prompt hash only (SR-52).
        let run_id = r["runId"].as_str().unwrap();
        let rec =
            std::fs::read_to_string(p.t.app_data().join("runs").join(run_id).join("run.json"))
                .unwrap();
        let rec: Value = serde_json::from_str(&rec).unwrap();
        assert_eq!(rec["kind"], "chatCopy");
        assert_eq!(rec["surface"], "chat");
        assert_eq!(rec["status"], "copied");
        assert!(rec["timeToStartMs"].is_u64());
        assert!(!rec.to_string().contains("product architect"));

        // AC-F012-03: with the Project link, Copy and open opens it (through Rust's opener).
        projects::project_url_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "url": "https://claude.ai/project/0199d-tradelog" })),
        )
        .unwrap();
        let r = copy_and_open(&p, "1.1").unwrap();
        assert_eq!(
            (r["opened"].clone(), r["missingLink"].clone()),
            (json!(true), Value::Null)
        );
        assert_eq!(
            p.t.os.opened(),
            ["https://claude.ai/project/0199d-tradelog"]
        );

        // A continuing step needs the conversation link; the Project link doesn't stand in.
        steps::run_input_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "1.2", "name": "X", "value": "B" })),
        )
        .unwrap();
        steps::run_input_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "1.2", "name": "changes", "value": "none" })),
        )
        .unwrap();
        let r = copy_and_open(&p, "1.2").unwrap();
        assert_eq!(r["missingLink"], "conversation");
        assert_eq!(p.t.os.opened().len(), 1);

        // AC-F012-02: Link this chat from the clipboard on Billy's click; off-site refused.
        for bad in [
            "https://claude.ai.evil.com/chat/x",
            "http://claude.ai/chat/x",
            "https://evil.com/?https://claude.ai/",
            "not a url",
        ] {
            p.t.os.set_clipboard(Some(bad));
            let e = chat::chat_link_set(
                &p.t.app,
                req(json!({ "projectId": p.id, "chainStart": "1.1", "source": { "kind": "clipboard" } })),
            )
            .unwrap_err();
            assert_eq!(e.code, IpcErrorCode::Invalid, "{bad}");
        }
        p.t.os.set_clipboard(None);
        let e = chat::chat_link_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "chainStart": "1.1", "source": { "kind": "clipboard" } })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Clipboard);
        // Linking from a continuing step stores the link on its chain start.
        p.t.os.set_clipboard(Some(
            "  https://claude.ai/chat/7c1e9a2b-55d0-4c1f-8f04-3b2a1d0e9f04\n",
        ));
        chat::chat_link_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "chainStart": "1.2", "source": { "kind": "clipboard" } })),
        )
        .unwrap();
        let pj: Value = serde_json::from_str(
            &std::fs::read_to_string(p.project.join(".foreman/project.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            pj["chatUrls"]["1.1"],
            "https://claude.ai/chat/7c1e9a2b-55d0-4c1f-8f04-3b2a1d0e9f04"
        );
        assert_eq!(
            view(&p, "1.4")["chips"]["linkLabel"],
            "claude.ai/chat/7c1e…f04"
        );
        let r = copy_and_open(&p, "1.2").unwrap();
        assert_eq!(r["opened"], true);
        assert_eq!(
            p.t.os.opened().last().unwrap(),
            "https://claude.ai/chat/7c1e9a2b-55d0-4c1f-8f04-3b2a1d0e9f04"
        );
        // Not a chat step: refused.
        let e = copy_and_open(&p, "2.2").unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
    }

    /// T-100: a link stored off-site (an outside edit of `project.json`) is never opened.
    #[test]
    fn stored_link_rechecked() {
        let p = ProjectApp::new(json!({}));
        let mut o = foreman_app::project::open(&p.t.app, &p.id).unwrap();
        let pf = o.project.as_mut().unwrap();
        pf.chat_project_url = Some("https://evil.example/".into());
        pf.chat_urls.insert(
            foreman_core::ids::StepKey::parse("1.1").unwrap(),
            "https://claude.ai.evil.example/chat".into(),
        );
        let s11 = o.step("1.1").unwrap().clone();
        let s12 = o.step("1.2").unwrap().clone();
        assert_eq!(
            chat::chat_target(&o, &s11),
            chat::ChatTarget::Missing(MissingLink::Project)
        );
        assert_eq!(
            chat::chat_target(&o, &s12),
            chat::ChatTarget::Missing(MissingLink::Conversation)
        );
    }
}

mod opener {
    use super::*;

    /// SR-25 / AC-XC-08 (chat URLs): Link this chat stores only `https://claude.ai/…`; the text
    /// form is checked when the request is parsed.
    #[test]
    fn only_allowed_urls() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        let bad: Result<foreman_app::ipc::dto::requests::ChatLinkSetReq, _> =
            serde_json::from_value(json!({
                "projectId": p.id, "chainStart": "1.1",
                "source": { "kind": "text", "url": "https://claude.ai.evil.com/x" }
            }));
        assert!(bad.is_err());
        chat::chat_link_set(
            &p.t.app,
            req(json!({ "projectId": p.id, "chainStart": "1.1",
                         "source": { "kind": "text", "url": "https://claude.ai/chat/abc" } })),
        )
        .unwrap();
        assert!(p.t.os.opened().is_empty(), "setting a link opens nothing");
    }
}

mod terminal_links {
    use super::*;

    /// SR-29 / AC-F009-17 / T-093: Rust decides. A refused scheme shows no dialog and opens
    /// nothing; an allowed link opens only after the native dialog, which shows the full URL with
    /// hidden characters visible; Cancel opens nothing.
    #[test]
    fn link_open_policy() {
        let p = ProjectApp::new(json!({}));
        let (r, _) = start(&p, "2.2");
        let open = |url: &str| {
            term_cmd::terminal_link_open(&p.t.app, req(json!({ "runId": r.run_id, "url": url })))
        };
        let asked = p.t.os.asked().len();
        for bad in [
            "javascript:alert(1)",
            "file:///Applications/Calculator.app",
            "vscode://file/x",
            "http://example.com/",
            "https://user@example.com/",
        ] {
            assert_eq!(open(bad).unwrap(), LinkOutcome::Refused, "{bad}");
        }
        assert_eq!(p.t.os.asked().len(), asked, "refusals show no dialog");
        p.t.os.answer_next(false);
        assert_eq!(
            open("https://example.com/").unwrap(),
            LinkOutcome::Cancelled
        );
        assert!(p.t.os.opened().is_empty());
        assert_eq!(
            open("https://example.com/docs?q=1").unwrap(),
            LinkOutcome::Opened
        );
        assert_eq!(p.t.os.opened(), ["https://example.com/docs?q=1"]);
        let last = p.t.os.asked().last().unwrap().clone();
        assert!(last.message.contains("https://example.com/docs?q=1"));
        assert_eq!(last.ok, "Open link");
        // Too long, unknown run.
        let e = open(&format!("https://e.com/{}", "a".repeat(9000))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Invalid);
        let e = term_cmd::terminal_link_open(
            &p.t.app,
            req(json!({ "runId": "7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11", "url": "https://e.com" })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotFound);
        assert!(!matches!(
            e.detail,
            Some(IpcErrorDetail::Unsupported { .. })
        ));
    }
}

mod terminal {
    /// SR-29 (webview half): both xterm link paths hand the URL to `terminal_link_open`; the
    /// terminal component never opens a window or navigates itself.
    #[test]
    fn link_handler_routes_to_rust() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../src/lib/components/TerminalView.svelte"),
        )
        .unwrap();
        let handler = src
            .split("linkHandler")
            .nth(1)
            .expect("an OSC 8 linkHandler");
        let body = &handler[..handler.find("});").unwrap_or(handler.len())];
        assert!(body.contains("terminalLinkOpen"), "{body}");
        for banned in [
            "window.open",
            "location.href",
            "location.assign",
            "addon-clipboard",
        ] {
            assert!(!src.contains(banned), "{banned}");
        }
    }
}
