//! An open project (ticket M2-19; ADR-022, ADR-023): the registry entry, the project root, the
//! approved decisions, the pinned template (verified against the lock), the applicable path, the
//! signed packets and `progress.json`. Every project command loads one through [`open`] and builds
//! its view from it; derived states are computed on every read, never stored (DR-050).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use foreman_core::decisions::{DecisionError, Decisions, OwnChange, Processed};
use foreman_core::ids::{StepKey as CoreStepKey, Surface as CoreSurface};
use foreman_core::lock::{self, ReadOnly as LockReadOnly};
use foreman_core::packet::{self, Signature as PacketSignature};
use foreman_core::pathpolicy::{self, ReadClass};
use foreman_core::policy;
use foreman_core::rules::{self, Applicability as CoreApp, ApplicablePath, StageState};
use foreman_core::schema::approvals::ApprovalsFile;
use foreman_core::schema::lock::LockFile;
use foreman_core::schema::overlay::OverlayFile;
use foreman_core::schema::progress::{
    DecisionFileKind as CoreKind, PendingOutside, ProgressFile, SigningState, StepProgress,
};
use foreman_core::schema::project::ProjectFile;
use foreman_core::schema::projects::ProjectEntry;
use foreman_core::schema::{self, Loaded};
use foreman_core::steps::{self, StepCtx, StepState as CoreState};
use foreman_core::store::{ReadStamp, Root, RootKind, sha256_hex};
use foreman_core::template::{self, Step, StepKind as CoreKindStep, Template};

use crate::ipc::IpcError;
use crate::ipc::dto::app::{
    BlueprintPin, ConditionView, PacketSummary, ProjectSummary, ProjectView, SheetStatus,
    SignedInfo, SpecLine, SpecSurface, StageView, StepSummary,
};
use crate::ipc::dto::events::{DecisionPending, Event};
use crate::ipc::types::{
    Applicability, ConditionId, Explained, ProjectId, ReadOnly, ReadOnlyReason, RepoPath, Rev,
    RunId, Stage, StepKey, StepKind, StepState, Tag, ValueOrigin,
};
use crate::render::escape_visible;
use crate::state::App;
use crate::text;

/// A packet file as read on open.
#[derive(Debug, Clone)]
pub struct PacketFile {
    /// The text.
    pub text: String,
    /// SHA-256 of the bytes.
    pub sha256: String,
    /// The read stamp.
    pub stamp: ReadStamp,
}

/// An open project.
#[derive(Debug)]
pub struct Opened {
    /// The registry entry.
    pub entry: ProjectEntry,
    /// The project root (`None` when the folder is missing).
    pub root: Option<Root>,
    /// The app-data root.
    pub app_root: Root,
    /// The approved `project.json`.
    pub project: Option<ProjectFile>,
    /// The approved overlay.
    pub overlay: OverlayFile,
    /// The approved lock.
    pub lock: Option<LockFile>,
    /// The pinned template.
    pub template: Option<Template>,
    /// The applicable path.
    pub path: Option<ApplicablePath>,
    /// `progress.json` and its stamp.
    pub progress: ProgressFile,
    progress_stamp: ReadStamp,
    /// `approvals/<id>.json`.
    pub approvals: ApprovalsFile,
    /// Why the project is read-only, if it is.
    pub read_only: Option<ReadOnly>,
    /// Packet files by stage.
    pub packets: BTreeMap<u8, PacketFile>,
    /// Pending outside changes found by this open (already merged into `progress`).
    pub new_pending: usize,
    /// Signed packets whose working file differs from the committed blob (DR-146): the working
    /// file as read. `packets` holds the committed version for those stages (the resolver uses
    /// it).
    pub tampered: BTreeMap<u8, PacketFile>,
    /// `progress.json` couldn't be read (only in [`OpenMode::Rebuild`]).
    pub progress_unreadable: bool,
    /// `progress.json` was rebuilt from the repo on this open because it didn't exist
    /// (AC-F020-11).
    pub rebuilt: bool,
}

/// How much `open` does beyond reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    /// Every command: read and classify.
    Read,
    /// `project_open`: also the idempotent `info/exclude` append (SR-32).
    Full,
    /// `progress_rebuild_*`: an unreadable `progress.json` opens with fresh progress (recorded in
    /// [`Opened::progress_unreadable`]) and nothing is written (AC-F020-12: the file is kept).
    Rebuild,
}

fn ro(reason: ReadOnlyReason, detail: impl Into<String>) -> ReadOnly {
    ReadOnly {
        reason,
        detail: detail.into(),
    }
}

/// `projects/<id>/progress.json`.
#[must_use]
pub fn progress_path(id: &str) -> String {
    format!("projects/{id}/progress.json")
}

/// The registry entry of a project.
///
/// # Errors
/// `NotFound` for an unknown ID; `Io`.
pub fn entry(app: &App, id: &str) -> Result<ProjectEntry, IpcError> {
    let (reg, _) = app.registry()?;
    reg.projects
        .into_iter()
        .find(|p| p.project_id == id)
        .ok_or_else(|| IpcError::not_found("That project isn't registered."))
}

/// Reads `progress.json` (fresh when missing).
///
/// # Errors
/// `Io` when unreadable (rebuild it: `progress_rebuild_*`) or written by a newer Foreman.
pub fn read_progress(app_root: &Root, id: &str) -> Result<(ProgressFile, ReadStamp), IpcError> {
    let rel = pathpolicy::check_app_data(&progress_path(id))?;
    let (bytes, stamp) = app_root.read_opt(&rel, ReadClass::AppState)?;
    let Some(bytes) = bytes else {
        return Ok((ProgressFile::new(id), stamp));
    };
    match schema::load::<ProgressFile>(&bytes) {
        Ok(Loaded::Current(p) | Loaded::Migrated { value: p, .. }) => Ok((p, stamp)),
        Ok(Loaded::NewerReadOnly(h)) => Err(IpcError::io(schema::newer_message(h.schema_version))),
        Err(e) => Err(IpcError::io(format!(
            "Foreman's progress file for this project (app data: {rel}) is unreadable: {}. It's kept as it is. Rebuild from repo replaces it after you review what the rebuild finds.",
            escape_visible(&e.to_string())
        ))),
    }
}

fn decision_kind_name(k: CoreKind) -> &'static str {
    k.repo_path()
}

/// Opens a project (read-only states are part of the result, not errors).
///
/// # Errors
/// `NotFound` (unknown ID); `Io`.
#[allow(
    clippy::too_many_lines,
    reason = "the open sequence (DR-150, DR-156) in one place, in order"
)]
pub fn open_with(app: &App, id: &str, mode: OpenMode) -> Result<Opened, IpcError> {
    let entry = entry(app, id)?;
    let app_root = app.app_root()?;
    let (mut progress, progress_stamp, unreadable) = match read_progress(&app_root, id) {
        Ok((p, s)) => (p, s, false),
        Err(_) if mode == OpenMode::Rebuild => {
            let rel = pathpolicy::check_app_data(&progress_path(id))?;
            let stamp = app_root
                .read_opt(&rel, ReadClass::AppState)
                .map_or(ReadStamp::Absent, |(_, s)| s);
            (ProgressFile::new(id), stamp, true)
        }
        Err(e) => return Err(e),
    };
    let missing = progress_stamp == ReadStamp::Absent;
    let original = progress.clone();
    let (approvals, _) = match Root::open(Path::new(&entry.path), RootKind::Project) {
        Ok(r) => Decisions {
            project: &r,
            app: &app_root,
            project_id: id,
        }
        .approvals()
        .map_err(|e| IpcError::io(e.to_string()))?,
        Err(_) => (ApprovalsFile::new(id), ReadStamp::Absent),
    };
    let mut opened = Opened {
        entry: entry.clone(),
        root: None,
        app_root: Root::open_app_data(&app.paths().app_data)
            .map_err(|e| IpcError::io(e.to_string()))?,
        project: None,
        overlay: OverlayFile::empty(),
        lock: None,
        template: None,
        path: None,
        progress: progress.clone(),
        progress_stamp: progress_stamp.clone(),
        approvals,
        read_only: None,
        packets: BTreeMap::new(),
        new_pending: 0,
        tampered: BTreeMap::new(),
        progress_unreadable: unreadable,
        rebuilt: false,
    };
    let folder = Path::new(&entry.path);
    if !folder.is_dir() {
        opened.read_only = Some(ro(
            ReadOnlyReason::FolderMissing,
            format!(
                "Project folder not found: {}. Move it back or remove the project.",
                escape_visible(&app.display(folder))
            ),
        ));
        return Ok(opened);
    }
    let root = Root::open(folder, RootKind::Project)?.with_recovery(&app.paths().app_data, id);
    let d = Decisions {
        project: &root,
        app: &app_root,
        project_id: id,
    };
    // DR-151: classify every decision file against its approved copy. While HEAD names a branch a
    // signing on another branch committed to, that branch's version is no change (ADR-022 §5).
    let now = app.now();
    let head_ref = if opened
        .approvals
        .files
        .project
        .committed_on_other_branches
        .is_empty()
        && opened
            .approvals
            .files
            .overlay
            .committed_on_other_branches
            .is_empty()
        && opened
            .approvals
            .files
            .blueprint_lock
            .committed_on_other_branches
            .is_empty()
    {
        None
    } else {
        crate::signing::head_ref(app, folder)
    };
    for kind in CoreKind::ALL {
        let outcome = match d.process_on(
            kind,
            &uuid::Uuid::new_v4().to_string(),
            &now,
            head_ref.as_deref(),
        ) {
            Ok(o) => o,
            Err(DecisionError::NoApprovedCopy(_)) => {
                opened.read_only.get_or_insert_with(|| {
                    ro(
                        ReadOnlyReason::DecisionFileInvalid,
                        format!(
                            "{} has no approved copy in this Foreman. Review it before Foreman uses it.",
                            decision_kind_name(kind)
                        ),
                    )
                });
                continue;
            }
            Err(e) => return Err(IpcError::io(e.to_string())),
        };
        match outcome {
            Processed::Unchanged | Processed::VariablesReloaded => {}
            Processed::Pending(p) => {
                let known = progress
                    .pending_outside
                    .iter()
                    .any(|x| x.file == p.file && x.seen_sha256 == p.seen_sha256);
                if !known {
                    progress.pending_outside.retain(|x| x.file != p.file);
                    progress.pending_outside.push(p);
                    opened.new_pending += 1;
                }
            }
            Processed::ReadOnly(why) => {
                let reason = if why.contains("newer Foreman") {
                    ReadOnlyReason::NewerSchema
                } else {
                    ReadOnlyReason::DecisionFileInvalid
                };
                opened
                    .read_only
                    .get_or_insert_with(|| ro(reason, escape_visible(&why)));
            }
        }
    }
    // An approved copy this Foreman can't use (a newer schema, adopted from a folder set up by a
    // newer Foreman) leaves the project read-only (AC-F018-07, AC-F030-03).
    for kind in CoreKind::ALL {
        if let Some(why) = d.approved_problem(kind) {
            let reason = if why.contains("newer Foreman") {
                ReadOnlyReason::NewerSchema
            } else {
                ReadOnlyReason::DecisionFileInvalid
            };
            opened
                .read_only
                .get_or_insert_with(|| ro(reason, escape_visible(&why)));
        }
    }
    // A decision file that no longer differs drops its stale pending entry.
    progress.pending_outside.retain(|p| still_pending(&root, p));
    opened.project = d.effective_project().ok();
    let mut overlay_flags: Vec<String> = Vec::new();
    opened.overlay = d
        .effective_overlay()
        .unwrap_or_else(|_| OverlayFile::empty());
    opened.lock = d.effective_lock().ok();
    if let Some(p) = &opened.project
        && p.project_id != id
    {
        opened.read_only.get_or_insert_with(|| {
            ro(
                ReadOnlyReason::DecisionFileInvalid,
                "This folder belongs to another Foreman project.",
            )
        });
    }
    // DR-156: the pinned template from app data or the blueprint repo, verified by hash.
    if let Some(lk) = &opened.lock {
        let (settings, _, _) = app.settings()?;
        let ctx = app.spawn_ctx_now()?;
        let repo = settings
            .blueprint_repo
            .as_ref()
            .map(|r| PathBuf::from(&r.path));
        let restored = lock::restore(&app_root, lk, || match &repo {
            Some(r) => foreman_sys::lock::fetch(r, lk, &ctx),
            None => lock::Fetched::NotFound,
        });
        match restored {
            Ok((text, _)) => {
                let (t, _) = template::cache::load_or_parse(&app_root, &text)?;
                // F-025: the overlay's added steps join a copy of the pinned template, in place.
                let t = if opened.overlay.added_steps.is_empty() {
                    t
                } else {
                    let (eff, flags) = rules::overlay::effective_template(&t, &opened.overlay);
                    overlay_flags = flags;
                    eff
                };
                opened.template = Some(t);
            }
            Err(e) => {
                let reason = match e {
                    LockReadOnly::HashMismatch { .. } => ReadOnlyReason::LockMismatch,
                    LockReadOnly::GitMissing => ReadOnlyReason::GitMissing,
                    LockReadOnly::NotFound { .. } | LockReadOnly::Store(_) => {
                        ReadOnlyReason::BlueprintMissing
                    }
                };
                opened
                    .read_only
                    .get_or_insert_with(|| ro(reason, e.to_string()));
            }
        }
    } else {
        opened.read_only.get_or_insert_with(|| {
            ro(
                ReadOnlyReason::BlueprintMissing,
                "This project has no blueprint pin. Re-pin the blueprint.",
            )
        });
    }
    if let (Some(t), Some(p)) = (&opened.template, &opened.project) {
        let mut path = rules::applicable_path(t, &p.conditions, &opened.overlay);
        path.flags.append(&mut overlay_flags);
        opened.path = Some(path);
    }
    for n in 0..=17u8 {
        let Ok(rel) = pathpolicy::check_read(&packet::file_name(n), ReadClass::Decision) else {
            continue;
        };
        if let Ok((Some(bytes), stamp)) = root.read_opt(&rel, ReadClass::Decision) {
            opened.packets.insert(
                n,
                PacketFile {
                    sha256: sha256_hex(&bytes),
                    text: String::from_utf8_lossy(&bytes).into_owned(),
                    stamp,
                },
            );
        }
    }
    // Signed packets: another branch's from its commit; a tampered one's committed blob (DR-146).
    crate::signing::load_signed_packets(app, &root, &progress, &mut opened);
    if mode == OpenMode::Full
        && let Ok(ctx) = app.spawn_ctx_now()
    {
        // SR-32: idempotent; a folder that isn't its own repo or a missing git just skips it.
        let _ = foreman_sys::git::ensure_exclude(&root, &ctx);
        // ADR-012 4.7: a pending real-index refresh is retried on open.
        crate::signing::retry_index_refresh(app, &root, &mut progress);
        for n in opened.tampered.keys() {
            app.emit(Event::TamperDetected(
                crate::ipc::dto::events::TamperDetected {
                    project_id: ProjectId::from_trusted(id),
                    stage: Stage(*n),
                    rev: Rev(progress.rev),
                },
            ));
        }
    }
    opened.root = Some(root);
    // AC-F020-11, NFR-17: missing app data is rebuilt from the repo's packets on open (nothing is
    // lost that the repo still has); an unreadable file waits for Billy's Rebuild (AC-F020-12).
    if missing && mode != OpenMode::Rebuild && !opened.packets.is_empty() && opened.path.is_some() {
        let pending = std::mem::take(&mut progress.pending_outside);
        progress = opened.rebuilt_progress(app, 0);
        progress.pending_outside = pending;
        opened.rebuilt = true;
    }
    opened.progress = progress;
    // AC-F007-04: a started manual step whose file changed since is done (re-stat on open).
    crate::ipc::commands::steps::manual_outs_changed(app, &mut opened);
    if mode != OpenMode::Rebuild && opened.progress != original {
        opened.save_progress(app)?;
    }
    if opened.new_pending > 0 {
        app.emit(Event::DecisionPending(DecisionPending {
            project_id: ProjectId::from_trusted(id),
            count: u32::try_from(opened.progress.pending_outside.len()).unwrap_or(u32::MAX),
        }));
    }
    app.inner()
        .summaries
        .insert(id.to_owned(), opened.summary(app));
    Ok(opened)
}

/// Re-classifies a project's decision files after a watcher hint or a re-stat (DR-151, ADR-013,
/// AC-F022-04): a new outside change becomes a pending review without a restart and emits
/// `decision:pending`; a variables-only change is reloaded. It changes only what [`open`] would
/// (the pending list and the approved copies of variables-only changes), never the repo, and acts
/// on nothing else (SR-18). Returns the number of new pending changes.
///
/// # Errors
/// `NotFound`, `Io` (an unreadable `progress.json` is left for the rebuild path).
pub fn refresh_decisions(app: &App, id: &str) -> Result<usize, IpcError> {
    let _w = app.writing();
    let entry = entry(app, id)?;
    let app_root = app.app_root()?;
    let (mut progress, stamp) = read_progress(&app_root, id)?;
    let root = Root::open(Path::new(&entry.path), RootKind::Project)?;
    let d = Decisions {
        project: &root,
        app: &app_root,
        project_id: id,
    };
    let before = progress.clone();
    let now = app.now();
    let mut new_pending = 0;
    let mut reloaded = false;
    for kind in CoreKind::ALL {
        match d.process(kind, &uuid::Uuid::new_v4().to_string(), &now) {
            Ok(Processed::Pending(p)) => {
                let known = progress
                    .pending_outside
                    .iter()
                    .any(|x| x.file == p.file && x.seen_sha256 == p.seen_sha256);
                if !known {
                    progress.pending_outside.retain(|x| x.file != p.file);
                    progress.pending_outside.push(p);
                    new_pending += 1;
                }
            }
            Ok(Processed::VariablesReloaded) => reloaded = true,
            // Read-only states and missing approved copies show on the next open.
            Ok(Processed::Unchanged | Processed::ReadOnly(_)) | Err(_) => {}
        }
    }
    progress.pending_outside.retain(|p| still_pending(&root, p));
    if progress != before || reloaded {
        progress.rev = progress.rev.wrapping_add(1);
        let rel = pathpolicy::check_app_data(&progress_path(id))?;
        let text = schema::to_file(&progress).map_err(|e| IpcError::io(e.to_string()))?;
        app_root.write(&rel, text.as_bytes(), &stamp)?;
        app.project_changed(id, progress.rev);
    }
    if new_pending > 0 {
        app.emit(Event::DecisionPending(DecisionPending {
            project_id: ProjectId::from_trusted(id),
            count: u32::try_from(progress.pending_outside.len()).unwrap_or(u32::MAX),
        }));
    }
    Ok(new_pending)
}

fn still_pending(root: &Root, p: &PendingOutside) -> bool {
    let Ok(rel) = pathpolicy::check_read(p.file.repo_path(), ReadClass::Decision) else {
        return false;
    };
    matches!(root.read(&rel, ReadClass::Decision), Ok((b, _)) if sha256_hex(&b) == p.seen_sha256)
}

/// [`open_with`] in `Read` mode.
///
/// # Errors
/// As [`open_with`].
pub fn open(app: &App, id: &str) -> Result<Opened, IpcError> {
    open_with(app, id, OpenMode::Read)
}

/// The light summary of a registry entry (NFR-01: `app_bootstrap` reads only `settings.json`,
/// `projects.json` and each `progress.json`): the last full summary this run, refreshed with the
/// progress revision, or one built from `progress.json` alone.
#[must_use]
pub fn light_summary(app: &App, e: &ProjectEntry, app_root: &Root) -> ProjectSummary {
    let progress = read_progress(app_root, &e.project_id).ok();
    let rev = progress.as_ref().map_or(0, |(p, _)| p.rev);
    if let Some(s) = app.inner().summaries.get(&e.project_id) {
        let mut s = s.clone();
        s.rev = Rev(rev.max(s.rev.0));
        s.pinned = e.pinned;
        s.name.clone_from(&e.name);
        return s;
    }
    let folder_missing = !Path::new(&e.path).is_dir();
    let current = progress.as_ref().and_then(|(p, _)| {
        let signed = signed_from_progress(p);
        let started = p
            .steps
            .keys()
            .filter_map(|k| stage_of_key(k.as_str()))
            .max();
        signed
            .iter()
            .max()
            .map(|s| s + 1)
            .max(started)
            .or(Some(0))
            .filter(|s| *s <= 18)
    });
    ProjectSummary {
        id: ProjectId::from_trusted(e.project_id.as_str()),
        name: e.name.clone(),
        display_path: app.display(Path::new(&e.path)),
        rev: Rev(rev),
        read_only: folder_missing.then(|| {
            ro(
                ReadOnlyReason::FolderMissing,
                "Project folder not found. Move it back or remove the project.",
            )
        }),
        current_stage: current.map(Stage),
        pinned: e.pinned,
    }
}

/// The stage of a step key (`2.3` → 2, `5:gate.A` → 5, `x-4-1` → 4, `council-setup` → 2).
#[must_use]
pub fn stage_of_key(k: &str) -> Option<u8> {
    if k == "council-setup" {
        return Some(2);
    }
    let k = k.strip_prefix("x-").unwrap_or(k);
    let n: String = k.chars().take_while(char::is_ascii_digit).collect();
    n.parse().ok()
}

fn signed_from_progress(p: &ProgressFile) -> BTreeSet<u8> {
    p.signing
        .iter()
        .filter(|(_, s)| s.state != SigningState::CommitPending)
        .filter_map(|(k, _)| k.parse().ok())
        .collect()
}

impl Opened {
    /// The stamp `progress.json` had when this open read it.
    #[must_use]
    pub fn progress_stamp(&self) -> &ReadStamp {
        &self.progress_stamp
    }

    /// The project ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.entry.project_id
    }

    /// Progress rebuilt from the repo (NFR-17, AC-F020-11, files.md §4.3 "Rebuild"): every step on
    /// the path of a signed stage is done, and so is the step that writes a stage's packet when
    /// the packet exists. Foreman's own pending decision changes are recomputed from the approvals.
    /// Chains, run inputs, overrides, optional parts, waits, gate runs and the run log are lost
    /// (they exist only in app data); from M4, stage records add the signing entries.
    #[must_use]
    pub fn rebuilt_progress(&self, app: &App, rev: u32) -> ProgressFile {
        let mut out = ProgressFile::new(self.id());
        out.rev = rev;
        let signed = self.signed();
        let now = app.now();
        if let (Some(t), Some(path)) = (&self.template, &self.path) {
            for s in &t.steps {
                if !path.step(s.key.as_str()).is_some_and(CoreApp::on_path) {
                    continue;
                }
                let writes_packet = self.packets.contains_key(&s.stage)
                    && s.out.iter().any(|o| *o == packet::file_name(s.stage));
                if signed.contains(&s.stage) || writes_packet {
                    out.steps.insert(
                        s.key.clone(),
                        StepProgress {
                            state: schema::progress::StoredState::Done,
                            ..steps::fresh()
                        },
                    );
                }
            }
        }
        for (kind, f) in [
            (CoreKind::Project, &self.approvals.files.project),
            (CoreKind::Overlay, &self.approvals.files.overlay),
            (
                CoreKind::BlueprintLock,
                &self.approvals.files.blueprint_lock,
            ),
        ] {
            if let (Some(a), Some(c)) = (&f.approved_sha256, &f.committed_sha256)
                && a != c
            {
                out.pending_own.push(schema::progress::PendingOwn {
                    file: kind,
                    sha256: a.clone(),
                    at: now.clone(),
                });
            }
        }
        out.first_launch_done = !out.steps.is_empty();
        out
    }

    /// The declared `out` paths of every step on the applicable path (the watcher's interest set,
    /// ADR-013), sorted.
    #[must_use]
    pub fn watch_outs(&self) -> Vec<String> {
        let (Some(t), Some(path)) = (&self.template, &self.path) else {
            return Vec::new();
        };
        let mut v: Vec<String> = t
            .steps
            .iter()
            .filter(|s| path.step(s.key.as_str()).is_some_and(CoreApp::on_path))
            .flat_map(|s| s.out.iter().cloned())
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// The read-only reason, or `BlueprintMissing` when the template isn't available.
    #[must_use]
    pub fn read_only_or_missing(&self) -> ReadOnly {
        self.read_only.clone().unwrap_or_else(|| {
            ro(
                ReadOnlyReason::BlueprintMissing,
                "The pinned blueprint isn't available.",
            )
        })
    }

    /// IPC-R9: mutations refuse a read-only project.
    ///
    /// # Errors
    /// `ReadOnly` with the reason.
    pub fn writable(&self) -> Result<(), IpcError> {
        match &self.read_only {
            Some(r) => Err(IpcError::read_only(r.clone())),
            None if self.template.is_none() || self.project.is_none() => {
                Err(IpcError::read_only(self.read_only_or_missing()))
            }
            None => Ok(()),
        }
    }

    /// The pinned blueprint commit.
    #[must_use]
    pub fn lock_commit(&self) -> String {
        self.lock
            .as_ref()
            .map(|l| l.blueprint_repo_commit.clone())
            .unwrap_or_default()
    }

    /// The project root.
    ///
    /// # Errors
    /// `ReadOnly(FolderMissing)`.
    pub fn root(&self) -> Result<&Root, IpcError> {
        self.root
            .as_ref()
            .ok_or_else(|| IpcError::read_only(self.read_only_or_missing()))
    }

    /// The template and path, or `ReadOnly`.
    ///
    /// # Errors
    /// `ReadOnly` when the template isn't available.
    pub fn parts(&self) -> Result<(&Template, &ApplicablePath, &ProjectFile), IpcError> {
        match (&self.template, &self.path, &self.project) {
            (Some(t), Some(p), Some(pr)) => Ok((t, p, pr)),
            _ => Err(IpcError::read_only(self.read_only_or_missing())),
        }
    }

    /// A step of the template.
    ///
    /// # Errors
    /// `NotFound`, or `ReadOnly` without a template.
    pub fn step(&self, key: &str) -> Result<&Step, IpcError> {
        let (t, _, _) = self.parts()?;
        t.step(key)
            .ok_or_else(|| IpcError::not_found("That step isn't in this project's blueprint."))
    }

    /// Stages signed by Foreman's signing or confirmed by Billy (DR-135: an outside signature
    /// counts only while the packet still has the hash Billy confirmed).
    #[must_use]
    pub fn signed(&self) -> BTreeSet<u8> {
        let mut out = signed_from_progress(&self.progress);
        for (k, s) in &self.approvals.outside_signatures {
            if let Ok(n) = k.parse::<u8>()
                && self
                    .packets
                    .get(&n)
                    .is_some_and(|p| p.sha256 == s.packet_sha256)
            {
                out.insert(n);
            }
        }
        out
    }

    /// The lowest applicable stage that isn't signed.
    #[must_use]
    pub fn current_stage(&self) -> Option<u8> {
        let path = self.path.as_ref()?;
        let signed = self.signed();
        path.stages
            .iter()
            .map(|(n, _)| *n)
            .find(|n| path.stage_applies(*n) && !signed.contains(n))
    }

    /// The summary.
    #[must_use]
    pub fn summary(&self, app: &App) -> ProjectSummary {
        ProjectSummary {
            id: ProjectId::from_trusted(self.id()),
            name: self
                .project
                .as_ref()
                .map_or_else(|| self.entry.name.clone(), |p| p.name.clone()),
            display_path: app.display(Path::new(&self.entry.path)),
            rev: Rev(self.progress.rev),
            read_only: self.read_only.clone(),
            current_stage: self.current_stage().map(Stage),
            pinned: self.entry.pinned,
        }
    }

    /// Writes `progress.json` with the next revision (CAS) and emits `project:changed`.
    ///
    /// # Errors
    /// `Conflict` when it changed underneath; `Io`.
    pub fn save_progress(&mut self, app: &App) -> Result<u32, IpcError> {
        self.progress.rev = self.progress.rev.wrapping_add(1);
        let rel = pathpolicy::check_app_data(&progress_path(self.id()))?;
        let text = schema::to_file(&self.progress).map_err(|e| IpcError::io(e.to_string()))?;
        let stamp = self
            .app_root
            .write(&rel, text.as_bytes(), &self.progress_stamp)?;
        self.progress_stamp = stamp;
        app.project_changed(self.id(), self.progress.rev);
        app.inner()
            .summaries
            .insert(self.id().to_owned(), self.summary(app));
        Ok(self.progress.rev)
    }

    /// Foreman's own change to a decision file (DR-153): the approved copy, then the repo file;
    /// before Stage 5 is signed it is committed at once by plumbing (SR-42, SR-44), from Stage 5
    /// on it waits for the next signing commit. Bumps the revision.
    ///
    /// # Errors
    /// `Conflict` (an outside edit is waiting), `ToolFailed`/`ToolMissing` (git), `Io`.
    pub fn own_change<T: schema::Versioned>(
        &mut self,
        app: &App,
        kind: CoreKind,
        value: &T,
    ) -> Result<(), IpcError> {
        let bytes = schema::to_file(value).map_err(|e| IpcError::io(e.to_string()))?;
        let stage5 = self.signed().contains(&5);
        let now = app.now();
        let (pending, commit_error, approvals) = {
            let root = self.root()?;
            let d = Decisions {
                project: root,
                app: &self.app_root,
                project_id: &self.entry.project_id,
            };
            let change =
                d.own_change(kind, bytes.as_bytes(), stage5, &now)
                    .map_err(|e| match e {
                        DecisionError::Store(s) => IpcError::from(s),
                        DecisionError::OutsidePending(p) => IpcError::conflict(&p, Vec::new()),
                        other => IpcError::io(other.to_string()),
                    })?;
            let mut pending = None;
            let mut commit_error = None;
            match &change {
                OwnChange::Pending(p) => pending = Some(p.clone()),
                OwnChange::CommitNow { sha256, .. } => {
                    let ctx = app.spawn_ctx_now()?;
                    let opts = foreman_sys::git::commit::CommitOpts {
                        app_data: app.paths().app_data.clone(),
                        ..Default::default()
                    };
                    match foreman_sys::decisions::commit_own(root, &change, &opts, &ctx) {
                        Ok(_) => {
                            let _ = d.mark_committed(kind, sha256);
                        }
                        // The change stands (approved and written); the commit is retried by the
                        // next own change or signing (DR-153). Reported after saving.
                        Err(e) => commit_error = Some(e.to_string()),
                    }
                }
            }
            (pending, commit_error, d.approvals().ok().map(|a| a.0))
        };
        if let Some(p) = pending {
            self.progress.pending_own.retain(|x| x.file != p.file);
            self.progress.pending_own.push(p);
        }
        if let Some(a) = approvals {
            self.approvals = a;
        }
        self.save_progress(app)?;
        if let Some(e) = commit_error {
            return Err(IpcError::tool_failed(
                "git",
                None,
                false,
                escape_visible(&e),
            ));
        }
        Ok(())
    }

    /// The stored progress of a step.
    #[must_use]
    pub fn step_progress(&self, key: &str) -> StepProgress {
        CoreStepKey::parse(key)
            .ok()
            .and_then(|k| self.progress.steps.get(&k).cloned())
            .unwrap_or_else(steps::fresh)
    }

    /// The displayed state of a step and the context transitions need.
    #[must_use]
    pub fn step_ctx(&self, app: &App, s: &Step) -> StepCtx {
        let applicability = self
            .path
            .as_ref()
            .and_then(|p| p.step(s.key.as_str()).cloned())
            .unwrap_or(CoreApp::Invalid);
        let wait_until = self
            .progress
            .waits
            .get(&s.key)
            .and_then(|d| d.parse::<jiff::civil::Date>().ok());
        StepCtx {
            kind: s.kind,
            surface: s.surface,
            applicability,
            today: app.os().today(),
            wait_until,
            at: app.now(),
        }
    }

    /// The displayed state of a step.
    #[must_use]
    pub fn display_state(&self, app: &App, s: &Step) -> CoreState {
        steps::display_state(&self.step_progress(s.key.as_str()), &self.step_ctx(app, s))
    }

    /// The `ProjectView`.
    #[must_use]
    pub fn view(&self, app: &App, branch: String) -> ProjectView {
        let signed = self.signed();
        let conditions = self
            .project
            .as_ref()
            .map(|p| {
                p.conditions
                    .iter()
                    .map(|(c, v)| {
                        let (label, effect) = text::condition(*c);
                        let lock_stage = if c.is_late() { 4 } else { 2 };
                        let locked = signed.contains(&lock_stage);
                        (
                            ConditionId::from(*c),
                            ConditionView {
                                value: (*v).into(),
                                locked,
                                lock_rule: locked.then(|| {
                                    if c.is_late() {
                                        "late-condition-locked-after-stage-4".to_owned()
                                    } else {
                                        "condition-locked-after-stage-2".to_owned()
                                    }
                                }),
                                label: label.to_owned(),
                                effect: effect.to_owned(),
                                late: c.is_late(),
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        ProjectView {
            summary: self.summary(app),
            conditions,
            stages: self.stage_views(app),
            pending_changes: u32::try_from(self.progress.pending_outside.len()).unwrap_or(0),
            read_only: self.read_only.clone(),
            branch,
            blueprint: BlueprintPin {
                version: self
                    .lock
                    .as_ref()
                    .map(|l| l.blueprint_version.clone())
                    .unwrap_or_default(),
                commit: self.lock_commit().chars().take(6).collect(),
            },
            dri: app.settings().ok().and_then(|(s, _, _)| s.dri_name),
            chat_project_url: self
                .project
                .as_ref()
                .and_then(|p| p.chat_project_url.clone()),
            // F-008's banner arrives with the fingerprint phase (M3-01); nothing is stored before.
            config_changed: None,
        }
    }

    /// Every stage's view.
    #[must_use]
    pub fn stage_views(&self, app: &App) -> Vec<StageView> {
        let Some(t) = &self.template else {
            return Vec::new();
        };
        t.stages
            .iter()
            .map(|st| self.stage_view(app, t, st.number, &st.title))
            .collect()
    }

    fn stage_view(&self, app: &App, t: &Template, n: u8, title: &str) -> StageView {
        let signed = self.signed();
        let path = self.path.as_ref();
        let state = path.and_then(|p| p.stage(n)).cloned();
        let steps: Vec<StepSummary> = t
            .stage_steps(n)
            .map(|s| self.step_summary(app, s))
            .collect();
        let undecided: Option<foreman_core::ids::ConditionId> = path.and_then(|p| {
            t.stage_steps(n).find_map(|s| match p.step(s.key.as_str()) {
                Some(CoreApp::Undecided(c)) => c.first().copied(),
                _ => None,
            })
        });
        let on_path = steps.iter().any(|s| {
            !matches!(
                s.state,
                StepState::NotApplicable
                    | StepState::Undecided
                    | StepState::NeedsAnnotation
                    | StepState::NeedsNewerForeman
                    | StepState::Skipped
            )
        });
        let applicable = match &state {
            Some(StageState::NotApplicable(r)) => {
                Applicability::NotApplicable { reason: r.clone() }
            }
            Some(StageState::NeedsAnnotation) => Applicability::NeedsAnnotation,
            _ => match undecided {
                Some(c) if !on_path => Applicability::Undecided {
                    condition: c.into(),
                },
                _ => Applicability::Applies,
            },
        };
        let prev = path.and_then(|p| p.previous_applicable_stage(n));
        let locked_by = prev.filter(|p| !signed.contains(p));
        let (status, why) = if signed.contains(&n) {
            (SheetStatus::Signed, None)
        } else {
            match &applicable {
                Applicability::NotApplicable { reason } => {
                    (SheetStatus::NotApplicable, Some(reason.clone()))
                }
                Applicability::NeedsAnnotation => (SheetStatus::NeedsAnnotation, None),
                Applicability::Undecided { condition } => (
                    SheetStatus::Undecided,
                    Some(format!(
                        "{} = undecided",
                        serde_json::to_value(condition)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_owned))
                            .unwrap_or_default()
                    )),
                ),
                Applicability::Applies => {
                    if let Some(p) = locked_by {
                        (
                            SheetStatus::Locked,
                            Some(format!("until Stage {p} is signed")),
                        )
                    } else {
                        let live: Vec<&StepSummary> = steps
                            .iter()
                            .filter(|s| {
                                !matches!(
                                    s.state,
                                    StepState::NotApplicable
                                        | StepState::Skipped
                                        | StepState::NeedsAnnotation
                                        | StepState::Undecided
                                )
                            })
                            .collect();
                        let own: Vec<&&StepSummary> = live
                            .iter()
                            .filter(|s| !matches!(s.kind, StepKind::Gate | StepKind::Lessons))
                            .collect();
                        if live.iter().any(|s| s.state == StepState::WaitingUntil) {
                            (SheetStatus::Waiting, None)
                        } else if !own.is_empty() && own.iter().all(|s| s.state == StepState::Done)
                        {
                            (SheetStatus::Inspecting, None)
                        } else if live.iter().any(|s| {
                            matches!(
                                s.state,
                                StepState::Done | StepState::Running | StepState::Copied
                            )
                        }) {
                            (SheetStatus::InProgress, None)
                        } else {
                            (SheetStatus::Ready, None)
                        }
                    }
                }
            }
        };
        let packet = (n <= 17).then(|| {
            let pk = self.packets.get(&n);
            PacketSummary {
                path: RepoPath::from_trusted(packet::file_name(n)),
                exists: pk.is_some(),
                problems: pk.map_or(0, |p| {
                    u32::try_from(self.packet_problems(n, &p.text).len()).unwrap_or(u32::MAX)
                }),
                signed: signed.contains(&n),
            }
        });
        // AC-F015-01 (P1): the close-out checklist once the packet is valid, until signed.
        let closeout_checklist = steps::closeout::checklist(
            n,
            packet.as_ref().is_some_and(|p| p.exists && p.problems == 0),
            signed.contains(&n),
        );
        StageView {
            stage: Stage(n),
            title: title.to_owned(),
            applicable,
            closeout_checklist,
            signed: signed.contains(&n).then(|| self.signed_info(n)),
            steps,
            packet,
            subtitle: text::stage_subtitle(n).to_owned(),
            status,
            status_why: why,
        }
    }

    fn signed_info(&self, n: u8) -> SignedInfo {
        let key = n.to_string();
        if let Some(s) = self.progress.signing.get(&key) {
            return SignedInfo {
                date: s.at.chars().take(10).collect(),
                commit: s
                    .commit
                    .as_deref()
                    .map(|c| c.chars().take(7).collect())
                    .unwrap_or_default(),
                branch: s.branch.clone(),
                merged: s.state == SigningState::Signed,
            };
        }
        let o = self.approvals.outside_signatures.get(&key);
        SignedInfo {
            date: o.map(|o| o.signed_on.clone()).unwrap_or_default(),
            commit: String::new(),
            branch: String::new(),
            merged: false,
        }
    }

    /// Validation problems of a stage's packet text (DR-133), errors only.
    #[must_use]
    pub fn packet_problems(&self, n: u8, text: &str) -> Vec<packet::Problem> {
        let (Some(t), Some(path)) = (&self.template, &self.path) else {
            return Vec::new();
        };
        let Some(fmt) = t.packet_format(n) else {
            return Vec::new();
        };
        let ctx = packet::PacketCtx {
            stage: n,
            next_stage: path.next_applicable_stage(n),
            lock_blueprint_version: self.lock.as_ref().map(|l| l.blueprint_version.clone()),
            has_skips: false,
        };
        packet::validate_packet(text, fmt, &ctx)
            .into_iter()
            .filter(|p| !p.warning)
            .collect()
    }

    /// The packet's signature as written (not the confirmation, DR-135).
    #[must_use]
    pub fn packet_signature(&self, n: u8) -> Option<PacketSignature> {
        let p = self.packets.get(&n)?;
        let parsed = packet::parse_packet(&p.text, &[]);
        parsed
            .get("APPROVED BY")
            .map(|f| packet::parse_signature(&f.value))
    }

    /// A step's row.
    #[must_use]
    pub fn step_summary(&self, app: &App, s: &Step) -> StepSummary {
        let state = self.display_state(app, s);
        let p = self.step_progress(s.key.as_str());
        let (state_text, condition_note, why) = state_words(&state, s, self.path.as_ref());
        let running_in_chain =
            s.chain.as_ref().is_some_and(|c| {
                self.progress.chains.iter().any(|ch| {
                    &ch.start == c
                        && ch.steps.iter().any(|k| {
                            k != &s.key
                                && self.progress.steps.get(k).is_some_and(|x| {
                                    x.state == schema::progress::StoredState::Running
                                })
                        })
                })
            });
        StepSummary {
            key: StepKey::from_trusted(s.key.as_str()),
            number: display_number(s),
            title: s.title.clone(),
            state: StepState::from(&state),
            tag: if s.key.as_str().starts_with("x-") {
                Tag::Added
            } else {
                s.tag.into()
            },
            surface: s.surface.map(Into::into),
            spec_line: self.spec_line(s),
            why_not_runnable: why,
            state_text,
            condition_note,
            kind: kind_of(s),
            run: (matches!(state, CoreState::Running))
                .then(|| p.last_run_id.clone().map(RunId::from_trusted))
                .flatten(),
            next: running_in_chain,
        }
    }

    /// A step's spec line (F-005), with each policy-changed value explained.
    #[must_use]
    pub fn spec_line(&self, s: &Step) -> Option<SpecLine> {
        if !s.runnable() {
            return None;
        }
        let p = self.project.as_ref()?;
        let over = CoreStepKey::parse(s.key.as_str())
            .ok()
            .and_then(|k| self.progress.overrides.get(&k))
            .map(|o| policy::RunOverride {
                model: o.model,
                effort: o.effort,
                mode: o.mode,
            });
        let surface = match (s.kind, s.surface) {
            (CoreKindStep::Action, _) => SpecSurface::Action,
            (_, Some(CoreSurface::Chat)) => SpecSurface::Chat,
            (_, Some(CoreSurface::Desktop)) => SpecSurface::Desktop,
            _ => SpecSurface::Cli,
        };
        let where_ = match (s.surface, s.cwd) {
            (Some(CoreSurface::Chat), _) => "claude.ai chat, in Project".to_owned(),
            (_, Some(foreman_core::ids::CwdKind::Spec)) => "spec/".to_owned(),
            (_, Some(foreman_core::ids::CwdKind::Project)) => "project root".to_owned(),
            _ => String::new(),
        };
        let eff = policy::effective_start(s, &p.model_policy, over.as_ref()).ok();
        let tpl = s.start;
        let explain =
            |v: String, f: Option<String>, changed: bool, origin: &ValueOrigin| Explained {
                value: v,
                origin: if changed {
                    origin.clone()
                } else {
                    ValueOrigin::Template
                },
                from: if changed { f } else { None },
            };
        let (model, effort, mode) = match (eff, tpl) {
            (Some(e), Some(t)) => {
                let origin = ValueOrigin::from(&e.origin);
                let v = e.value;
                (
                    Some(explain(
                        model_name(v.model),
                        Some(model_name(t.model)),
                        v.model != t.model,
                        &origin,
                    )),
                    Some(explain(
                        v.effort.as_str().to_owned(),
                        Some(t.effort.as_str().to_owned()),
                        v.effort != t.effort,
                        &origin,
                    )),
                    v.mode.map(|m| {
                        explain(
                            mode_name(m),
                            t.mode.map(mode_name),
                            Some(m) != t.mode,
                            &origin,
                        )
                    }),
                )
            }
            _ => (None, None, None),
        };
        let agents = policy::run_config(s)
            .agents
            .map(|a| {
                a.split([',', ' '])
                    .filter(|x| !x.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let note = policy::effective_start(s, &p.model_policy, over.as_ref())
            .ok()
            .and_then(|e| policy::substitution_line(s, &e));
        Some(SpecLine {
            surface,
            r#where: where_,
            model,
            effort,
            mode,
            agents,
            dropped_agents: Vec::new(),
            note,
        })
    }
}

/// How a model is named in the UI (DR-023 aliases → product names).
#[must_use]
pub fn model_name(m: foreman_core::ids::Model) -> String {
    use foreman_core::ids::Model as M;
    match m {
        M::Fable => "Fable 5.1",
        M::Opus => "Opus 5.5",
        M::Sonnet => "Sonnet 5.5",
        M::Haiku => "Haiku 5.5",
        M::Opusplan => "Opus Plan",
    }
    .to_owned()
}

/// How a mode is named in the UI (the blueprint's words).
#[must_use]
pub fn mode_name(m: foreman_core::ids::Mode) -> String {
    use foreman_core::ids::Mode as M;
    match m {
        M::Plan => "Plan",
        M::Default => "Manual",
        M::AcceptEdits => "Accept edits",
        M::Auto => "Auto",
        M::DontAsk => "Don't ask",
    }
    .to_owned()
}

pub(crate) fn display_number(s: &Step) -> String {
    match s.template_id.as_deref() {
        Some("gate.A") => "Gate A".to_owned(),
        Some("gate.B") => "Gate B".to_owned(),
        Some("gate.C") => "Gate C".to_owned(),
        Some(other) => other.to_owned(),
        None => s.id.clone(),
    }
}

fn kind_of(s: &Step) -> StepKind {
    match s.template_id.as_deref() {
        Some(t) if t.starts_with("gate.") => StepKind::Gate,
        Some("F.2") => StepKind::Lessons,
        _ => s.kind.into(),
    }
}

/// The state word with its detail, the condition note and why the step can't run.
pub(crate) fn state_words(
    state: &CoreState,
    s: &Step,
    path: Option<&ApplicablePath>,
) -> (String, Option<String>, Option<String>) {
    let undecided = || {
        path.and_then(|p| match p.step(s.key.as_str()) {
            Some(CoreApp::Undecided(cs)) => Some(cs.clone()),
            _ => None,
        })
    };
    match state {
        CoreState::NotStarted => ("Not started".into(), None, None),
        CoreState::Copied => ("Copied".into(), None, None),
        CoreState::Running => ("Running".into(), None, None),
        CoreState::Done => ("Done".into(), None, None),
        CoreState::Skipped(r) => (
            format!("Skipped: {}", escape_visible(r)),
            None,
            Some(format!("Skipped: {}", escape_visible(r))),
        ),
        CoreState::NotApplicable(r) => (
            "Not applicable".into(),
            Some(r.clone()),
            Some(format!("Not applicable ({r})")),
        ),
        CoreState::WaitingUntil(d) => (
            format!("Waiting until {d}"),
            None,
            Some(format!("Waiting until {d}")),
        ),
        CoreState::Undecided => {
            // AC-F004-04: "Decide <condition> to continue"; the note keeps the full `x = undecided`.
            let cs = undecided();
            let note = cs.as_ref().map(|cs| {
                cs.iter()
                    .map(|c| format!("{c} = undecided"))
                    .collect::<Vec<_>>()
                    .join(", ")
            });
            let names = cs.map_or_else(
                || "its condition".to_owned(),
                |cs| {
                    cs.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                },
            );
            (
                "Undecided".into(),
                note,
                Some(format!("Decide {names} to continue")),
            )
        }
        CoreState::NeedsAnnotation => (
            "Needs annotation".into(),
            None,
            Some("This step has no annotation in the blueprint, so Foreman can't run it.".into()),
        ),
        CoreState::NeedsNewerForeman => (
            "Needs a newer Foreman".into(),
            None,
            Some("The blueprint uses grammar this Foreman doesn't know. Update Foreman.".into()),
        ),
    }
}

/// The current branch (`symbolic-ref -q HEAD`), or `detached`/empty.
#[must_use]
pub fn branch(app: &App, root: &Path) -> String {
    let Ok(ctx) = app.spawn_ctx_now() else {
        return String::new();
    };
    match foreman_sys::git::run(
        root,
        &foreman_core::git::GitOp::SymbolicRefHead,
        &foreman_sys::git::GitOpts::default(),
        &ctx,
    ) {
        Ok(out) if out.ok() => out
            .text()
            .strip_prefix("refs/heads/")
            .map(escape_visible)
            .unwrap_or_default(),
        Ok(_) => "detached".to_owned(),
        Err(_) => String::new(),
    }
}

/// `project_open`'s view after a mutation (no git call for the branch: it doesn't change).
///
/// # Errors
/// As [`open`].
pub fn reopen_view(app: &App, id: &str) -> Result<ProjectView, IpcError> {
    let p = open(app, id)?;
    let b = p
        .root
        .as_ref()
        .map(|r| branch(app, r.path()))
        .unwrap_or_default();
    Ok(p.view(app, b))
}
