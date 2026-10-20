//! M3-26, M4-03 (F-026, ADR-018): run metrics, Export metrics, the Run log's strip and search,
//! against real runs (fake-claude) and recorded chat copies. The numbers in the strip and in the
//! export come from the same function and must agree.
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

use common::project::{ProjectApp, collect, req};
use foreman_app::ipc::commands::{launch, runs as cmd, steps};
use foreman_app::ipc::dto::launch::{PrepareOutcome, RunStarted};
use foreman_app::ipc::dto::records::{ExportResult, MetricsView, RunsPage};
use foreman_app::ipc::error::IpcErrorCode;
use foreman_app::runs::{self, ChatCopy};
use foreman_core::ids::StepKey;
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

fn start(p: &ProjectApp, step: &str) -> RunStarted {
    let PrepareOutcome::Ready(pv) = p.prepare(step) else {
        panic!("blocked");
    };
    let (ch, buf) = collect();
    let r = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), ch).unwrap();
    wait_for(&buf, "FAKE-CLAUDE READY");
    r
}

/// Ends every live session and waits until the exit handler wrote the index line.
fn finish(p: &ProjectApp, lines: usize) {
    p.t.app.runs().hangup_all();
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if index_lines(p).len() >= lines && p.t.app.runs().live().is_empty() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("no index line: {:?}", index_lines(p));
}

fn index_lines(p: &ProjectApp) -> Vec<Value> {
    let dir = p.t.app_data().join("runs");
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut files: Vec<_> = rd
        .map(|e| e.unwrap().path())
        .filter(|f| {
            f.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("index-")
        })
        .collect();
    files.sort();
    for f in files {
        for l in std::fs::read_to_string(f).unwrap().lines() {
            out.push(serde_json::from_str(l).unwrap());
        }
    }
    out
}

fn chat_copy(p: &ProjectApp, step: &str, stage: u8, tts: u64) -> String {
    runs::record_chat_copy(
        &p.t.app,
        &ChatCopy {
            project_id: &p.id,
            stage,
            step: StepKey::parse(step).unwrap(),
            chain: None,
            entry: foreman_core::schema::run::EntryPoint::Palette,
            kind: foreman_core::schema::run::RunKind::ChatCopy,
            intended: None,
            prompt_sha256: Some("a".repeat(64)),
            time_to_start_ms: tts,
        },
    )
    .unwrap()
}

fn view(p: &ProjectApp, project: Option<&str>) -> MetricsView {
    cmd::metrics_view(&p.t.app, req(json!({ "projectId": project }))).unwrap()
}

fn export(p: &ProjectApp, name: &str) -> String {
    let out = p.root.join("exports");
    std::fs::create_dir_all(&out).unwrap();
    let path = out.join(name);
    p.t.os.save_next(Some(path.clone()));
    let r = cmd::metrics_export(&p.t.app, req(json!({}))).unwrap();
    assert!(matches!(r, ExportResult::Saved { .. }), "{r:?}");
    std::fs::read_to_string(path).unwrap()
}

fn search(
    p: &ProjectApp,
    q: &str,
    project: Option<&str>,
    cursor: Option<&str>,
    limit: u16,
) -> RunsPage {
    cmd::runs_search(
        &p.t.app,
        req(json!({ "query": q, "projectId": project, "cursor": cursor, "limit": limit })),
    )
    .unwrap()
}

mod runlog {
    use super::*;

    /// AC-F026-01, AC-F017-06: a CLI run's index line carries the metrics fields; the clock stops
    /// at the spawn.
    #[test]
    fn cli_index_line_has_the_metrics_fields() {
        let p = ProjectApp::new(json!({}));
        let r = start(&p, "2.2");
        finish(&p, 1);
        let line = &index_lines(&p)[0];
        assert_eq!(line["runId"], r.run_id.as_str());
        assert_eq!(line["surface"], "cli");
        assert_eq!(line["entryPoint"], "stageView");
        assert_eq!(line["stage"], 2);
        assert!(line["timeToStartMs"].is_u64(), "{line}");
        assert!(line["durationMs"].is_u64(), "{line}");
        assert_eq!(line["waitMs"], 0);
        assert_eq!(line["runInputInterventions"], 0);
        assert_eq!(line["copyPromptOnCli"], false);
    }

    /// AC-KC-03: Copy prompt on a CLI step is recorded on the next run of that step.
    #[test]
    fn copy_prompt_on_cli_is_recorded() {
        let p = ProjectApp::new(json!({}));
        steps::clipboard_copy(
            &p.t.app,
            req(json!({ "projectId": p.id, "target": "prompt", "step": "2.2" })),
        )
        .unwrap();
        let r = start(&p, "2.2");
        let rec: Value = serde_json::from_str(
            &std::fs::read_to_string(
                p.t.app_data()
                    .join("runs")
                    .join(r.run_id.as_str())
                    .join("run.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(rec["copyPromptOnCli"], true);
        finish(&p, 1);
        assert_eq!(index_lines(&p)[0]["copyPromptOnCli"], true);
        let v = view(&p, None);
        assert_eq!(v.manual_edits_by_stage[0].edits, 1);
        // The mark was used up by that run.
        assert_eq!(
            p.t.app.runs().take_marks(&p.id, "2.2"),
            runs::Marks::default()
        );
    }

    /// AC-F026-01: setting a run input again is an intervention; the first value is not.
    #[test]
    fn retyped_run_input_is_an_intervention() {
        let p = ProjectApp::new(json!({}));
        let set = |v: &str| {
            steps::run_input_set(
                &p.t.app,
                req(json!({ "projectId": p.id, "step": "1.2", "name": "X", "value": v })),
            )
            .unwrap();
        };
        set("first");
        assert_eq!(p.t.app.runs().take_marks(&p.id, "1.2").interventions, 0);
        set("first");
        set("second");
        set("third");
        let m = p.t.app.runs().take_marks(&p.id, "1.2");
        assert_eq!(m.interventions, 2, "the same value again is not a change");
        assert_eq!(p.t.app.runs().take_marks(&p.id, "1.2").interventions, 0);
    }

    /// AC-F026-02: the chat clock ends when Copy and open returns — the run is recorded with the
    /// time it took, and the step-done line adds the active time.
    #[test]
    fn chat_clock_ends_at_copy_and_open() {
        let p = ProjectApp::new(json!({}));
        let id = chat_copy(&p, "1.1", 1, 6500);
        let rec: Value = serde_json::from_str(
            &std::fs::read_to_string(p.t.app_data().join("runs").join(&id).join("run.json"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(rec["surface"], "chat");
        assert_eq!(rec["kind"], "chatCopy");
        assert_eq!(rec["status"], "copied");
        assert_eq!(rec["timeToStartMs"], 6500);
        assert_eq!(rec["t"]["spawned"], 6500);
        assert_eq!(rec["argv"], json!([]));
        assert_eq!(index_lines(&p).len(), 1);
        assert_eq!(index_lines(&p)[0]["durationMs"], Value::Null);
        assert_eq!(view(&p, None).per_surface[0].median_time_to_start_ms, 6500);
        assert!(
            view(&p, None).per_surface[0].median_active_ms_per_step == 0,
            "not done yet"
        );

        // Paste-back finishes the step two minutes later.
        p.t.os.set_now(
            foreman_app::os::Os::now(&*p.t.os)
                .checked_add(jiff::SignedDuration::from_secs(120))
                .unwrap(),
        );
        runs::note_step_done(&p.t.app, &p.id, "1.1");
        runs::note_step_done(&p.t.app, &p.id, "1.1"); // idempotent
        let lines = index_lines(&p);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1]["runId"], id.as_str());
        // Two minutes after Copy and open returned.
        assert_eq!(lines[1]["durationMs"], 120_000);
        let v = view(&p, None);
        let chat = &v.per_surface[0];
        assert_eq!(chat.median_time_to_start_ms, 6500);
        assert_eq!(chat.median_active_ms_per_step, 126_500);
        assert_eq!(v.runs_counted, 1, "two lines, one run");
    }
}

mod metrics_view {
    use super::*;

    /// AC-F026-06 / the empty registry: zero runs, no baseline.
    #[test]
    fn empty() {
        let p = ProjectApp::new(json!({}));
        let v = view(&p, None);
        assert_eq!(v.runs_counted, 0);
        assert!(v.per_surface.is_empty() && v.baseline.is_none());
        assert_eq!(v.p95_all_ms, 0);
        let e = cmd::metrics_view(
            &p.t.app,
            req(json!({ "projectId": "00000000-0000-4000-8000-000000000000" })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotFound);
    }

    /// AC-F026-09: the strip shows per-surface medians and p95, edits per stage, stages run, the
    /// comparison with the baseline — and Export metrics, from the same function, agrees.
    #[test]
    fn strip_matches_the_export() {
        let p = ProjectApp::new(json!({}));
        std::fs::create_dir_all(p.project.join("docs")).unwrap();
        std::fs::write(
            p.project.join("docs/metrics-baseline.md"),
            "## Log\n\n| Date | Project | Stage.step | Surface | Decided at | Ready at | Seconds to start | Done at | Waits (min) | Active minutes | Manual edits | Mistakes (describe) |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n| 2026-10-01 | x | 5.1 | CLI | | | 90 | | 0 | 20 | 0 | |\n| 2026-10-02 | x | 1.1 | chat | | | 60 | | 0 | 30 | 1 | |\n",
        )
        .unwrap();
        for (i, (step, tts)) in [("1.1", 5000), ("1.2", 7000), ("1.4", 9000)]
            .iter()
            .enumerate()
        {
            chat_copy(&p, step, 1, *tts);
            let _ = i;
        }
        let _ = start(&p, "2.2");
        finish(&p, 4);
        let v = view(&p, None);
        assert_eq!(v.runs_counted, 4);
        assert_eq!(v.stages_through_foreman, 2);
        let chat = v
            .per_surface
            .iter()
            .find(|s| format!("{:?}", s.surface) == "Chat")
            .unwrap();
        assert_eq!(chat.median_time_to_start_ms, 7000);
        assert_eq!(chat.p95_time_to_start_ms, 9000);
        let b = v.baseline.unwrap();
        assert_eq!(b.baseline_median_ms, 75_000, "90 s and 60 s");
        let by_project = view(&p, Some(&p.id));
        assert_eq!(by_project.runs_counted, 4);

        let text = export(&p, "metrics.md");
        assert!(
            text.contains("| Median seconds to start | chat | 60.0 | 7.0 |"),
            "{text}"
        );
        assert!(text.contains("| Steps logged | chat | 1 | 3 |"), "{text}");
        assert!(text.contains("| Steps logged | CLI | 1 | 1 |"), "{text}");
        assert!(text.contains("TradeLog"), "project names, not IDs");
        assert!(text.contains("4 runs counted"), "{text}");
    }
}

mod export {
    use super::*;

    /// AC-F026-06: with no runs the tables are written with a note.
    #[test]
    fn no_runs_writes_the_tables_and_a_note() {
        let p = ProjectApp::new(json!({}));
        let text = export(&p, "metrics.md");
        assert!(text.contains("No runs were recorded"));
        assert!(text.contains("## Summary"));
    }

    /// SR-24: a cancelled save dialog writes nothing and says so.
    #[test]
    fn cancel_writes_nothing() {
        let p = ProjectApp::new(json!({}));
        p.t.os.save_next(None);
        let r = cmd::metrics_export(&p.t.app, req(json!({}))).unwrap();
        assert_eq!(r, ExportResult::Cancelled);
    }

    /// SR-30: the chosen path is checked by the core's policy — `.md` only, never in `.claude`,
    /// `.git` or `.foreman`, never over a symlink, never in app data.
    #[test]
    fn the_path_policy_applies() {
        let p = ProjectApp::new(json!({}));
        let claude = p.project.join(".claude");
        std::fs::create_dir_all(&claude).unwrap();
        for target in [
            claude.join("metrics.md"),
            p.project.join("metrics.txt"),
            p.t.app_data().join("metrics.md"),
        ] {
            p.t.os.save_next(Some(target.clone()));
            let e = cmd::metrics_export(&p.t.app, req(json!({}))).unwrap_err();
            assert_eq!(e.code, IpcErrorCode::PolicyViolation, "{target:?}: {e:?}");
            assert!(!target.exists());
        }
    }

    /// FILE-R9: an export replaces an older export, atomically, at the path Billy chose.
    #[test]
    fn replaces_an_older_export() {
        let p = ProjectApp::new(json!({}));
        let one = export(&p, "metrics.md");
        chat_copy(&p, "1.1", 1, 4000);
        let two = export(&p, "metrics.md");
        assert_ne!(one, two);
        assert!(two.contains("1 runs counted"));
    }
}

mod search {
    use super::*;

    /// AC-F026-07: empty state, then every run newest first.
    #[test]
    fn empty_then_listed() {
        let p = ProjectApp::new(json!({}));
        let page = search(&p, "", None, None, 50);
        assert!(page.runs.is_empty() && page.total == 0 && page.next.is_none());
        chat_copy(&p, "1.1", 1, 4000);
        let r = start(&p, "2.2");
        let page = search(&p, "", None, None, 50);
        assert_eq!(
            page.total, 2,
            "the live run is listed before it has an index line"
        );
        assert_eq!(page.runs[0].run_id.as_str(), r.run_id.as_str());
        assert_eq!(
            page.runs[0].status,
            foreman_app::ipc::types::RunStatus::Running
        );
        finish(&p, 2);
        let page = search(&p, "", None, None, 50);
        assert_eq!(page.total, 2);
        let cli = &page.runs[0];
        assert_eq!(cli.project_name, "TradeLog");
        assert!(
            !cli.step_title.is_empty() && cli.step_title != "2.2",
            "{}",
            cli.step_title
        );
        assert!(
            cli.argv.iter().any(|a| a.contains("sha256:")),
            "{:?}",
            cli.argv
        );
    }

    /// AC-F026-07: search matches argv, session name, outcome, step and project.
    #[test]
    fn query_matches_argv_session_outcome_and_step() {
        let p = ProjectApp::new(json!({}));
        chat_copy(&p, "1.1", 1, 4000);
        let r = start(&p, "2.2");
        finish(&p, 2);
        let all = search(&p, "", None, None, 50);
        let cli = all
            .runs
            .iter()
            .find(|x| x.run_id.as_str() == r.run_id.as_str())
            .unwrap();
        assert!(!cli.tab_name.is_empty());
        // The session was ended by hangup, so Claude Code may or may not have reported a code.
        let outcome = if cli.status == foreman_app::ipc::types::RunStatus::Errored {
            "errored"
        } else {
            "exited"
        };
        for q in [
            cli.tab_name.as_str(),
            "--session-id",
            "2.2",
            "tradelog",
            "cli",
            outcome,
        ] {
            let page = search(&p, q, None, None, 50);
            assert!(
                page.runs
                    .iter()
                    .any(|x| x.run_id.as_str() == r.run_id.as_str()),
                "{q}"
            );
        }
        assert_eq!(search(&p, "copied", None, None, 50).total, 1);
        assert_eq!(
            search(&p, &format!("TRADELOG {outcome}"), None, None, 50).total,
            1,
            "all words, any case"
        );
        assert_eq!(search(&p, "zzzz-nothing", None, None, 50).total, 0);
    }

    /// Paging and the project filter.
    #[test]
    fn pages_and_project_filter() {
        let p = ProjectApp::new(json!({}));
        for (i, step) in ["1.1", "1.2", "1.4"].iter().enumerate() {
            p.t.os.set_now(
                jiff::Timestamp::from_second(1_792_400_400 + i64::try_from(i).unwrap() * 60)
                    .unwrap(),
            );
            chat_copy(&p, step, 1, 1000);
        }
        let a = search(&p, "", None, None, 2);
        assert_eq!((a.runs.len(), a.total), (2, 3));
        let b = search(&p, "", None, a.next.as_deref(), 2);
        assert_eq!(b.runs.len(), 1);
        assert!(b.next.is_none());
        let mut ids: Vec<_> = a
            .runs
            .iter()
            .chain(&b.runs)
            .map(|r| r.run_id.as_str().to_owned())
            .collect();
        ids.dedup();
        assert_eq!(ids.len(), 3);
        assert!(
            a.runs[0].started_at.0 > a.runs[1].started_at.0,
            "newest first"
        );
        assert_eq!(search(&p, "", Some(&p.id), None, 50).total, 3);
        let e = cmd::runs_search(
            &p.t.app,
            req(json!({ "query": "", "projectId": "00000000-0000-4000-8000-000000000000", "cursor": null, "limit": 5 })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotFound);
        for bad in [0, 201] {
            let e = cmd::runs_search(
                &p.t.app,
                req(json!({ "query": "", "projectId": null, "cursor": null, "limit": bad })),
            )
            .unwrap_err();
            assert_eq!(e.code, IpcErrorCode::Invalid);
        }
    }

    /// AC-F026-07: Resume is offered where Claude Code still has the transcript.
    #[test]
    fn resume_only_where_the_transcript_exists() {
        let p = ProjectApp::new(json!({}));
        let r = start(&p, "2.2");
        finish(&p, 1);
        let page = search(&p, "", None, None, 5);
        let run = &page.runs[0];
        let cwd = foreman_sys::spawn::canonical(&p.project.join("spec"));
        let t = foreman_core::chain::transcript_path(
            &p.t.home().join(".claude"),
            &cwd,
            uuid::Uuid::parse_str(run.chain.as_str()).unwrap(),
        );
        let _ = std::fs::remove_file(&t);
        let page = search(&p, "", None, None, 5);
        let run = &page.runs[0];
        assert!(!run.resumable, "no transcript");
        assert!(run.why_not_resumable.is_some());
        // Claude Code wrote its transcript.
        std::fs::create_dir_all(t.parent().unwrap()).unwrap();
        std::fs::write(&t, "{}").unwrap();
        let page = search(&p, "", None, None, 5);
        assert_eq!(page.runs[0].run_id.as_str(), r.run_id.as_str());
        assert!(
            page.runs[0].resumable,
            "{:?}",
            page.runs[0].why_not_resumable
        );
    }
}
