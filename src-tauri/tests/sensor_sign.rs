//! M4-09 retention at sign (SR-53, AC-F011-07): when a stage is signed, the sensor events of every
//! run of that stage are deleted; other stages' events stay.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs and run git (design doc §3.2)"
)]

mod common;

use std::path::Path;
use std::process::Command;

use common::project::{ProjectApp, channel, req};
use foreman_app::ipc::commands::{gates, signing};
use foreman_app::ipc::dto::gates::SignResult;
use foreman_app::project;
use foreman_core::schema::progress::DecisionFileKind;
use serde_json::json;

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("/usr/bin/git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgSign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Stage 1 ready to sign (as `tests/sign.rs` sets it up): identity, a committed baseline, a valid
/// packet, a lessons entry, every gate item attested.
fn ready(p: &ProjectApp) {
    std::fs::write(
        p.t.home().join(".gitconfig"),
        "[user]\n\tname = Billy Ribeiro\n\temail = billy@example.com\n",
    )
    .unwrap();
    git(&p.project, &["add", ".foreman"]);
    git(&p.project, &["commit", "-q", "-m", "foreman: setup"]);
    let o = project::open(&p.t.app, &p.id).unwrap();
    let root = o.root().unwrap();
    let d = foreman_core::decisions::Decisions {
        project: root,
        app: &o.app_root,
        project_id: &p.id,
    };
    for k in DecisionFileKind::ALL {
        let b = std::fs::read(p.project.join(k.repo_path())).unwrap();
        d.mark_committed(k, &foreman_core::store::sha256_hex(&b))
            .unwrap();
    }
    let f = o
        .template
        .as_ref()
        .unwrap()
        .packet_format(1)
        .unwrap()
        .clone();
    let mut s = String::from("=== HANDOFF PACKET · Stage 1 → Stage 2 ===\n");
    for k in &f.keys {
        let v = match k.key.as_str() {
            "APPROVED BY" => "<blank>".to_owned(),
            "OPEN ITEMS" => "none".to_owned(),
            other => format!("the {} for TradeLog", other.to_lowercase()),
        };
        s.push_str(&format!("{}: {v}\n", k.key));
    }
    let dir = p.project.join("docs/handoff");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("stage-01.md"), s).unwrap();
    let lessons = p.project.join("docs/lessons.md");
    let mut text = std::fs::read_to_string(&lessons).unwrap_or_else(|_| "# Lessons\n\n".into());
    text.push_str("\n## L-001 · Stage 1 · 2026-10-20 · clean stage\nSTATUS: applied\n");
    std::fs::write(&lessons, text).unwrap();
    let v = gates::gate_view(&p.t.app, req(json!({ "projectId": p.id, "stage": 1 }))).unwrap();
    for (i, it) in v.items.iter().enumerate() {
        gates::gate_item_attest(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "item": it.id, "outcome": { "kind": "done", "note": format!("checked item number {i} by hand"), "link": null } })),
        )
        .unwrap();
    }
}

/// A run record of `stage` with a token and two event files.
fn run_with_events(p: &ProjectApp, run: &str, stage: u8) {
    let dir = p.t.app_data().join("runs").join(run);
    std::fs::create_dir_all(&dir).unwrap();
    let rec = json!({
        "argv": ["/x/claude"], "chainId": null, "claudeVersion": null, "copyPromptOnCli": false,
        "cwd": "project", "entryPoint": "stageView", "envNames": [], "exitCode": 0,
        "intended": null, "kind": "launch", "name": null, "observed": null, "override": false,
        "pid": null, "projectId": p.id, "promptSha256": null, "runId": run,
        "runInputInterventions": 0, "runSettingsSha256": null, "schemaVersion": 1,
        "sensorToken": "b".repeat(64), "stage": stage, "status": "exited",
        "stepId": format!("{stage}.1"), "surface": "cli",
        "t": { "confirmed": null, "exited": 1000, "lookStart": null, "prepared": null,
               "spawned": null, "wallStart": "2026-10-19T08:00:00Z" },
        "timeToStartMs": null
    });
    std::fs::write(
        dir.join("run.json"),
        serde_json::to_string_pretty(&rec).unwrap(),
    )
    .unwrap();
    let ev = p.t.app_data().join("events").join(run);
    std::fs::create_dir_all(&ev).unwrap();
    for (i, e) in ["SessionStart", "Stop"].iter().enumerate() {
        std::fs::write(ev.join(format!("{:020}-0000000001-{e}.json", i + 1)), "{}").unwrap();
    }
}

fn files(dir: &Path) -> usize {
    std::fs::read_dir(dir).map(Iterator::count).unwrap_or(0)
}

mod retention {
    use super::*;

    /// SR-53, AC-F011-07: signing Stage 1 deletes the sensor events of Stage 1's runs; a Stage 2
    /// run keeps its events.
    #[test]
    fn events_deleted_on_sign() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let s1 = "0c000000-0000-4000-8000-000000000001";
        let s2 = "0c000000-0000-4000-8000-000000000002";
        run_with_events(&p, s1, 1);
        run_with_events(&p, s2, 2);
        let ev = p.t.app_data().join("events");
        assert_eq!(files(&ev.join(s1)), 2);
        let pv = signing::sign_preview(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "branch": { "kind": "current" }, "name": "Billy Ribeiro", "date": "2026-10-20" })),
        )
        .unwrap();
        let r = signing::sign_apply(
            &p.t.app,
            req(json!({ "previewId": pv.preview_id })),
            channel(),
        )
        .unwrap();
        assert!(matches!(r, SignResult::Signed { .. }), "{r:?}");
        assert_eq!(files(&ev.join(s1)), 0, "Stage 1's events went at its sign");
        assert_eq!(files(&ev.join(s2)), 2, "Stage 2's stay");
    }
}
