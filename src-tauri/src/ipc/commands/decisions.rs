//! Decisions and recovery (`docs/contracts/ipc.md` §4.9; F-022, F-020; tickets M2-24, M3-16,
//! M4-17). Accepting an outside change is a native confirmation (SR-24a, SR-40); rejecting writes
//! the approved content back by compare-and-swap (DR-152, SR-33). Both work on a read-only
//! project (IPC-R9).

use foreman_core::decisions::{Decisions, OwnChange};
use foreman_core::pathpolicy::{self, ReadClass, WritePurpose};
use foreman_core::schema::progress::{DecisionFileKind, PendingOutside};
use foreman_core::schema::{self, progress::ProgressFile};
use foreman_core::store::{ReadStamp, sha256_hex};

use crate::ipc::IpcError;
use crate::ipc::dto::app::ProjectView;
use crate::ipc::dto::gates::GateView;
use crate::ipc::dto::records::{
    ChangeContext, ChangeEffect, Consequence, ConsequenceGlyph, PendingChange, RebuildPreview,
};
use crate::ipc::dto::requests::{
    ItemKindConfirmReq, PendingChangeAcceptReq, PendingChangeRejectReq, PendingChangesListReq,
    ProgressRebuildApplyReq, ProgressRebuildPreviewReq,
};
use crate::ipc::error::StaleWhat;
use crate::ipc::types::{ChangeId, PreviewId, RedactedDiff, RedactedEntry, Sha256, Timestamp};
use crate::os::Confirm;
use crate::preview::Preview;
use crate::project::{self, OpenMode, Opened};
use crate::render::escape_visible;
use crate::state::App;
use crate::state::PREVIEW_TTL;

fn effect(p: &PendingOutside) -> ChangeEffect {
    let path_keys = [
        "conditions",
        "skips",
        "addedSteps",
        "addedItems",
        "items",
        "sourceSha256",
    ];
    if p.diff.iter().any(|e| {
        path_keys
            .iter()
            .any(|k| e.key_path == *k || e.key_path.starts_with(&format!("{k}.")))
    }) {
        ChangeEffect::ChangesPath
    } else {
        ChangeEffect::NoEffect
    }
}

fn pending_view(p: &Opened, x: &PendingOutside) -> PendingChange {
    let file = x.file.repo_path();
    let keys: Vec<String> = x.diff.iter().map(|e| escape_visible(&e.key_path)).collect();
    let approved = match x.file {
        DecisionFileKind::Project => &p.approvals.files.project,
        DecisionFileKind::Overlay => &p.approvals.files.overlay,
        DecisionFileKind::BlueprintLock => &p.approvals.files.blueprint_lock,
    };
    let short = |h: &Option<String>| {
        h.as_deref()
            .map_or_else(|| "none".to_owned(), |h| h.chars().take(8).collect())
    };
    let eff = effect(x);
    let mut consequences = vec![Consequence {
        glyph: ConsequenceGlyph::Info,
        text: format!("Foreman keeps using the approved {file} until you accept."),
    }];
    if eff == ChangeEffect::ChangesPath {
        consequences.push(Consequence {
            glyph: ConsequenceGlyph::Lock,
            text: "Accepting changes which steps apply.".to_owned(),
        });
    }
    PendingChange {
        id: ChangeId::from_trusted(x.id.as_str()),
        file: x.file.into(),
        detected_at: Timestamp(x.detected_at.clone()),
        seen_sha256: Sha256::from_trusted(x.seen_sha256.as_str()),
        diff: RedactedDiff {
            source: file.to_owned(),
            entries: x.diff.iter().map(RedactedEntry::from).collect(),
        },
        effect: eff,
        consequences,
        context: ChangeContext {
            changed: format!(
                "{file} changed outside Foreman ({}).",
                if keys.is_empty() {
                    "formatting only".to_owned()
                } else {
                    keys.join(", ")
                }
            ),
            git: format!(
                "Last committed by Foreman: {}",
                short(&approved.committed_sha256)
            ),
            approved: format!("Approved copy: {}", short(&approved.approved_sha256)),
        },
    }
}

/// `pending_changes_list` (P1).
///
/// # Errors
/// `NotFound`, `Io`.
pub fn pending_changes_list(
    app: &App,
    req: PendingChangesListReq,
) -> Result<Vec<PendingChange>, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    Ok(p.progress
        .pending_outside
        .iter()
        .map(|x| pending_view(&p, x))
        .collect())
}

/// The pending change with this ID and the repo file's current bytes, checked against what the
/// review showed (DR-152: a file that changed again refreshes the review).
fn current(
    p: &Opened,
    id: &str,
    seen: &str,
) -> Result<(PendingOutside, Vec<u8>, ReadStamp), IpcError> {
    let x = p
        .progress
        .pending_outside
        .iter()
        .find(|x| x.id == id)
        .cloned()
        .ok_or_else(|| IpcError::not_found("That change isn't pending any more."))?;
    let rel = pathpolicy::check_read(x.file.repo_path(), ReadClass::Decision)?;
    let (bytes, stamp) = p.root()?.read(&rel, ReadClass::Decision)?;
    if sha256_hex(&bytes) != seen || x.seen_sha256 != seen {
        let fresh: Vec<PendingChange> = p
            .progress
            .pending_outside
            .iter()
            .map(|y| pending_view(p, y))
            .collect();
        return Err(IpcError::stale(
            StaleWhat::ReadStamp,
            format!(
                "{} changed again. Review the new change.",
                x.file.repo_path()
            ),
            serde_json::to_value(fresh).ok(),
        ));
    }
    Ok((x, bytes, stamp))
}

/// `pending_change_accept` (P1): after Billy's native confirmation, the repo file becomes the
/// approved copy (and, before Stage 5 is signed, is committed by plumbing; DR-153). Cancel leaves
/// the approved copy, its hash and the pending change exactly as they were.
///
/// # Errors
/// `Cancelled`, `Stale` (the file changed again), `NotFound`, `ToolFailed` (commit).
pub fn pending_change_accept(
    app: &App,
    req: PendingChangeAcceptReq,
) -> Result<ProjectView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    let (x, bytes, _) = current(&p, req.change.as_str(), req.seen_sha256.as_str())?;
    let keys: Vec<String> = x.diff.iter().map(|e| escape_visible(&e.key_path)).collect();
    let ok = app.os().confirm(&Confirm::new(
        format!("Accept this change to {}?", x.file.repo_path()),
        format!(
            "It changes: {}.\nForeman will use the file as it is now.",
            if keys.is_empty() {
                "formatting only".to_owned()
            } else {
                keys.join(", ")
            }
        ),
        "Accept change",
    ));
    if !ok {
        return Err(IpcError::cancelled());
    }
    let stage5 = p.signed().contains(&5);
    let mut commit_error = None;
    {
        let root = p.root()?;
        let d = Decisions {
            project: root,
            app: &p.app_root,
            project_id: &p.entry.project_id,
        };
        d.approve(x.file, &bytes, false)
            .map_err(|e| IpcError::io(e.to_string()))?;
        if !stage5 {
            let sha = sha256_hex(&bytes);
            let change = OwnChange::CommitNow {
                file: x.file,
                path: x.file.repo_path().to_owned(),
                sha256: sha.clone(),
            };
            let ctx = app.spawn_ctx_now()?;
            let opts = foreman_sys::git::commit::CommitOpts {
                app_data: app.paths().app_data.clone(),
                ..Default::default()
            };
            match foreman_sys::decisions::commit_own(root, &change, &opts, &ctx) {
                Ok(_) => {
                    let _ = d.mark_committed(x.file, &sha);
                }
                Err(e) => commit_error = Some(e.to_string()),
            }
        }
    }
    p.progress.pending_outside.retain(|y| y.id != x.id);
    p.save_progress(app)?;
    if let Some(e) = commit_error {
        return Err(IpcError::tool_failed(
            "git",
            None,
            false,
            escape_visible(&e),
        ));
    }
    project::reopen_view(app, req.project_id.as_str())
}

/// `pending_change_reject` (P1): writes the approved content back by CAS (DR-152); a newer edit
/// is a `Conflict` and the review repeats.
///
/// # Errors
/// `Stale`, `Conflict`, `NotFound`.
pub fn pending_change_reject(
    app: &App,
    req: PendingChangeRejectReq,
) -> Result<ProjectView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    let (x, _, stamp) = current(&p, req.change.as_str(), req.seen_sha256.as_str())?;
    {
        let root = p.root()?;
        let d = Decisions {
            project: root,
            app: &p.app_root,
            project_id: &p.entry.project_id,
        };
        let (approved, _) = d
            .approved(x.file)
            .map_err(|e| IpcError::io(e.to_string()))?;
        let dp = foreman_core::decisions::decision_path(x.file);
        let rel = pathpolicy::check_write(&dp.path(), WritePurpose::DecisionFile(dp))?;
        root.write(&rel, &approved, &stamp).map_err(|e| match e {
            foreman_core::store::StoreError::Conflict { name, .. } => {
                IpcError::conflict(&name, Vec::new())
            }
            other => other.into(),
        })?;
    }
    p.progress.pending_outside.retain(|y| y.id != x.id);
    p.save_progress(app)?;
    project::reopen_view(app, req.project_id.as_str())
}

/// `item_kind_confirm` (M4-17, DR-157, AC-F001-17): re-confirms a drifted item's kind in a native
/// dialog; built with the gate commands.
///
/// # Errors
/// As [`crate::ipc::commands::gates::item_kind_confirm`].
pub fn item_kind_confirm(app: &App, req: ItemKindConfirmReq) -> Result<GateView, IpcError> {
    crate::ipc::commands::gates::item_kind_confirm(app, req)
}

/// What `progress_rebuild_apply` writes, fixed at preview time (IPC-R7).
#[derive(Debug, Clone)]
pub struct RebuildPlan {
    /// The project.
    pub project_id: String,
    /// The rebuilt `progress.json`.
    pub progress: ProgressFile,
    /// The stamp of `progress.json` the preview read (`Absent` when there was none).
    pub stamp: ReadStamp,
    /// The file couldn't be read: it is kept as `progress.json.corrupt-<timestamp>`.
    pub unreadable: bool,
}

/// What a rebuild can't recover: it exists only in app data (files.md §4.3 "Rebuild").
const LOST: [&str; 6] = [
    "Session chains: earlier Claude Code sessions can't be resumed from Foreman (their transcripts stay where Claude Code keeps them).",
    "Run inputs and remembered values.",
    "Model, effort and mode overrides, and optional parts you turned off.",
    "Started and copied markers on steps of stages that aren't signed, and waits.",
    "Signature confirmations stay (they're in approvals); a stage signed outside Foreman whose confirmation is gone asks again.",
    "The run log is kept as it is.",
];

/// `progress_rebuild_preview` (P1): what Rebuild from repo would make of this project's state
/// (NFR-17, AC-F020-11, AC-F020-12). Works on an unreadable `progress.json` and on a read-only
/// project (IPC-R9); writes nothing.
///
/// # Errors
/// `NotFound`, `Io`.
pub fn progress_rebuild_preview(
    app: &App,
    req: ProgressRebuildPreviewReq,
) -> Result<RebuildPreview, IpcError> {
    let id = req.project_id.as_str();
    let mut p = project::open_with(app, id, OpenMode::Rebuild)?;
    let seen = app.inner().summaries.get(id).map_or(0, |s| s.rev.0);
    let rev = p.progress.rev.max(seen).wrapping_add(1);
    let mut rebuilt = p.rebuilt_progress(app, rev);
    rebuilt
        .pending_outside
        .clone_from(&p.progress.pending_outside);
    let stamp = p.progress_stamp().clone();
    let unreadable = p.progress_unreadable;
    p.progress = rebuilt.clone();
    let stages = p.stage_views(app);
    let mut lost: Vec<String> = LOST.iter().map(|s| (*s).to_owned()).collect();
    if unreadable {
        lost.insert(
            0,
            "The unreadable progress file isn't deleted: it's kept next to the new one as progress.json.corrupt-<time>.".to_owned(),
        );
    }
    let preview_id = app.inner().previews.hold(
        Preview::Rebuild(Box::new(RebuildPlan {
            project_id: id.to_owned(),
            progress: rebuilt,
            stamp,
            unreadable,
        })),
        PREVIEW_TTL,
    );
    Ok(RebuildPreview {
        preview_id: PreviewId::from_trusted(preview_id),
        stages,
        lost,
    })
}

/// `progress_rebuild_apply` (P1): writes the previewed state after Billy reviewed it (AC-F020-12).
/// An unreadable file is first kept as `progress.json.corrupt-<timestamp>`; the new file replaces
/// it by compare-and-swap against the stamp the preview read, so a change in between is a
/// conflict (SR-33).
///
/// # Errors
/// `Expired`, `Conflict`, `Io`.
pub fn progress_rebuild_apply(
    app: &App,
    req: ProgressRebuildApplyReq,
) -> Result<ProjectView, IpcError> {
    let _w = app.writing();
    let plan = match app.inner().previews.take(req.preview_id.as_str())? {
        Preview::Rebuild(p) => p,
        _ => return Err(IpcError::expired()),
    };
    let root = app.app_root()?;
    let rel = pathpolicy::check_app_data(&project::progress_path(&plan.project_id))?;
    let (bytes, now_stamp) = root.read_opt(&rel, ReadClass::AppState)?;
    if now_stamp != plan.stamp {
        return Err(IpcError::conflict(rel.as_str(), Vec::new()));
    }
    if plan.unreadable
        && let Some(bytes) = bytes
    {
        let ts = jiff::Timestamp::from_second(app.os().now().as_second()).map_or_else(
            |_| "unknown".to_owned(),
            |t| t.strftime("%Y%m%dT%H%M%SZ").to_string(),
        );
        let keep = pathpolicy::check_app_data(&format!("{rel}.corrupt-{ts}"))?;
        root.write(&keep, &bytes, &ReadStamp::Absent)?;
    }
    let text = schema::to_file(&plan.progress).map_err(|e| IpcError::io(e.to_string()))?;
    root.write(&rel, text.as_bytes(), &plan.stamp)?;
    app.project_changed(&plan.project_id, plan.progress.rev);
    project::reopen_view(app, &plan.project_id)
}
