//! Chat (`docs/contracts/ipc.md` §4.5; F-012 P1; tickets M3-23, M3-24): Copy and open, Link this
//! chat, and the two-phase paste-back.
//!
//! - Copy and open (design doc §3.5.3): the resolved prompt is checked (SR-28), sanitized onto the
//!   clipboard, and Rust opens the Project link (new conversation) or the chain's conversation
//!   link (continuing step) through `foreman-sys`'s `open`, only after the stored URL passes the
//!   SR-25 allowlist again (a `project.json` edited outside Foreman can't redirect it, T-100).
//! - Paste-back: Rust reads the clipboard (text, ≤ 1 MB, never logged; SR-56), plans the writes
//!   from the step's declared paths only (`foreman_core::pasteback`), checks each with the write
//!   policy, and shows every target with its diff before anything is written (SR-35). Apply
//!   writes by compare-and-swap against the stamps read at preview time (SR-33, IPC-R10).
//!   Pasted text never reaches the webview as HTML (SR-26): diffs are escaped spans.

use std::time::Instant;

use foreman_core::ids::{StepKey as CoreKey, Surface as CoreSurface};
use foreman_core::pasteback::{self, PasteError, PasteInput, PasteKind as CorePaste, Role};
use foreman_core::pathpolicy::{self, ReadClass, WritePurpose};
use foreman_core::policy;
use foreman_core::rules::Applicability as CoreApp;
use foreman_core::sanitize;
use foreman_core::schema::progress::{DecisionFileKind, StoredState};
use foreman_core::schema::run::{
    EntryPoint as RunEntry, IndexStatus, RunFile, RunKind, RunStatus as FileStatus, RunSurface,
    Timings,
};
use foreman_core::steps::StepEvent;
use foreman_core::store::{ReadStamp, StoreError, sha256_hex};
use foreman_core::template::Step;
use foreman_core::template::grammar::Session;
use foreman_core::urls::{self, OpenUrl};

use crate::diff::diff;
use crate::ipc::IpcError;
use crate::ipc::dto::events::{Event, PacketChanged};
use crate::ipc::dto::requests::{
    ChatCopyAndOpenReq, ChatLinkSetReq, LinkSource, PasteBackApplyReq, PasteBackPreviewReq,
    PasteSource,
};
use crate::ipc::dto::steps::{
    ChatCopyResult, MissingLink, PacketFields, PasteApplyResult, PastePreview,
};
use crate::ipc::error::ClipboardProblem;
use crate::ipc::types::{
    EntryPoint, FixAction, PasteKind, PreviewId, Problem, ProjectId, RepoPath, Rev, RunId, Stage,
};
use crate::preview::Preview;
use crate::project::{self, Opened};
use crate::render::escape_visible;
use crate::runs;
use crate::state::{App, PREVIEW_TTL};

use super::steps::{apply_event, resolve};

/// Where Copy and open goes for a step (AC-F012-03, -04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatTarget {
    /// The checked URL to open.
    Open(OpenUrl),
    /// Nothing to open; the link Billy should set.
    Missing(MissingLink),
}

/// The chain start a chat step continues, or `None` when it starts its own conversation: a
/// `session=new` step, or a continuing step whose chain start doesn't apply (DR-113: it starts
/// fresh, with context).
fn continued_start<'a>(p: &Opened, s: &'a Step) -> Option<&'a CoreKey> {
    if s.session != Session::Continue {
        return None;
    }
    let start = s.chain.as_ref()?;
    let applies = p
        .path
        .as_ref()
        .and_then(|path| path.step(start.as_str()))
        .is_some_and(|a| matches!(a, CoreApp::Applicable));
    applies.then_some(start)
}

/// AC-F012-03/-04, SR-25: the Project link for a new conversation, the chain's conversation link
/// for a continuing step. A stored link that fails the allowlist counts as missing.
#[must_use]
pub fn chat_target(p: &Opened, s: &Step) -> ChatTarget {
    let Some(project) = p.project.as_ref() else {
        return ChatTarget::Missing(MissingLink::Project);
    };
    let (stored, missing) = match continued_start(p, s) {
        Some(start) => (
            project.chat_urls.get(start).cloned(),
            MissingLink::Conversation,
        ),
        None => (project.chat_project_url.clone(), MissingLink::Project),
    };
    match stored.as_deref().map(urls::foreman_url) {
        Some(Ok(u)) => ChatTarget::Open(u),
        _ => ChatTarget::Missing(missing),
    }
}

/// Which link a chat step uses: the Project link (a new conversation) or the chain's
/// conversation (a continuing step).
#[must_use]
pub fn chat_target_kind(p: &Opened, s: &Step) -> MissingLink {
    if continued_start(p, s).is_some() {
        MissingLink::Conversation
    } else {
        MissingLink::Project
    }
}

/// A short display form of a chat link (`claude.ai/chat/7c1e…f04`).
#[must_use]
pub fn short_link(url: &str) -> String {
    let bare = url.strip_prefix("https://").unwrap_or(url);
    let chars: Vec<char> = bare.chars().collect();
    let Some(slash) = bare.rfind('/') else {
        return bare.to_owned();
    };
    let last: Vec<char> = bare[slash + 1..].chars().collect();
    if last.len() <= 12 {
        return chars.iter().collect();
    }
    let head: String = last[..4].iter().collect();
    let tail: String = last[last.len() - 3..].iter().collect();
    format!("{}{head}…{tail}", &bare[..=slash])
}

fn run_entry(e: EntryPoint) -> RunEntry {
    match e {
        EntryPoint::DoNext => RunEntry::DoNext,
        EntryPoint::Palette => RunEntry::Palette,
        EntryPoint::StageView => RunEntry::StageView,
    }
}

/// `chat_copy_and_open` (P1, ⌘↵ on a chat step): copies the resolved, sanitized prompt, opens the
/// right claude.ai link in the default browser, marks the step `copied`, and writes a run record
/// (kind `chatCopy`, prompt hash only, time to start; SR-52, AC-F026-02).
///
/// # Errors
/// `NotFound`, `ReadOnly`, `NotAllowed("not-a-chat-step" | "not-runnable" | "unresolved-tokens" |
/// "prompt-control-char")`, `Io`.
pub fn chat_copy_and_open(app: &App, req: ChatCopyAndOpenReq) -> Result<ChatCopyResult, IpcError> {
    let started = Instant::now();
    let wall_start = app.now();
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let s = p.step(req.step.as_str())?.clone();
    if s.surface != Some(CoreSurface::Chat) {
        return Err(IpcError::not_allowed(
            "not-a-chat-step",
            "Copy and open works on a chat step.",
        ));
    }
    let r = resolve(&p, &s)?;
    let Ok(resolved) = r.result else {
        return Err(IpcError::not_allowed(
            "unresolved-tokens",
            "Fill in what the prompt still needs first.",
        ));
    };
    if let Err(sanitize::Blocker::PromptControlChar { .. }) = sanitize::check_prompt(&resolved.text)
    {
        return Err(IpcError::not_allowed(
            "prompt-control-char",
            "The prompt has a control character. Fix the input it came from.",
        ));
    }
    // The step must accept a copy (not applicable, undecided, waiting … refuse with the reason).
    let key = s.key.clone();
    let ctx = p.step_ctx(app, &s);
    let cur = p.step_progress(key.as_str());
    let mut next = foreman_core::steps::transition(&cur, &ctx, &StepEvent::Copy)
        .map_err(|r| IpcError::not_allowed("not-runnable", escape_visible(&r.0)))?;
    let (clean, report) = sanitize::for_paste(&resolved.text);
    app.os()
        .clipboard_write(&clean)
        .map_err(|_| IpcError::io("The clipboard refused the prompt."))?;
    let target = chat_target(&p, &s);
    let (opened, missing_link) = match &target {
        ChatTarget::Open(url) => {
            let ctx = app.spawn_ctx_now()?;
            (app.os().open_url(url, &ctx).is_ok(), None)
        }
        ChatTarget::Missing(m) => (false, Some(*m)),
    };
    let time_to_start = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let run_id = uuid::Uuid::new_v4().hyphenated().to_string();
    // The run record (SR-52: the prompt's hash, never its text).
    let (_, _, pf) = p.parts()?;
    let over = p.progress.overrides.get(&key).copied();
    let intended = policy::effective_start(
        &s,
        &pf.model_policy,
        over.map(|o| policy::RunOverride {
            model: o.model,
            effort: o.effort,
            mode: o.mode,
        })
        .as_ref(),
    )
    .ok()
    .map(|e| e.value);
    let file = RunFile {
        schema_version: 1,
        run_id: run_id.clone(),
        project_id: p.id().to_owned(),
        stage: s.stage,
        step_id: key.clone(),
        surface: RunSurface::Chat,
        kind: RunKind::ChatCopy,
        chain_id: None,
        name: None,
        argv: Vec::new(),
        env_names: Vec::new(),
        cwd: None,
        prompt_sha256: Some(sha256_hex(clean.as_bytes())),
        run_settings_sha256: None,
        intended,
        r#override: over.is_some(),
        observed: None,
        entry_point: run_entry(req.entry),
        t: Timings {
            wall_start,
            look_start: Some(0),
            prepared: Some(0),
            confirmed: None,
            spawned: Some(time_to_start),
            exited: None,
        },
        time_to_start_ms: Some(time_to_start),
        run_input_interventions: 0,
        copy_prompt_on_cli: false,
        status: FileStatus::Copied,
        exit_code: None,
        claude_version: None,
        pid: None,
        sensor_token: None,
    };
    runs::write_run(&p.app_root, &file, &ReadStamp::Absent)?;
    runs::append_index(&p.app_root, &file, IndexStatus::Copied);
    next.last_run_id = Some(run_id.clone());
    // DR-103: an override is for the next run only.
    p.progress.overrides.remove(&key);
    p.progress.steps.insert(key, next);
    p.save_progress(app)?;
    Ok(ChatCopyResult {
        run_id: RunId::from_trusted(run_id),
        opened,
        missing_link,
        removed_controls: u32::try_from(report.removed).unwrap_or(u32::MAX),
    })
}

/// `chat_link_set` (P1, Link this chat): stores a chain's conversation URL in `project.json`
/// `chatUrls` (a variable field, DR-151), from the clipboard on Billy's click or from text. Only
/// `https://claude.ai/…` is accepted (SR-25, AC-F012-02).
///
/// # Errors
/// `Clipboard`, `Invalid` (not `https://claude.ai/…`, or not a chat chain start), `NotFound`,
/// `ReadOnly`, `Conflict`.
pub fn chat_link_set(app: &App, req: ChatLinkSetReq) -> Result<Rev, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    // A continuing chat step links its chain's conversation (the chain start's key).
    let given = p.step(req.chain_start.as_str())?.clone();
    let start = given.chain.clone().unwrap_or_else(|| given.key.clone());
    let s = p.step(start.as_str())?.clone();
    if given.surface != Some(CoreSurface::Chat)
        || s.surface != Some(CoreSurface::Chat)
        || s.session != Session::New
    {
        return Err(IpcError::invalid("chainStart", "chatChainStart"));
    }
    let raw = match req.source {
        LinkSource::Clipboard(_) => app.os().clipboard_read().map_err(IpcError::clipboard)?,
        LinkSource::Text(t) => t.url.into_string(),
    };
    let url = urls::chat_url(raw.trim()).map_err(|_| IpcError::invalid("url", "claudeUrl"))?;
    let mut pf = p.parts()?.2.clone();
    pf.chat_urls.insert(s.key.clone(), url.as_str().to_owned());
    p.own_change(app, DecisionFileKind::Project, &pf)?;
    Ok(Rev(p.progress.rev))
}

/// One planned write, with the stamp read at preview time.
#[derive(Debug, Clone)]
pub struct HeldTarget {
    /// The checked path.
    pub path: RepoPath,
    /// What it is.
    pub role: Role,
    /// The exact bytes.
    pub text: String,
    /// The file as read at preview time.
    pub stamp: ReadStamp,
}

/// A paste-back held between preview and apply (IPC-R7).
#[derive(Debug)]
pub struct HeldPaste {
    /// Project.
    pub project: String,
    /// Step.
    pub step: String,
    /// Packet or output.
    pub kind: CorePaste,
    /// The packet's stage.
    pub stage: Option<u8>,
    /// Every write, in order.
    pub targets: Vec<HeldTarget>,
    /// The paste is a valid packet or an output (the step becomes done, AC-F012-12).
    pub valid: bool,
}

fn paste_error(e: &PasteError) -> IpcError {
    let rule = match e {
        PasteError::Empty => return IpcError::clipboard(ClipboardProblem::Empty),
        PasteError::NoPacket => "no-packet",
        PasteError::TwoPackets(_) => "two-packets",
        PasteError::WrongStage { .. } => "wrong-stage-packet",
        PasteError::NotAPacketStep => "not-a-packet-step",
        PasteError::AppendsInP1(_) => "appends-in-p1",
        PasteError::NoTarget => "no-output",
    };
    IpcError::not_allowed(rule, e.to_string())
}

/// `paste_back_preview` (P1: packets and outputs): every target with its diff, the packet's
/// problems and the normalization notes, under a one-time preview ID (AC-F012-06…-10, -13, -14).
///
/// # Errors
/// `Clipboard` (empty, not text, over 1 MB), `Unsupported` (`sensor`, ADR set, lessons: P2),
/// `NotAllowed("wrong-stage-packet" | "two-packets" | "appends-in-p1" | "no-packet" |
/// "not-a-packet-step" | "no-output")`, `PolicyViolation`, `ReadOnly`, `NotFound`.
pub fn paste_back_preview(app: &App, req: PasteBackPreviewReq) -> Result<PastePreview, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let s = p.step(req.step.as_str())?.clone();
    let kind = match req.kind {
        PasteKind::Auto => CorePaste::Auto,
        PasteKind::Packet => CorePaste::Packet,
        PasteKind::Output => CorePaste::Output,
        PasteKind::AdrSet => {
            return Err(IpcError::not_yet_available(
                "paste_back_preview.adrSet",
                "M4-24",
            ));
        }
        PasteKind::Lessons => {
            return Err(IpcError::not_yet_available(
                "paste_back_preview.lessons",
                "M4-18",
            ));
        }
    };
    if req.source == PasteSource::Sensor {
        return Err(IpcError::unsupported(
            "sensor",
            "noSensor",
            "Capture from the session needs the session sensor, which this build doesn't have. Copy the reply and paste it back.",
        ));
    }
    let text = app.os().clipboard_read().map_err(IpcError::clipboard)?;
    let (t, _, _) = p.parts()?;
    let stage = pasteback::packet_stage_of(&s.out);
    let format = stage.and_then(|n| t.packet_format(n));
    let planned = pasteback::plan(
        &PasteInput {
            text: &text,
            kind,
            out: &s.out,
            appends: s.appends.as_deref(),
            format,
        },
        // DR-131 in full (M3-20): fence, BOM, line ends, NBSP, smart quotes, header variants,
        // unsigned `APPROVED BY` forms and `none`, each with its note.
        &foreman_core::packet::normalize_paste,
    )
    .map_err(|e| paste_error(&e))?;
    let root = p.root()?;
    let mut held = Vec::new();
    let mut diffs = Vec::new();
    for target in &planned.targets {
        let rel = pathpolicy::check_write(&target.path, WritePurpose::Repo)?;
        let (existing, stamp) = root.read_opt(&rel, ReadClass::Decision)?;
        let old = existing.map(|b| String::from_utf8_lossy(&b).into_owned());
        diffs.push(diff(rel.as_str(), old.as_deref(), &target.text));
        held.push(HeldTarget {
            path: RepoPath::from_trusted(rel.as_str()),
            role: target.role,
            text: target.text.clone(),
            stamp,
        });
    }
    let problems: Vec<Problem> = match (&planned.packet, stage) {
        (Some(text), Some(n)) => p
            .packet_problems(n, text)
            .iter()
            .map(|x| Problem {
                rule: x.rule.clone(),
                line: Some(x.line),
                message: escape_visible(&x.message),
                fix: Some(vec![FixAction::OpenInEditor, FixAction::CopyFixRequest]),
            })
            .collect(),
        _ => Vec::new(),
    };
    let preview_kind = if planned.kind == CorePaste::Packet {
        PasteKind::Packet
    } else {
        PasteKind::Output
    };
    let id = app.inner().previews.hold(
        Preview::Paste(Box::new(HeldPaste {
            project: req.project_id.as_str().to_owned(),
            step: req.step.as_str().to_owned(),
            kind: planned.kind,
            stage,
            targets: held,
            valid: problems.is_empty(),
        })),
        PREVIEW_TTL,
    );
    Ok(PastePreview {
        preview_id: PreviewId::from_trusted(id),
        kind: preview_kind,
        targets: diffs,
        problems,
        notes: planned.notes.iter().map(|n| escape_visible(n)).collect(),
        packet_fields: planned.header.map(|(from, to)| PacketFields {
            from_stage: Stage(from),
            to_stage: to.map(Stage),
        }),
    })
}

/// `paste_back_apply` (P1): writes the previewed targets by compare-and-swap, in order. Every
/// stamp is checked before the first write, so a target changed since the preview writes nothing
/// (AC-F012-08); a change racing the writes reports exactly what was written (IPC-R10). A valid
/// paste-back makes the step `done` (AC-F012-12).
///
/// # Errors
/// `Expired`, `Conflict` (with `written`), `PolicyViolation`, `ReadOnly`.
pub fn paste_back_apply(app: &App, req: PasteBackApplyReq) -> Result<PasteApplyResult, IpcError> {
    let _w = app.writing();
    let held = match app.inner().previews.take(req.preview_id.as_str())? {
        Preview::Paste(h) => h,
        _ => return Err(IpcError::expired()),
    };
    let mut p = project::open(app, &held.project)?;
    p.writable()?;
    let mut written: Vec<RepoPath> = Vec::new();
    {
        let root = p.root()?;
        for t in &held.targets {
            let rel = pathpolicy::check_write(t.path.as_str(), WritePurpose::Repo)?;
            let (_, now) = root.read_opt(&rel, ReadClass::Decision)?;
            if now != t.stamp {
                return Err(IpcError::conflict(t.path.as_str(), Vec::new()));
            }
        }
        for t in &held.targets {
            let rel = pathpolicy::check_write(t.path.as_str(), WritePurpose::Repo)?;
            match root.write(&rel, t.text.as_bytes(), &t.stamp) {
                Ok(_) => written.push(t.path.clone()),
                Err(StoreError::Conflict { .. }) => {
                    return Err(IpcError::conflict(t.path.as_str(), written));
                }
                Err(e) => {
                    if written.is_empty() {
                        return Err(e.into());
                    }
                    let mut err: IpcError = e.into();
                    err.message = format!(
                        "{} Already written: {}.",
                        err.message,
                        written
                            .iter()
                            .map(|w| escape_visible(w.as_str()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    return Err(err);
                }
            }
        }
    }
    let mut step_done = false;
    if held.valid {
        let key = held.step.as_str();
        step_done =
            apply_event(app, &mut p, key, &StepEvent::PasteBackValid, "not-runnable").is_ok();
        // Copied by hand (Copy prompt) rather than Copy and open: the paste-back is the copy.
        if !step_done && p.step_progress(key).state == StoredState::NotStarted {
            step_done = apply_event(app, &mut p, key, &StepEvent::Copy, "not-runnable")
                .and_then(|()| {
                    apply_event(app, &mut p, key, &StepEvent::PasteBackValid, "not-runnable")
                })
                .is_ok();
        }
        step_done = step_done || p.step_progress(key).state == StoredState::Done;
    }
    let rev = p.save_progress(app)?;
    if let (CorePaste::Packet, Some(n)) = (held.kind, held.stage) {
        app.emit(Event::PacketChanged(PacketChanged {
            project_id: ProjectId::from_trusted(&held.project),
            stage: Stage(n),
            rev: Rev(rev),
        }));
    }
    Ok(PasteApplyResult { written, step_done })
}
