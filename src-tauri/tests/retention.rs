//! M4-02 (PR-04, PR-05, SR-53; AC-F026-10, AC-F029-06): tails pruned after 90 days, Clear run
//! history, and project removal that leaves the repo alone. Every destructive action is a native
//! confirmation (a scripted Cancel changes nothing).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::project::{ProjectApp, collect, req};
use foreman_app::ipc::commands::{launch, projects, runs as cmd};
use foreman_app::ipc::dto::launch::PrepareOutcome;
use foreman_app::ipc::error::IpcErrorCode;
use foreman_app::runs::{self, ChatCopy};
use foreman_core::ids::StepKey;
use serde_json::json;

fn copy(p: &ProjectApp, step: &str) -> String {
    runs::record_chat_copy(
        &p.t.app,
        &ChatCopy {
            project_id: &p.id,
            stage: 1,
            step: StepKey::parse(step).unwrap(),
            chain: None,
            entry: foreman_core::schema::run::EntryPoint::Palette,
            kind: foreman_core::schema::run::RunKind::ChatCopy,
            intended: None,
            over: false,
            prompt_sha256: None,
            time_to_start_ms: 1000,
        },
    )
    .unwrap()
}

fn put(p: &ProjectApp, rel: &str, text: &str) {
    let f = p.t.app_data().join(rel);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(f, text).unwrap();
}

fn at(secs: i64) -> jiff::Timestamp {
    jiff::Timestamp::from_second(secs).unwrap()
}

/// Every file under `dir` with its bytes.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let rel = p.strip_prefix(base).unwrap().display().to_string();
            if p.is_dir() {
                out.insert(format!("{rel}/"), Vec::new());
                walk(base, &p, out);
            } else {
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn wait_idle(p: &ProjectApp) {
    p.t.app.runs().hangup_all();
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if p.t.app.runs().live().is_empty() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("a session is still live");
}

fn start(p: &ProjectApp, step: &str) {
    let PrepareOutcome::Ready(pv) = p.prepare(step) else {
        panic!("blocked");
    };
    let (ch, buf): (_, Arc<Mutex<Vec<u8>>>) = collect();
    launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !String::from_utf8_lossy(&buf.lock().unwrap()).contains("FAKE-CLAUDE READY") {
        assert!(Instant::now() < deadline, "fake-claude never came up");
        std::thread::sleep(Duration::from_millis(20));
    }
}

mod retention {
    use super::*;

    /// AC-F026-10, SR-53: output tails older than 90 days go; younger ones, the run records and
    /// the index lines stay (the metrics keep working).
    #[test]
    fn tail_pruned_after_90_days() {
        let p = ProjectApp::new(json!({}));
        let day = 86_400;
        let now = 1_792_400_400;
        p.t.os.set_now(at(now - 91 * day));
        let old = copy(&p, "1.1");
        p.t.os.set_now(at(now - 89 * day));
        let young = copy(&p, "1.2");
        p.t.os.set_now(at(now));
        for id in [&old, &young] {
            put(&p, &format!("runs/{id}/output.log"), "terminal text");
        }
        let n = runs::prune_tails(&p.t.app).unwrap();
        assert_eq!(n, 1);
        let runs_dir = p.t.app_data().join("runs");
        assert!(!runs_dir.join(&old).join("output.log").exists());
        assert!(runs_dir.join(&old).join("run.json").exists());
        assert!(runs_dir.join(&young).join("output.log").exists());
        // Nothing left to prune; the index still counts both runs.
        assert_eq!(runs::prune_tails(&p.t.app).unwrap(), 0);
        let v = cmd::metrics_view(&p.t.app, req(json!({ "projectId": null }))).unwrap();
        assert_eq!(v.runs_counted, 2);
    }

    /// ADR-018 §5, files.md rows 20 and 21: the startup pruner deletes a template no project pins
    /// (with its parse caches) once it has been unused for 90 days, and keeps the pinned one,
    /// whatever its age, refreshing the time that means "last used".
    #[test]
    fn unused_template_pruned_after_90_days() {
        use std::time::{Duration, SystemTime};
        let p = ProjectApp::new(json!({}));
        let now = 1_792_400_400_u64;
        let age = |rel: &str, days: u64| {
            let f = p.t.app_data().join(rel);
            std::fs::File::options()
                .write(true)
                .open(f)
                .unwrap()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(now - days * 86_400))
                .unwrap();
        };
        let templates: Vec<String> = std::fs::read_dir(p.t.app_data().join("templates"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        let pinned = templates
            .iter()
            .find(|n| n.ends_with(".md"))
            .expect("the project pinned a template")
            .clone();
        for n in &templates {
            age(&format!("templates/{n}"), 400);
        }
        let stale = "c".repeat(64);
        put(&p, &format!("templates/{stale}.md"), "unused text");
        put(&p, &format!("templates/{stale}.p3.json"), "{}");
        age(&format!("templates/{stale}.md"), 91);
        age(&format!("templates/{stale}.p3.json"), 91);
        let young = "d".repeat(64);
        put(&p, &format!("templates/{young}.md"), "recent text");
        age(&format!("templates/{young}.md"), 89);

        p.t.os.set_now(at(i64::try_from(now).unwrap()));
        let pass = runs::prune_templates(&p.t.app).unwrap();
        assert_eq!(pass.removed, 2, "{pass:?}");
        let dir = p.t.app_data().join("templates");
        assert!(!dir.join(format!("{stale}.md")).exists());
        assert!(!dir.join(format!("{stale}.p3.json")).exists());
        assert!(dir.join(format!("{young}.md")).exists(), "89 days: kept");
        assert!(dir.join(&pinned).exists(), "pinned: kept however old");
        let m = std::fs::metadata(dir.join(&pinned))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(
            m.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            now,
            "a pinned template's last use is now"
        );
        // The project still opens from the kept text.
        assert!(
            foreman_app::ipc::commands::projects::project_open(
                &p.t.app,
                req(json!({ "projectId": p.id }))
            )
            .is_ok()
        );
    }

    /// If a project's lock can't be read, nothing is called unused: no deletes at all.
    #[test]
    fn unreadable_lock_prunes_nothing() {
        use std::time::{Duration, SystemTime};
        let p = ProjectApp::new(json!({}));
        let stale = "e".repeat(64);
        put(&p, &format!("templates/{stale}.md"), "unused text");
        std::fs::File::options()
            .write(true)
            .open(p.t.app_data().join(format!("templates/{stale}.md")))
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000))
            .unwrap();
        put(
            &p,
            &format!("approvals/{}/blueprint.lock.json", p.id),
            "{ not json",
        );
        p.t.os.set_now(at(1_792_400_400));
        assert!(runs::prune_templates(&p.t.app).is_err());
        assert!(
            p.t.app_data()
                .join(format!("templates/{stale}.md"))
                .exists()
        );
    }

    /// SR-53: the first scan at startup prunes (the same pass that finds interrupted runs).
    #[test]
    fn startup_scan_prunes() {
        let p = ProjectApp::new(json!({}));
        p.t.os.set_now(at(1_792_400_400 - 100 * 86_400));
        let old = copy(&p, "1.1");
        p.t.os.set_now(at(1_792_400_400));
        put(&p, &format!("runs/{old}/output.log"), "x");
        let stale = "f".repeat(64);
        put(&p, &format!("templates/{stale}.md"), "unused");
        std::fs::File::options()
            .write(true)
            .open(p.t.app_data().join(format!("templates/{stale}.md")))
            .unwrap()
            .set_modified(std::time::SystemTime::UNIX_EPOCH)
            .unwrap();
        runs::interrupted(&p.t.app).unwrap();
        assert!(
            !p.t.app_data()
                .join("runs")
                .join(&old)
                .join("output.log")
                .exists()
        );
        assert!(
            !p.t.app_data()
                .join(format!("templates/{stale}.md"))
                .exists(),
            "the same scan prunes templates no project pins"
        );
    }
}

mod privacy {
    use super::*;

    /// AC-F026-10, PR-05: Clear run history deletes run records, tails and events — after a native
    /// confirmation. Cancel keeps all of it; stage records and progress are untouched.
    #[test]
    fn clear_history() {
        let p = ProjectApp::new(json!({}));
        let id = copy(&p, "1.1");
        put(&p, &format!("runs/{id}/output.log"), "terminal text");
        put(&p, "events/abc/00001-Stop.json", "{}");
        let progress =
            p.t.app_data()
                .join(format!("projects/{}/progress.json", p.id));
        let before = std::fs::read(&progress).unwrap();

        p.t.os.answer_next(false);
        let e = cmd::runs_clear(&p.t.app, req(json!({}))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert!(
            p.t.app_data()
                .join("runs")
                .join(&id)
                .join("run.json")
                .exists()
        );
        assert!(p.t.app_data().join("events").exists());

        p.t.os.answer_next(true);
        cmd::runs_clear(&p.t.app, req(json!({}))).unwrap();
        assert!(!p.t.app_data().join("runs").exists());
        assert!(!p.t.app_data().join("events").exists());
        assert_eq!(std::fs::read(&progress).unwrap(), before);
        let asked = p.t.os.asked();
        assert_eq!(asked.len(), 2);
        assert_eq!(asked[1].ok, "Clear run history");
        assert!(asked[1].message.contains("1 runs"), "{}", asked[1].message);
        let v = cmd::metrics_view(&p.t.app, req(json!({ "projectId": null }))).unwrap();
        assert_eq!(v.runs_counted, 0);
        // Settings → Data shows the empty history.
        let s =
            foreman_app::ipc::commands::settings::settings_get(&p.t.app, req(json!({}))).unwrap();
        assert_eq!((s.run_history.runs, s.run_history.megabytes), (0, 0));
    }

    /// A live session blocks Clear and project removal (`runs-live`), before any dialog.
    #[test]
    fn live_runs_block_destruction() {
        let p = ProjectApp::new(json!({}));
        start(&p, "2.2");
        let asked = p.t.os.asked().len();
        let e = cmd::runs_clear(&p.t.app, req(json!({}))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        let e = projects::project_remove(&p.t.app, req(json!({ "projectId": p.id }))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        assert_eq!(
            p.t.os.asked().len(),
            asked,
            "no dialog for a refused action"
        );
        wait_idle(&p);
    }

    /// AC-F029-06, PR-05, SR-55: removing a project deletes its app-data folder, approvals and
    /// configuration snapshots, and leaves the repo exactly as it was.
    #[test]
    fn remove_project_leaves_repo() {
        let p = ProjectApp::new(json!({}));
        // A run leaves a fingerprint snapshot for the spec folder.
        start(&p, "2.2");
        wait_idle(&p);
        let app = p.t.app_data();
        let id = &p.id;
        assert!(app.join(format!("projects/{id}")).exists());
        assert!(app.join(format!("approvals/{id}.json")).exists());
        assert!(app.join(format!("approvals/{id}")).exists());
        let snapshots = || {
            std::fs::read_dir(app.join("cache/fingerprints"))
                .map(|r| r.count())
                .unwrap_or(0)
        };
        assert!(snapshots() > 0, "a launch leaves a config snapshot");
        let repo_before = tree(&p.project);
        // Run records only: the month index may still gain the exit line of the run above.
        let run_dirs = || {
            std::fs::read_dir(app.join("runs"))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.path().is_dir())
                .count()
        };
        let runs_before = run_dirs();

        // Cancel changes nothing.
        p.t.os.answer_next(false);
        let e = projects::project_remove(&p.t.app, req(json!({ "projectId": id }))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert!(app.join(format!("projects/{id}")).exists());
        assert!(snapshots() > 0);

        let left = projects::project_remove(&p.t.app, req(json!({ "projectId": id }))).unwrap();
        assert!(left.is_empty());
        assert!(!app.join(format!("projects/{id}")).exists());
        assert!(!app.join(format!("approvals/{id}.json")).exists());
        assert!(!app.join(format!("approvals/{id}")).exists());
        assert_eq!(snapshots(), 0, "the redacted config snapshots went too");
        assert_eq!(tree(&p.project), repo_before, "the repo is untouched");
        assert_eq!(
            run_dirs(),
            runs_before,
            "run history is kept for the metrics"
        );
        let asked = p.t.os.asked();
        let last = asked.last().unwrap();
        assert!(
            last.title.contains("TradeLog") && last.message.contains("stay exactly as they are")
        );
        assert_eq!(last.ok, "Remove project");
        // Removing it again is a clean NotFound.
        let e = projects::project_remove(&p.t.app, req(json!({ "projectId": id }))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotFound);
    }
}
