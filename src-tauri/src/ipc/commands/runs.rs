//! Run log commands (`docs/contracts/ipc.md` §4.10; F-026; tickets M3-26, M4-02, M4-03): Export
//! metrics, the Run log's metrics strip, search, and Clear run history. The numbers come from
//! `foreman_core::runlog`, one function for the strip and the export, so they always agree
//! (AC-F026-09).

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "handlers take requests by value and return the IPC Result"
)]

use std::collections::BTreeMap;
use std::path::Path;

use foreman_core::runlog::{self, ExportContext, Stats};
use foreman_core::schema::run::IndexSurface;

use crate::ipc::IpcError;
use crate::ipc::dto::records::{
    BaselineComparison, ExportResult, MetricsView, RunsPage, StageEdits, SurfaceMetrics,
};
use crate::ipc::dto::requests::{MetricsExportReq, MetricsViewReq, RunsClearReq, RunsSearchReq};
use crate::ipc::types::{Stage, Surface};
use crate::runs;
use crate::state::App;

fn ms32(v: u64) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

fn median(s: Option<Stats>) -> u32 {
    s.map_or(0, |s| ms32(s.median))
}

/// `metrics_export` (P1): Export metrics. The Rust save dialog picks the path (`.md` only,
/// `WritePurpose::UserExport`); the Markdown is `core::runlog::export_metrics` over every run in
/// the index, with the by-hand baseline from a registered project's `docs/metrics-baseline.md`.
///
/// # Errors
/// `PolicyViolation` (the chosen path isn't an allowed export), `Conflict`, `Io`.
pub fn metrics_export(app: &App, req: MetricsExportReq) -> Result<ExportResult, IpcError> {
    let MetricsExportReq {} = req;
    let index = runs::read_index(app)?;
    let (reg, _) = app.registry()?;
    let names: BTreeMap<String, String> = reg
        .projects
        .iter()
        .map(|p| (p.project_id.clone(), p.name.clone()))
        .collect();
    let today: String = app.now().chars().take(10).collect();
    let baseline = runs::read_baseline(app);
    let text = runlog::export_metrics(
        &index,
        baseline.as_ref(),
        &ExportContext {
            generated_on: &today,
            project_names: &names,
        },
    );
    let Some(path) = app
        .os()
        .save("Export metrics", &format!("foreman-metrics-{today}.md"))
    else {
        return Ok(ExportResult::Cancelled);
    };
    foreman_core::store::export::write_user_export(&path, text.as_bytes(), &app.paths().app_data)?;
    Ok(ExportResult::Saved {
        display_path: app.display(Path::new(&path)),
    })
}

/// `metrics_view` (P2): the Run log's metrics strip.
///
/// # Errors
/// `NotFound` (project), `Io`.
pub fn metrics_view(app: &App, req: MetricsViewReq) -> Result<MetricsView, IpcError> {
    let project = req.project_id.as_ref().map(|p| p.as_str().to_owned());
    if let Some(p) = &project {
        let (reg, _) = app.registry()?;
        if !reg.projects.iter().any(|e| &e.project_id == p) {
            return Err(IpcError::not_found("That project isn't registered."));
        }
    }
    let index = runs::of_project(runs::read_index(app)?, project.as_deref());
    let report = runlog::analyze(&index);
    let baseline = runs::read_baseline(app).unwrap_or_default();
    Ok(MetricsView {
        per_surface: report
            .surfaces
            .iter()
            .map(|r| SurfaceMetrics {
                surface: match r.surface {
                    IndexSurface::Chat => Surface::Chat,
                    IndexSurface::Cli => Surface::Cli,
                    IndexSurface::Desktop => Surface::Desktop,
                },
                median_time_to_start_ms: median(r.starts),
                p95_time_to_start_ms: r.starts.map_or(0, |s| ms32(s.p95)),
                median_active_ms_per_step: median(r.active),
            })
            .collect(),
        manual_edits_by_stage: report
            .stages
            .iter()
            .map(|s| StageEdits {
                stage: Stage(s.stage),
                edits: s.edits,
            })
            .collect(),
        stages_through_foreman: report.stages_through_foreman,
        baseline: runlog::comparison(&report, &baseline).map(|(by_hand, foreman)| {
            BaselineComparison {
                baseline_median_ms: by_hand,
                foreman_median_ms: foreman,
            }
        }),
        runs_counted: report.runs_counted,
        p95_all_ms: report.all_starts.map_or(0, |s| ms32(s.p95)),
    })
}

/// `runs_clear` (P2): after a native confirmation, deletes `runs/` and `events/` (PR-05).
///
/// # Errors
/// `NotAllowed("runs-live")`, `Cancelled`, `Io`.
pub fn runs_clear(app: &App, req: RunsClearReq) -> Result<(), IpcError> {
    let RunsClearReq {} = req;
    let _w = app.writing();
    runs::clear_history(app)
}

/// `runs_search` (P2): the Run log's list.
///
/// # Errors
/// `Invalid` (limit outside 1–200), `NotFound` (project), `Io`.
pub fn runs_search(app: &App, req: RunsSearchReq) -> Result<RunsPage, IpcError> {
    if !(1..=200).contains(&req.limit) {
        return Err(IpcError::invalid("limit", "range"));
    }
    let (runs, next, total) = runs::search(
        app,
        req.query.as_str(),
        req.project_id.as_ref().map(|p| p.as_str()),
        req.cursor.as_ref().map(|c| c.as_str()),
        usize::from(req.limit),
    )?;
    Ok(RunsPage {
        runs,
        next,
        total: u32::try_from(total).unwrap_or(u32::MAX),
    })
}
