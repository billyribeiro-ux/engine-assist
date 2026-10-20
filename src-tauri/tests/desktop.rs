//! M4-29 and M4-32: Desktop handoff and the run sheet export through the IPC commands
//! (AC-F024-01…-04, AC-F010-15, AC-F027-01…-03; T-030), against a real project and fake-claude.
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

use common::project::{ProjectApp, channel, collect, git_identity, req};
use foreman_app::ipc::commands::{desktop, launch, terminal};
use foreman_app::ipc::dto::launch::{LaunchPreview, PrepareOutcome, RunStarted};
use foreman_app::ipc::dto::records::ExportResult;
use foreman_app::ipc::error::{IpcErrorCode, IpcErrorDetail};
use foreman_core::schema::progress::ChainStatus;
use serde_json::{Value, json};

fn ready(p: &ProjectApp, step: &str) -> LaunchPreview {
    match p.prepare(step) {
        PrepareOutcome::Ready(pv) => pv,
        PrepareOutcome::Blocked { blockers, .. } => panic!("blocked: {blockers:?}"),
    }
}

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

/// Starts `step`, then ends the session with `/exit` so the chain isn't live.
fn run_and_exit(p: &ProjectApp, step: &str) -> RunStarted {
    let pv = ready(p, step);
    let (ch, buf) = collect();
    let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
    wait_for(&buf, "FAKE-CLAUDE READY");
    terminal::pty_write(
        &p.t.app,
        req(json!({ "runId": r.run_id, "data": "/exit\r" })),
    )
    .unwrap();
    let live = p.t.app.runs().get(r.run_id.as_str()).unwrap();
    assert!(live.session.wait_exit(Duration::from_secs(10)).is_some());
    r
}

/// Every fake-claude invocation so far (argv and cwd).
fn calls(p: &ProjectApp) -> Vec<Value> {
    let mut path = p.record.as_os_str().to_owned();
    path.push(".calls.jsonl");
    std::fs::read_to_string(std::path::PathBuf::from(path))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn desktop_calls(p: &ProjectApp) -> Vec<Value> {
    calls(p)
        .into_iter()
        .filter(|c| {
            c["argv"]
                .as_array()
                .is_some_and(|a| a.iter().any(|x| x == "--desktop"))
        })
        .collect()
}

fn args(c: &Value) -> Vec<String> {
    c["argv"].as_array().unwrap()[1..]
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}

fn chain_status(p: &ProjectApp, chain: &str) -> (ChainStatus, Option<String>) {
    let o = foreman_app::project::open(&p.t.app, &p.id).unwrap();
    let c = o
        .progress
        .chains
        .iter()
        .find(|c| c.id == chain)
        .unwrap()
        .clone();
    (c.status, c.forked_from)
}

fn rule(e: &foreman_app::ipc::IpcError) -> String {
    match &e.detail {
        Some(IpcErrorDetail::NotAllowed { rule }) => rule.clone(),
        other => format!("{other:?}"),
    }
}

/// A `surface=desktop` step added through the overlay (v2.2 has none).
fn add_desktop_step(p: &ProjectApp) {
    git_identity(p);
    desktop::add_step(
        &p.t.app,
        req(json!({
            "projectId": p.id, "stage": 2, "after": "2.4", "copyConfigFrom": "2.4",
            "title": "Review the stack in Desktop",
            "text": "Read council/r7-stack.md for [APP_NAME] and list what could break first.",
            "config": { "surface": "desktop", "cwd": "spec",
                        "start": { "model": "opus", "effort": "high", "mode": "plan" }, "out": [] },
            "tag": "optional"
        })),
    )
    .unwrap();
}

mod chain {
    use super::*;

    /// AC-F024-02, -03 (S-23 shared), AC-F010-15 (S-8 pass): `claude --desktop --resume <uuid>`
    /// in the chain's cwd hands the chain off; the next continuing step relaunches with
    /// `--resume <old> --fork-session --session-id <new>` as a new chain forked from the old one.
    #[test]
    fn fork_argv() {
        let p = ProjectApp::new(json!({}));
        let r = run_and_exit(&p, "2.2");
        let chain = r.chain.as_str().to_owned();
        desktop::desktop_handoff(&p.t.app, req(json!({ "chain": chain }))).unwrap();
        let d = desktop_calls(&p);
        assert_eq!(d.len(), 1);
        assert_eq!(args(&d[0]), ["--desktop", "--resume", chain.as_str()]);
        assert!(
            d[0]["cwd"].as_str().unwrap().ends_with("work/tradelog/spec"),
            "{}",
            d[0]["cwd"]
        );
        // The same allowlisted environment as a launch: no stray variables.
        let env = d[0]["env"].as_object().unwrap();
        assert!(env.contains_key("PATH") && env.contains_key("HOME"));
        assert!(!env.contains_key("ANTHROPIC_API_KEY"));
        assert_eq!(chain_status(&p, &chain).0, ChainStatus::HandedOff);
        // The next continuing step forks.
        let pv = ready(&p, "2.3");
        let argv = &pv.argv;
        let at = argv.iter().position(|a| a == "--resume").unwrap();
        assert_eq!(argv[at + 1], chain);
        assert_eq!(argv[at + 2], "--fork-session");
        assert_eq!(argv[at + 3], "--session-id");
        let new_id = argv[at + 4].clone();
        assert_ne!(new_id, chain);
        let (ch, buf) = collect();
        let started =
            launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
        wait_for(&buf, "FAKE-CLAUDE READY");
        assert_eq!(started.chain.as_str(), new_id);
        let (status, from) = chain_status(&p, &new_id);
        assert_eq!(status, ChainStatus::Open);
        assert_eq!(from.as_deref(), Some(chain.as_str()), "recorded as forked");
        p.t.app.runs().hangup_all();
    }

    /// AC-F010-15's exception: Billy confirms (natively) that Desktop is closed, so the chain
    /// continues with `--resume` and no fork; Cancel leaves it handed off.
    #[test]
    fn desktop_closed_continues_the_chain() {
        let p = ProjectApp::new(json!({}));
        let r = run_and_exit(&p, "2.2");
        let chain = r.chain.as_str().to_owned();
        desktop::desktop_handoff(&p.t.app, req(json!({ "chain": chain }))).unwrap();
        let asked = p.t.os.asked().len();
        p.t.os.answer_next(false);
        let e = desktop::desktop_handoff(&p.t.app, req(json!({ "chain": chain }))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert_eq!(p.t.os.asked().len(), asked + 1);
        assert!(p.t.os.asked()[asked].title.contains("Claude Desktop closed"));
        assert_eq!(chain_status(&p, &chain).0, ChainStatus::HandedOff);
        p.t.os.answer_next(true);
        desktop::desktop_handoff(&p.t.app, req(json!({ "chain": chain }))).unwrap();
        assert_eq!(chain_status(&p, &chain).0, ChainStatus::Open);
        assert_eq!(desktop_calls(&p).len(), 1, "no second handoff spawn");
        let pv = ready(&p, "2.3");
        let at = pv.argv.iter().position(|a| a == "--resume").unwrap();
        assert_eq!(pv.argv[at + 1], chain);
        assert!(!pv.argv.iter().any(|a| a == "--fork-session"));
    }

    /// AC-F024-03, S-23's copied branch: no fork; the CLI chain continues unaffected.
    #[test]
    fn copied_branch_keeps_the_chain() {
        use foreman_core::chain::{DesktopShare, after_handoff};
        let p = ProjectApp::new(json!({}));
        let r = run_and_exit(&p, "2.2");
        let o = foreman_app::project::open(&p.t.app, &p.id).unwrap();
        let id = uuid::Uuid::parse_str(r.chain.as_str()).unwrap();
        let copied = after_handoff(&o.progress.chains, id, DesktopShare::Copied);
        assert_eq!(copied, o.progress.chains);
        let shared = after_handoff(&o.progress.chains, id, DesktopShare::Shared);
        assert!(
            shared
                .iter()
                .any(|c| c.id == r.chain.as_str() && c.status == ChainStatus::HandedOff)
        );
        assert_eq!(desktop::DESKTOP_SHARE, DesktopShare::Shared);
    }

    /// `desktop_handoff` refuses a live chain ("end it first").
    #[test]
    fn live_chain_is_refused() {
        let p = ProjectApp::new(json!({}));
        let pv = ready(&p, "2.2");
        let (ch, buf) = collect();
        let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
        wait_for(&buf, "FAKE-CLAUDE READY");
        let e = desktop::desktop_handoff(&p.t.app, req(json!({ "chain": r.chain }))).unwrap_err();
        assert_eq!(rule(&e), "chain-live");
        assert!(desktop_calls(&p).is_empty());
        p.t.app.runs().hangup_all();
    }
}

mod desktop_open {
    use super::*;

    /// AC-F024-01: Open in Desktop copies the resolved prompt and runs `claude --desktop` in the
    /// step's cwd with no other flags, after the auth check in that cwd with the launch
    /// environment; the step gets chips for the model, effort and mode to pick.
    #[test]
    fn copies_prompt_and_runs_desktop_only() {
        let p = ProjectApp::new(json!({}));
        add_desktop_step(&p);
        let before = calls(&p).len();
        let o = desktop::desktop_open(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "x-2-1" })),
            channel(),
        )
        .unwrap();
        let labels: Vec<&str> = o.chips.chips.iter().map(|c| c.label.as_str()).collect();
        assert!(labels.contains(&"Opus 5.5"), "{labels:?}");
        assert!(labels.contains(&"high effort"), "{labels:?}");
        assert!(labels.contains(&"Plan"), "{labels:?}");
        assert_eq!(
            p.t.os.clipboard().as_deref().map(str::trim_end),
            Some("Read council/r7-stack.md for TradeLog and list what could break first.")
        );
        let new: Vec<Value> = calls(&p).into_iter().skip(before).collect();
        let auth = new
            .iter()
            .position(|c| args(c) == ["auth", "status"])
            .expect("auth status ran");
        let d = new
            .iter()
            .position(|c| args(c).first().map(String::as_str) == Some("--desktop"))
            .expect("--desktop ran");
        assert!(auth < d, "auth before the handoff");
        assert_eq!(args(&new[d]), ["--desktop"], "no other flags");
        assert_eq!(new[auth]["cwd"], new[d]["cwd"]);
        assert!(new[d]["cwd"].as_str().unwrap().ends_with("/spec"));
        assert_eq!(
            new[auth]["env"], new[d]["env"],
            "the auth check and Desktop get the same environment"
        );
    }

    /// Only a `surface=desktop` step opens in Desktop; a failed auth check opens nothing.
    #[test]
    fn refuses_other_steps_and_failed_auth() {
        let p = ProjectApp::new(json!({}));
        let e = desktop::desktop_open(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "2.2" })),
            channel(),
        )
        .unwrap_err();
        assert_eq!(rule(&e), "not-desktop-step");
        let p = ProjectApp::new(json!({ "auth": { "loggedIn": true, "authMethod": "api_key" } }));
        add_desktop_step(&p);
        let e = desktop::desktop_open(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "x-2-1" })),
            channel(),
        )
        .unwrap_err();
        assert_eq!(rule(&e), "auth-failed");
        assert!(desktop_calls(&p).is_empty());
    }
}

mod compat {
    use super::*;

    /// AC-F024-04: below 2.1.285 both Desktop actions are blocked by the floor; nothing runs.
    #[test]
    fn desktop_blocked_below_floor() {
        let p = ProjectApp::new(json!({}));
        let r = run_and_exit(&p, "2.2");
        add_desktop_step(&p);
        // Claude Code is now older than `--desktop`.
        let cfg = p.root.join("bin/fake.json");
        let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        v["version"] = json!("2.1.284 (Claude Code)");
        std::fs::write(&cfg, v.to_string()).unwrap();
        let e = desktop::desktop_open(
            &p.t.app,
            req(json!({ "projectId": p.id, "step": "x-2-1" })),
            channel(),
        )
        .unwrap_err();
        assert_eq!(rule(&e), "version-below-floor");
        assert!(e.message.contains("2.1.285"), "{}", e.message);
        let e = desktop::desktop_handoff(&p.t.app, req(json!({ "chain": r.chain }))).unwrap_err();
        assert_eq!(rule(&e), "version-below-floor");
        assert!(desktop_calls(&p).is_empty());
        assert_eq!(chain_status(&p, r.chain.as_str()).0, ChainStatus::Open);
    }
}

mod runsheet {
    use super::*;

    /// AC-F027-02: the run sheet is written only to the path chosen in the Rust save dialog;
    /// Cancel writes nothing; the export path policy refuses a path inside the repo's `.git`.
    #[test]
    fn export_through_save_dialog() {
        let p = ProjectApp::new(json!({}));
        p.t.os.save_next(None);
        let r = desktop::run_sheet_export(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 2 })),
        )
        .unwrap();
        assert_eq!(r, ExportResult::Cancelled);
        let out = p.root.join("exports/tradelog-stage-02.md");
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        p.t.os.save_next(Some(out.clone()));
        let r = desktop::run_sheet_export(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 2 })),
        )
        .unwrap();
        assert!(matches!(r, ExportResult::Saved { .. }), "{r:?}");
        let md = std::fs::read_to_string(&out).unwrap();
        assert!(md.starts_with("# Run sheet: TradeLog · Stage 2 · "), "{md}");
        assert!(md.contains("## 2.2 Convene the council"));
        assert!(md.contains("--permission-mode"));
        assert!(md.contains("## Inspection items"));
        // AC-F027-03: this project has no signed Stage 1 packet, so 2.2's packet input is marked.
        assert!(md.contains("⟦UNRESOLVED"), "{md}");
        // Never inside the repo's .git, never a non-Markdown name.
        let bad = p.project.join(".git/sheet.md");
        p.t.os.save_next(Some(bad.clone()));
        let e = desktop::run_sheet_export(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 2 })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::PolicyViolation, "{e:?}");
        assert!(!bad.exists());
        let txt = p.root.join("exports/sheet.txt");
        p.t.os.save_next(Some(txt.clone()));
        let e = desktop::run_sheet_export(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 2 })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::PolicyViolation);
        assert!(!txt.exists());
    }
}
