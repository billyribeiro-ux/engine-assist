//! NFR-21 / AC-F026-08: the Run log panel reads 100,000 index entries (about 300 bytes each across
//! monthly files) and computes its metrics strip. The criterion target is the project-switch
//! budget (design doc §3.4), well under 500 ms; see `docs/progress.md` for the measured figure.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::disallowed_methods,
    reason = "bench code"
)]

use criterion::{Criterion, criterion_group, criterion_main};
use foreman_core::ids::StepKey;
use foreman_core::runlog;
use foreman_core::schema::run::{IndexEntryPoint, IndexStatus, IndexSurface, RunIndexLine};

fn month_text(n: u32, runs: u32) -> String {
    let mut text = String::new();
    for i in 0..runs {
        let line = RunIndexLine {
            schema_version: 1,
            run_id: format!("{n:08}-0000-4000-8000-{i:012}"),
            project_id: "11111111-1111-4111-8111-111111111111".to_owned(),
            stage: u8::try_from(i % 18).unwrap(),
            step_id: StepKey::parse(&format!("{}.{}", i % 18, i % 9 + 1)).unwrap(),
            surface: if i % 3 == 0 {
                IndexSurface::Chat
            } else {
                IndexSurface::Cli
            },
            entry_point: IndexEntryPoint::DoNext,
            wall_start: format!("2026-{:02}-{:02}T09:00:00Z", n % 12 + 1, i % 28 + 1),
            time_to_start_ms: Some(u64::from(4000 + i % 5000)),
            duration_ms: Some(u64::from(60_000 + i % 90_000)),
            wait_ms: 0,
            status: IndexStatus::Exited,
            run_input_interventions: i % 7 / 6,
            copy_prompt_on_cli: false,
            r#override: false,
        };
        text.push_str(&serde_json::to_string(&line).unwrap());
        text.push('\n');
    }
    text
}

fn run_index(c: &mut Criterion) {
    // 100,000 entries over four monthly files.
    let months: Vec<String> = (0..4).map(|n| month_text(n, 25_000)).collect();
    c.bench_function("bench_metrics_100k", |b| {
        b.iter(|| {
            let mut all = Vec::new();
            for m in &months {
                all.extend(runlog::parse_index(m));
            }
            let merged = runlog::merge_latest(all);
            runlog::analyze(&merged)
        });
    });
}

criterion_group!(benches, run_index);
criterion_main!(benches);
