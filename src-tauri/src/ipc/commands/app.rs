//! App (`docs/contracts/ipc.md` §4.1; F-017, F-019, F-020; tickets M2-19, M2-20).

use foreman_core::ids::Surface as CoreSurface;
use foreman_core::steps::StepState as CoreState;
use foreman_core::template::StepKind as CoreKind;

use crate::ipc::IpcError;
use crate::ipc::commands::projects::summaries;
use crate::ipc::commands::steps;
use crate::ipc::dto::app::{AppBootstrap, Capabilities, DoNextOutcome, NextStepRow, TodayView};
use crate::ipc::dto::requests::{AppBootstrapReq, DoNextReq, TodayGetReq};
use crate::ipc::types::{DoctorLevel, ProjectId, Rev, StepKey};
use crate::project::{self, Opened};
use crate::state::App;

/// The `--desktop` floor (compat table, ADR-017).
const DESKTOP_MIN: &str = "2.1.285";

fn capabilities(app: &App, s: &foreman_core::schema::settings::SettingsFile) -> Capabilities {
    let version = app
        .inner()
        .claude_version
        .clone()
        .or_else(|| s.last_seen_claude_version.clone());
    let parsed = version
        .as_deref()
        .and_then(foreman_core::compat::parse_version_output);
    Capabilities {
        desktop: parsed.as_ref().is_some_and(|v| semver_ge(v, DESKTOP_MIN)),
        sensor: false,
        probes_verified: s
            .contract
            .as_ref()
            .is_some_and(|c| Some(c.version.as_str()) == version.as_deref()),
        claude_version: version,
    }
}

fn semver_ge(v: &impl std::fmt::Display, min: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.split(['.', '-', '+'])
            .take(3)
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    parse(&v.to_string()) >= parse(min)
}

/// `app_bootstrap` (P1, mutates nothing): reads only `settings.json`, `projects.json` and each
/// `progress.json` (NFR-01). Interrupted runs are listed, never relaunched (SR-57).
///
/// # Errors
/// `Io`.
pub fn app_bootstrap(app: &App, req: AppBootstrapReq) -> Result<AppBootstrap, IpcError> {
    let AppBootstrapReq {} = req;
    let (s, _, exists) = app.settings()?;
    let projects = summaries(app)?;
    let (settings_rev, doctor) = {
        let i = app.inner();
        (i.settings_rev, i.doctor.unwrap_or(DoctorLevel::Unknown))
    };
    Ok(AppBootstrap {
        first_run: !exists || (s.blueprint_repo.is_none() && projects.is_empty()),
        settings_rev: Rev(settings_rev),
        projects,
        // Run records exist once the launch pipeline lands (M2-10, M3-10); none before.
        interrupted: crate::runs::interrupted(app)?,
        doctor,
        capabilities: capabilities(app, &s),
    })
}

fn runnable_now(state: &CoreState) -> bool {
    matches!(state, CoreState::NotStarted | CoreState::Copied)
}

/// The project's next step (F-017): the first step of the current stage, in template order, that
/// is on the path and not started or copied. `Err` carries why there is none.
pub fn next_step<'a>(app: &App, p: &'a Opened) -> Result<&'a foreman_core::template::Step, String> {
    if let Some(r) = &p.read_only {
        return Err(r.detail.clone());
    }
    let Some(t) = &p.template else {
        return Err("The pinned blueprint isn't available.".to_owned());
    };
    let Some(stage) = p.current_stage() else {
        return Err("Every applicable stage is signed.".to_owned());
    };
    let mut running = false;
    for s in t.stage_steps(stage) {
        let state = p.display_state(app, s);
        if matches!(state, CoreState::Running) {
            running = true;
        }
        if runnable_now(&state) && s.runnable() && s.kind != CoreKind::Repeatable {
            return Ok(s);
        }
    }
    Err(if running {
        format!("A Stage {stage} session is running.")
    } else {
        format!("Stage {stage} is waiting for its packet to be signed.")
    })
}

/// `today_get` (P1 Next step rows; the P2 inbox is empty in P1).
///
/// # Errors
/// `Io`.
pub fn today_get(app: &App, req: TodayGetReq) -> Result<TodayView, IpcError> {
    let TodayGetReq {} = req;
    let mut rows = Vec::new();
    for summary in summaries(app)? {
        let row = match project::open(app, summary.id.as_str()) {
            Ok(p) => {
                let strip = p.stage_views(app).iter().map(|v| v.status).collect();
                let stage_title = p.current_stage().and_then(|n| {
                    p.template
                        .as_ref()
                        .and_then(|t| t.stages.iter().find(|s| s.number == n))
                        .map(|s| s.title.clone())
                });
                match next_step(app, &p) {
                    Ok(s) => NextStepRow {
                        project: p.summary(app),
                        next: Some(p.step_summary(app, s)),
                        why_none: None,
                        strip,
                        stage_title,
                    },
                    Err(why) => NextStepRow {
                        project: p.summary(app),
                        next: None,
                        why_none: Some(why),
                        strip,
                        stage_title,
                    },
                }
            }
            Err(e) => NextStepRow {
                project: summary,
                next: None,
                why_none: Some(e.message),
                strip: Vec::new(),
                stage_title: None,
            },
        };
        rows.push(row);
    }
    Ok(TodayView {
        rows,
        inbox: Vec::new(),
    })
}

/// `do_next` (P1): the first Today row with a next step. A chat step returns its readiness; a
/// CLI step runs the same preparation as `launch_prepare` (`entry = doNext`). Nothing launches.
///
/// # Errors
/// `Io`; for a CLI step, whatever `launch_prepare` returns.
pub fn do_next(app: &App, req: DoNextReq) -> Result<DoNextOutcome, IpcError> {
    let DoNextReq {} = req;
    let mut why = "No project has a next step.".to_owned();
    for summary in summaries(app)? {
        let Ok(p) = project::open(app, summary.id.as_str()) else {
            continue;
        };
        let s = match next_step(app, &p) {
            Ok(s) => s.clone(),
            Err(w) => {
                why = w;
                continue;
            }
        };
        let project = ProjectId::from_trusted(p.id());
        let step = StepKey::from_trusted(s.key.as_str());
        if s.surface == Some(CoreSurface::Chat) || s.kind != CoreKind::Prompt {
            let r = if s.prompt.is_some() {
                Some(steps::resolve(&p, &s)?)
            } else {
                None
            };
            let blockers = steps::blockers(&p, app, &s, r.as_ref());
            return Ok(DoNextOutcome::Chat {
                project,
                step,
                ready: blockers.is_empty(),
                blockers,
            });
        }
        let key = s.key.to_string();
        let mut p = p;
        let prepare = crate::ipc::commands::launch::prepare(
            app,
            &mut p,
            &key,
            crate::ipc::types::EntryPoint::DoNext,
            crate::ipc::dto::requests::PrepareMode::Normal,
            &|_| {},
        )?;
        return Ok(DoNextOutcome::Cli {
            project,
            step,
            prepare: Box::new(prepare),
        });
    }
    Ok(DoNextOutcome::Nothing { why })
}
