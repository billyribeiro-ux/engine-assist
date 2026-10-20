//! Projects (`docs/contracts/ipc.md` §4.3; F-018, F-002, F-004, F-020; tickets M2-17, M2-19,
//! M2-24, M4-02, M4-25). Folders come only from Rust's dialog (SR-24); decision files are written
//! through `core::decisions` (ADR-022); git is plumbing only (SR-44).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use foreman_core::decisions::{Decisions, OwnChange};
use foreman_core::git::{GitOp, RevParse};
use foreman_core::ids::{ConditionId as CoreCondition, TriState as CoreTri};
use foreman_core::lock::{self, BlueprintState, LockError};
use foreman_core::pathpolicy::{self, ReadClass, WritePurpose};
use foreman_core::resolver::tokens::PROJECT_VARIABLES;
use foreman_core::rules::{self, Applicability as CoreApp, RuleError, StageState};
use foreman_core::schema::overlay::OverlayFile;
use foreman_core::schema::progress::{DecisionFileKind, ProgressFile};
use foreman_core::schema::project::{ModelPolicy, Plan, ProjectFile};
use foreman_core::schema::projects::ProjectEntry;
use foreman_core::schema::{self, Header, Versioned};
use foreman_core::store::{ReadStamp, Root, RootKind, dirs};
use foreman_core::template::Template;
use foreman_sys::git::{GitError, GitOpts};

use crate::ipc::IpcError;
use crate::ipc::commands::first_run::{self, BLUEPRINT_FILE};
use crate::ipc::dto::app::{
    ApplicablePathRow, ApplicablePathSummary, FolderPick, PathRowStatus, PinPreview, PlannedAction,
    PlannedWrite, ProjectSummary, ProjectView, SchemaState, SetupPreview, SheetStatus, StripCell,
};
use crate::ipc::dto::requests::{
    BlueprintRepinReq, ConditionSetReq, ProjectOpenReq, ProjectPinSetReq, ProjectRemoveReq,
    ProjectUrlSetReq, ProjectsListReq, SetupApplyReq, SetupPickFolderReq, SetupPreviewReq,
    VariableSetReq,
};
use crate::ipc::types::{PickId, PreviewId, RepoPath, Rev, Stage, ToolKind};
use crate::os::{Confirm, Pick};
use crate::preview::Preview;
use crate::project::{self, OpenMode};
use crate::render::escape_visible;
use crate::state::{App, PREVIEW_TTL};

/// Every registered project's light summary, sorted by name.
///
/// # Errors
/// `Io`.
pub fn summaries(app: &App) -> Result<Vec<ProjectSummary>, IpcError> {
    let (reg, _) = app.registry()?;
    let root = app.app_root()?;
    let mut out: Vec<ProjectSummary> = reg
        .projects
        .iter()
        .map(|e| project::light_summary(app, e, &root))
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    Ok(out)
}

/// `projects_list` (P1).
///
/// # Errors
/// `Io`.
pub fn projects_list(app: &App, req: ProjectsListReq) -> Result<Vec<ProjectSummary>, IpcError> {
    let ProjectsListReq {} = req;
    summaries(app)
}

fn git_error(e: GitError) -> IpcError {
    if foreman_sys::lock::git_missing(&e) {
        IpcError::tool_missing(ToolKind::Git, None)
    } else {
        IpcError::tool_failed("git", None, false, escape_visible(&e.to_string()))
    }
}

/// Whether `folder` is its own git repository (an enclosing repo doesn't count, DR-141).
///
/// # Errors
/// `ToolMissing` (git), `ToolFailed`.
pub fn is_own_repo(app: &App, folder: &Path) -> Result<bool, IpcError> {
    let ctx = app.spawn_ctx_now()?;
    match foreman_sys::git::run(
        folder,
        &GitOp::RevParse(RevParse::VerifyHead),
        &GitOpts::default(),
        &ctx,
    ) {
        Ok(_) => Ok(true),
        Err(GitError::NotOwnRepo { .. }) => Ok(false),
        Err(e) => Err(git_error(e)),
    }
}

/// SR-24, AC-F018-06: why a picked folder can't hold a new project.
fn refusal(app: &App, folder: &Path) -> Result<Option<String>, IpcError> {
    let shown = escape_visible(&app.display(folder));
    let (reg, _) = app.registry()?;
    for p in &reg.projects {
        let root = foreman_sys::spawn::canonical(Path::new(&p.path));
        if folder.starts_with(&root) && folder != root {
            return Ok(Some(format!(
                "{shown} is inside another Foreman project ({}). Choose a folder outside it.",
                escape_visible(&app.display(&root))
            )));
        }
        if root.starts_with(folder) && folder != root {
            return Ok(Some(format!(
                "{shown} contains another Foreman project ({}). Choose a folder outside it.",
                escape_visible(&app.display(&root))
            )));
        }
        if root == folder {
            return Ok(Some(format!(
                "{shown} is already a Foreman project ({}).",
                p.name
            )));
        }
    }
    let (s, _, _) = app.settings()?;
    if let Some(b) = &s.blueprint_repo {
        let b = foreman_sys::spawn::canonical(Path::new(&b.path));
        if folder.starts_with(&b) || b.starts_with(folder) {
            return Ok(Some(format!(
                "{shown} is the blueprint repo or holds it. Choose another folder."
            )));
        }
    }
    if rustix::fs::access(folder, rustix::fs::Access::WRITE_OK).is_err() {
        return Ok(Some(format!("Foreman can't write to {shown}.")));
    }
    Ok(None)
}

fn foreman_schema(folder: &Path) -> Option<(SchemaState, Header)> {
    let root = Root::open(folder, RootKind::Project).ok()?;
    let rel =
        pathpolicy::check_read(DecisionFileKind::Project.repo_path(), ReadClass::Decision).ok()?;
    let (bytes, _) = root.read_opt(&rel, ReadClass::Decision).ok()?;
    let h = Header::read(&bytes?).ok()?;
    let state = match h.schema_version.cmp(&ProjectFile::CURRENT) {
        std::cmp::Ordering::Less => SchemaState::Older,
        std::cmp::Ordering::Equal => SchemaState::Current,
        std::cmp::Ordering::Greater => SchemaState::Newer,
    };
    Some((state, h))
}

/// `setup_pick_folder` (P1): Rust's folder dialog; the folder is held under a one-time `PickId`.
///
/// # Errors
/// `Cancelled`, `ToolMissing` (git), `Io`.
pub fn setup_pick_folder(app: &App, req: SetupPickFolderReq) -> Result<FolderPick, IpcError> {
    let SetupPickFolderReq {} = req;
    let Some(picked) = app.os().pick(&Pick::ProjectFolder) else {
        return Err(IpcError::cancelled());
    };
    let folder = foreman_sys::spawn::canonical(&picked);
    if !folder.is_dir() {
        return Err(IpcError::not_found("That folder doesn't exist."));
    }
    let refusal = refusal(app, &folder)?;
    let is_git_repo = is_own_repo(app, &folder)?;
    let has_foreman = foreman_schema(&folder).map(|(s, _)| s);
    let id = app.inner().picks.hold(folder.clone(), PREVIEW_TTL);
    Ok(FolderPick {
        pick_id: PickId::from_trusted(id),
        display_path: escape_visible(&app.display(&folder)),
        is_git_repo,
        has_foreman,
        refusal,
    })
}

/// What `setup_apply` does, fixed at preview time (IPC-R7).
#[derive(Debug, Clone)]
pub struct SetupPlan {
    /// The canonical folder.
    pub folder: PathBuf,
    /// An existing `.foreman/` is registered as is (AC-F018-07).
    pub adopt: Option<String>,
    /// `git init` first.
    pub git_init: bool,
    /// `project.json` as it will be written.
    pub project: ProjectFile,
    /// The blueprint repo and the pinned commit and blob.
    pub repo: PathBuf,
    /// The pinned commit and the blueprint blob.
    pub pin: (String, String),
    /// The planned writes (the apply writes exactly these, AC-F018-02).
    pub writes: Vec<PlannedWrite>,
}

/// The folders setup creates (AC-F018-03).
pub const SETUP_DIRS: [&str; 5] = [
    "docs/handoff",
    "docs/templates",
    "docs/change-requests",
    "docs/postmortems",
    "spec",
];
/// The decision files setup creates.
const SETUP_FILES: [DecisionFileKind; 3] = [
    DecisionFileKind::Project,
    DecisionFileKind::Overlay,
    DecisionFileKind::BlueprintLock,
];

fn planned(path: &str, action: PlannedAction, reason: Option<&str>) -> PlannedWrite {
    PlannedWrite {
        path: RepoPath::from_trusted(path),
        action,
        reason: reason.map(str::to_owned),
    }
}

fn exists(root: Option<&Root>, rel: &str) -> bool {
    root.is_some_and(|r| {
        pathpolicy::check_read(rel.trim_end_matches('/'), ReadClass::Decision)
            .is_ok_and(|p| r.exists(&p))
    })
}

fn plan_writes(folder: &Path, git_init: bool, adopt: bool) -> Vec<PlannedWrite> {
    if adopt {
        return vec![planned(
            ".foreman/",
            PlannedAction::LeaveAlone,
            Some("Already a Foreman project: registered as it is, nothing written."),
        )];
    }
    let root = Root::open(folder, RootKind::Project).ok();
    let mut out = Vec::new();
    for k in SETUP_FILES {
        if exists(root.as_ref(), k.repo_path()) {
            out.push(planned(
                k.repo_path(),
                PlannedAction::Refuse,
                Some("Already exists; Foreman never overwrites it. Move it aside to set up here."),
            ));
        } else {
            out.push(planned(k.repo_path(), PlannedAction::Create, None));
        }
    }
    for d in SETUP_DIRS {
        let shown = format!("{d}/");
        if exists(root.as_ref(), d) {
            out.push(planned(
                &shown,
                PlannedAction::LeaveAlone,
                Some("Already exists."),
            ));
        } else {
            out.push(planned(&shown, PlannedAction::Create, None));
        }
    }
    if exists(root.as_ref(), "docs/lessons.md") {
        out.push(planned(
            "docs/lessons.md",
            PlannedAction::LeaveAlone,
            Some("Already exists; Foreman never overwrites it."),
        ));
    } else {
        out.push(planned(
            "docs/lessons.md",
            PlannedAction::Create,
            Some("The lessons header from the blueprint (Appendix F.1)."),
        ));
    }
    out.push(if git_init {
        planned(
            ".git/",
            PlannedAction::Create,
            Some("git init: the folder isn't its own repository."),
        )
    } else {
        planned(
            ".git/",
            PlannedAction::LeaveAlone,
            Some("Already a repository."),
        )
    });
    out
}

/// The tailored path for a set of conditions (setup review, AC-F018-08).
#[must_use]
pub fn path_summary(
    t: &Template,
    conditions: &BTreeMap<CoreCondition, CoreTri>,
) -> ApplicablePathSummary {
    let path = rules::applicable_path(t, conditions, &OverlayFile::empty());
    let mut rows = Vec::new();
    let mut strip = Vec::new();
    let mut applies = 0u32;
    let mut first = true;
    for st in &t.stages {
        let state = path.stage(st.number).cloned();
        let undecided: Vec<String> = t
            .stage_steps(st.number)
            .filter_map(|s| match path.step(s.key.as_str()) {
                Some(CoreApp::Undecided(cs)) => Some(cs.iter().map(|c| format!("{c} = undecided"))),
                _ => None,
            })
            .flatten()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let (status, why, cell, cell_why) = match &state {
            Some(StageState::NotApplicable(r)) => (
                PathRowStatus::NotApplicable,
                format!("Not applicable ({r})"),
                SheetStatus::NotApplicable,
                Some(r.clone()),
            ),
            Some(StageState::NeedsAnnotation) => (
                PathRowStatus::NeedsAnnotation,
                "Shown but not runnable until the blueprint annotates it".to_owned(),
                SheetStatus::NeedsAnnotation,
                None,
            ),
            _ => {
                let has_live = t
                    .stage_steps(st.number)
                    .any(|s| path.step(s.key.as_str()).is_some_and(CoreApp::on_path));
                if !has_live && !undecided.is_empty() {
                    (
                        PathRowStatus::Undecided,
                        format!("Decide {} first", undecided.join(", ")),
                        SheetStatus::Undecided,
                        Some(undecided.join(", ")),
                    )
                } else {
                    let skipped: Vec<String> = t
                        .stage_steps(st.number)
                        .filter_map(|s| match path.step(s.key.as_str()) {
                            Some(CoreApp::NotApplicable(r)) => Some(format!("{} ({r})", s.id)),
                            _ => None,
                        })
                        .collect();
                    let why = if skipped.is_empty() {
                        "Applies".to_owned()
                    } else {
                        format!("Applies; not applicable: {}", skipped.join(", "))
                    };
                    applies += 1;
                    let cell = if first {
                        first = false;
                        SheetStatus::Ready
                    } else {
                        SheetStatus::Locked
                    };
                    (PathRowStatus::Applies, why, cell, None)
                }
            }
        };
        rows.push(ApplicablePathRow {
            stages: st.number.to_string(),
            title: st.title.clone(),
            status,
            why,
        });
        strip.push(StripCell {
            stage: Stage(st.number),
            status: cell,
            why: cell_why,
        });
    }
    let total = u32::try_from(t.stages.len()).unwrap_or(u32::MAX);
    ApplicablePathSummary {
        applies,
        total,
        summary: format!("{applies} of {total} stages apply"),
        rows,
        strip,
    }
}

fn is_slug(s: &str) -> bool {
    (1..=40).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `setup_preview` (P1): everything setup will write, leave alone or refuse, the pin and the
/// tailored path. Nothing is written.
///
/// # Errors
/// `Expired` (pick), `Invalid` (slug, variable names), `NotAllowed("blueprint-dirty" |
/// "folder-refused" | "no-blueprint")`, `ToolMissing` (git), `PolicyViolation`.
pub fn setup_preview(app: &App, req: SetupPreviewReq) -> Result<SetupPreview, IpcError> {
    let folder = app.inner().picks.peek(req.pick_id.as_str())?.clone();
    let name = req.name.as_str().trim();
    if name.is_empty() {
        return Err(IpcError::invalid("name", "required"));
    }
    if !is_slug(req.slug.as_str()) {
        return Err(IpcError::invalid("slug", "slug"));
    }
    for k in req.variables.keys() {
        if !PROJECT_VARIABLES.contains(&k.as_str()) || k == "APP_NAME" {
            return Err(IpcError::invalid("variables", "variableName"));
        }
    }
    if let Some(r) = refusal(app, &folder)? {
        return Err(IpcError::not_allowed("folder-refused", r));
    }
    let (s, _, _) = app.settings()?;
    let Some(repo) = s.blueprint_repo.clone() else {
        return Err(IpcError::not_allowed(
            "no-blueprint",
            "Choose the blueprint repo first (Settings).",
        ));
    };
    let r = first_run::read_repo(app, Path::new(&repo.path))?;
    if r.dirty {
        return Err(IpcError::not_allowed(
            "blueprint-dirty",
            format!(
                "{BLUEPRINT_FILE} differs from HEAD (working {} vs HEAD {}). Commit or discard the change, then pin.",
                r.oids.0.chars().take(7).collect::<String>(),
                r.oids
                    .1
                    .as_deref()
                    .map_or("missing".to_owned(), |h| h.chars().take(7).collect())
            ),
        ));
    }
    let adopt = foreman_schema(&folder).map(|(_, h)| h);
    let git_init = !is_own_repo(app, &folder)?;
    let conditions: BTreeMap<CoreCondition, CoreTri> = CoreCondition::ALL
        .iter()
        .map(|c| {
            let v = req
                .conditions
                .get(&(*c).into())
                .map_or(CoreTri::Undecided, |v| (*v).into());
            (*c, v)
        })
        .collect();
    let mut variables: BTreeMap<String, String> = s.global_variables.clone();
    variables.retain(|k, _| PROJECT_VARIABLES.contains(&k.as_str()));
    for (k, v) in &req.variables {
        variables.insert(k.clone(), v.as_str().to_owned());
    }
    variables.insert("APP_NAME".to_owned(), name.to_owned());
    let project = ProjectFile {
        schema_version: ProjectFile::CURRENT,
        project_id: adopt
            .as_ref()
            .and_then(|h| h.project_id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        name: name.to_owned(),
        slug: req.slug.as_str().to_owned(),
        spec_folder: "spec".to_owned(),
        variables,
        conditions: conditions.clone(),
        condition_audit: Vec::new(),
        model_policy: ModelPolicy {
            plan: Plan::Max,
            substitutions: Vec::new(),
        },
        chat_project_url: req.chat_project_url.map(|u| u.into_string()),
        chat_urls: BTreeMap::new(),
    };
    let writes = plan_writes(&folder, git_init, adopt.is_some());
    let pin = PinPreview {
        version: r.template.blueprint_version.clone(),
        commit: r.commit.chars().take(7).collect(),
        clean: !r.dirty,
    };
    let path = path_summary(&r.template, &conditions);
    let plan = SetupPlan {
        folder,
        adopt: adopt.map(|h| h.project_id.unwrap_or_default()),
        git_init,
        project,
        repo: r.path.clone(),
        pin: (r.commit.clone(), r.oids.0.clone()),
        writes: writes.clone(),
    };
    let id = app
        .inner()
        .previews
        .hold(Preview::Setup(Box::new(plan)), PREVIEW_TTL);
    Ok(SetupPreview {
        preview_id: PreviewId::from_trusted(id),
        writes,
        git_init,
        pin,
        path,
    })
}

/// The `docs/lessons.md` header (blueprint Appendix F.1).
fn lessons_header(app_name: &str) -> String {
    format!(
        "# Lessons — {app_name}\n\nFormat for every entry:\n\n## L-NNN · Stage [N] · <date> · <short title>\n\
WHAT HAPPENED: what went wrong or was wasted, with evidence\n\
CAUGHT BY: me | code-reviewer | security-reviewer | sre-reviewer |\n           design-reviewer | CI gate | inspection | production\n\
SHOULD HAVE BEEN CAUGHT BY: the earliest point it could have been caught\n\
ROOT CAUSE: prompt | agent | CLAUDE.md | spec gap | model choice |\n            effort choice | missing gate | external change\n\
COST: rework time, extra sessions, usage, or user impact\n\
FIX (proposed): the exact edit — file, section, old text → new text\n\
FIX (applied): blueprint vX.Y | project only | rejected (why)\n\
STATUS: open | applied | rejected\n"
    )
}

fn creates(plan: &SetupPlan, path: &str) -> bool {
    plan.writes
        .iter()
        .any(|w| w.path.as_str() == path && w.action == PlannedAction::Create)
}

fn register(app: &App, id: &str, name: &str, folder: &Path) -> Result<(), IpcError> {
    let (mut reg, stamp) = app.registry()?;
    if reg.projects.iter().any(|p| p.project_id == id) {
        return Err(IpcError::not_allowed(
            "already-registered",
            "That project is already registered.",
        ));
    }
    reg.projects.push(ProjectEntry {
        project_id: id.to_owned(),
        name: name.to_owned(),
        path: folder.display().to_string(),
        added_at: app.now(),
        pinned: None,
    });
    app.save_registry(&mut reg, &stamp)
}

/// `setup_apply` (P1): exactly the previewed writes, by CAS, no commit (AC-F018-02, -03).
///
/// # Errors
/// `Expired`, `Stale`, `Conflict`, `PolicyViolation`, `ToolFailed` (git init), `Io`.
pub fn setup_apply(app: &App, req: SetupApplyReq) -> Result<ProjectSummary, IpcError> {
    let _w = app.writing();
    let plan = match app.inner().previews.take(req.preview_id.as_str())? {
        Preview::Setup(p) => p,
        Preview::Action(_) => return Err(IpcError::expired()),
    };
    let id = plan.project.project_id.clone();
    if plan.adopt.is_some() {
        let root = Root::open(&plan.folder, RootKind::Project)?;
        let app_root = app.app_root()?;
        let d = Decisions {
            project: &root,
            app: &app_root,
            project_id: &id,
        };
        for k in SETUP_FILES {
            let rel = pathpolicy::check_read(k.repo_path(), ReadClass::Decision)?;
            if let (Some(bytes), _) = root.read_opt(&rel, ReadClass::Decision)? {
                d.approve(k, &bytes, false)
                    .map_err(|e| IpcError::io(e.to_string()))?;
            }
        }
        register(app, &id, &plan.project.name, &plan.folder)?;
        return Ok(project::open(app, &id)?.summary(app));
    }
    if plan
        .writes
        .iter()
        .any(|w| w.action == PlannedAction::Refuse)
    {
        return Err(IpcError::not_allowed(
            "setup-refused",
            "A file setup would create already exists. Move it aside, then review again.",
        ));
    }
    if let Some(r) = refusal(app, &plan.folder)? {
        return Err(IpcError::not_allowed("folder-refused", r));
    }
    let ctx = app.spawn_ctx_now()?;
    if plan.git_init {
        foreman_sys::git::run_ok(&plan.folder, &GitOp::Init, &GitOpts::default(), &ctx)
            .map_err(git_error)?;
    }
    let state = foreman_sys::lock::blueprint_state(&plan.repo, BLUEPRINT_FILE, &ctx)
        .map_err(|e| IpcError::tool_failed("git", None, false, escape_visible(&e.to_string())))?;
    if (state.commit.clone(), state.working_oid.clone()) != plan.pin {
        return Err(IpcError::stale(
            crate::ipc::error::StaleWhat::ReadStamp,
            "The blueprint changed since this review. Review again.",
            None,
        ));
    }
    let root =
        Root::open(&plan.folder, RootKind::Project)?.with_recovery(&app.paths().app_data, &id);
    let app_root = app.app_root()?;
    let d = Decisions {
        project: &root,
        app: &app_root,
        project_id: &id,
    };
    let now = app.now();
    for (kind, bytes) in [
        (DecisionFileKind::Project, schema::to_file(&plan.project)),
        (
            DecisionFileKind::Overlay,
            schema::to_file(&OverlayFile::empty()),
        ),
    ] {
        let bytes = bytes.map_err(|e| IpcError::io(e.to_string()))?;
        // No commit at setup (AC-F018-03): the change is approved and written, not committed.
        let _: OwnChange = d
            .own_change(kind, bytes.as_bytes(), false, &now)
            .map_err(|e| IpcError::io(e.to_string()))?;
    }
    let pinned =
        lock::pin(&root, &app_root, BLUEPRINT_FILE, &state, false, &now).map_err(|e| match e {
            LockError::Dirty { .. } => IpcError::not_allowed("blueprint-dirty", e.to_string()),
            LockError::RepinClosed => {
                IpcError::not_allowed("repin-after-stage-1-signed", e.to_string())
            }
            LockError::Store(s) => s.into(),
        })?;
    let lock_bytes =
        foreman_core::json::to_canonical(&pinned).map_err(|e| IpcError::io(e.to_string()))?;
    d.approve(
        DecisionFileKind::BlueprintLock,
        lock_bytes.as_bytes(),
        false,
    )
    .map_err(|e| IpcError::io(e.to_string()))?;
    for dir in SETUP_DIRS {
        if creates(&plan, &format!("{dir}/")) {
            dirs::ensure_dir(&root, &pathpolicy::check_read(dir, ReadClass::Decision)?)?;
        }
    }
    if creates(&plan, "docs/lessons.md") {
        let rel = pathpolicy::check_write("docs/lessons.md", WritePurpose::Repo)?;
        root.write(
            &rel,
            lessons_header(&plan.project.name).as_bytes(),
            &ReadStamp::Absent,
        )?;
    }
    let progress = ProgressFile::new(&id);
    let rel = pathpolicy::check_app_data(&project::progress_path(&id))?;
    let text = schema::to_file(&progress).map_err(|e| IpcError::io(e.to_string()))?;
    let (_, stamp) = app_root.read_opt(&rel, ReadClass::AppState)?;
    app_root.write(&rel, text.as_bytes(), &stamp)?;
    register(app, &id, &plan.project.name, &plan.folder)?;
    Ok(project::open_with(app, &id, OpenMode::Full)?.summary(app))
}

/// `project_open` (P1): the full view; read-only states are in the view, not errors.
///
/// # Errors
/// `NotFound`, `Io`.
pub fn project_open(app: &App, req: ProjectOpenReq) -> Result<ProjectView, IpcError> {
    let p = project::open_with(app, req.project_id.as_str(), OpenMode::Full)?;
    let b = p
        .root
        .as_ref()
        .map(|r| project::branch(app, r.path()))
        .unwrap_or_default();
    Ok(p.view(app, b))
}

/// `project_url_set` (P1): the claude.ai Project URL (a variable field, ADR-022).
///
/// # Errors
/// `ReadOnly`, `Conflict`, `ToolFailed`.
pub fn project_url_set(app: &App, req: ProjectUrlSetReq) -> Result<Rev, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let mut pf = p.parts()?.2.clone();
    pf.chat_project_url = req.url.map(|u| u.into_string());
    p.own_change(app, DecisionFileKind::Project, &pf)?;
    Ok(Rev(p.progress.rev))
}

/// `condition_set` (P1): inside its edit window, with an audit line (DR-052).
///
/// # Errors
/// `NotAllowed("condition-locked-after-stage-2" | "late-condition-locked-after-stage-4")`,
/// `ReadOnly`, `Conflict`, `ToolFailed`.
pub fn condition_set(app: &App, req: ConditionSetReq) -> Result<ProjectView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let c: CoreCondition = req.condition.into();
    let pf = rules::set_condition(
        p.parts()?.2,
        c,
        req.to.into(),
        req.note.as_str(),
        &app.now(),
        &p.signed(),
    )
    .map_err(|e| match e {
        RuleError::ConditionLocked if c.is_late() => IpcError::not_allowed(
            "late-condition-locked-after-stage-4",
            escape_visible(&e.to_string()),
        ),
        _ => IpcError::not_allowed(
            "condition-locked-after-stage-2",
            escape_visible(&e.to_string()),
        ),
    })?;
    p.own_change(app, DecisionFileKind::Project, &pf)?;
    project::reopen_view(app, req.project_id.as_str())
}

/// `variable_set` (P1).
///
/// # Errors
/// `NotFound` (unknown variable), `ReadOnly`, `Conflict`, `ToolFailed`.
pub fn variable_set(app: &App, req: VariableSetReq) -> Result<ProjectView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let name = req.name.as_str();
    if !PROJECT_VARIABLES.contains(&name) {
        return Err(IpcError::not_found("That isn't a blueprint variable."));
    }
    let mut pf = p.parts()?.2.clone();
    pf.variables
        .insert(name.to_owned(), req.value.as_str().to_owned());
    p.own_change(app, DecisionFileKind::Project, &pf)?;
    project::reopen_view(app, req.project_id.as_str())
}

/// `blueprint_repin` (P1): only before Stage 1 is signed and only from a clean blueprint
/// (DR-155). Allowed on a read-only project (IPC-R9: it is how a lock mismatch is fixed).
///
/// # Errors
/// `NotAllowed("repin-after-stage-1-signed" | "blueprint-dirty" | "no-blueprint")`,
/// `ToolMissing`, `Conflict`.
pub fn blueprint_repin(app: &App, req: BlueprintRepinReq) -> Result<ProjectView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    let (s, _, _) = app.settings()?;
    let Some(repo) = s.blueprint_repo else {
        return Err(IpcError::not_allowed(
            "no-blueprint",
            "Choose the blueprint repo first (Settings).",
        ));
    };
    let ctx = app.spawn_ctx_now()?;
    let state: BlueprintState =
        foreman_sys::lock::blueprint_state(Path::new(&repo.path), BLUEPRINT_FILE, &ctx).map_err(
            |e| IpcError::tool_failed("git", None, false, escape_visible(&e.to_string())),
        )?;
    let stage1 = p.signed().contains(&1);
    let now = app.now();
    let root = p.root()?;
    let pinned = lock::pin(root, &p.app_root, BLUEPRINT_FILE, &state, stage1, &now).map_err(
        |e| match e {
            LockError::RepinClosed => {
                IpcError::not_allowed("repin-after-stage-1-signed", e.to_string())
            }
            LockError::Dirty { .. } => IpcError::not_allowed("blueprint-dirty", e.to_string()),
            LockError::Store(s) => s.into(),
        },
    )?;
    let bytes =
        foreman_core::json::to_canonical(&pinned).map_err(|e| IpcError::io(e.to_string()))?;
    let d = Decisions {
        project: root,
        app: &p.app_root,
        project_id: &p.entry.project_id,
    };
    d.approve(DecisionFileKind::BlueprintLock, bytes.as_bytes(), false)
        .map_err(|e| IpcError::io(e.to_string()))?;
    let change = OwnChange::CommitNow {
        file: DecisionFileKind::BlueprintLock,
        path: DecisionFileKind::BlueprintLock.repo_path().to_owned(),
        sha256: foreman_core::store::sha256_hex(bytes.as_bytes()),
    };
    let opts = foreman_sys::git::commit::CommitOpts {
        app_data: app.paths().app_data.clone(),
        ..Default::default()
    };
    if !p.signed().contains(&5)
        && foreman_sys::decisions::commit_own(root, &change, &opts, &ctx).is_ok()
        && let OwnChange::CommitNow { sha256, .. } = &change
    {
        let _ = d.mark_committed(DecisionFileKind::BlueprintLock, sha256);
    }
    p.save_progress(app)?;
    project::reopen_view(app, req.project_id.as_str())
}

/// `project_remove` (P1 registry): after a native confirmation naming what goes; the repo is
/// never touched (PR-05). Deleting the project's app data (`projects/<id>/`, approvals,
/// fingerprints) needs the store's delete, which arrives with M4-02.
///
/// # Errors
/// `Cancelled`, `NotFound`, `Io`.
pub fn project_remove(app: &App, req: ProjectRemoveReq) -> Result<Vec<ProjectSummary>, IpcError> {
    let _w = app.writing();
    let (mut reg, stamp) = app.registry()?;
    let Some(e) = reg
        .projects
        .iter()
        .find(|p| p.project_id == req.project_id.as_str())
        .cloned()
    else {
        return Err(IpcError::not_found("That project isn't registered."));
    };
    let ok = app.os().confirm(&Confirm::new(
        format!("Remove {}?", escape_visible(&e.name)),
        format!(
            "Foreman forgets this project and its progress. The folder {} and everything in it stay as they are.",
            escape_visible(&app.display(Path::new(&e.path)))
        ),
        "Remove project",
    ));
    if !ok {
        return Err(IpcError::cancelled());
    }
    reg.projects.retain(|p| p.project_id != e.project_id);
    app.save_registry(&mut reg, &stamp)?;
    app.inner().summaries.remove(&e.project_id);
    summaries(app)
}

/// `project_pin_set` (P2): a sidebar slot 1–9 (⌘1–⌘9); a slot held by another project moves.
///
/// # Errors
/// `NotFound`, `Invalid` (slot outside 1–9), `Io`.
pub fn project_pin_set(app: &App, req: ProjectPinSetReq) -> Result<Vec<ProjectSummary>, IpcError> {
    if req.slot.is_some_and(|s| !(1..=9).contains(&s)) {
        return Err(IpcError::invalid("slot", "range"));
    }
    let _w = app.writing();
    let (mut reg, stamp) = app.registry()?;
    if !reg
        .projects
        .iter()
        .any(|p| p.project_id == req.project_id.as_str())
    {
        return Err(IpcError::not_found("That project isn't registered."));
    }
    for p in &mut reg.projects {
        if p.project_id == req.project_id.as_str() {
            p.pinned = req.slot;
        } else if req.slot.is_some() && p.pinned == req.slot {
            p.pinned = None;
        }
    }
    app.save_registry(&mut reg, &stamp)?;
    {
        let mut i = app.inner();
        for p in &reg.projects {
            if let Some(s) = i.summaries.get_mut(&p.project_id) {
                s.pinned = p.pinned;
            }
        }
    }
    summaries(app)
}
