//! M3-22: live outside-change detection and config-changed runs (AC-F022-04, ADR-013, ADR-021 §7,
//! ADR-022). With Foreman open, an outside edit to a decision file becomes a pending review
//! without a restart and has no effect until accepted; a change to a live run's configuration
//! arrives as a watcher event and marks the run.
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
use foreman_app::ipc::commands::{decisions as dec, launch, projects};
use foreman_app::ipc::dto::launch::PrepareOutcome;
use serde_json::{Value, json};

fn open(p: &ProjectApp) -> Value {
    serde_json::to_value(
        projects::project_open(&p.t.app, req(json!({ "projectId": p.id }))).unwrap(),
    )
    .unwrap()
}

fn events(p: &ProjectApp, name: &str) -> Vec<Value> {
    p.t.os
        .events()
        .iter()
        .filter(|e| e.name() == name)
        .map(|e| e.payload())
        .collect()
}

fn wait_for(p: &ProjectApp, name: &str, at_least: usize, within: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < within {
        if events(p, name).len() >= at_least {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

mod decisions {
    use super::*;

    /// AC-F022-04 (live case), SR-40, DR-150: with the project open, Claude (or an editor, or a
    /// checkout) edits `.foreman/project.json`; the pending review appears without a restart or
    /// any command, and the effective config stays the approved copy until Billy accepts.
    #[test]
    fn outside_edit_has_no_effect_until_accepted() {
        let p = ProjectApp::new(json!({}));
        // Accepting commits the decision file (DR-153); git needs an identity in Foreman's HOME.
        std::fs::write(
            p.t.home().join(".gitconfig"),
            "[user]\n\tname = Billy\n\temail = billy@example.com\n",
        )
        .unwrap();
        let v = open(&p);
        assert_eq!(v["conditions"]["commercial"]["value"], "no");
        assert_eq!(v["pendingChanges"], 0);
        let file = p.project.join(".foreman/project.json");
        let mut pj: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        pj["conditions"]["commercial"] = json!("yes");
        let seen = events(&p, "decision:pending").len();
        std::fs::write(
            &file,
            format!("{}\n", serde_json::to_string_pretty(&pj).unwrap()),
        )
        .unwrap();
        // Live: the watcher alone classifies it.
        assert!(
            wait_for(&p, "decision:pending", seen + 1, Duration::from_secs(5)),
            "no decision:pending"
        );
        let ev = events(&p, "decision:pending").pop().unwrap();
        assert_eq!(ev, json!({ "projectId": p.id, "count": 1 }));
        // No effect yet.
        let v = open(&p);
        assert_eq!(v["pendingChanges"], 1);
        assert_eq!(v["conditions"]["commercial"]["value"], "no");
        let list = dec::pending_changes_list(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        assert_eq!(list.len(), 1);
        let c = serde_json::to_value(&list[0]).unwrap();
        // Accept is a native dialog; Cancel changes nothing.
        p.t.os.answer_next(false);
        let e = dec::pending_change_accept(
            &p.t.app,
            req(json!({ "projectId": p.id, "change": c["id"], "seenSha256": c["seenSha256"] })),
        )
        .unwrap_err();
        assert_eq!(e.code, foreman_app::ipc::error::IpcErrorCode::Cancelled);
        assert_eq!(open(&p)["conditions"]["commercial"]["value"], "no");
        p.t.os.answer_next(true);
        dec::pending_change_accept(
            &p.t.app,
            req(json!({ "projectId": p.id, "change": c["id"], "seenSha256": c["seenSha256"] })),
        )
        .unwrap();
        let v = open(&p);
        assert_eq!(v["conditions"]["commercial"]["value"], "yes");
        assert_eq!(v["pendingChanges"], 0);
    }
}

mod watcher {
    use super::*;

    /// ADR-021 §7, M3-22: a change to a live run's configuration source reaches the run through a
    /// watcher event (not the 60 s fallback): the run is marked and `config:changed` is emitted once.
    #[test]
    fn config_changed_by_watcher_event() {
        let p = ProjectApp::new(json!({}));
        foreman_app::watch::start(p.t.app.weak());
        let pv = match p.prepare("2.2") {
            PrepareOutcome::Ready(pv) => pv,
            PrepareOutcome::Blocked { blockers, .. } => panic!("blocked: {blockers:?}"),
        };
        let (ch, buf): (_, Arc<Mutex<Vec<u8>>>) = collect();
        let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
        let start = Instant::now();
        while !String::from_utf8_lossy(&buf.lock().unwrap()).contains("FAKE-CLAUDE READY") {
            assert!(start.elapsed() < Duration::from_secs(20));
            std::thread::sleep(Duration::from_millis(20));
        }
        // Let the config thread pick the run up and arm its watcher.
        std::thread::sleep(Duration::from_millis(1200));
        assert!(events(&p, "config:changed").is_empty());
        // A local settings file appears in the project's `.claude/` (Claude editing its own config).
        let dir = p.project.join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::thread::sleep(Duration::from_millis(1200));
        std::fs::write(
            dir.join("settings.local.json"),
            r#"{ "permissions": { "allow": ["Bash(ls)"] } }"#,
        )
        .unwrap();
        assert!(
            wait_for(&p, "config:changed", 1, Duration::from_secs(10)),
            "no config:changed from the watcher"
        );
        let live = p.t.app.runs().get(r.run_id.as_str()).unwrap();
        assert!(
            live.config_changed
                .load(std::sync::atomic::Ordering::SeqCst)
        );
        assert!(live.session.is_live(), "a warning only");
        std::thread::sleep(Duration::from_millis(600));
        assert_eq!(events(&p, "config:changed").len(), 1, "once per run");
        p.t.app.runs().hangup_all();
    }

    /// The hint filter: only the run's sources are relevant; Claude Code's own churn in the config
    /// folder (transcripts, history) and `.claude/worktrees/` are not.
    #[test]
    fn config_targets_relevance() {
        let home = Path::new("/h");
        let root = Path::new("/h/work/app");
        let cwd = Path::new("/h/work/app/spec");
        let t = foreman_app::watch::config_targets(cwd, root, home, None, Path::new("/m"));
        for yes in [
            "/h/.claude/settings.json",
            "/h/.claude/CLAUDE.md",
            "/h/.claude/agents/x.md",
            "/h/.claude.json",
            "/h/work/app/CLAUDE.md",
            "/h/work/app/spec/CLAUDE.local.md",
            "/h/work/app/.claude/settings.local.json",
            "/h/work/app/spec/.mcp.json",
            "/h/work/CLAUDE.md",
            "/m/managed-settings.json",
        ] {
            assert!(t.relevant(Path::new(yes)), "{yes}");
        }
        for no in [
            "/h/.claude/projects/-h-work-app/abc.jsonl",
            "/h/.claude/history.jsonl",
            "/h/work/app/.claude/worktrees/w/settings.json",
            "/h/work/app/src/main.rs",
            "/h/work/app/docs/handoff/stage-01.md",
        ] {
            assert!(!t.relevant(Path::new(no)), "{no}");
        }
        // Folder sources recursively; files through the cwd, the repo root and the config folder.
        assert!(t.watch.contains(&(root.join(".claude"), true)));
        assert!(t.watch.contains(&(home.join(".claude"), false)));
        assert!(t.watch.contains(&(cwd.to_path_buf(), false)));
        assert!(t.watch.contains(&(home.join(".claude.json"), false)));
        assert!(
            !t.watch
                .iter()
                .any(|(w, _)| w == Path::new("/") || w == home),
            "never the whole disk or home"
        );
    }
}
