//! The knowledge manifest and the build-package assembler (`docs/contracts/ipc.md` §4.13; F-023,
//! F-021; tickets M4-27, M4-28).
//!
//! The manifest lists signed packets and chat outputs (`foreman_core::manifest`); Billy's "in the
//! claude.ai Project" marks live in memory (CR-014). The assembler reads the Stage 4 packet's
//! REPO FILES list (`foreman_core::assembler`): every entry is checked by the write policy before
//! anything else, a refused entry stays a refused row, and Paste into is a two-phase write (the
//! clipboard text and the file's stamp held under a preview ID, then a compare-and-swap write).
//! Root `CLAUDE.md` is written only after the native confirmation Rust raises (AC-F021-06,
//! SR-24a). Late conditions get DR-063 proposals from paths only; recording one is
//! `condition_set` (Billy confirms).

#![allow(
    clippy::needless_pass_by_value,
    reason = "handlers take requests by value and return the IPC Result"
)]

use foreman_core::assembler::{self as core_asm, RepoFile, RowState};
use foreman_core::ids::TriState as CoreTri;
use foreman_core::pathpolicy::{ReadClass, WritePurpose, check_read};
use foreman_core::rules::evidence;
use foreman_core::store::{ReadStamp, StoreError, dirs};

use crate::diff::diff;
use crate::ipc::IpcError;
use crate::ipc::dto::gates::{
    Assembler, AssemblerRow, AssemblerState, LateConditionView, LessonsStatus, Manifest,
    ManifestRow,
};
use crate::ipc::dto::requests::{
    AssemblerPasteApplyReq, AssemblerPastePreviewReq, AssemblerViewReq, LessonsStatusReq,
    ManifestMarkReq, ManifestViewReq,
};
use crate::ipc::dto::steps::PastePreview;
use crate::ipc::types::{ConditionId, PasteKind, PreviewId, RepoPath, StepKey, TriState};
use crate::os::Confirm;
use crate::preview::Preview;
use crate::project::{self, Opened};
use crate::render::escape_visible;
use crate::state::{App, PREVIEW_TTL};

/// `lessons_status` (M4-18): built in [`crate::ipc::commands::lessons`].
///
/// # Errors
/// As [`crate::ipc::commands::lessons::lessons_status`].
pub fn lessons_status(app: &App, req: LessonsStatusReq) -> Result<LessonsStatus, IpcError> {
    crate::ipc::commands::lessons::lessons_status(app, req)
}

// ------------------------------------------------------------------------------ manifest (M4-27)

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The manifest of an open project with Billy's marks.
fn manifest_of(app: &App, p: &Opened) -> Result<Manifest, IpcError> {
    let (t, path, _) = p.parts()?;
    let entries = foreman_core::manifest::entries(t, path, &p.signed());
    let marks = app
        .inner()
        .knowledge
        .get(p.id())
        .cloned()
        .unwrap_or_default();
    let root = p.root.as_ref();
    let files: Vec<ManifestRow> = entries
        .iter()
        .map(|e| {
            let exists = check_read(&e.path, ReadClass::Decision)
                .ok()
                .zip(root)
                .is_some_and(|(rel, r)| {
                    matches!(dirs::entry_kind(r, &rel), Ok(dirs::EntryKind::File(_)))
                });
            ManifestRow {
                path: RepoPath::from_trusted(&e.path),
                exists,
                in_project: marks.contains(&e.path),
                producer: Some(StepKey::from_trusted(e.producer.as_str())),
            }
        })
        .collect();
    let ready = files.iter().filter(|f| f.in_project && f.exists).count();
    let first_producer = if files.is_empty() {
        foreman_core::manifest::first_producer(t, path).map(|k| StepKey::from_trusted(k.as_str()))
    } else {
        None
    };
    Ok(Manifest {
        ready: to_u32(ready),
        total: to_u32(files.len()),
        files,
        first_producer,
    })
}

/// `manifest_view` (AC-F023-01, -02): every signed packet and declared chat output with
/// whether it exists, Billy's mark and the step that produces it; with none, the first step that
/// will produce one.
///
/// # Errors
/// `NotFound` (project), `ReadOnly` (no template).
pub fn manifest_view(app: &App, req: ManifestViewReq) -> Result<Manifest, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    manifest_of(app, &p)
}

/// `manifest_mark` (AC-F023-01): marks or unmarks a manifest file as in the claude.ai Project.
/// Only a file the manifest lists can be marked.
///
/// # Errors
/// `NotFound` (project, or a path the manifest doesn't list), `ReadOnly`.
pub fn manifest_mark(app: &App, req: ManifestMarkReq) -> Result<Manifest, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    let before = manifest_of(app, &p)?;
    if !before.files.iter().any(|f| f.path == req.path) {
        return Err(IpcError::not_found(
            "That file isn't in the knowledge manifest.",
        ));
    }
    {
        let mut inner = app.inner();
        let set = inner.knowledge.entry(p.id().to_owned()).or_default();
        if req.in_project {
            set.insert(req.path.as_str().to_owned());
        } else {
            set.remove(req.path.as_str());
        }
    }
    manifest_of(app, &p)
}

// ----------------------------------------------------------------------------- assembler (M4-28)

/// A held Paste into (`assembler_paste_preview` → `assembler_paste_apply`).
#[derive(Debug)]
pub struct HeldAssembler {
    /// The project.
    pub project: String,
    /// The REPO FILES entry (repo-relative, checked).
    pub path: String,
    /// The text to write (as previewed).
    pub text: String,
    /// The file's stamp at preview time.
    pub stamp: ReadStamp,
}

/// The Stage 4 packet's REPO FILES list.
fn repo_files(p: &Opened) -> Result<Vec<RepoFile>, IpcError> {
    let (t, _, _) = p.parts()?;
    let Some(packet) = p.packets.get(&4) else {
        return Err(IpcError::not_allowed(
            "no-stage-4-packet",
            "There's no Stage 4 packet yet. Run 4.8; its packet lands in docs/handoff/stage-04.md.",
        ));
    };
    let known: Vec<String> = t
        .packet_format(4)
        .map(|f| f.keys.iter().map(|k| k.key.clone()).collect())
        .unwrap_or_default();
    core_asm::repo_files(&packet.text, &known).ok_or_else(|| {
        IpcError::not_allowed(
            "no-repo-files",
            "The Stage 4 packet has no REPO FILES list. Fix the packet, then open the build package again.",
        )
    })
}

/// One row: the policy first, then what is on disk.
fn row(root: &foreman_core::store::Root, f: &RepoFile) -> (AssemblerRow, RowState) {
    let refused = |rule: &str, note: String| {
        (
            AssemblerRow {
                path: escape_visible(&f.path),
                state: AssemblerState::Refused,
                rule: Some(rule.to_owned()),
                note: Some(note),
            },
            RowState::Refused(rule.to_owned()),
        )
    };
    let (rel, _) = match core_asm::target(&f.path) {
        Ok(t) => t,
        Err(v) => return refused(v.rule(), core_asm::refused_note(&v)),
    };
    let state = match dirs::entry_kind(root, &rel) {
        Ok(k) => core_asm::row_state(f, k),
        Err(StoreError::Policy(v)) => return refused(v.rule(), core_asm::refused_note(&v)),
        Err(e) => RowState::Refused(format!("io: {e}")),
    };
    let (st, note) = match &state {
        RowState::Exists => (AssemblerState::Exists, None),
        RowState::Missing => (AssemblerState::Missing, None),
        RowState::Empty => (
            AssemblerState::Empty,
            Some(if f.dir {
                "The folder exists but is empty.".to_owned()
            } else {
                "The file exists but has no content.".to_owned()
            }),
        ),
        RowState::Refused(rule) => {
            return refused(
                if rule.starts_with("io:") {
                    "path.unreadable"
                } else {
                    rule
                },
                "What is at this path isn't something Foreman writes (a symlink, a folder where a file is listed, or unreadable). Fix it yourself.".to_owned(),
            );
        }
    };
    let note = if f.path == "CLAUDE.md" {
        Some(core_asm::CLAUDE_MD_NOTE.to_owned())
    } else {
        note
    };
    (
        AssemblerRow {
            path: escape_visible(&f.path),
            state: st,
            rule: None,
            note,
        },
        state,
    )
}

fn decided_text(p: &Opened, c: foreman_core::ids::ConditionId) -> Option<String> {
    let project = p.project.as_ref()?;
    let a = project
        .condition_audit
        .iter()
        .rev()
        .find(|a| a.condition == c)?;
    let date: String = a.at.chars().take(10).collect();
    Some(if a.note.trim().is_empty() {
        format!("Decided by you on {date}.")
    } else {
        format!(
            "Decided by you on {date} (\"{}\").",
            escape_visible(a.note.trim())
        )
    })
}

/// AC-F023-06 / DR-063: the four late conditions while any is undecided and Stage 4 isn't
/// signed, each undecided one with a proposal from path evidence (REPO FILES first, then the
/// paths in the project). Nothing is recorded here.
fn late_conditions(p: &Opened, list: &[RepoFile]) -> Vec<LateConditionView> {
    let Some(project) = p.project.as_ref() else {
        return Vec::new();
    };
    let value = |c| {
        project
            .conditions
            .get(&c)
            .copied()
            .unwrap_or(CoreTri::Undecided)
    };
    if p.signed().contains(&4)
        || evidence::LATE
            .iter()
            .all(|c| value(*c) != CoreTri::Undecided)
    {
        return Vec::new();
    }
    let mut paths: Vec<String> = list.iter().map(|f| f.path.clone()).collect();
    if let Some(root) = p.root.as_ref() {
        paths.extend(dirs::list_paths(root, 6, 5000));
    }
    let proposals = evidence::late_condition_evidence(&paths, &project.conditions);
    evidence::LATE
        .iter()
        .map(|c| {
            let v = value(*c);
            let prop = proposals.get(c);
            LateConditionView {
                condition: ConditionId::from(*c),
                value: TriState::from(v),
                proposed: prop.map(|x| TriState::from(x.value)),
                evidence: prop.map(|x| escape_visible(&x.evidence)),
                decided: (v != CoreTri::Undecided)
                    .then(|| decided_text(p, *c))
                    .flatten(),
            }
        })
        .collect()
}

/// The assembler view of an open project.
fn view_of(p: &Opened) -> Result<Assembler, IpcError> {
    let list = repo_files(p)?;
    let root = p.root()?;
    let (rows, states): (Vec<AssemblerRow>, Vec<RowState>) =
        list.iter().map(|f| row(root, f)).unzip();
    Ok(Assembler {
        complete: core_asm::complete(&states),
        late_conditions: late_conditions(p, &list),
        rows,
    })
}

/// `assembler_view` (AC-F023-03…-06): the REPO FILES rows (exists, missing, empty or refused
/// with the rule), whether the package is complete (a refused row doesn't block it) and the late
/// conditions with their evidence proposals.
///
/// # Errors
/// `NotFound`, `NotAllowed("no-stage-4-packet" | "no-repo-files")`, `ReadOnly`.
pub fn assembler_view(app: &App, req: AssemblerViewReq) -> Result<Assembler, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    view_of(&p)
}

/// `assembler_paste_preview` (AC-F023-03, -04): the clipboard text as the new content of one
/// REPO FILES row, with its diff against the current file (an overwrite says so). The path must
/// be a row of the list and pass the write policy; nothing is written.
///
/// # Errors
/// `NotFound` (not a REPO FILES row), `PolicyViolation`, `NotAllowed("folder-row")`,
/// `Clipboard`, `ReadOnly`.
pub fn assembler_paste_preview(
    app: &App,
    req: AssemblerPastePreviewReq,
) -> Result<PastePreview, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let list = repo_files(&p)?;
    let Some(entry) = list.iter().find(|f| f.path == req.path.as_str()) else {
        return Err(IpcError::not_found(
            "That path isn't in the Stage 4 packet's REPO FILES list.",
        ));
    };
    let (rel, _) = core_asm::target(&entry.path)
        .map_err(|v| IpcError::policy(&escape_visible(&entry.path), v.rule()))?;
    if entry.dir {
        return Err(IpcError::not_allowed(
            "folder-row",
            format!(
                "{} is a folder. Add its files yourself; Foreman pastes into files only.",
                escape_visible(&entry.path)
            ),
        ));
    }
    let text = app.os().clipboard_read().map_err(IpcError::clipboard)?;
    let (clean, mut notes) = foreman_core::pasteback::prepare(&text);
    let mut new = clean;
    new.push('\n');
    let root = p.root()?;
    let (existing, stamp) = root.read_opt(&rel, ReadClass::Decision)?;
    let old = existing.map(|b| String::from_utf8_lossy(&b).into_owned());
    if old.is_some() {
        notes.push("Overwrites the current file".to_owned());
    }
    if entry.path == "CLAUDE.md" {
        notes.push(core_asm::CLAUDE_MD_NOTE.to_owned());
    }
    let target = diff(rel.as_str(), old.as_deref(), &new);
    let id = app.inner().previews.hold(
        Preview::Assembler(Box::new(HeldAssembler {
            project: req.project_id.as_str().to_owned(),
            path: entry.path.clone(),
            text: new,
            stamp,
        })),
        PREVIEW_TTL,
    );
    Ok(PastePreview {
        preview_id: PreviewId::from_trusted(id),
        kind: PasteKind::Output,
        targets: vec![target],
        problems: Vec::new(),
        notes: notes.iter().map(|n| escape_visible(n)).collect(),
        packet_fields: None,
    })
}

/// The native confirmation before root `CLAUDE.md` is written (AC-F021-06, SR-24a). It shows
/// what will be written (security review 2026-10-20, W-2): the text came from the clipboard,
/// which the webview can set, so the dialog itself carries the hash, size and first lines.
#[must_use]
pub fn claude_md_confirm(text: &str) -> Confirm {
    Confirm::new(
        "Write CLAUDE.md?",
        format!(
            "Claude Code loads this file in every session in this project, so whatever it says \
             steers every Claude Code run here. Foreman writes this text to CLAUDE.md at the repo \
             root:\n\n{}",
            crate::render::content_summary(text)
        ),
        "Write CLAUDE.md",
    )
}

/// `assembler_paste_apply` (AC-F023-03, AC-F021-06): writes the previewed text by
/// compare-and-swap. Root `CLAUDE.md` first asks in a native dialog; Cancel writes nothing. A
/// file changed since the preview is a `Conflict` and nothing is written.
///
/// # Errors
/// `Expired`, `Cancelled`, `Conflict`, `PolicyViolation`, `ReadOnly`.
pub fn assembler_paste_apply(
    app: &App,
    req: AssemblerPasteApplyReq,
) -> Result<Assembler, IpcError> {
    let _w = app.writing();
    let held = match app.inner().previews.take(req.preview_id.as_str())? {
        Preview::Assembler(h) => h,
        _ => return Err(IpcError::expired()),
    };
    let p = project::open(app, &held.project)?;
    p.writable()?;
    // Re-checked at apply: the entry is still a row, and the policy still allows it.
    let list = repo_files(&p)?;
    if !list.iter().any(|f| f.path == held.path && !f.dir) {
        return Err(IpcError::not_found(
            "That path is no longer in the Stage 4 packet's REPO FILES list.",
        ));
    }
    let (rel, purpose) = core_asm::target(&held.path)
        .map_err(|v| IpcError::policy(&escape_visible(&held.path), v.rule()))?;
    let root = p.root()?;
    let (_, now) = root.read_opt(&rel, ReadClass::Decision)?;
    if now != held.stamp {
        return Err(IpcError::conflict(rel.as_str(), Vec::new()));
    }
    if purpose == WritePurpose::AssemblerClaudeMd
        && !app.os().confirm(&claude_md_confirm(&held.text))
    {
        return Err(IpcError::cancelled());
    }
    root.write(&rel, held.text.as_bytes(), &held.stamp)
        .map_err(|e| match e {
            StoreError::Conflict { .. } => IpcError::conflict(rel.as_str(), Vec::new()),
            other => other.into(),
        })?;
    let p = project::open(app, &held.project)?;
    view_of(&p)
}
