//! The run log's numbers (ADR-018, F-026): reading `runs/index-YYYY-MM.jsonl`, the two kill-check
//! measures per surface and per stage, the by-hand baseline from `docs/metrics-baseline.md`, and
//! the Markdown that Export metrics writes. Pure: the app reads the files and passes text in.
//!
//! One function feeds both the Run log strip (`metrics_view`) and Export metrics (`metrics_export`),
//! so the two always agree (AC-F026-09).
//!
//! Definitions (`docs/metrics-baseline.md`, AC-KC-01, AC-KC-02):
//! - **Time to start** is `timeToStartMs` of a run that starts a step (re-arms and repairs
//!   continue a step, so they are not starts). It ends at the spawn (CLI) or when the filled prompt
//!   is on the clipboard and the chat is open (chat).
//! - **Active time per step** sums `timeToStartMs + durationMs - waitMs` over every run of one step
//!   (so re-arm and paste-back overhead counts) and is counted only for a step with at least one
//!   finished run. In P1 only `wait`-step time is excluded automatically (`waitMs` is 0 for runs).
//! - **Median** of an even count is the integer mean of the two middle values; **p95** is the
//!   nearest rank, `ceil(0.95 n)`.
//! - A chat run's index line is written at Copy and open; the line written when its step is done
//!   carries the same `runId` with `durationMs` filled. Readers keep the last line per run
//!   ([`merge_latest`]).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use crate::schema::run::{IndexEntryPoint, IndexStatus, IndexSurface, RunIndexLine};

/// The by-hand chat baseline needs at least this many steps to pass (AC-KC-04).
pub const MIN_CHAT_BASELINE_STEPS: u32 = 10;

/// A line is one `write` of at most this many bytes (files.md §4.7); longer text isn't a line.
const MAX_LINE_BYTES: usize = 4096;

/// Parses index text: one JSON object per line. A blank, torn (cut off mid-write) or otherwise
/// unreadable line is skipped, never an error (`runlog::index_torn_line_skipped`).
#[must_use]
pub fn parse_index(text: &str) -> Vec<RunIndexLine> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter(|l| l.len() <= MAX_LINE_BYTES)
        .filter_map(|l| serde_json::from_str::<RunIndexLine>(l).ok())
        .collect()
}

/// Keeps the last line per `runId` (a step-done line replaces the Copy-and-open line), in the order
/// of each run's first line.
#[must_use]
pub fn merge_latest(lines: Vec<RunIndexLine>) -> Vec<RunIndexLine> {
    let mut at: HashMap<String, usize> = HashMap::with_capacity(lines.len());
    let mut out: Vec<RunIndexLine> = Vec::with_capacity(lines.len());
    for l in lines {
        if let Some(&i) = at.get(&l.run_id) {
            out[i] = l;
        } else {
            at.insert(l.run_id.clone(), out.len());
            out.push(l);
        }
    }
    out
}

/// Count, median and p95 of a set of millisecond values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// How many values.
    pub n: u32,
    /// Median.
    pub median: u64,
    /// 95th percentile (nearest rank).
    pub p95: u64,
}

/// [`Stats`] of `values`, or `None` when there are none.
#[must_use]
pub fn stats(values: &[u64]) -> Option<Stats> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_unstable();
    let n = v.len();
    let median = if n % 2 == 1 {
        v[n / 2]
    } else {
        let (a, b) = (v[n / 2 - 1], v[n / 2]);
        u64::midpoint(a, b)
    };
    // Nearest rank: ceil(95 n / 100), at least 1.
    let rank = (n * 95).div_ceil(100).max(1);
    Some(Stats {
        n: u32::try_from(n).unwrap_or(u32::MAX),
        median,
        p95: v[rank - 1],
    })
}

fn counted(s: IndexSurface) -> bool {
    matches!(s, IndexSurface::Chat | IndexSurface::Cli)
}

fn starts_a_step(e: IndexEntryPoint) -> bool {
    !matches!(e, IndexEntryPoint::Rearm | IndexEntryPoint::Repair)
}

fn clamp32(v: u64) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

/// The measures for one surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceReport {
    /// Chat or CLI.
    pub surface: IndexSurface,
    /// Runs counted.
    pub runs: u32,
    /// Steps run (a step run again counts once).
    pub steps: u32,
    /// Time to start.
    pub starts: Option<Stats>,
    /// Active time per step (after excluded waits).
    pub active: Option<Stats>,
    /// Elapsed time per step (before excluded waits).
    pub elapsed: Option<Stats>,
    /// Excluded waits, total.
    pub waits_ms: u64,
    /// Run-input interventions, total.
    pub interventions: u32,
    /// Copy prompt on a CLI step, total.
    pub copy_prompts: u32,
    /// Runs with an override applied.
    pub overrides: u32,
}

/// The measures for one stage (both surfaces together).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageReport {
    /// Stage.
    pub stage: u8,
    /// Steps run.
    pub steps: u32,
    /// Time to start.
    pub starts: Option<Stats>,
    /// Active time per step.
    pub active: Option<Stats>,
    /// Manual-edit proxies: interventions plus Copy prompt on CLI.
    pub edits: u32,
}

/// Everything Export metrics and the Run log strip show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// CLI then chat.
    pub surfaces: Vec<SurfaceReport>,
    /// By stage, ascending.
    pub stages: Vec<StageReport>,
    /// Time to start over both surfaces (NFR-09's p95).
    pub all_starts: Option<Stats>,
    /// Chat and CLI runs counted.
    pub runs_counted: u32,
    /// (Project, stage) pairs with a counted run.
    pub stages_through_foreman: u32,
}

impl Report {
    /// One surface's report.
    #[must_use]
    pub fn surface(&self, s: IndexSurface) -> Option<&SurfaceReport> {
        self.surfaces.iter().find(|r| r.surface == s)
    }

    /// Manual-edit proxies over every stage.
    #[must_use]
    pub fn edits_total(&self) -> u32 {
        self.stages.iter().map(|s| s.edits).sum()
    }
}

#[derive(Default)]
struct Group {
    stage: u8,
    elapsed: u64,
    waits: u64,
    finished: bool,
}

/// Computes the report from index lines already merged by [`merge_latest`]. Chat and CLI runs
/// only; Desktop hand-offs and probes are not part of the kill check.
#[must_use]
pub fn analyze(index: &[RunIndexLine]) -> Report {
    let mut starts: BTreeMap<IndexSurface, Vec<u64>> = BTreeMap::new();
    let mut all_starts: Vec<u64> = Vec::new();
    let mut groups: BTreeMap<(IndexSurface, String, String), Group> = BTreeMap::new();
    let mut rows: BTreeMap<IndexSurface, SurfaceReport> = BTreeMap::new();
    let mut stage_starts: BTreeMap<u8, Vec<u64>> = BTreeMap::new();
    let mut stage_edits: BTreeMap<u8, u32> = BTreeMap::new();
    let mut stage_pairs: BTreeSet<(String, u8)> = BTreeSet::new();
    let mut runs_counted = 0_u32;

    for l in index.iter().filter(|l| counted(l.surface)) {
        runs_counted += 1;
        stage_pairs.insert((l.project_id.clone(), l.stage));
        let row = rows.entry(l.surface).or_insert_with(|| SurfaceReport {
            surface: l.surface,
            runs: 0,
            steps: 0,
            starts: None,
            active: None,
            elapsed: None,
            waits_ms: 0,
            interventions: 0,
            copy_prompts: 0,
            overrides: 0,
        });
        row.runs += 1;
        row.interventions += l.run_input_interventions;
        row.copy_prompts += u32::from(l.copy_prompt_on_cli);
        row.overrides += u32::from(l.r#override);
        *stage_edits.entry(l.stage).or_default() +=
            l.run_input_interventions + u32::from(l.copy_prompt_on_cli);

        if starts_a_step(l.entry_point)
            && let Some(t) = l.time_to_start_ms
        {
            starts.entry(l.surface).or_default().push(t);
            all_starts.push(t);
            stage_starts.entry(l.stage).or_default().push(t);
        }
        let g = groups
            .entry((l.surface, l.project_id.clone(), l.step_id.to_string()))
            .or_default();
        g.stage = l.stage;
        g.elapsed += l.time_to_start_ms.unwrap_or(0) + l.duration_ms.unwrap_or(0);
        g.waits += l.wait_ms;
        g.finished |= l.duration_ms.is_some() && l.status != IndexStatus::Interrupted;
    }

    let mut active_by_surface: BTreeMap<IndexSurface, Vec<u64>> = BTreeMap::new();
    let mut elapsed_by_surface: BTreeMap<IndexSurface, Vec<u64>> = BTreeMap::new();
    let mut active_by_stage: BTreeMap<u8, Vec<u64>> = BTreeMap::new();
    let mut steps_by_stage: BTreeMap<u8, u32> = BTreeMap::new();
    for ((surface, _, _), g) in &groups {
        if let Some(r) = rows.get_mut(surface) {
            r.steps += 1;
        }
        *steps_by_stage.entry(g.stage).or_default() += 1;
        if g.finished {
            let active = g.elapsed.saturating_sub(g.waits);
            active_by_surface.entry(*surface).or_default().push(active);
            elapsed_by_surface
                .entry(*surface)
                .or_default()
                .push(g.elapsed);
            active_by_stage.entry(g.stage).or_default().push(active);
            if let Some(r) = rows.get_mut(surface) {
                r.waits_ms += g.waits.min(g.elapsed);
            }
        }
    }

    let mut surfaces = Vec::new();
    for s in [IndexSurface::Cli, IndexSurface::Chat] {
        if let Some(mut r) = rows.remove(&s) {
            r.starts = starts.get(&s).and_then(|v| stats(v));
            r.active = active_by_surface.get(&s).and_then(|v| stats(v));
            r.elapsed = elapsed_by_surface.get(&s).and_then(|v| stats(v));
            surfaces.push(r);
        }
    }
    let stages = steps_by_stage
        .iter()
        .map(|(&stage, &steps)| StageReport {
            stage,
            steps,
            starts: stage_starts.get(&stage).and_then(|v| stats(v)),
            active: active_by_stage.get(&stage).and_then(|v| stats(v)),
            edits: stage_edits.get(&stage).copied().unwrap_or(0),
        })
        .collect();
    Report {
        surfaces,
        stages,
        all_starts: stats(&all_starts),
        runs_counted,
        stages_through_foreman: u32::try_from(stage_pairs.len()).unwrap_or(u32::MAX),
    }
}

// ---------------------------------------------------------------------------------------------
// The by-hand baseline (`docs/metrics-baseline.md`)
// ---------------------------------------------------------------------------------------------

/// One row of the baseline's Log table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineRow {
    /// The stage of `Stage.step`.
    pub stage: Option<u8>,
    /// Chat or CLI.
    pub surface: IndexSurface,
    /// "Seconds to start", in milliseconds.
    pub start_ms: Option<u64>,
    /// "Active minutes", in milliseconds.
    pub active_ms: Option<u64>,
    /// "Manual edits".
    pub manual_edits: u32,
    /// Whether "Mistakes" describes anything.
    pub mistake: bool,
}

/// The by-hand log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Baseline {
    /// Every filled row.
    pub rows: Vec<BaselineRow>,
}

/// Parses a decimal such as `94` or `1.5` into thousandths.
fn thousandths(s: &str) -> Option<u64> {
    let s = s.trim();
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    if whole.is_empty() && frac.is_empty() {
        return None;
    }
    if !whole.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let w: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let mut f = frac.chars().take(3).collect::<String>();
    while f.len() < 3 {
        f.push('0');
    }
    w.checked_mul(1000)?.checked_add(f.parse::<u64>().ok()?)
}

impl Baseline {
    /// Reads the Log table of `docs/metrics-baseline.md`: the table whose header starts
    /// `| Date | Project | Stage.step | Surface |`. Rows with no surface (the empty template row)
    /// or an unknown one are skipped.
    #[must_use]
    pub fn parse(markdown: &str) -> Self {
        let mut rows = Vec::new();
        let mut in_table = false;
        for line in markdown.lines() {
            let t = line.trim();
            if !t.starts_with('|') {
                in_table = false;
                continue;
            }
            let cells: Vec<&str> = t.trim_matches('|').split('|').map(str::trim).collect();
            if cells.len() >= 4
                && cells[0].eq_ignore_ascii_case("date")
                && cells[3].eq_ignore_ascii_case("surface")
            {
                in_table = true;
                continue;
            }
            if !in_table
                || cells
                    .iter()
                    .all(|c| c.chars().all(|ch| ch == '-' || ch == ':'))
            {
                continue;
            }
            let cell = |i: usize| cells.get(i).copied().unwrap_or("");
            let surface = match cell(3).to_ascii_lowercase().as_str() {
                "chat" => IndexSurface::Chat,
                "cli" => IndexSurface::Cli,
                _ => continue,
            };
            rows.push(BaselineRow {
                stage: cell(2)
                    .split('.')
                    .next()
                    .and_then(|s| s.trim().parse::<u8>().ok()),
                surface,
                start_ms: thousandths(cell(6)),
                active_ms: thousandths(cell(9)).and_then(|m| m.checked_mul(60)),
                manual_edits: cell(10).parse().unwrap_or(0),
                mistake: !cell(11).is_empty(),
            });
        }
        Self { rows }
    }

    /// Whether any step is logged.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    fn of(&self, s: IndexSurface) -> impl Iterator<Item = &BaselineRow> {
        self.rows.iter().filter(move |r| r.surface == s)
    }

    /// Steps logged for a surface.
    #[must_use]
    pub fn steps(&self, s: IndexSurface) -> u32 {
        u32::try_from(self.of(s).count()).unwrap_or(u32::MAX)
    }

    /// Time to start for a surface.
    #[must_use]
    pub fn starts(&self, s: IndexSurface) -> Option<Stats> {
        stats(&self.of(s).filter_map(|r| r.start_ms).collect::<Vec<_>>())
    }

    /// Active time per step for a surface.
    #[must_use]
    pub fn active(&self, s: IndexSurface) -> Option<Stats> {
        stats(&self.of(s).filter_map(|r| r.active_ms).collect::<Vec<_>>())
    }

    /// Time to start over both surfaces.
    #[must_use]
    pub fn starts_all(&self) -> Option<Stats> {
        stats(
            &self
                .rows
                .iter()
                .filter_map(|r| r.start_ms)
                .collect::<Vec<_>>(),
        )
    }

    fn edits(&self) -> u32 {
        self.rows.iter().map(|r| r.manual_edits).sum()
    }

    fn mistakes(&self) -> u32 {
        u32::try_from(self.rows.iter().filter(|r| r.mistake).count()).unwrap_or(u32::MAX)
    }

    fn stages(&self) -> u32 {
        let s: BTreeSet<u8> = self.rows.iter().filter_map(|r| r.stage).collect();
        u32::try_from(s.len()).unwrap_or(u32::MAX)
    }
}

/// The strip's comparison: median time to start over both surfaces, by hand against Foreman.
/// `None` when either side has no data.
#[must_use]
pub fn comparison(report: &Report, baseline: &Baseline) -> Option<(u32, u32)> {
    let by_hand = baseline.starts_all()?;
    let foreman = report.all_starts?;
    Some((clamp32(by_hand.median), clamp32(foreman.median)))
}

// ---------------------------------------------------------------------------------------------
// Export metrics
// ---------------------------------------------------------------------------------------------

/// What the export needs besides the index.
#[derive(Debug, Clone, Copy)]
pub struct ExportContext<'a> {
    /// The date shown in the heading (`2026-10-20`).
    pub generated_on: &'a str,
    /// Project names by ID.
    pub project_names: &'a BTreeMap<String, String>,
}

fn name(s: IndexSurface) -> &'static str {
    match s {
        IndexSurface::Chat => "chat",
        IndexSurface::Cli => "CLI",
        IndexSurface::Desktop => "Desktop",
    }
}

/// Milliseconds as seconds with one decimal.
fn secs(ms: u64) -> String {
    let t = (ms + 50) / 100;
    format!("{}.{}", t / 10, t % 10)
}

/// Milliseconds as minutes with one decimal.
fn mins(ms: u64) -> String {
    let t = (ms + 3000) / 6000;
    format!("{}.{}", t / 10, t % 10)
}

fn cell(v: Option<u64>, f: fn(u64) -> String) -> String {
    v.map_or_else(|| "—".to_owned(), f)
}

fn lower(foreman: Option<u64>, by_hand: Option<u64>) -> &'static str {
    match (foreman, by_hand) {
        (Some(f), Some(h)) if f < h => "yes",
        (Some(_), Some(_)) => "no",
        (None, _) => "no Foreman runs",
        (_, None) => "no baseline",
    }
}

fn escape_cell(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .replace('|', "\\|")
}

/// The Markdown of Export metrics (AC-F026-04, -05, -06): the summary in the layout of
/// `docs/metrics-baseline.md`, the kill-check comparison, then Foreman's numbers per surface,
/// per stage and per run, with elapsed time and excluded waits shown separately. `index` must
/// already be merged ([`merge_latest`]). Deterministic: the same input gives the same text.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one document, written top to bottom")]
pub fn export_metrics(
    index: &[RunIndexLine],
    baseline: Option<&Baseline>,
    cx: &ExportContext<'_>,
) -> String {
    let report = analyze(index);
    let empty = Baseline::default();
    let base = baseline.unwrap_or(&empty);
    let mut o = String::new();
    let _ = writeln!(o, "# Foreman metrics export\n");
    let _ = writeln!(
        o,
        "Generated {} · {} runs counted · {} stages run through Foreman\n",
        cx.generated_on, report.runs_counted, report.stages_through_foreman
    );
    let _ = writeln!(
        o,
        "Both measures are defined as in `docs/metrics-baseline.md`: time to start ends at the spawn (CLI) \
         or when the filled prompt is on the clipboard and the chat is open (chat); active time runs from \
         the same start to the step done. Press Send is not part of either. Medians of an even count are \
         the mean of the middle two; p95 is the nearest rank.\n"
    );
    if report.runs_counted == 0 {
        let _ = writeln!(
            o,
            "> No runs were recorded yet, so the Foreman columns are empty.\n"
        );
    }
    if baseline.is_none() {
        let _ = writeln!(
            o,
            "> No `docs/metrics-baseline.md` was found in a registered project, so the by-hand column is empty.\n"
        );
    }

    let _ = writeln!(o, "## Summary\n");
    let _ = writeln!(
        o,
        "| Measure | Surface | By hand (this file) | Foreman (Export metrics) |"
    );
    let _ = writeln!(o, "|---|---|---|---|");
    let surfaces = [IndexSurface::Chat, IndexSurface::Cli];
    for s in surfaces {
        let f = report.surface(s);
        let _ = writeln!(
            o,
            "| Steps logged | {} | {} | {} |",
            name(s),
            base.steps(s),
            f.map_or(0, |r| r.steps)
        );
    }
    for s in surfaces {
        let _ = writeln!(
            o,
            "| Median seconds to start | {} | {} | {} |",
            name(s),
            cell(base.starts(s).map(|x| x.median), secs),
            cell(
                report.surface(s).and_then(|r| r.starts).map(|x| x.median),
                secs
            )
        );
    }
    for s in surfaces {
        let _ = writeln!(
            o,
            "| p95 seconds to start | {} | {} | {} |",
            name(s),
            cell(base.starts(s).map(|x| x.p95), secs),
            cell(
                report.surface(s).and_then(|r| r.starts).map(|x| x.p95),
                secs
            )
        );
    }
    for s in surfaces {
        let _ = writeln!(
            o,
            "| Median active minutes per step | {} | {} | {} |",
            name(s),
            cell(base.active(s).map(|x| x.median), mins),
            cell(
                report.surface(s).and_then(|r| r.active).map(|x| x.median),
                mins
            )
        );
    }
    let foreman_steps: u32 = report.surfaces.iter().map(|r| r.steps).sum();
    let per_step = |n: u32, steps: u32| -> String {
        if steps == 0 {
            "—".to_owned()
        } else {
            let t = (u64::from(n) * 100 + u64::from(steps) / 2) / u64::from(steps);
            format!("{}.{:02}", t / 100, t % 100)
        }
    };
    let by_hand_steps = base.steps(IndexSurface::Chat) + base.steps(IndexSurface::Cli);
    let _ = writeln!(
        o,
        "| Manual edits per step | all | {} | {} |",
        per_step(base.edits(), by_hand_steps),
        per_step(report.edits_total(), foreman_steps)
    );
    let _ = writeln!(
        o,
        "| Mistakes per stage | all | {} | — |",
        per_step(base.mistakes(), base.stages())
    );
    let _ = writeln!(
        o,
        "\nForeman's manual edits are the proxies it can see: run-input interventions plus every Copy prompt on a CLI step. \
         Foreman builds the command from the template, so it does not log mistakes; overrides applied are listed below.\n"
    );

    let _ = writeln!(o, "## Kill check\n");
    let _ = writeln!(
        o,
        "| Surface | Baseline present | Median to start lower | Median active lower | Manual-edit proxies |"
    );
    let _ = writeln!(o, "|---|---|---|---|---|");
    for s in surfaces {
        let n = base.steps(s);
        let present = match s {
            IndexSurface::Chat if n >= MIN_CHAT_BASELINE_STEPS => format!("yes ({n} steps)"),
            IndexSurface::Chat => format!("no ({n} of {MIN_CHAT_BASELINE_STEPS} steps)"),
            _ if n > 0 => format!("yes ({n} steps)"),
            _ => "no (0 steps)".to_owned(),
        };
        let f = report.surface(s);
        let _ = writeln!(
            o,
            "| {} | {} | {} | {} | {} |",
            name(s),
            present,
            lower(
                f.and_then(|r| r.starts).map(|x| x.median),
                base.starts(s).map(|x| x.median)
            ),
            lower(
                f.and_then(|r| r.active).map(|x| x.median),
                base.active(s).map(|x| x.median)
            ),
            f.map_or(0, |r| r.interventions + r.copy_prompts),
        );
    }
    let _ = writeln!(
        o,
        "\nThe check passes only when every row says yes and no step needed a manual prompt edit. Each proxy event needs an explanation in your Stage 9 attestation.\n"
    );

    let _ = writeln!(o, "## Foreman by surface\n");
    let _ = writeln!(
        o,
        "| Surface | Runs | Steps | Median to start (s) | p95 to start (s) | Median elapsed per step (min) | Waits excluded (min) | Median active per step (min) | Interventions | Copy prompt on CLI | Overrides |"
    );
    let _ = writeln!(o, "|---|---|---|---|---|---|---|---|---|---|---|");
    for s in [IndexSurface::Cli, IndexSurface::Chat] {
        match report.surface(s) {
            Some(r) => {
                let _ = writeln!(
                    o,
                    "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                    name(s),
                    r.runs,
                    r.steps,
                    cell(r.starts.map(|x| x.median), secs),
                    cell(r.starts.map(|x| x.p95), secs),
                    cell(r.elapsed.map(|x| x.median), mins),
                    mins(r.waits_ms),
                    cell(r.active.map(|x| x.median), mins),
                    r.interventions,
                    r.copy_prompts,
                    r.overrides
                );
            }
            None => {
                let _ = writeln!(
                    o,
                    "| {} | 0 | 0 | — | — | — | 0.0 | — | 0 | 0 | 0 |",
                    name(s)
                );
            }
        }
    }

    let _ = writeln!(o, "\n## Foreman by stage\n");
    let _ = writeln!(
        o,
        "| Stage | Steps | Median to start (s) | p95 to start (s) | Median active per step (min) | Manual-edit proxies |"
    );
    let _ = writeln!(o, "|---|---|---|---|---|---|");
    if report.stages.is_empty() {
        let _ = writeln!(o, "| — | 0 | — | — | — | 0 |");
    }
    for s in &report.stages {
        let _ = writeln!(
            o,
            "| {} | {} | {} | {} | {} | {} |",
            s.stage,
            s.steps,
            cell(s.starts.map(|x| x.median), secs),
            cell(s.starts.map(|x| x.p95), secs),
            cell(s.active.map(|x| x.median), mins),
            s.edits
        );
    }

    let _ = writeln!(o, "\n## Run log\n");
    let _ = writeln!(
        o,
        "| Date | Project | Stage.step | Surface | Entry | Seconds to start | Duration (min) | Waits (min) | Interventions | Copy prompt on CLI | Override | Status |"
    );
    let _ = writeln!(o, "|---|---|---|---|---|---|---|---|---|---|---|---|");
    let mut runs: Vec<&RunIndexLine> = index.iter().filter(|l| counted(l.surface)).collect();
    runs.sort_by(|a, b| {
        a.wall_start
            .cmp(&b.wall_start)
            .then(a.run_id.cmp(&b.run_id))
    });
    if runs.is_empty() {
        let _ = writeln!(o, "| | | | | | | | | | | | |");
    }
    for l in runs {
        let project = cx
            .project_names
            .get(&l.project_id)
            .map_or(l.project_id.as_str(), String::as_str);
        let _ = writeln!(
            o,
            "| {} | {} | {}.{} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            escape_cell(l.wall_start.get(..10).unwrap_or(&l.wall_start)),
            escape_cell(project),
            l.stage,
            escape_cell(
                l.step_id
                    .as_str()
                    .split_once('.')
                    .map_or(l.step_id.as_str(), |(_, s)| s)
            ),
            name(l.surface),
            entry_name(l.entry_point),
            cell(l.time_to_start_ms, secs),
            cell(l.duration_ms, mins),
            mins(l.wait_ms),
            l.run_input_interventions,
            if l.copy_prompt_on_cli { "yes" } else { "no" },
            if l.r#override { "yes" } else { "no" },
            status_name(l.status)
        );
    }
    o
}

fn entry_name(e: IndexEntryPoint) -> &'static str {
    match e {
        IndexEntryPoint::DoNext => "Do next",
        IndexEntryPoint::Palette => "Palette",
        IndexEntryPoint::StageView => "Stage view",
        IndexEntryPoint::Rearm => "Re-arm",
        IndexEntryPoint::Repair => "Repair",
    }
}

fn status_name(s: IndexStatus) -> &'static str {
    match s {
        IndexStatus::Exited => "exited",
        IndexStatus::Errored => "errored",
        IndexStatus::Interrupted => "interrupted",
        IndexStatus::Copied => "copied",
    }
}
