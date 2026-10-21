//! M3-26 / M4-03 (F-026, ADR-018): the run log's numbers and Export metrics.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

use std::collections::BTreeMap;

use foreman_core::ids::StepKey;
use foreman_core::runlog::{self as rl, Baseline, ExportContext};
use foreman_core::schema::run::{
    IndexEntryPoint as Entry, IndexStatus as Status, IndexSurface as Surface, RunIndexLine,
};

const P1: &str = "11111111-1111-4111-8111-111111111111";

#[allow(clippy::too_many_arguments, reason = "a fixture builder")]
fn line(
    n: u32,
    surface: Surface,
    stage: u8,
    step: &str,
    entry: Entry,
    tts: Option<u64>,
    dur: Option<u64>,
    status: Status,
) -> RunIndexLine {
    RunIndexLine {
        schema_version: 1,
        run_id: format!("00000000-0000-4000-8000-{n:012}"),
        project_id: P1.to_owned(),
        stage,
        step_id: StepKey::parse(step).unwrap(),
        surface,
        entry_point: entry,
        wall_start: format!("2026-10-19T09:{n:02}:00Z"),
        time_to_start_ms: tts,
        duration_ms: dur,
        wait_ms: 0,
        status,
        run_input_interventions: 0,
        copy_prompt_on_cli: false,
        r#override: false,
    }
}

fn names() -> BTreeMap<String, String> {
    BTreeMap::from([(P1.to_owned(), "TradeLog".to_owned())])
}

fn cx(names: &BTreeMap<String, String>) -> ExportContext<'_> {
    ExportContext {
        generated_on: "2026-10-20",
        project_names: names,
    }
}

mod runlog {
    use super::*;

    #[test]
    fn index_torn_line_skipped() {
        let a = line(
            1,
            Surface::Cli,
            1,
            "1.1",
            Entry::DoNext,
            Some(5000),
            Some(60_000),
            Status::Exited,
        );
        let b = line(
            2,
            Surface::Chat,
            1,
            "1.2",
            Entry::Palette,
            Some(8000),
            None,
            Status::Copied,
        );
        let mut text = String::new();
        text.push_str(&serde_json::to_string(&a).unwrap());
        text.push('\n');
        text.push_str("\n   \n{\"schemaVersion\":1,\"runId\":\"nope\"}\n");
        text.push_str(&serde_json::to_string(&b).unwrap());
        text.push('\n');
        // A write cut off by a crash: the last line has no end.
        let torn = serde_json::to_string(&a).unwrap();
        text.push_str(&torn[..torn.len() / 2]);
        let got = rl::parse_index(&text);
        assert_eq!(
            got.len(),
            2,
            "the blank, invalid and torn lines are skipped"
        );
        assert_eq!(got[0], a);
        assert_eq!(got[1], b);
    }

    #[test]
    fn last_line_per_run_wins() {
        let copied = line(
            1,
            Surface::Chat,
            1,
            "1.1",
            Entry::Palette,
            Some(8000),
            None,
            Status::Copied,
        );
        let mut done = copied.clone();
        done.duration_ms = Some(240_000);
        let other = line(
            2,
            Surface::Cli,
            1,
            "1.2",
            Entry::DoNext,
            Some(5000),
            Some(1000),
            Status::Exited,
        );
        let merged = rl::merge_latest(vec![copied, other.clone(), done.clone()]);
        assert_eq!(merged, vec![done, other]);
    }

    #[test]
    fn median_and_p95() {
        assert_eq!(rl::stats(&[]), None);
        let one = rl::stats(&[7000]).unwrap();
        assert_eq!((one.n, one.median, one.p95), (1, 7000, 7000));
        let even = rl::stats(&[4000, 1000, 2000, 3000]).unwrap();
        assert_eq!(even.median, 2500, "the mean of the middle two");
        assert_eq!(even.p95, 4000);
        let twenty: Vec<u64> = (1..=20).map(|v| v * 1000).collect();
        let s = rl::stats(&twenty).unwrap();
        assert_eq!(s.median, 10_500);
        assert_eq!(s.p95, 19_000, "nearest rank: ceil(0.95 * 20) = 19th");
        let hundred: Vec<u64> = (1..=100).collect();
        assert_eq!(rl::stats(&hundred).unwrap().p95, 95);
    }

    #[test]
    fn per_surface_and_active_time() {
        let mut i = vec![
            // CLI step 1.1: one run, 5 s to start, 100 s running.
            line(
                1,
                Surface::Cli,
                1,
                "1.1",
                Entry::DoNext,
                Some(5000),
                Some(100_000),
                Status::Exited,
            ),
            // CLI step 1.3: first run exits, then a re-arm continues it. Re-arms are not starts.
            line(
                2,
                Surface::Cli,
                1,
                "1.3",
                Entry::StageView,
                Some(9000),
                Some(60_000),
                Status::Exited,
            ),
            line(
                3,
                Surface::Cli,
                1,
                "1.3",
                Entry::Rearm,
                Some(2000),
                Some(30_000),
                Status::Exited,
            ),
            // Chat step 1.2: copy line, replaced by the step-done line.
            line(
                4,
                Surface::Chat,
                1,
                "1.2",
                Entry::Palette,
                Some(7000),
                Some(200_000),
                Status::Copied,
            ),
            // A chat step never finished: counted as a start, not as an active sample.
            line(
                5,
                Surface::Chat,
                2,
                "2.1",
                Entry::DoNext,
                Some(11_000),
                None,
                Status::Copied,
            ),
            // Desktop is not part of the kill check.
            RunIndexLine {
                surface: Surface::Desktop,
                ..line(
                    6,
                    Surface::Cli,
                    2,
                    "2.2",
                    Entry::DoNext,
                    Some(1),
                    Some(1),
                    Status::Copied,
                )
            },
        ];
        i[0].run_input_interventions = 1;
        i[1].copy_prompt_on_cli = true;
        i[4].r#override = true;
        let r = rl::analyze(&i);
        assert_eq!(r.runs_counted, 5);
        assert_eq!(
            r.stages_through_foreman, 2,
            "(project, stage) pairs: 1 and 2"
        );

        let cli = r.surface(Surface::Cli).unwrap();
        assert_eq!((cli.runs, cli.steps), (3, 2));
        assert_eq!(cli.starts.unwrap().n, 2, "the re-arm is not a start");
        assert_eq!(cli.starts.unwrap().median, 7000, "5 s and 9 s");
        // Active per step: 1.1 = 105 s, 1.3 = 9 + 60 + 2 + 30 = 101 s (re-arm overhead counts).
        assert_eq!(cli.active.unwrap().n, 2);
        assert_eq!(cli.active.unwrap().median, 103_000);
        assert_eq!((cli.interventions, cli.copy_prompts), (1, 1));

        let chat = r.surface(Surface::Chat).unwrap();
        assert_eq!((chat.runs, chat.steps), (2, 2));
        assert_eq!(chat.starts.unwrap().median, 9000);
        assert_eq!(
            chat.active.unwrap().n,
            1,
            "the unfinished chat step has no active time"
        );
        assert_eq!(
            chat.active.unwrap().median,
            207_000,
            "7 s to copy and open, then 200 s"
        );
        assert_eq!(chat.overrides, 1);

        assert_eq!(r.all_starts.unwrap().n, 4);
        assert_eq!(r.edits_total(), 2);
        let s1 = r.stages.iter().find(|s| s.stage == 1).unwrap();
        assert_eq!((s1.steps, s1.edits), (3, 2));
    }

    #[test]
    fn chat_clock_ends_when_the_prompt_is_copied_and_the_chat_open() {
        // The index records the moment Copy and open returned as timeToStartMs, and the
        // step-done line adds the time to the packet saved; Send is in neither.
        let copied = line(
            1,
            Surface::Chat,
            1,
            "1.1",
            Entry::Palette,
            Some(6500),
            None,
            Status::Copied,
        );
        let done = RunIndexLine {
            duration_ms: Some(120_000),
            ..copied.clone()
        };
        let r = rl::analyze(&rl::merge_latest(vec![copied, done]));
        let chat = r.surface(Surface::Chat).unwrap();
        assert_eq!(chat.starts.unwrap().median, 6500);
        assert_eq!(chat.active.unwrap().median, 126_500);
    }

    #[test]
    fn interrupted_runs_do_not_finish_a_step() {
        let r = rl::analyze(&[line(
            1,
            Surface::Cli,
            1,
            "1.1",
            Entry::DoNext,
            Some(4000),
            Some(50_000),
            Status::Interrupted,
        )]);
        let cli = r.surface(Surface::Cli).unwrap();
        assert_eq!(cli.starts.unwrap().median, 4000);
        assert!(cli.active.is_none());
    }

    const BASELINE: &str = "# Metrics baseline\n\n## Log\n\n\
| Date | Project | Stage.step | Surface | Decided at | Ready at | Seconds to start | Done at | Waits (min) | Active minutes | Manual edits | Mistakes (describe) |\n\
|---|---|---|---|---|---|---|---|---|---|---|---|\n\
| 2026-10-01 | engine-assist | 5.1 | CLI | 09:00:00 | 09:01:30 | 90 | 09:20 | 0 | 19.5 | 2 | |\n\
| 2026-10-01 | engine-assist | 5.2 | CLI | 10:00:00 | 10:02:10 | 130.5 | 10:40 | 10 | 28 | 0 | wrong model |\n\
| 2026-10-02 | TradeLog | 1.1 | chat | 09:00:00 | 09:00:45 | 45 | 09:30 | 5 | 25 | 1 | |\n\
| | | | | | | | | | | | |\n\n\
## Summary\n\n| Measure | Surface | By hand | Foreman |\n|---|---|---|---|\n| Steps logged | chat | | |\n";

    #[test]
    fn baseline_table_is_read() {
        let b = Baseline::parse(BASELINE);
        assert_eq!(
            b.rows.len(),
            3,
            "the empty template row and the Summary table are skipped"
        );
        assert_eq!(b.steps(Surface::Cli), 2);
        assert_eq!(b.starts(Surface::Cli).unwrap().median, 110_250);
        assert_eq!(
            b.active(Surface::Cli).unwrap().median,
            1_425_000,
            "19.5 and 28 minutes"
        );
        assert_eq!(b.starts(Surface::Chat).unwrap().median, 45_000);
        assert_eq!(b.active(Surface::Chat).unwrap().median, 25 * 60_000);
        assert_eq!(b.rows[0].stage, Some(5));
        assert_eq!(b.rows[1].manual_edits, 0);
        assert!(b.rows[1].mistake && !b.rows[0].mistake);
    }

    #[test]
    fn the_repos_own_baseline_parses_empty() {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/metrics-baseline.md"),
        )
        .unwrap();
        let b = Baseline::parse(&text);
        assert!(
            b.rows.iter().all(|r| r.start_ms.is_some()),
            "every row Billy has filled in has a start time"
        );
    }

    #[test]
    fn comparison_for_the_strip() {
        let b = Baseline::parse(BASELINE);
        let r = rl::analyze(&[line(
            1,
            Surface::Cli,
            1,
            "1.1",
            Entry::DoNext,
            Some(6000),
            Some(1000),
            Status::Exited,
        )]);
        assert_eq!(rl::comparison(&r, &b), Some((90_000, 6000)));
        assert_eq!(rl::comparison(&rl::analyze(&[]), &b), None);
        assert_eq!(rl::comparison(&r, &Baseline::default()), None);
    }

    fn sample_index() -> Vec<RunIndexLine> {
        let mut v = vec![
            line(
                1,
                Surface::Cli,
                1,
                "1.1",
                Entry::DoNext,
                Some(5200),
                Some(1_200_000),
                Status::Exited,
            ),
            line(
                2,
                Surface::Chat,
                1,
                "1.2",
                Entry::Palette,
                Some(8400),
                Some(600_000),
                Status::Copied,
            ),
            line(
                3,
                Surface::Cli,
                1,
                "1.3",
                Entry::StageView,
                Some(6100),
                Some(300_000),
                Status::Exited,
            ),
            line(
                4,
                Surface::Cli,
                1,
                "1.3",
                Entry::Rearm,
                Some(1800),
                Some(240_000),
                Status::Exited,
            ),
            line(
                5,
                Surface::Cli,
                2,
                "2.1",
                Entry::DoNext,
                Some(4900),
                Some(900_000),
                Status::Exited,
            ),
        ];
        v[2].run_input_interventions = 1;
        v[4].copy_prompt_on_cli = true;
        v[4].r#override = true;
        v
    }

    #[test]
    fn export_matches_baseline_layout() {
        let n = names();
        let text = rl::export_metrics(&sample_index(), Some(&Baseline::parse(BASELINE)), &cx(&n));
        // The summary is the baseline file's Summary table: same columns, same rows, per surface.
        assert!(text.contains(
            "| Measure | Surface | By hand (this file) | Foreman (Export metrics) |\n|---|---|---|---|\n| Steps logged | chat | 1 | 1 |\n| Steps logged | CLI | 2 | 3 |"
        ));
        for row in [
            "Median seconds to start",
            "p95 seconds to start",
            "Median active minutes per step",
            "Manual edits per step",
            "Mistakes per stage",
        ] {
            assert!(text.contains(&format!("| {row} |")), "{row}");
        }
        insta::assert_snapshot!("export_metrics_sample", text);
    }

    #[test]
    fn export_without_runs_says_so() {
        let n = names();
        let text = rl::export_metrics(&[], None, &cx(&n));
        assert!(text.contains("No runs were recorded"));
        // The tables are still there (AC-F026-06).
        assert!(text.contains("## Summary"));
        assert!(text.contains("## Foreman by surface"));
        assert!(text.contains("## Foreman by stage"));
        assert!(text.contains("## Run log"));
        assert!(text.contains("| Steps logged | CLI | 0 | 0 |"));
    }

    #[test]
    fn export_shows_elapsed_and_waits_apart() {
        let n = names();
        let mut i = sample_index();
        i[0].wait_ms = 300_000;
        let text = rl::export_metrics(&i, None, &cx(&n));
        assert!(text.contains("Median elapsed per step (min)"));
        assert!(text.contains("Waits excluded (min)"));
        // 1.1 elapsed = 5.2 s + 1,200 s = 20.1 min; its 5 min wait is excluded from active.
        let cli_row = text
            .lines()
            .find(|l| l.starts_with("| CLI | 4 | 3 |"))
            .unwrap();
        assert!(cli_row.contains("| 5.0 |"), "{cli_row}");
    }

    #[test]
    fn export_is_markdown_safe() {
        let mut n = names();
        n.insert(P1.to_owned(), "Trade|Log\nx".to_owned());
        let text = rl::export_metrics(&sample_index(), None, &cx(&n));
        assert!(text.contains("Trade\\|Log x"));
    }

    #[test]
    fn index_scales_to_100k_lines() {
        let one = serde_json::to_string(&line(
            1,
            Surface::Cli,
            1,
            "1.1",
            Entry::DoNext,
            Some(5000),
            Some(9000),
            Status::Exited,
        ))
        .unwrap();
        assert!(one.len() < 400, "a typical line is a few hundred bytes");
        let mut text = String::with_capacity(one.len() * 100_000);
        for n in 0..100_000_u32 {
            text.push_str(&one.replace("000000000001", &format!("{n:012}")));
            text.push('\n');
        }
        let merged = rl::merge_latest(rl::parse_index(&text));
        assert_eq!(merged.len(), 100_000);
        assert_eq!(rl::analyze(&merged).runs_counted, 100_000);
    }
}
