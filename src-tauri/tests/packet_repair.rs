//! M4-23: packet repair (AC-F013-11): a re-arm of the stage's chain with Foreman's own fix
//! prompt, started only by Billy's click and the native idle dialog.
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

use common::project::{ProjectApp, channel, collect, req};
use foreman_app::ipc::commands::{launch, signing, terminal};
use foreman_app::ipc::dto::launch::PrepareOutcome;
use foreman_app::ipc::error::{IpcErrorCode, IpcErrorDetail};
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

/// Sessions fake-claude was started as (launches, not the `-v` / `auth status` probes).
fn sessions(p: &ProjectApp) -> usize {
    let mut path = p.record.as_os_str().to_owned();
    path.push(".calls.jsonl");
    std::fs::read_to_string(std::path::PathBuf::from(path))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains("--session-id") || l.contains("--resume"))
        .count()
}

fn repair(p: &ProjectApp) -> Result<PrepareOutcome, foreman_app::ipc::IpcError> {
    signing::packet_repair_prepare(
        &p.t.app,
        req(json!({ "projectId": p.id, "stage": 2 })),
        channel(),
    )
}

mod rearm {
    use super::*;

    /// AC-F013-11, SR-24a: Repair only prepares (nothing spawns, nothing is typed); the problems
    /// come from Rust's validation; the re-arm of the 2.2 chain starts only through `rearm_start`
    /// and its native idle dialog (Cancel sends nothing).
    #[test]
    fn only_from_user_action() {
        let p = ProjectApp::new(json!({}));
        // No packet yet.
        assert_eq!(repair(&p).unwrap_err().code, IpcErrorCode::NotFound);
        let dir = p.project.join("docs/handoff");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("stage-02.md"),
            "=== HANDOFF PACKET · Stage 2 → Stage 3 ===\nPROJECT: TradeLog\nAPPROVED BY: <blank>\nOPEN ITEMS: two left\n",
        )
        .unwrap();
        // The chain hasn't run.
        assert_eq!(repair(&p).unwrap_err().code, IpcErrorCode::NotFound);
        // Run 2.2 (the chain 2.6 continues).
        let pv = match p.prepare("2.2") {
            PrepareOutcome::Ready(pv) => pv,
            PrepareOutcome::Blocked { blockers, .. } => panic!("{blockers:?}"),
        };
        let (ch, buf) = collect();
        launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
        wait_for(&buf, "FAKE-CLAUDE READY");
        let before = sessions(&p);
        let out = repair(&p).unwrap();
        let PrepareOutcome::Ready(pv) = out else {
            panic!("{out:?}")
        };
        assert_eq!(sessions(&p), before, "preparing started no session");
        assert!(pv.rearm.is_some(), "the live 2.2 chain is re-armed");
        let prompt = serde_json::to_string(&pv.prompt).unwrap();
        assert!(
            prompt.contains("Fix only these problems in docs/handoff/stage-02.md"),
            "{prompt}"
        );
        assert!(prompt.contains("Do not change anything else."), "{prompt}");
        assert!(prompt.contains("OPEN ITEMS"), "{prompt}");
        assert!(pv.argv.iter().any(|a| a == "--resume"), "{:?}", pv.argv);
        // launch_start refuses a plan that needs a re-arm; rearm_start asks natively.
        let e = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), channel())
            .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        let PrepareOutcome::Ready(pv) = repair(&p).unwrap() else {
            panic!()
        };
        let asked = p.t.os.asked().len();
        p.t.os.answer_next(false);
        let e = terminal::rearm_start(
            &p.t.app,
            req(json!({ "planId": pv.plan_id })),
            channel(),
            channel(),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert_eq!(p.t.os.asked().len(), asked + 1);
        assert!(!String::from_utf8_lossy(&buf.lock().unwrap()).contains("Fix only"));
        assert_eq!(sessions(&p), before, "Cancel started nothing");
        // A valid packet has nothing to repair.
        p.t.app.runs().hangup_all();
    }

    /// AC-F013-12 (chat): a chat stage's packet isn't re-armed; Copy fix request is the path.
    #[test]
    fn chat_packet_not_repaired() {
        let p = ProjectApp::new(json!({}));
        let dir = p.project.join("docs/handoff");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("stage-01.md"),
            "=== HANDOFF PACKET · Stage 1 → Stage 2 ===\n",
        )
        .unwrap();
        let e = signing::packet_repair_prepare(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1 })),
            channel(),
        )
        .unwrap_err();
        let Some(IpcErrorDetail::NotAllowed { rule }) = e.detail else {
            panic!("{e:?}")
        };
        assert_eq!(rule, "not-cli-packet");
    }
}
