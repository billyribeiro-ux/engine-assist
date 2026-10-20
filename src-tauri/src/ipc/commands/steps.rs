//! Steps (`docs/contracts/ipc.md` §4.4; F-004…F-007; tickets M2-21, M2-22, M2-26, M3-07, M3-17).

use std::collections::{BTreeMap, BTreeSet};

use foreman_core::ids::{StepKey as CoreKey, Surface as CoreSurface};
use foreman_core::pathpolicy::{self, ReadClass};
use foreman_core::policy::{self, PolicyError, RunOverride as CoreOverride};
use foreman_core::resolver::tokens::{self, TokenClass as CoreClass};
use foreman_core::resolver::{self, PacketText, PromptSegment as CoreSeg, ResolveCtx, Unresolved};
use foreman_core::rules::{self, RuleError};
use foreman_core::sanitize;
use foreman_core::schema::progress::{DecisionFileKind, ItemOutcome, StoredOverride};
use foreman_core::steps::actions::{self, ActionError, ActionPlan};
use foreman_core::steps::{self, StepEvent, StepState as CoreState};
use foreman_core::template::{InputType, Step, StepKind as CoreKind};

use crate::diff::diff;
use crate::ipc::IpcError;
use crate::ipc::dto::app::ProjectView;
use crate::ipc::dto::requests::{
    ActionApplyReq, ActionPreviewReq, ClipboardCopyReq, OptionalPartSetReq, RunInputSetReq,
    RunOverrideSetReq, StepMarkDoneReq, StepSkipReq, StepViewReq, WaitStartReq,
};
use crate::ipc::dto::steps::{
    ActionPreview, ChatChip, ChatChips, CopyResult, OutView, RunInputView, StepPart, StepView,
    SwitchCue,
};
use crate::ipc::error::StaleWhat;
use crate::ipc::types::{
    ActionKind, Blocker, CopyTarget, Explained, NotRunnableReason, PreviewId, PromptSegment,
    RepoPath, RunOverride, Start, Timestamp, ValueOrigin,
};
use crate::os::Confirm;
use crate::preview::Preview;
use crate::project::{self, Opened, model_name};
use crate::render::{escape_visible, spans};
use crate::state::{App, PREVIEW_TTL};

/// Everything resolution needs, owned (DR-093).
#[derive(Debug)]
pub struct Resolution {
    /// The resolved prompt or what's missing.
    pub result: Result<resolver::ResolvedPrompt, Vec<Unresolved>>,
    /// The run inputs a step needs after part removal (DR-089).
    pub required: Vec<String>,
}

fn key_of(k: &str) -> Result<CoreKey, IpcError> {
    CoreKey::parse(k).map_err(|_| IpcError::invalid("step", "stepKey"))
}

/// The signed (or not yet signed) packets, as the resolver reads them.
fn packet_texts(p: &Opened) -> BTreeMap<u8, PacketText> {
    let signed = p.signed();
    p.packets
        .iter()
        .map(|(n, f)| {
            (
                *n,
                PacketText {
                    text: f.text.clone(),
                    signed: signed.contains(n),
                },
            )
        })
        .collect()
}

/// Resolves a step's prompt against the project's current state.
///
/// # Errors
/// `ReadOnly` without a template.
pub fn resolve(p: &Opened, s: &Step) -> Result<Resolution, IpcError> {
    let (t, path, project) = p.parts()?;
    let packets = packet_texts(p);
    let inputs = p
        .progress
        .run_inputs
        .get(&s.key)
        .cloned()
        .unwrap_or_default();
    let optional = p
        .progress
        .optional_parts
        .get(&s.key)
        .cloned()
        .unwrap_or_default();
    let attested_na: BTreeSet<String> = p
        .progress
        .gate_runs
        .values()
        .flat_map(|g| g.items.iter())
        .filter(|(_, r)| r.outcome == ItemOutcome::NotApplicable)
        .map(|(id, _)| id.to_string())
        .collect();
    let ctx = ResolveCtx {
        template: t,
        step: s,
        project,
        path,
        packets: &packets,
        inputs: &inputs,
        remembered: &p.progress.remembered,
        optional: &optional,
        attested_na: &attested_na,
        overlay_items: &p.overlay.added_items,
    };
    let mut result = resolver::resolve(&ctx);
    // AC-F016-01 (M4-18): an F.2 prompt ends with the entry format from docs/lessons.md's header.
    if s.template_id.as_deref() == Some("F.2")
        && let Ok(ok) = &mut result
        && let Some(suffix) = crate::closeout::lessons_text(p)
            .as_deref()
            .and_then(foreman_core::lessons::f2_suffix)
    {
        ok.text.push_str(&sanitize::for_paste(&suffix).0);
    }
    Ok(Resolution {
        result,
        required: resolver::required_inputs(&ctx),
    })
}

fn class(c: tokens::TokenClass) -> crate::ipc::types::TokenClass {
    c.into()
}

/// The optional parts of a prompt (`opt-N` → the part's text).
fn optional_parts(s: &Step) -> Vec<(String, String)> {
    let text = s.prompt.as_ref().map(|p| p.joined()).unwrap_or_default();
    let declared: Vec<String> = s.inputs().iter().map(|i| i.name.clone()).collect();
    let (toks, _) = tokens::scan(&text, &declared);
    toks.into_iter()
        .filter(|t| t.class == Some(CoreClass::OptionalPart))
        .filter_map(|t| t.part.map(|id| (id, t.inner)))
        .collect()
}

/// The resolved prompt as segments; on a resolution error, the template's text with every token
/// as an attention pill.
fn segments(s: &Step, r: &Resolution) -> Vec<PromptSegment> {
    let parts: BTreeMap<String, String> = optional_parts(s).into_iter().collect();
    match &r.result {
        Ok(ok) => ok
            .segments
            .iter()
            .map(|seg| match seg {
                CoreSeg::Text { text } => PromptSegment::Text { spans: spans(text) },
                CoreSeg::Resolved {
                    token,
                    class: c,
                    text,
                    note,
                } => PromptSegment::Resolved {
                    token: escape_visible(token),
                    class: class(*c),
                    text: text.clone(),
                    note: note.clone(),
                },
                CoreSeg::OptionalPart { part_id, included } => PromptSegment::OptionalPart {
                    part_id: part_id.clone(),
                    included: *included,
                    text: parts.get(part_id).cloned().unwrap_or_default(),
                },
                CoreSeg::RemovedPart { expr, reason } => PromptSegment::RemovedPart {
                    expr: expr.clone(),
                    reason: reason.clone(),
                },
            })
            .collect(),
        Err(_) => {
            let text = s.prompt.as_ref().map(|p| p.joined()).unwrap_or_default();
            let declared: Vec<String> = s.inputs().iter().map(|i| i.name.clone()).collect();
            let (toks, _) = tokens::scan(&text, &declared);
            let mut out = Vec::new();
            let mut last = 0;
            for t in toks
                .iter()
                .filter(|t| t.part.is_none() || t.class == Some(CoreClass::OptionalPart))
            {
                if t.offset < last || t.offset + t.len > text.len() {
                    continue;
                }
                if t.offset > last {
                    out.push(PromptSegment::Text {
                        spans: spans(&text[last..t.offset]),
                    });
                }
                let c = t.class.unwrap_or(CoreClass::Literal);
                out.push(PromptSegment::Unresolved {
                    token: escape_visible(&t.text),
                    class: class(c),
                    input: (c == CoreClass::RunInput).then(|| tokens::input_name(&t.inner)),
                });
                last = t.offset + t.len;
            }
            if last < text.len() {
                out.push(PromptSegment::Text {
                    spans: spans(&text[last..]),
                });
            }
            out
        }
    }
}

fn not_runnable(state: &CoreState, why: Option<String>) -> Option<Blocker> {
    let reason = match state {
        CoreState::NotApplicable(_) | CoreState::Skipped(_) => NotRunnableReason::NotApplicable,
        CoreState::Undecided => NotRunnableReason::Undecided,
        CoreState::NeedsAnnotation => NotRunnableReason::NeedsAnnotation,
        CoreState::NeedsNewerForeman => NotRunnableReason::NeedsNewerForeman,
        CoreState::WaitingUntil(_) => NotRunnableReason::Waiting,
        CoreState::Running => NotRunnableReason::Running,
        CoreState::NotStarted | CoreState::Copied | CoreState::Done => return None,
    };
    Some(Blocker::NotRunnable {
        reason,
        text: why.unwrap_or_default(),
    })
}

/// A step's blockers (DR-120's prompt-side checks): not runnable, read-only, unresolved tokens,
/// prompt hygiene. Launch-side checks (version, config, auth) are `launch_prepare`'s.
#[must_use]
pub fn blockers(p: &Opened, app: &App, s: &Step, r: Option<&Resolution>) -> Vec<Blocker> {
    let mut out = Vec::new();
    if let Some(ro) = &p.read_only {
        out.push(Blocker::ReadOnly(ro.clone()));
    }
    let summary = p.step_summary(app, s);
    let state = p.display_state(app, s);
    if let Some(b) = not_runnable(&state, summary.why_not_runnable) {
        out.push(b);
    }
    let Some(r) = r else { return out };
    match &r.result {
        Err(missing) => {
            for m in missing {
                out.push(match m {
                    Unresolved::Variable { token } | Unresolved::UnknownToken { token } => {
                        Blocker::UnresolvedToken {
                            token: escape_visible(token),
                        }
                    }
                    Unresolved::PacketUnsigned { stage } => Blocker::PacketUnsigned {
                        stage: crate::ipc::types::Stage(*stage),
                    },
                    Unresolved::PacketMissing { stage } => Blocker::SignedPacketMissing {
                        stage: crate::ipc::types::Stage(*stage),
                    },
                    Unresolved::RunInputMissing { name } => {
                        Blocker::RunInputMissing { name: name.clone() }
                    }
                    Unresolved::Undecided { message } => Blocker::NotRunnable {
                        reason: NotRunnableReason::Undecided,
                        text: message.clone(),
                    },
                });
            }
        }
        Ok(ok) => match sanitize::check_prompt(&ok.text) {
            Ok(()) => {}
            Err(sanitize::Blocker::PromptOptionLike { first }) => {
                out.push(Blocker::PromptOptionLike {
                    first: first.to_string(),
                });
            }
            Err(sanitize::Blocker::PromptControlChar { offset, codepoint }) => {
                out.push(Blocker::PromptControlChar { offset, codepoint });
            }
        },
    }
    out
}

fn humanize(name: &str) -> String {
    let t = name.replace('_', " ");
    let mut c = t.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

fn stored_override(p: &Opened, s: &Step) -> Option<StoredOverride> {
    p.progress.overrides.get(&s.key).copied()
}

fn core_override(o: &StoredOverride) -> CoreOverride {
    CoreOverride {
        model: o.model,
        effort: o.effort,
        mode: o.mode,
    }
}

/// The `StepView` of a step.
///
/// # Errors
/// `NotFound`; `ReadOnly` without a template.
pub fn view_of(app: &App, p: &Opened, key: &str) -> Result<StepView, IpcError> {
    let s = p.step(key)?;
    let (_, _, project) = p.parts()?;
    let summary = p.step_summary(app, s);
    let prompted = matches!(
        s.kind,
        CoreKind::Prompt | CoreKind::Followup | CoreKind::Repeatable
    ) && s.prompt.is_some();
    let r = if prompted { Some(resolve(p, s)?) } else { None };
    let stored = p.progress.run_inputs.get(&s.key);
    let inputs = s
        .inputs()
        .iter()
        .map(|i| {
            let value = stored
                .and_then(|m| m.get(&i.name).cloned())
                .or_else(|| {
                    i.remember
                        .then(|| p.progress.remembered.get(&i.name).cloned())
                        .flatten()
                })
                .unwrap_or_default();
            RunInputView {
                name: i.name.clone(),
                label: humanize(&i.name),
                token: format!("[{}]", i.name.replace('_', " ")),
                multiline: i.kind == InputType::Multiline,
                remember: i.remember,
                required: r.as_ref().is_some_and(|r| r.required.contains(&i.name)),
                warning: (value.len() > resolver::LARGE_INPUT).then(|| {
                    "Over 64 KB: it is sent as is, but check it's what you meant.".to_owned()
                }),
                value,
            }
        })
        .collect();
    let over = stored_override(p, s);
    let start = policy::effective_start(
        s,
        &project.model_policy,
        over.map(|o| core_override(&o)).as_ref(),
    )
    .ok()
    .map(|e| Explained {
        value: Start::from(e.value),
        origin: ValueOrigin::from(&e.origin),
        from: e.from.map(Start::from),
    });
    let chips = (s.surface == Some(CoreSurface::Chat)).then(|| {
        let link = s
            .chain
            .as_ref()
            .and_then(|c| project.chat_urls.get(c).cloned())
            .map(|u| (u, "Open conversation"))
            .or_else(|| {
                project
                    .chat_project_url
                    .clone()
                    .map(|u| (u, "Open Project"))
            });
        let mut chips = Vec::new();
        if let Some(st) = start.as_ref() {
            chips.push(ChatChip {
                id: "model".into(),
                label: model_name(st.value.model.into()),
                strong: true,
            });
            chips.push(ChatChip {
                id: "effort".into(),
                label: format!("{} thinking", st.value.effort_label()),
                strong: false,
            });
        }
        ChatChips {
            chips,
            link_label: link.as_ref().map(|(_, l)| (*l).to_owned()),
            link: link.map(|(u, _)| u),
        }
    });
    let root = p.root()?;
    let outs = s
        .out
        .iter()
        .map(|o| OutView {
            path: RepoPath::from_trusted(o.as_str()),
            exists: pathpolicy::check_read(o, ReadClass::Decision)
                .is_ok_and(|rel| root.exists(&rel)),
        })
        .collect();
    let toggles = p.progress.optional_parts.get(&s.key);
    let parts = optional_parts(s)
        .into_iter()
        .map(|(id, text)| StepPart {
            included: toggles.and_then(|t| t.get(&id)).copied().unwrap_or(true),
            label: text.chars().take(80).collect(),
            part_id: id,
            locked: false,
        })
        .collect();
    let progress = p.step_progress(key);
    Ok(StepView {
        prompt: r.as_ref().map(|r| segments(s, r)).unwrap_or_default(),
        inputs,
        start,
        switches: policy::switch_cues(s, &project.model_policy)
            .into_iter()
            .map(|text| SwitchCue { text })
            .collect(),
        chips,
        // The argv builder is M2-06; the preview shows it once it lands.
        argv_preview: None,
        env_names: Vec::new(),
        blockers: blockers(p, app, s, r.as_ref()),
        warnings: r
            .as_ref()
            .and_then(|r| r.result.as_ref().ok())
            .map(|ok| ok.notes.clone())
            .unwrap_or_default(),
        remaps: Vec::new(),
        outs,
        r#override: over.map(|o| RunOverride {
            model: o.model.map(Into::into),
            effort: o.effort.map(Into::into),
            mode: o.mode.map(Into::into),
        }),
        parts,
        copied_at: progress.copied_at.map(Timestamp),
        summary,
    })
}

trait EffortLabel {
    fn effort_label(&self) -> String;
}

impl EffortLabel for Start {
    fn effort_label(&self) -> String {
        let core: foreman_core::ids::Effort = self.effort.into();
        let s = core.as_str();
        let mut c = s.chars();
        c.next()
            .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
            .unwrap_or_default()
    }
}

/// `step_view` (P1, mutates nothing).
///
/// # Errors
/// `NotFound`, `Io`.
pub fn step_view(app: &App, req: StepViewReq) -> Result<StepView, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    view_of(app, &p, req.step.as_str())
}

/// Applies a step event (DR-065) and saves.
fn apply_event(
    app: &App,
    p: &mut Opened,
    key: &str,
    ev: &StepEvent,
    rule: &str,
) -> Result<(), IpcError> {
    let s = p.step(key)?.clone();
    let ctx = p.step_ctx(app, &s);
    let cur = p.step_progress(key);
    let next = steps::transition(&cur, &ctx, ev)
        .map_err(|r| IpcError::not_allowed(rule, escape_visible(&r.0)))?;
    p.progress.steps.insert(key_of(key)?, next);
    Ok(())
}

/// `run_input_set` (P1): stores the value in `progress.json`; a `remember` input is also stored
/// per project as the next prefill.
///
/// # Errors
/// `NotFound` (input not declared), `ReadOnly`, `Conflict`, `Io`.
pub fn run_input_set(app: &App, req: RunInputSetReq) -> Result<StepView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let s = p.step(req.step.as_str())?.clone();
    let name = req.name.as_str();
    let Some(decl) = s.inputs().iter().find(|i| i.name == name) else {
        return Err(IpcError::not_found(
            "That run input isn't declared on this step.",
        ));
    };
    let value = req.value.into_string();
    if decl.remember {
        p.progress.remembered.insert(name.to_owned(), value.clone());
    }
    p.progress
        .run_inputs
        .entry(s.key.clone())
        .or_default()
        .insert(name.to_owned(), value);
    p.save_progress(app)?;
    view_of(app, &p, req.step.as_str())
}

/// `optional_part_set` (P1).
///
/// # Errors
/// `NotFound` (no such part), `ReadOnly`, `Conflict`, `Io`.
pub fn optional_part_set(app: &App, req: OptionalPartSetReq) -> Result<StepView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let s = p.step(req.step.as_str())?.clone();
    let id = req.part_id.as_str();
    if !optional_parts(&s).iter().any(|(k, _)| k == id) {
        return Err(IpcError::not_found(
            "That optional part isn't in this prompt.",
        ));
    }
    p.progress
        .optional_parts
        .entry(s.key.clone())
        .or_default()
        .insert(id.to_owned(), req.include);
    p.save_progress(app)?;
    view_of(app, &p, req.step.as_str())
}

/// `run_override_set` (P1): an override for the next run; `None` clears it (F-005, DR-103).
///
/// # Errors
/// `NotFound`, `NotAllowed("override-doesnt-fit")`, `ReadOnly`, `Conflict`, `Io`.
pub fn run_override_set(app: &App, req: RunOverrideSetReq) -> Result<StepView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let s = p.step(req.step.as_str())?.clone();
    let (_, _, project) = p.parts()?;
    match req.over {
        None => {
            p.progress.overrides.remove(&s.key);
        }
        Some(o) => {
            let stored = StoredOverride {
                model: o.model.map(Into::into),
                effort: o.effort.map(Into::into),
                mode: o.mode.map(Into::into),
            };
            match policy::effective_start(&s, &project.model_policy, Some(&core_override(&stored)))
            {
                Ok(_) => {}
                Err(PolicyError::OverrideDoesntFit { why, .. }) => {
                    return Err(IpcError::not_allowed("override-doesnt-fit", why));
                }
                Err(PolicyError::NoStart(_)) => {
                    return Err(IpcError::not_allowed(
                        "override-doesnt-fit",
                        "This step has no run config to override.",
                    ));
                }
            }
            p.progress.overrides.insert(s.key.clone(), stored);
        }
    }
    p.save_progress(app)?;
    view_of(app, &p, req.step.as_str())
}

/// `step_mark_done` (P1, ⌘D): chat, manual and elapsed wait steps (DR-066).
///
/// # Errors
/// `NotAllowed("not-markable")` with the reason (CLI prompt steps finish by exit; a wait before
/// its date), `ReadOnly`, `Conflict`, `Io`.
pub fn step_mark_done(app: &App, req: StepMarkDoneReq) -> Result<StepView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    apply_event(
        app,
        &mut p,
        req.step.as_str(),
        &StepEvent::MarkDone,
        "not-markable",
    )?;
    p.save_progress(app)?;
    view_of(app, &p, req.step.as_str())
}

fn rule_error(e: &RuleError) -> IpcError {
    let rule = match e {
        RuleError::CoreStepSkip => "core-step-skip",
        RuleError::ConditionalStepSkip => "conditional-step-skip",
        RuleError::NotSkippable(_) => "not-skippable",
        RuleError::BadReason => "reason-required",
        RuleError::GateItemSkip => "gate-item-skip",
        RuleError::ConditionLocked => "condition-locked",
        RuleError::UnknownStep(_) => {
            return IpcError::not_found("That step isn't in this project's blueprint.");
        }
    };
    IpcError::not_allowed(rule, escape_visible(&e.to_string()))
}

/// `step_skip` (P1): an optional step with a reason, recorded in `overlay.json` (a decision
/// file: committed by plumbing before Stage 5 is signed, pending after; DR-056, DR-153).
///
/// # Errors
/// `NotAllowed("core-step-skip" | "conditional-step-skip" | "reason-required" | …)`,
/// `ReadOnly`, `Conflict`, `ToolFailed`.
pub fn step_skip(app: &App, req: StepSkipReq) -> Result<ProjectView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let (t, path, _) = p.parts()?;
    let overlay = rules::skip(
        t,
        path,
        &p.overlay,
        req.step.as_str(),
        req.reason.as_str(),
        &app.now(),
    )
    .map_err(|e| rule_error(&e))?;
    p.own_change(app, DecisionFileKind::Overlay, &overlay)?;
    project::reopen_view(app, req.project_id.as_str())
}

/// The action plan held between `action_preview` and `action_apply` (IPC-R7).
#[derive(Debug)]
pub struct HeldAction {
    /// Project.
    pub project: String,
    /// Step.
    pub step: String,
    /// The project revision it was planned at.
    pub rev: u32,
    /// The plan with the stamps it read.
    pub plan: ActionPlan,
}

fn action_error(e: ActionError) -> IpcError {
    match e {
        ActionError::Unknown(_) => {
            IpcError::not_allowed("not-an-action", "Foreman has no action for this step.")
        }
        ActionError::MissingSource(s) => IpcError::not_found(format!("{s} is missing.")),
        ActionError::NeedsConfirmation(_) => IpcError::cancelled(),
        ActionError::Store(s) => s.into(),
    }
}

fn plan_for(p: &Opened, key: &str) -> Result<(ActionPlan, ActionKind), IpcError> {
    let s = p.step(key)?;
    if s.kind != CoreKind::Action {
        return Err(IpcError::not_allowed(
            "not-an-action",
            "This step isn't an action.",
        ));
    }
    let (t, _, _) = p.parts()?;
    let plan = actions::plan(t, &s.id, p.root()?).map_err(action_error)?;
    let kind = if plan.action == "copy-file" {
        ActionKind::CopyFile
    } else {
        ActionKind::WriteBlocks
    };
    Ok((plan, kind))
}

fn action_preview_of(p: &Opened, plan: &ActionPlan, kind: ActionKind, id: String) -> ActionPreview {
    let _ = p;
    ActionPreview {
        preview_id: PreviewId::from_trusted(id),
        action: kind,
        writes: plan
            .writes
            .iter()
            .map(|w| {
                diff(
                    w.path.as_str(),
                    w.existing
                        .as_ref()
                        .map(|e| String::from_utf8_lossy(e).into_owned())
                        .as_deref(),
                    &String::from_utf8_lossy(&w.bytes),
                )
            })
            .collect(),
    }
}

/// `action_preview` (P1): every destination listed before anything is written (SR-35).
///
/// # Errors
/// `NotFound`, `NotAllowed("not-an-action")`, `PolicyViolation`, `ReadOnly`.
pub fn action_preview(app: &App, req: ActionPreviewReq) -> Result<ActionPreview, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let (plan, kind) = plan_for(&p, req.step.as_str())?;
    let held = HeldAction {
        project: req.project_id.as_str().to_owned(),
        step: req.step.as_str().to_owned(),
        rev: p.progress.rev,
        plan: plan.clone(),
    };
    let id = app
        .inner()
        .previews
        .hold(Preview::Action(Box::new(held)), PREVIEW_TTL);
    Ok(action_preview_of(&p, &plan, kind, id))
}

/// `action_apply` (P1): writes the previewed files by CAS. Overwriting a file that already has
/// different content needs a native confirmation (SR-24a).
///
/// # Errors
/// `Expired`, `Stale` (with a fresh preview), `Cancelled`, `Conflict`, `PolicyViolation`.
pub fn action_apply(app: &App, req: ActionApplyReq) -> Result<StepView, IpcError> {
    let _w = app.writing();
    let held = match app.inner().previews.take(req.preview_id.as_str())? {
        Preview::Action(h) => h,
        _ => return Err(IpcError::expired()),
    };
    let mut p = project::open(app, &held.project)?;
    p.writable()?;
    if p.progress.rev != held.rev {
        let (plan, kind) = plan_for(&p, &held.step)?;
        let id = app.inner().previews.hold(
            Preview::Action(Box::new(HeldAction {
                project: held.project.clone(),
                step: held.step.clone(),
                rev: p.progress.rev,
                plan: plan.clone(),
            })),
            PREVIEW_TTL,
        );
        let fresh = serde_json::to_value(action_preview_of(&p, &plan, kind, id)).ok();
        return Err(IpcError::stale(
            StaleWhat::ProjectRev,
            "The project changed since this preview.",
            fresh,
        ));
    }
    let overwrites: Vec<String> = held
        .plan
        .writes
        .iter()
        .filter(|w| w.existing.as_ref().is_some_and(|e| *e != w.bytes))
        .map(|w| escape_visible(w.path.as_str()))
        .collect();
    if !overwrites.is_empty()
        && !app.os().confirm(&Confirm::new(
            "Replace existing files?",
            format!(
                "These files already exist with different content:\n{}",
                overwrites.join("\n")
            ),
            "Replace",
        ))
    {
        return Err(IpcError::cancelled());
    }
    actions::apply(&held.plan, p.root()?, true).map_err(|e| match e {
        ActionError::Store(foreman_core::store::StoreError::Conflict { name, .. }) => {
            IpcError::conflict(&name, Vec::new())
        }
        other => action_error(other),
    })?;
    apply_event(
        app,
        &mut p,
        &held.step,
        &StepEvent::ActionApplied,
        "not-runnable",
    )?;
    p.save_progress(app)?;
    view_of(app, &p, &held.step)
}

/// `wait_start` (P1): a wait step runs until the date Billy enters (DR-067). The notification on
/// that date is a Rust-side template (SR-56) and ships with the notification plugin (M3-07).
///
/// # Errors
/// `NotAllowed("not-a-wait-step" | "date-not-after-today")`, `ReadOnly`, `Conflict`.
pub fn wait_start(app: &App, req: WaitStartReq) -> Result<StepView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let s = p.step(req.step.as_str())?.clone();
    if s.kind != CoreKind::Wait {
        return Err(IpcError::not_allowed(
            "not-a-wait-step",
            "This step isn't a wait step.",
        ));
    }
    let until: jiff::civil::Date = req
        .until
        .as_str()
        .parse()
        .map_err(|_| IpcError::invalid("until", "isoDate"))?;
    if until <= app.os().today() {
        return Err(IpcError::not_allowed(
            "date-not-after-today",
            "Choose a date after today.",
        ));
    }
    apply_event(
        app,
        &mut p,
        req.step.as_str(),
        &StepEvent::StartWait(until.to_string()),
        "not-a-wait-step",
    )?;
    p.progress.waits.insert(s.key.clone(), until.to_string());
    p.save_progress(app)?;
    view_of(app, &p, req.step.as_str())
}

/// DR-137: the fix request for a packet's problems.
#[must_use]
pub fn fix_request(problems: &[foreman_core::packet::Problem]) -> String {
    let missing: Vec<String> = problems
        .iter()
        .filter(|p| p.rule == "packet.missing-field")
        .map(|p| p.message.clone())
        .collect();
    let mut out = String::from("Your packet is missing: ");
    if missing.is_empty() {
        out = String::from("Your packet has problems: ");
        out.push_str(
            &problems
                .iter()
                .map(|p| p.message.clone())
                .collect::<Vec<_>>()
                .join("; "),
        );
    } else {
        out.push_str(&missing.join(", "));
    }
    out.push('.');
    if problems.iter().any(|p| p.rule == "packet.open-items") {
        out.push_str(" OPEN ITEMS must be exactly `none`.");
    }
    out.push_str(" Re-output the complete packet in one code block.");
    out
}

fn required<T>(v: Option<T>, field: &str) -> Result<T, IpcError> {
    v.ok_or_else(|| IpcError::invalid(field, "required"))
}

fn copy_text(app: &App, text: &str) -> Result<u32, IpcError> {
    let (clean, report) = sanitize::for_paste(text);
    app.os()
        .clipboard_write(&clean)
        .map_err(|_| IpcError::io("The clipboard refused the text."))?;
    Ok(u32::try_from(report.removed).unwrap_or(u32::MAX))
}

/// `clipboard_copy` (P1): every copy target, text sanitized before it reaches the clipboard
/// (SR-28). Copying a followup's prompt marks it copied (DR-068).
///
/// # Errors
/// `Invalid` (a needed field is null), `NotFound`, `NotAllowed("unresolved-tokens" |
/// "prompt-control-char")`, `PolicyViolation`, `Unsupported` (`command` before M2-06).
pub fn clipboard_copy(app: &App, req: ClipboardCopyReq) -> Result<CopyResult, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    match req.target {
        CopyTarget::Prompt => {
            let key = required(req.step, "step")?;
            let s = p.step(key.as_str())?.clone();
            let r = resolve(&p, &s)?;
            let text = match r.result {
                Ok(ok) => ok.text,
                Err(_) => {
                    return Err(IpcError::not_allowed(
                        "unresolved-tokens",
                        "Fill in what the prompt still needs first.",
                    ));
                }
            };
            if let Err(sanitize::Blocker::PromptControlChar { .. }) = sanitize::check_prompt(&text)
            {
                return Err(IpcError::not_allowed(
                    "prompt-control-char",
                    "The prompt has a control character. Fix the input it came from.",
                ));
            }
            let removed = copy_text(app, &text)?;
            if s.kind == CoreKind::Followup && p.read_only.is_none() {
                apply_event(app, &mut p, key.as_str(), &StepEvent::Copy, "not-runnable")?;
                p.progress
                    .steps
                    .entry(s.key.clone())
                    .and_modify(|x| x.copied_at = Some(app.now()));
                p.save_progress(app)?;
            }
            Ok(CopyResult {
                removed_controls: removed,
                focus_run: None,
            })
        }
        CopyTarget::Command => Err(IpcError::not_yet_available(
            "clipboard_copy.command",
            "M2-06",
        )),
        CopyTarget::SessionId => {
            let chain = required(req.chain, "chain")?;
            let c = p
                .progress
                .chains
                .iter()
                .find(|c| c.id == chain.as_str())
                .ok_or_else(|| IpcError::not_found("That session chain isn't in this project."))?;
            let removed = copy_text(app, &c.id)?;
            Ok(CopyResult {
                removed_controls: removed,
                focus_run: None,
            })
        }
        CopyTarget::FileContents => {
            let path = required(req.path, "path")?;
            let rel = pathpolicy::check_read(path.as_str(), ReadClass::Decision)?;
            let (bytes, _) = p.root()?.read(&rel, ReadClass::Decision)?;
            let removed = copy_text(app, &String::from_utf8_lossy(&bytes))?;
            Ok(CopyResult {
                removed_controls: removed,
                focus_run: None,
            })
        }
        CopyTarget::FixRequest => {
            let stage = required(req.stage, "stage")?;
            let text = p
                .packets
                .get(&stage.0)
                .ok_or_else(|| IpcError::not_found("That packet doesn't exist yet."))?
                .text
                .clone();
            let problems = p.packet_problems(stage.0, &text);
            if problems.is_empty() {
                return Err(IpcError::not_allowed(
                    "no-problems",
                    "The packet has no problems.",
                ));
            }
            let removed = copy_text(app, &fix_request(&problems))?;
            Ok(CopyResult {
                removed_controls: removed,
                focus_run: None,
            })
        }
    }
}
