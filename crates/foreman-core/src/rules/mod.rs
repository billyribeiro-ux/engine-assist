//! Tags, tri-state conditions and the applicable path (F-004, DR-050–DR-058). Every state here is
//! derived on each read from the template, the approved decisions and the overlay; none is stored.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::ids::{ConditionId, StepKey, TriState};
use crate::schema::overlay::{OverlayFile, Skip};
use crate::schema::project::{ConditionAudit, Conditions, ProjectFile};
use crate::template::{Expr, Step, StepKind, StepStatus, Tag, Template};

pub mod overlay;

pub use crate::template::expr::{ExprError, parse_expr};

/// Kleene evaluation (DR-051): `!U = U`; `Y & U = U`, `N & U = N`; `Y | U = Y`, `N | U = U`.
#[must_use]
pub fn eval(e: &Expr, c: &Conditions) -> TriState {
    match e {
        Expr::Id(id) => c.get(id).copied().unwrap_or(TriState::Undecided),
        Expr::Not(a) => match eval(a, c) {
            TriState::Yes => TriState::No,
            TriState::No => TriState::Yes,
            TriState::Undecided => TriState::Undecided,
        },
        Expr::And(a, b) => match (eval(a, c), eval(b, c)) {
            (TriState::No, _) | (_, TriState::No) => TriState::No,
            (TriState::Yes, TriState::Yes) => TriState::Yes,
            _ => TriState::Undecided,
        },
        Expr::Or(a, b) => match (eval(a, c), eval(b, c)) {
            (TriState::Yes, _) | (_, TriState::Yes) => TriState::Yes,
            (TriState::No, TriState::No) => TriState::No,
            _ => TriState::Undecided,
        },
    }
}

/// A step's state on the applicable path (DR-050).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "state", content = "detail")]
pub enum Applicability {
    /// On the path; can't be skipped.
    Applicable,
    /// On the path; may be skipped with a reason.
    Skippable,
    /// Skipped by Billy (overlay), with the reason.
    Skipped(String),
    /// Off the path, with the reason (`commercial = no`).
    NotApplicable(String),
    /// Waits for these conditions to be decided.
    Undecided(Vec<ConditionId>),
    /// No annotation.
    NeedsAnnotation,
    /// A newer grammar.
    NeedsNewerForeman,
    /// A template error in its stage.
    Invalid,
}

impl Applicability {
    /// On the path (applicable or skippable).
    #[must_use]
    pub fn on_path(&self) -> bool {
        matches!(self, Self::Applicable | Self::Skippable)
    }
}

/// A stage's state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "state", content = "detail")]
pub enum StageState {
    /// Something applies.
    Applicable,
    /// Nothing applies (DR-054): no gate, no F.2, no packet.
    NotApplicable(String),
    /// Every step needs an annotation.
    NeedsAnnotation,
}

/// The applicable path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicablePath {
    /// Every step in template order with its state.
    pub steps: Vec<(StepKey, Applicability)>,
    /// Every stage with its state.
    pub stages: Vec<(u8, StageState)>,
    /// Problems found (a recorded skip on a step that is no longer optional, DR-056).
    pub flags: Vec<String>,
}

impl ApplicablePath {
    /// The state of a step.
    #[must_use]
    pub fn step(&self, key: &str) -> Option<&Applicability> {
        self.steps
            .iter()
            .find(|(k, _)| k.as_str() == key)
            .map(|(_, a)| a)
    }

    /// The state of a stage.
    #[must_use]
    pub fn stage(&self, n: u8) -> Option<&StageState> {
        self.stages.iter().find(|(s, _)| *s == n).map(|(_, a)| a)
    }

    /// Whether a stage applies.
    #[must_use]
    pub fn stage_applies(&self, n: u8) -> bool {
        matches!(
            self.stage(n),
            Some(StageState::Applicable | StageState::NeedsAnnotation)
        )
    }

    /// The lowest applicable stage above `n` (DR-054).
    #[must_use]
    pub fn next_applicable_stage(&self, n: u8) -> Option<u8> {
        self.stages
            .iter()
            .map(|(s, _)| *s)
            .find(|s| *s > n && self.stage_applies(*s))
    }

    /// The latest applicable stage below `n`.
    #[must_use]
    pub fn previous_applicable_stage(&self, n: u8) -> Option<u8> {
        self.stages
            .iter()
            .rev()
            .map(|(s, _)| *s)
            .find(|s| *s < n && self.stage_applies(*s))
    }
}

/// `commercial = no, payments = undecided` for the conditions an expression names.
#[must_use]
pub fn explain(e: &Expr, c: &Conditions) -> String {
    let mut seen = BTreeSet::new();
    e.conditions()
        .into_iter()
        .filter(|id| seen.insert(*id))
        .map(|id| {
            format!(
                "{} = {}",
                id.as_str(),
                c.get(&id).copied().unwrap_or(TriState::Undecided).as_str()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn undecided_of(e: &Expr, c: &Conditions) -> Vec<ConditionId> {
    let mut v: Vec<ConditionId> = e
        .conditions()
        .into_iter()
        .filter(|id| c.get(id).copied().unwrap_or(TriState::Undecided) == TriState::Undecided)
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The state of one step by the DR-050 table (before overlay skips).
#[must_use]
pub fn step_state(s: &Step, c: &Conditions) -> Applicability {
    match s.status {
        StepStatus::NeedsAnnotation => return Applicability::NeedsAnnotation,
        StepStatus::NeedsNewerForeman => return Applicability::NeedsNewerForeman,
        StepStatus::Invalid => return Applicability::Invalid,
        StepStatus::Runnable => {}
    }
    if s.template_id.is_some() {
        return Applicability::Applicable; // gate and F.2 instances are core (DR-057)
    }
    let when = s.when();
    match (s.tag, when) {
        (Tag::Core, _) => Applicability::Applicable,
        (Tag::Optional, None) => Applicability::Skippable,
        (Tag::Conditional | Tag::Optional, Some(e)) => match eval(e, c) {
            TriState::Yes => {
                if s.tag == Tag::Conditional {
                    Applicability::Applicable
                } else {
                    Applicability::Skippable
                }
            }
            TriState::No => Applicability::NotApplicable(explain(e, c)),
            TriState::Undecided => Applicability::Undecided(undecided_of(e, c)),
        },
        (Tag::Conditional, None) => Applicability::Invalid,
    }
}

/// Computes the applicable path (DR-050, DR-054, DR-056).
#[must_use]
pub fn applicable_path(t: &Template, c: &Conditions, overlay: &OverlayFile) -> ApplicablePath {
    let mut flags = Vec::new();
    let mut steps: Vec<(StepKey, Applicability)> = Vec::with_capacity(t.steps.len());
    for s in &t.steps {
        let mut a = step_state(s, c);
        if let Some(skip) = overlay.skips.get(&s.key) {
            if a == Applicability::Skippable {
                a = Applicability::Skipped(skip.reason.clone());
            } else if a.on_path() || matches!(a, Applicability::Undecided(_)) {
                flags.push(format!(
                    "rules.stale-skip: {} is no longer optional; its recorded skip is ignored",
                    s.key
                ));
            }
        }
        steps.push((s.key.clone(), a));
    }
    // Stages (DR-054): template steps only (instances follow their stage).
    let mut stages = Vec::new();
    for st in &t.stages {
        let own: Vec<&Applicability> = t
            .steps
            .iter()
            .zip(steps.iter())
            .filter(|(s, _)| {
                s.stage == st.number && s.template_id.is_none() && !s.key.as_str().starts_with("x-")
            })
            .map(|(_, (_, a))| a)
            .collect();
        let state = if own.is_empty()
            || own.iter().all(|a| {
                matches!(
                    a,
                    Applicability::NeedsAnnotation
                        | Applicability::NeedsNewerForeman
                        | Applicability::Invalid
                )
            }) {
            StageState::NeedsAnnotation
        } else if own.iter().all(|a| {
            matches!(
                a,
                Applicability::NotApplicable(_) | Applicability::Skipped(_)
            )
        }) {
            let reasons: Vec<String> = own
                .iter()
                .filter_map(|a| match a {
                    Applicability::NotApplicable(r) | Applicability::Skipped(r) => Some(r.clone()),
                    _ => None,
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            StageState::NotApplicable(reasons.join("; "))
        } else {
            StageState::Applicable
        };
        stages.push((st.number, state));
    }
    // A stage that doesn't apply has no gate and no F.2.
    for (s, (_, a)) in t.steps.iter().zip(steps.iter_mut()) {
        if s.template_id.is_some()
            && let Some((_, StageState::NotApplicable(r))) =
                stages.iter().find(|(n, _)| *n == s.stage)
        {
            *a = Applicability::NotApplicable(format!("Stage {} doesn't apply ({r})", s.stage));
        }
    }
    ApplicablePath {
        steps,
        stages,
        flags,
    }
}

/// The packet path of a stage (`docs/handoff/stage-NN.md`, DR-130).
#[must_use]
pub fn packet_path(stage: u8) -> String {
    format!("docs/handoff/stage-{stage:02}.md")
}

/// DR-053: the applicable step whose `out` names the stage's packet (`N:gate.B` for 5–17).
#[must_use]
pub fn packet_producer<'a>(t: &'a Template, path: &ApplicablePath, stage: u8) -> Option<&'a Step> {
    let pkt = packet_path(stage);
    t.steps
        .iter()
        .filter(|s| s.stage == stage && s.out.contains(&pkt))
        .find(|s| {
            path.step(s.key.as_str())
                .is_some_and(Applicability::on_path)
        })
}

/// DR-055: the SKIPPED lines for stage `stage`'s packet: every skipped or not-applicable step on
/// the stage's path, the removed parts (`parts`, from the resolver), and every not-applicable stage
/// since the last signed packet (`since`, exclusive).
#[must_use]
pub fn skipped_queue(
    t: &Template,
    path: &ApplicablePath,
    stage: u8,
    since: Option<u8>,
    parts: &[String],
) -> Vec<String> {
    let mut out = Vec::new();
    for (n, st) in &path.stages {
        if *n < stage
            && since.is_none_or(|s| *n > s)
            && let StageState::NotApplicable(r) = st
        {
            let title = t
                .stages
                .iter()
                .find(|s| s.number == *n)
                .map_or("", |s| s.title.as_str());
            out.push(format!("Stage {n} {title}: not applicable ({r})"));
        }
    }
    for s in t
        .steps
        .iter()
        .filter(|s| s.stage == stage && s.template_id.is_none())
    {
        match path.step(s.key.as_str()) {
            Some(Applicability::NotApplicable(r)) => {
                out.push(format!("{} {}: not applicable ({r})", s.key, s.title));
            }
            Some(Applicability::Skipped(r)) => {
                out.push(format!("{} {}: skipped: {r}", s.key, s.title));
            }
            _ => {}
        }
    }
    out.extend(parts.iter().cloned());
    out
}

/// A refused rule operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    /// Core steps can't be skipped.
    #[error(
        "core-step-skip: Core: protects production quality. Change it in the blueprint with a Change Request."
    )]
    CoreStepSkip,
    /// Conditional steps follow their condition.
    #[error("conditional-step-skip: a conditional step applies when its condition is true")]
    ConditionalStepSkip,
    /// Only skippable steps can be skipped (not applicable, undecided, needs annotation…).
    #[error("not-skippable: {0}")]
    NotSkippable(String),
    /// The reason must be 1–200 characters.
    #[error("skip-reason: a skip needs a one-line reason of 1–200 characters")]
    BadReason,
    /// Gate items, F.2, packets and gate steps are core (DR-057).
    #[error(
        "gate-item-skip: inspection items and the gate are core; attest \"not applicable\" with a reason instead"
    )]
    GateItemSkip,
    /// The condition's edit window closed (DR-052).
    #[error("Change requires a Change Request (v1.1): docs/templates/change-request.md")]
    ConditionLocked,
    /// No such step.
    #[error("unknown step {0}")]
    UnknownStep(String),
}

/// Skips a skippable step with a reason (DR-056), returning the new overlay.
///
/// # Errors
/// See [`RuleError`].
pub fn skip(
    t: &Template,
    path: &ApplicablePath,
    overlay: &OverlayFile,
    key: &str,
    reason: &str,
    at: &str,
) -> Result<OverlayFile, RuleError> {
    let s = t
        .step(key)
        .ok_or_else(|| RuleError::UnknownStep(key.to_owned()))?;
    if s.template_id.is_some() {
        return Err(RuleError::GateItemSkip);
    }
    match s.tag {
        Tag::Core => return Err(RuleError::CoreStepSkip),
        Tag::Conditional => return Err(RuleError::ConditionalStepSkip),
        Tag::Optional => {}
    }
    match path.step(key) {
        Some(Applicability::Skippable) => {}
        Some(other) => return Err(RuleError::NotSkippable(format!("{key} is {other:?}"))),
        None => return Err(RuleError::UnknownStep(key.to_owned())),
    }
    let reason = reason.trim();
    if reason.is_empty() || reason.chars().count() > 200 || reason.contains('\n') {
        return Err(RuleError::BadReason);
    }
    let mut o = overlay.clone();
    o.skips.insert(
        s.key.clone(),
        Skip {
            reason: reason.to_owned(),
            at: at.to_owned(),
        },
    );
    Ok(o)
}

/// Inspection items can never be skipped (DR-057).
///
/// # Errors
/// Always [`RuleError::GateItemSkip`].
pub fn skip_item(_item: &str) -> Result<(), RuleError> {
    Err(RuleError::GateItemSkip)
}

/// Changes a condition inside its edit window (DR-052), appending the audit line.
///
/// # Errors
/// [`RuleError::ConditionLocked`] after the window.
pub fn set_condition(
    d: &ProjectFile,
    id: ConditionId,
    to: TriState,
    note: &str,
    at: &str,
    signed: &BTreeSet<u8>,
) -> Result<ProjectFile, RuleError> {
    let locked_by = if id.is_late() { 4 } else { 2 };
    if signed.contains(&locked_by) {
        return Err(RuleError::ConditionLocked);
    }
    let mut out = d.clone();
    let from = out
        .conditions
        .get(&id)
        .copied()
        .unwrap_or(TriState::Undecided);
    out.conditions.insert(id, to);
    out.condition_audit.push(ConditionAudit {
        condition: id,
        from,
        to,
        at: at.to_owned(),
        note: note.to_owned(),
    });
    Ok(out)
}

/// Conditions from a list (missing ones are undecided).
#[must_use]
pub fn conditions(pairs: &[(ConditionId, TriState)]) -> Conditions {
    let mut c: Conditions = ConditionId::ALL
        .iter()
        .map(|id| (*id, TriState::Undecided))
        .collect();
    for (k, v) in pairs {
        c.insert(*k, *v);
    }
    c
}

/// Whether a kind counts toward stage applicability (DR-054).
#[must_use]
pub fn counts_for_stage(kind: StepKind) -> bool {
    matches!(
        kind,
        StepKind::Prompt
            | StepKind::Followup
            | StepKind::Action
            | StepKind::Manual
            | StepKind::Wait
            | StepKind::Repeatable
    )
}

/// Every step key on the path, by state (handy for snapshots).
#[must_use]
pub fn summary(path: &ApplicablePath) -> BTreeMap<String, String> {
    path.steps
        .iter()
        .map(|(k, a)| {
            (
                k.to_string(),
                match a {
                    Applicability::Applicable => "applicable".into(),
                    Applicability::Skippable => "skippable".into(),
                    Applicability::Skipped(r) => format!("skipped({r})"),
                    Applicability::NotApplicable(r) => format!("notApplicable({r})"),
                    Applicability::Undecided(c) => format!(
                        "undecided({})",
                        c.iter().map(|x| x.as_str()).collect::<Vec<_>>().join(",")
                    ),
                    Applicability::NeedsAnnotation => "needsAnnotation".into(),
                    Applicability::NeedsNewerForeman => "needsNewerForeman".into(),
                    Applicability::Invalid => "invalid".into(),
                },
            )
        })
        .collect()
}
