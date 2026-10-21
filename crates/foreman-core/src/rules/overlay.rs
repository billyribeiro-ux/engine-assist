//! Overlay additions (F-025, DR-058–DR-062, M4-30): a project-only prompt or inspection item.
//! The overlay can only add (and skip, in the parent module); an added step goes through the same
//! checks as a template step (DR-061), is placed after the step Billy chose, always starts a new
//! session and always confirms before running (DR-059, SR-19). [`effective_template`] splices the
//! added steps into a copy of the pinned template, so the path, the stage view, the resolver and
//! the launch pipeline see them exactly like template steps.

use crate::ids::{CwdKind, Effort, Start, StepKey, Surface};
use crate::pathpolicy::{self, PolicyViolation, WritePurpose};
use crate::rules::{ApplicablePath, StageState};
use crate::schema::overlay::{AddedItem, AddedStep, AddedTag, OverlayFile};
use crate::template::{Prompt, PromptPart, Session, Step, StepKind, StepStatus, Tag, Template};

/// Longest title of an added step (the IPC `ShortText`).
pub const TITLE_MAX: usize = 200;
/// Longest prompt of an added step (the IPC `Note`).
pub const TEXT_MAX: usize = 4000;
/// Most `out` paths on an added step.
pub const OUT_MAX: usize = 8;
/// The highest `n` in `x-<stage>-<n>` and `xi-<stage>-<n>` (the key grammar allows three digits).
pub const N_MAX: u32 = 999;

/// A refused addition.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OverlayError {
    /// `bypassPermissions` anywhere in the step (DR-016, SR-03).
    #[error("bypass-permissions: bypassPermissions is never allowed (SR-03)")]
    BypassPermissions,
    /// The run config doesn't fit the surface (DR-023).
    #[error("invalid-start: {0}")]
    InvalidStart(String),
    /// An `out` path the write allowlist refuses (DR-026, DR-160).
    #[error("out path refused: {0}")]
    Path(PolicyViolation),
    /// `after` or `copied_from` isn't a step of this project.
    #[error("unknown step {0}")]
    UnknownStep(String),
    /// `after` isn't in the stage it is added to.
    #[error("{0} isn't a step of Stage {1}")]
    WrongStage(String, u8),
    /// Only a stage that applies can take an addition.
    #[error("Stage {0} doesn't apply to this project, so nothing can be added to it")]
    StageNotApplicable(u8),
    /// The title must be one line of 1–200 characters.
    #[error("bad-title: a title is one line of 1–200 characters")]
    BadTitle,
    /// The prompt must be 1–4,000 characters.
    #[error("bad-text: the text is 1–4,000 characters")]
    BadText,
    /// Too many `out` paths, or a repeated one.
    #[error("bad-out: at most {OUT_MAX} different output paths")]
    BadOut,
    /// `n` would pass 999.
    #[error("too-many: this stage has the most additions Foreman keeps")]
    TooMany,
}

/// What Billy typed in the insert point (the IPC request without its project).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewStep {
    /// The stage.
    pub stage: u8,
    /// The step it goes after.
    pub after: String,
    /// The step whose run config it started from.
    pub copied_from: String,
    /// Title.
    pub title: String,
    /// Prompt text.
    pub text: String,
    /// Surface.
    pub surface: Surface,
    /// Working directory.
    pub cwd: Option<CwdKind>,
    /// Run config.
    pub start: Start,
    /// Declared outputs.
    pub out: Vec<String>,
    /// `optional` by default; Billy may make it `core` for this project.
    pub tag: AddedTag,
}

fn has_bypass(s: &str) -> bool {
    s.to_ascii_lowercase().contains("bypasspermissions")
}

fn one_line(s: &str, max: usize) -> bool {
    let n = s.trim().chars().count();
    (1..=max).contains(&n) && !s.contains(['\n', '\r'])
}

/// The checks every added step passes (DR-061): `bypassPermissions` (DR-016), the run-config
/// vocabulary for the surface (DR-023) and the `out` paths through the write allowlist (DR-026).
///
/// # Errors
/// See [`OverlayError`].
pub fn validate_step(
    title: &str,
    text: &str,
    surface: Surface,
    cwd: Option<CwdKind>,
    start: &Start,
    out: &[String],
) -> Result<(), OverlayError> {
    if has_bypass(title) || has_bypass(text) || out.iter().any(|o| has_bypass(o)) {
        return Err(OverlayError::BypassPermissions);
    }
    if !one_line(title, TITLE_MAX) {
        return Err(OverlayError::BadTitle);
    }
    if text.trim().is_empty() || text.chars().count() > TEXT_MAX {
        return Err(OverlayError::BadText);
    }
    match (surface, start.mode) {
        (Surface::Chat, Some(_)) => {
            return Err(OverlayError::InvalidStart("chat steps have no mode".into()));
        }
        (Surface::Cli | Surface::Desktop, None) => {
            return Err(OverlayError::InvalidStart(
                "cli and desktop steps need a mode".into(),
            ));
        }
        _ => {}
    }
    if (start.effort == Effort::Adaptive) != (surface == Surface::Chat) {
        return Err(OverlayError::InvalidStart(
            "effort `adaptive` is for chat only, and chat uses only `adaptive`".into(),
        ));
    }
    match (surface, cwd) {
        (Surface::Chat, Some(_)) => {
            return Err(OverlayError::InvalidStart("chat steps have no cwd".into()));
        }
        (Surface::Cli | Surface::Desktop, None) => {
            return Err(OverlayError::InvalidStart(
                "cli and desktop steps need a cwd".into(),
            ));
        }
        _ => {}
    }
    let distinct: std::collections::BTreeSet<&String> = out.iter().collect();
    if out.len() > OUT_MAX || distinct.len() != out.len() {
        return Err(OverlayError::BadOut);
    }
    for o in out {
        pathpolicy::check_write(o, WritePurpose::TemplateOut).map_err(OverlayError::Path)?;
    }
    Ok(())
}

fn next_n(used: impl Iterator<Item = String>, prefix: &str) -> Result<u32, OverlayError> {
    let hi = used
        .filter_map(|id| id.strip_prefix(prefix)?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    if hi >= N_MAX {
        Err(OverlayError::TooMany)
    } else {
        Ok(hi + 1)
    }
}

fn stage_applies(path: &ApplicablePath, stage: u8) -> Result<(), OverlayError> {
    match path.stage(stage) {
        Some(StageState::Applicable) => Ok(()),
        _ => Err(OverlayError::StageNotApplicable(stage)),
    }
}

/// Adds a step (DR-059): ID `x-<stage>-<n>`, placed after `after`, run config as given (the
/// insert point starts it from `copied_from`'s), always a new session. Returns the new overlay and
/// the ID.
///
/// `t` is the project's effective template (see [`effective_template`]), so `after` and
/// `copied_from` may be steps added earlier.
///
/// # Errors
/// See [`OverlayError`].
pub fn add_step(
    t: &Template,
    path: &ApplicablePath,
    overlay: &OverlayFile,
    new: &NewStep,
) -> Result<(OverlayFile, String), OverlayError> {
    stage_applies(path, new.stage)?;
    let after = t
        .step(&new.after)
        .ok_or_else(|| OverlayError::UnknownStep(new.after.clone()))?;
    if after.stage != new.stage {
        return Err(OverlayError::WrongStage(new.after.clone(), new.stage));
    }
    if t.step(&new.copied_from).is_none() {
        return Err(OverlayError::UnknownStep(new.copied_from.clone()));
    }
    validate_step(
        &new.title,
        &new.text,
        new.surface,
        new.cwd,
        &new.start,
        &new.out,
    )?;
    let prefix = format!("x-{}-", new.stage);
    let n = next_n(overlay.added_steps.iter().map(|s| s.id.clone()), &prefix)?;
    let id = format!("{prefix}{n}");
    StepKey::parse(&id).map_err(|_| OverlayError::TooMany)?;
    let mut o = overlay.clone();
    o.added_steps.push(AddedStep {
        id: id.clone(),
        stage: new.stage,
        after: StepKey::parse(&new.after)
            .map_err(|_| OverlayError::UnknownStep(new.after.clone()))?,
        copied_from: StepKey::parse(&new.copied_from)
            .map_err(|_| OverlayError::UnknownStep(new.copied_from.clone()))?,
        title: new.title.trim().to_owned(),
        tag: new.tag,
        text: new.text.clone(),
        surface: new.surface,
        cwd: new.cwd,
        start: new.start,
        out: new.out.clone(),
    });
    Ok((o, id))
}

/// Adds an inspection item (DR-060): ID `xi-<stage>-<n>`, kind `attest` only. Returns the new
/// overlay and the ID.
///
/// # Errors
/// See [`OverlayError`].
pub fn add_item(
    path: &ApplicablePath,
    overlay: &OverlayFile,
    stage: u8,
    text: &str,
) -> Result<(OverlayFile, String), OverlayError> {
    stage_applies(path, stage)?;
    if has_bypass(text) {
        return Err(OverlayError::BypassPermissions);
    }
    if !one_line(text, TITLE_MAX) {
        return Err(OverlayError::BadText);
    }
    let prefix = format!("xi-{stage}-");
    let n = next_n(overlay.added_items.iter().map(|i| i.id.clone()), &prefix)?;
    let id = format!("{prefix}{n}");
    let mut o = overlay.clone();
    o.added_items.push(AddedItem {
        id: id.clone(),
        stage,
        text: text.trim().to_owned(),
        kind: "attest".to_owned(),
    });
    Ok((o, id))
}

/// The synthetic template step for an added one (DR-059): a `prompt` that always starts a new
/// session, never joins a chain, and has no `when`.
fn synth(a: &AddedStep) -> Option<Step> {
    let key = StepKey::parse(&a.id).ok()?;
    let mut text = a.text.clone();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Some(Step {
        key,
        id: a.id.clone(),
        stage: a.stage,
        title: a.title.clone(),
        line: 0,
        status: StepStatus::Runnable,
        annotation: None,
        kind: StepKind::Prompt,
        tag: match a.tag {
            AddedTag::Optional => Tag::Optional,
            AddedTag::Core => Tag::Core,
        },
        surface: Some(a.surface),
        cwd: a.cwd,
        session: Session::New,
        chain: None,
        start: Some(a.start),
        out: a.out.clone(),
        appends: None,
        run_with: None,
        prompt: Some(Prompt {
            parts: vec![PromptPart {
                text,
                line: 0,
                cond: None,
            }],
        }),
        template_id: None,
    })
}

/// A copy of the pinned template with the overlay's added steps spliced in after the step each
/// names, in the order they were added (so a later addition after the same step sits directly
/// below it). An added step that no longer passes [`validate_step`] (a hand-edited overlay), names
/// a step that doesn't exist, or reuses an ID is left out and reported in the second value
/// (`rules.invalid-added-step`); nothing it says is ever run (AC-F025-05).
#[must_use]
pub fn effective_template(t: &Template, overlay: &OverlayFile) -> (Template, Vec<String>) {
    let mut out = t.clone();
    let mut flags = Vec::new();
    for a in &overlay.added_steps {
        let problem = validate_step(&a.title, &a.text, a.surface, a.cwd, &a.start, &a.out)
            .err()
            .map(|e| e.to_string())
            .or_else(|| {
                out.step(a.id.as_str())
                    .is_some()
                    .then(|| "that ID is already a step".to_owned())
            });
        let at = out
            .steps
            .iter()
            .position(|s| s.key == a.after && s.stage == a.stage);
        match (problem, at, synth(a)) {
            (None, Some(i), Some(step)) => out.steps.insert(i + 1, step),
            (Some(p), _, _) => flags.push(format!("rules.invalid-added-step: {}: {p}", a.id)),
            _ => flags.push(format!(
                "rules.invalid-added-step: {}: it goes after {}, which isn't in Stage {}",
                a.id, a.after, a.stage
            )),
        }
    }
    (out, flags)
}
