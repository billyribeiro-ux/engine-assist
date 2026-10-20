//! First run and Template health (`docs/contracts/ipc.md` §4.2; F-019, F-001; tickets M2-16,
//! M4-06). The blueprint repo is found only among the fixed candidates or picked in Rust's folder
//! dialog (SR-31a, SR-24); git is plumbing only (`git::run`, SR-44).

use std::path::{Path, PathBuf};

use foreman_core::schema::settings::{BlueprintRepo, RepoSource, SettingsFile};
use foreman_core::template::{self, StepStatus, Template};
use foreman_sys::git::GitError;
use foreman_sys::lock::{StateError, blueprint_state};

use crate::ipc::IpcError;
use crate::ipc::dto::app::{BlueprintRepoInfo, ConditionSpec, TemplateHealthSummary, VariableSpec};
use crate::ipc::dto::doctor::{
    Counts, Diagnostic, Severity, StageHealth, StageHealthStatus, TemplateHealth,
};
use crate::ipc::dto::requests::{BlueprintPickRepoReq, PickMode, TemplateHealthReq};
use crate::ipc::types::{ConditionId, Stage, ToolKind};
use crate::os::Pick;
use crate::render::spans;
use crate::state::App;
use crate::text;

/// The blueprint file inside the repo (DR-001).
pub const BLUEPRINT_FILE: &str = template::FILE;

/// SR-31a: the only folders auto-detect looks at.
#[must_use]
pub fn candidates(home: &Path) -> [PathBuf; 2] {
    [
        home.join("claude-blueprint"),
        home.join("Developer/claude-blueprint"),
    ]
}

fn git_error(e: GitError) -> IpcError {
    match e {
        GitError::NotOwnRepo { .. } => IpcError::not_allowed(
            "not-repo-root",
            "That folder isn't the top of a git repository. Choose the repository's own folder.",
        ),
        e if foreman_sys::lock::git_missing(&e) => IpcError::tool_missing(ToolKind::Git, None),
        other => IpcError::tool_failed(
            "git",
            None,
            false,
            crate::render::escape_visible(&other.to_string()),
        ),
    }
}

fn state_error(e: StateError) -> IpcError {
    match e {
        StateError::Store(s) => s.into(),
        StateError::Git(g) => git_error(g),
        StateError::NoCommit => IpcError::not_allowed(
            "no-commit",
            "The blueprint repo has no commit yet. Commit the blueprint, then try again.",
        ),
    }
}

/// The one-line health of a parsed blueprint.
#[must_use]
pub fn health_summary(t: &Template) -> TemplateHealthSummary {
    let all = t.steps.iter().filter(|s| s.template_id.is_none()).count();
    let runnable = t
        .steps
        .iter()
        .filter(|s| s.template_id.is_none() && s.runnable())
        .count();
    let needs = t
        .steps
        .iter()
        .filter(|s| s.status == StepStatus::NeedsAnnotation)
        .count();
    let errors = t.errors().count();
    TemplateHealthSummary {
        runnable: format!("{runnable} of {all} steps runnable"),
        needs_annotation: format!(
            "{needs} step{} need annotation",
            if needs == 1 { "" } else { "s" }
        ),
        errors: u32::try_from(errors).unwrap_or(u32::MAX),
        warnings: u32::try_from(t.diagnostics.len() - errors).unwrap_or(u32::MAX),
    }
}

fn variable_specs() -> Vec<VariableSpec> {
    text::VARIABLES
        .iter()
        .map(
            |(name, label, placeholder, multiline, from_settings)| VariableSpec {
                name: (*name).to_owned(),
                label: (*label).to_owned(),
                placeholder: (*placeholder).to_owned(),
                multiline: *multiline,
                from_settings: *from_settings,
            },
        )
        .collect()
}

fn condition_specs() -> Vec<ConditionSpec> {
    foreman_core::ids::ConditionId::ALL
        .iter()
        .map(|c| {
            let (label, effect) = text::condition(*c);
            ConditionSpec {
                id: ConditionId::from(*c),
                label: label.to_owned(),
                effect: effect.to_owned(),
                late: c.is_late(),
            }
        })
        .collect()
}

/// A parsed blueprint repo: the template plus what Settings and setup show.
#[derive(Debug, Clone)]
pub struct RepoRead {
    /// The canonical repo folder.
    pub path: PathBuf,
    /// The parsed template.
    pub template: Template,
    /// HEAD.
    pub commit: String,
    /// The blueprint file differs from HEAD.
    pub dirty: bool,
    /// The bytes read.
    pub bytes: Vec<u8>,
    /// `hash-object` of the working file and the blob at HEAD.
    pub oids: (String, Option<String>),
}

/// Reads the blueprint repo at `path`: own repo root, the blueprint file, HEAD, dirty check by
/// hash (never `git status`), then the template (through the app-data cache).
///
/// # Errors
/// `NotAllowed("not-repo-root")`, `ToolMissing`, `NotFound` (no blueprint file), `Io`.
pub fn read_repo(app: &App, path: &Path) -> Result<RepoRead, IpcError> {
    let ctx = app.spawn_ctx_now()?;
    let path = foreman_sys::spawn::canonical(path);
    let state = blueprint_state(&path, BLUEPRINT_FILE, &ctx).map_err(state_error)?;
    let source = String::from_utf8_lossy(&state.bytes).into_owned();
    let root = app.app_root()?;
    let (t, _) = template::cache::load_or_parse(&root, &source)?;
    Ok(RepoRead {
        path,
        template: t,
        dirty: state.head_oid.as_deref() != Some(state.working_oid.as_str()),
        commit: state.commit,
        bytes: state.bytes,
        oids: (state.working_oid, state.head_oid),
    })
}

/// The `BlueprintRepoInfo` of a read repo.
#[must_use]
pub fn info(app: &App, r: &RepoRead) -> BlueprintRepoInfo {
    BlueprintRepoInfo {
        display_path: app.display(&r.path),
        commit: r.commit.chars().take(7).collect(),
        version: r.template.blueprint_version.clone(),
        grammar_version: r.template.grammar_version.unwrap_or(0),
        dirty: r.dirty,
        health: health_summary(&r.template),
        variables: variable_specs(),
        conditions: condition_specs(),
    }
}

/// What Settings shows for the blueprint repo: the last read, or a fresh read when Settings
/// names a repo Foreman hasn't read since it started. `None` when none is set or it can't be read.
#[must_use]
pub fn cached_repo_info(app: &App, s: &SettingsFile) -> Option<BlueprintRepoInfo> {
    let repo = s.blueprint_repo.as_ref()?;
    if let Some((p, i)) = &app.inner().blueprint_info
        && p == &repo.path
    {
        return Some(i.clone());
    }
    let r = read_repo(app, Path::new(&repo.path)).ok()?;
    let i = info(app, &r);
    app.inner().blueprint_info = Some((repo.path.clone(), i.clone()));
    Some(i)
}

/// `blueprint_pick_repo` (P1): `autoDetect` looks only at the fixed candidates; `choose` opens
/// Rust's folder dialog. The chosen repo is stored in Settings.
///
/// # Errors
/// `Cancelled`, `NotFound` (no candidate), `NotAllowed("not-repo-root")`, `ToolMissing` (git),
/// `Io`.
pub fn blueprint_pick_repo(
    app: &App,
    req: BlueprintPickRepoReq,
) -> Result<BlueprintRepoInfo, IpcError> {
    let (picked, source) = match req.mode {
        PickMode::AutoDetect => {
            let found = candidates(&app.paths().home)
                .into_iter()
                .find(|c| c.join(BLUEPRINT_FILE).is_file());
            let Some(p) = found else {
                return Err(IpcError::not_found(
                    "No blueprint repo at ~/claude-blueprint or ~/Developer/claude-blueprint. Choose its folder.",
                ));
            };
            (p, RepoSource::AutoDetect)
        }
        PickMode::Choose => match app.os().pick(&Pick::BlueprintRepo) {
            Some(p) => (p, RepoSource::Chosen),
            None => return Err(IpcError::cancelled()),
        },
    };
    let r = read_repo(app, &picked)?;
    let i = info(app, &r);
    let _w = app.writing();
    let (mut s, stamp, _) = app.settings()?;
    s.blueprint_repo = Some(BlueprintRepo {
        path: r.path.display().to_string(),
        source,
    });
    app.save_settings(&s, &stamp)?;
    app.inner().blueprint_info = Some((r.path.display().to_string(), i.clone()));
    Ok(i)
}

fn stage_health(t: &Template) -> Vec<StageHealth> {
    t.stages
        .iter()
        .map(|st| {
            let steps: Vec<_> = t
                .steps
                .iter()
                .filter(|s| s.stage == st.number && s.template_id.is_none())
                .collect();
            let runnable = steps.iter().filter(|s| s.runnable()).count();
            let needs = steps
                .iter()
                .filter(|s| s.status == StepStatus::NeedsAnnotation)
                .count();
            let errors = t
                .diagnostics
                .iter()
                .filter(|d| d.stage == Some(st.number) && d.is_error())
                .count();
            let warnings = t
                .diagnostics
                .iter()
                .filter(|d| d.stage == Some(st.number) && !d.is_error())
                .count();
            let (status, text) = if errors > 0 {
                (
                    StageHealthStatus::Partial,
                    format!("{errors} error{}", if errors == 1 { "" } else { "s" }),
                )
            } else if runnable == 0 {
                (
                    StageHealthStatus::NeedsAnnotation,
                    "Needs annotation".to_owned(),
                )
            } else if needs > 0 {
                (
                    StageHealthStatus::Partial,
                    format!(
                        "{needs} step{} need annotation",
                        if needs == 1 { "" } else { "s" }
                    ),
                )
            } else if warnings > 0 {
                (
                    StageHealthStatus::Warning,
                    format!("{warnings} warning{}", if warnings == 1 { "" } else { "s" }),
                )
            } else {
                (StageHealthStatus::Runnable, "Runnable".to_owned())
            };
            StageHealth {
                stages: st.number.to_string(),
                title: st.title.clone(),
                status,
                text,
                steps: format!("{runnable} of {}", steps.len()),
            }
        })
        .collect()
}

/// The full health of a template.
#[must_use]
pub fn health(t: &Template, commit: &str) -> TemplateHealth {
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    let errors = t.errors().count();
    TemplateHealth {
        version: t.blueprint_version.clone(),
        grammar_version: t.grammar_version.unwrap_or(0),
        counts: Counts {
            stages: count(t.stages.len()),
            runnable_steps: count(
                t.steps
                    .iter()
                    .filter(|s| s.template_id.is_none() && s.runnable())
                    .count(),
            ),
            items: count(t.items.len()),
            errors: count(errors),
            warnings: count(t.diagnostics.len() - errors),
        },
        stages: stage_health(t),
        diagnostics: t
            .diagnostics
            .iter()
            .map(|d| Diagnostic {
                severity: if d.is_error() {
                    Severity::Error
                } else {
                    Severity::Warning
                },
                rule: d.rule.clone(),
                file: d.file.clone(),
                line: d.line,
                column: d.column,
                message: spans(&d.message),
                stage: Stage(d.stage.unwrap_or(0)),
            })
            .collect(),
        commit: commit.chars().take(7).collect(),
        file: BLUEPRINT_FILE.to_owned(),
    }
}

/// `template_health` (P1 summary, P2 screen): the blueprint in Settings, or a project's pinned
/// blueprint.
///
/// # Errors
/// `NotFound` (no blueprint set, unknown project), `ReadOnly` (pinned text unavailable), `Io`.
pub fn template_health(app: &App, req: TemplateHealthReq) -> Result<TemplateHealth, IpcError> {
    if let Some(id) = req.project_id {
        let p = crate::project::open(app, id.as_str())?;
        let Some(t) = p.template.as_ref() else {
            return Err(IpcError::read_only(p.read_only_or_missing()));
        };
        return Ok(health(t, &p.lock_commit()));
    }
    let (s, _, _) = app.settings()?;
    let Some(repo) = s.blueprint_repo else {
        return Err(IpcError::not_found(
            "No blueprint repo is set. Choose it in Settings.",
        ));
    };
    let r = read_repo(app, Path::new(&repo.path))?;
    Ok(health(&r.template, &r.commit))
}
