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
//! - P2 kinds (M4-18, M4-24) and Capture from session (M4-23): `lessons` appends to
//!   `docs/lessons.md` through `foreman_core::lessons` (`read` for the diff, `append` by CAS);
//!   `adrSet` splits on `^# ADR-\d{3}:` through `foreman_core::packet::adr_split` and lists every
//!   ADR file before any write; `source: "sensor"` takes the text from the step's last run's
//!   `last_assistant_message` (the session sensor's events) and then follows the clipboard path.

use std::time::Instant;

use foreman_core::ids::{StepKey as CoreKey, Surface as CoreSurface};
use foreman_core::lessons::{self, AppendError};
use foreman_core::packet::adr_split::{self, SplitError};
use foreman_core::pasteback::{self, PasteError, PasteInput, PasteKind as CorePaste, Role};
use foreman_core::pathpolicy::{self, ReadClass, WritePurpose};
use foreman_core::policy;
use foreman_core::rules::Applicability as CoreApp;
use foreman_core::sanitize;
use foreman_core::schema::progress::{DecisionFileKind, StoredState};
use foreman_core::schema::run::{EntryPoint as RunEntry, RunKind};
use foreman_core::session_state::SessionState;
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
use crate::ipc::error::{ClipboardProblem, IpcErrorDetail};
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
    // The run record (SR-52: the sanitized prompt's hash, never its text) and its index line,
    // through the run log's one path (M3-26): the chat clock stops here (AC-F026-02), and the
    // step's Mark done or paste-back adds the active time (`runs::note_step_done`).
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
    let run_id = runs::record_chat_copy(
        app,
        &runs::ChatCopy {
            project_id: p.id(),
            stage: s.stage,
            step: key.clone(),
            chain: None,
            entry: run_entry(req.entry),
            kind: RunKind::ChatCopy,
            intended,
            over: over.is_some(),
            prompt_sha256: Some(sha256_hex(clean.as_bytes())),
            time_to_start_ms: time_to_start,
        },
    )?;
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

/// What a held paste-back writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeldKind {
    /// A packet (plus attachments and extra outs).
    Packet,
    /// The step's declared outputs.
    Output,
    /// `docs/adr/ADR-NNN.md` files cut from an ADR-set paste (M4-24).
    AdrSet,
    /// Entries appended to `docs/lessons.md` (M4-18); `lessons::append` re-derives the text
    /// from these entries against the stamp read at preview time.
    Lessons {
        /// The paste (normalized), from its first entry heading on.
        entries: String,
    },
}

/// A paste-back held between preview and apply (IPC-R7).
#[derive(Debug)]
pub struct HeldPaste {
    /// Project.
    pub project: String,
    /// Step.
    pub step: String,
    /// What it writes.
    pub kind: HeldKind,
    /// The packet's stage.
    pub stage: Option<u8>,
    /// Every write, in order.
    pub targets: Vec<HeldTarget>,
    /// The paste is a valid packet, output, lessons append or ADR set (the step becomes done,
    /// AC-F012-12).
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

fn lessons_error(e: AppendError) -> IpcError {
    match e {
        AppendError::Missing => IpcError::not_allowed("lessons-missing", e.to_string()),
        AppendError::NoEntries => IpcError::not_allowed("no-lessons-entries", e.to_string()),
        AppendError::Changed => IpcError::conflict(lessons::PATH, Vec::new()),
        AppendError::Store(s) => s.into(),
    }
}

fn adr_error(e: &SplitError) -> IpcError {
    let rule = match e {
        SplitError::NoAdrs => "no-adrs",
        SplitError::Duplicate(_) => "duplicate-adr",
    };
    IpcError::not_allowed(rule, e.to_string())
}

/// `source: "sensor"` (AC-F013-14, M4-23): the `last_assistant_message` the session sensor
/// recorded for the step's last run (its events in app data, read through
/// `foreman_sys::sensor::ingest` against the run's own token). Without it (no run, a run started
/// without the sensor, no `Stop` event yet) the answer is `Unsupported { feature: "sensor", why:
/// "noSensor" }` and paste-back is offered instead.
fn captured(p: &Opened, s: &Step) -> Result<String, IpcError> {
    let none = |why: &str| {
        IpcError::unsupported(
            "sensor",
            "noSensor",
            format!("{why} Copy the reply and paste it back."),
        )
    };
    let Some(run_id) = p.step_progress(s.key.as_str()).last_run_id else {
        return Err(none(
            "This step hasn't run in Claude Code, so there's no session to capture from.",
        ));
    };
    let no_events = "Foreman has no session events for this step's last run.";
    let Ok((file, _)) = runs::read_run(&p.app_root, &run_id) else {
        return Err(none(no_events));
    };
    let Some(token) = file.sensor_token else {
        return Err(none(no_events));
    };
    let state = foreman_sys::sensor::ingest(&p.app_root, &run_id, &token, SessionState::default())
        .map_err(|_| none(no_events))?;
    state
        .last_assistant_message
        .filter(|m| !m.trim().is_empty())
        .ok_or_else(|| none("Claude Code hasn't finished a reply in this step's last run."))
}

fn hold(app: &App, held: HeldPaste) -> PreviewId {
    PreviewId::from_trusted(
        app.inner()
            .previews
            .hold(Preview::Paste(Box::new(held)), PREVIEW_TTL),
    )
}

/// `kind: "lessons"` (F-016, M4-18; AC-F016-02…-04): the step must append to `docs/lessons.md`;
/// the file is read through `lessons::read` (missing: nothing is written, AC-F016-04) and the
/// diff is the append with each entry renumbered to the next free `L-NNN`, header kept.
fn lessons_preview(
    app: &App,
    p: &Opened,
    s: &Step,
    project_id: &str,
    text: &str,
) -> Result<PastePreview, IpcError> {
    if s.appends.as_deref() != Some(lessons::PATH) {
        return Err(IpcError::not_allowed(
            "not-a-lessons-step",
            "This step doesn't append to docs/lessons.md.",
        ));
    }
    let rel = pathpolicy::check_write(lessons::PATH, WritePurpose::Repo)?;
    let root = p.root()?;
    let Some((existing, stamp)) = lessons::read(root)? else {
        return Err(lessons_error(AppendError::Missing));
    };
    let (clean, mut notes) = pasteback::prepare(text);
    let lead = lessons::header(&clean);
    let mut entries = clean[lead.len()..].to_owned();
    // A code block opened right before the first entry (Claude's "one block") and closed at the
    // end: its closing fence isn't part of the entries.
    if lead
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .is_some_and(|l| l.starts_with("```"))
        && let Some(cut) = entries.trim_end().strip_suffix("```")
        && (cut.is_empty() || cut.ends_with('\n'))
    {
        entries = cut.to_owned();
        if !notes.iter().any(|n| n == "removed code fence") {
            notes.push("removed code fence".to_owned());
        }
    }
    let new = lessons::appended(&existing, &entries).map_err(lessons_error)?;
    let before = lessons::entries(&existing).len();
    for (pasted, written) in lessons::entries(&entries)
        .iter()
        .zip(lessons::entries(&new).iter().skip(before))
    {
        if pasted.id != written.id {
            notes.push(format!(
                "{} becomes {} (the next free ID)",
                pasted.id, written.id
            ));
        }
        if pasted.stage != s.stage {
            notes.push(format!(
                "{} is for Stage {}, so it doesn't count for Stage {}",
                written.id, pasted.stage, s.stage
            ));
        }
    }
    let target = diff(rel.as_str(), Some(&existing), &new);
    let id = hold(
        app,
        HeldPaste {
            project: project_id.to_owned(),
            step: s.key.as_str().to_owned(),
            kind: HeldKind::Lessons { entries },
            stage: None,
            targets: vec![HeldTarget {
                path: RepoPath::from_trusted(rel.as_str()),
                role: Role::Output,
                text: new,
                stamp,
            }],
            valid: true,
        },
    );
    Ok(PastePreview {
        preview_id: id,
        kind: PasteKind::Lessons,
        targets: vec![target],
        problems: Vec::new(),
        notes: notes.iter().map(|n| escape_visible(n)).collect(),
        packet_fields: None,
    })
}

/// `kind: "adrSet"` (AC-F012-15, SR-35; M4-24): split on `^# ADR-\d{3}:` into
/// `docs/adr/ADR-NNN.md`; every target is checked by the write policy and listed with its diff
/// against the current file before anything is written. A target the policy refuses refuses the
/// whole paste (AC-F012-08).
fn adr_preview(
    app: &App,
    p: &Opened,
    s: &Step,
    project_id: &str,
    text: &str,
) -> Result<PastePreview, IpcError> {
    let (clean, mut notes) = pasteback::prepare(text);
    let parts = adr_split::split_adrs(&clean).map_err(|e| adr_error(&e))?;
    let targets = adr_split::plan(p.root()?, parts)?;
    let mut held = Vec::new();
    let mut diffs = Vec::new();
    for t in &targets {
        // `plan` leaves `rel` empty only when the write policy refused the path: the refusal is
        // a `PolicyViolation` naming it (N4; there is no other outcome).
        let Some(rel) = t.rel.as_ref() else {
            return Err(pathpolicy::check_write(&t.part.path, WritePurpose::Repo)
                .err()
                .map_or_else(|| IpcError::policy(&t.part.path, "write"), IpcError::from));
        };
        diffs.push(diff(rel.as_str(), t.current.as_deref(), &t.part.text));
        held.push(HeldTarget {
            path: RepoPath::from_trusted(rel.as_str()),
            role: Role::Output,
            text: t.part.text.clone(),
            stamp: t.stamp.clone(),
        });
    }
    notes.push(format!(
        "split into {} ADR {}",
        held.len(),
        if held.len() == 1 { "file" } else { "files" }
    ));
    let id = hold(
        app,
        HeldPaste {
            project: project_id.to_owned(),
            step: s.key.as_str().to_owned(),
            kind: HeldKind::AdrSet,
            stage: None,
            targets: held,
            // The ADR set is the step's whole output only when it declares nothing else to write.
            valid: s.out.is_empty() && s.appends.is_none(),
        },
    );
    Ok(PastePreview {
        preview_id: id,
        kind: PasteKind::AdrSet,
        targets: diffs,
        problems: Vec::new(),
        notes: notes.iter().map(|n| escape_visible(n)).collect(),
        packet_fields: None,
    })
}

/// `paste_back_preview`: every target with its diff, the packet's problems and the
/// normalization notes, under a one-time preview ID (AC-F012-06…-10, -13…-15, AC-F013-14,
/// AC-F016-02…-04). Packets and outputs (P1), lessons (M4-18) and ADR sets (M4-24); the text
/// comes from the clipboard or, for `source: "sensor"`, from the step's last run (M4-23).
///
/// # Errors
/// `Clipboard` (empty, not text, over 1 MB), `Unsupported` (`sensor` without a recorded reply),
/// `NotAllowed("wrong-stage-packet" | "two-packets" | "appends-in-p1" | "no-packet" |
/// "not-a-packet-step" | "no-output" | "not-a-lessons-step" | "lessons-missing" |
/// "no-lessons-entries" | "no-adrs" | "duplicate-adr")`, `PolicyViolation`, `ReadOnly`, `NotFound`.
pub fn paste_back_preview(app: &App, req: PasteBackPreviewReq) -> Result<PastePreview, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let s = p.step(req.step.as_str())?.clone();
    let text = match req.source {
        PasteSource::Clipboard => app.os().clipboard_read().map_err(IpcError::clipboard)?,
        PasteSource::Sensor => captured(&p, &s)?,
    };
    let kind = match req.kind {
        PasteKind::Auto => CorePaste::Auto,
        PasteKind::Packet => CorePaste::Packet,
        PasteKind::Output => CorePaste::Output,
        PasteKind::AdrSet => return adr_preview(app, &p, &s, req.project_id.as_str(), &text),
        PasteKind::Lessons => return lessons_preview(app, &p, &s, req.project_id.as_str(), &text),
    };
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
    let (preview_kind, held_kind) = if planned.kind == CorePaste::Packet {
        (PasteKind::Packet, HeldKind::Packet)
    } else {
        (PasteKind::Output, HeldKind::Output)
    };
    let id = hold(
        app,
        HeldPaste {
            project: req.project_id.as_str().to_owned(),
            step: req.step.as_str().to_owned(),
            kind: held_kind,
            stage,
            targets: held,
            valid: problems.is_empty(),
        },
    );
    Ok(PastePreview {
        preview_id: id,
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

/// Writes one held target by compare-and-swap: a lessons append through `lessons::append`
/// (which re-reads the file and refuses a change since the preview, AC-F016-03), every other
/// target as the exact previewed bytes.
fn write_target(
    root: &foreman_core::store::Root,
    kind: &HeldKind,
    t: &HeldTarget,
) -> Result<(), IpcError> {
    let rel = pathpolicy::check_write(t.path.as_str(), WritePurpose::Repo)?;
    if let HeldKind::Lessons { entries } = kind {
        return lessons::append(root, &t.stamp, entries)
            .map(|_| ())
            .map_err(lessons_error);
    }
    root.write(&rel, t.text.as_bytes(), &t.stamp)
        .map(|_| ())
        .map_err(|e| match e {
            StoreError::Conflict { .. } => IpcError::conflict(t.path.as_str(), Vec::new()),
            other => other.into(),
        })
}

/// `paste_back_apply`: writes the previewed targets by compare-and-swap, in order. Every
/// stamp is checked before the first write, so a target changed since the preview writes nothing
/// (AC-F012-08, AC-F016-03); a change racing the writes reports exactly what was written
/// (IPC-R10). A valid paste-back makes the step `done` (AC-F012-12), which ends its chat run's
/// clock (`runs::note_step_done`, M3-26).
///
/// # Errors
/// `Expired`, `Conflict` (with `written`), `PolicyViolation`, `ReadOnly`,
/// `NotAllowed("lessons-missing")`.
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
            match write_target(root, &held.kind, t) {
                Ok(()) => written.push(t.path.clone()),
                Err(mut err) => {
                    if let Some(IpcErrorDetail::Conflict { written: w, .. }) = &mut err.detail {
                        w.clone_from(&written);
                        return Err(err);
                    }
                    if written.is_empty() {
                        return Err(err);
                    }
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
    if step_done {
        runs::note_step_done(app, &held.project, &held.step);
    }
    if let (HeldKind::Packet, Some(n)) = (&held.kind, held.stage) {
        app.emit(Event::PacketChanged(PacketChanged {
            project_id: ProjectId::from_trusted(&held.project),
            stage: Stage(n),
            rev: Rev(rev),
        }));
    }
    Ok(PasteApplyResult { written, step_done })
}
