//! Run configs and model policy (F-005, DR-100–DR-106): effective start =
//! override > substitution > alternative > template, each value with its origin.

use serde::{Deserialize, Serialize};

use crate::ids::{CwdKind, Effort, Mode, Model, Start, StepKey, Surface};
use crate::schema::project::{ModelPolicy, Plan};
use crate::template::{AltRule, Session, Step, Switch};

/// Where a value came from (`docs/contracts/ipc.md` `ValueOrigin`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ValueOrigin {
    /// The annotation.
    Template,
    /// An `alt=` rule selected by the plan.
    Alternative {
        /// `pro-plan` / `max-plan`.
        rule: String,
    },
    /// A project substitution.
    Substitution {
        /// `fable → opus@xhigh`.
        rule: String,
    },
    /// A per-run override.
    Override,
}

/// A value with its origin and the template value it replaced (F-005 "explain").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Explained<T> {
    /// The effective value.
    pub value: T,
    /// The last layer that changed it.
    pub origin: ValueOrigin,
    /// The template value, when different.
    pub from: Option<T>,
}

/// A per-run override (DR-103): each part optional, next run only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunOverride {
    /// Model.
    pub model: Option<Model>,
    /// Effort.
    pub effort: Option<Effort>,
    /// Mode.
    pub mode: Option<Mode>,
}

/// A step's run config (DR-100).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunConfig {
    /// Surface.
    pub surface: Option<Surface>,
    /// cwd.
    pub cwd: Option<CwdKind>,
    /// Session.
    pub session: Session,
    /// Chain start.
    pub chain: Option<StepKey>,
    /// Template start.
    pub start: Option<Start>,
    /// Switches (cues).
    pub switches: Vec<Switch>,
    /// Alternatives `(rule, model, effort)`.
    pub alts: Vec<(String, Model, Effort)>,
    /// Display-only, from the RUN WITH text.
    pub agents: Option<String>,
}

/// A policy problem.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    /// The step has no start (needs annotation, or not a prompt kind).
    #[error("{0} has no run config")]
    NoStart(String),
    /// An override that doesn't fit the surface (DR-103).
    #[error("override doesn't fit a {surface} step: {why}")]
    OverrideDoesntFit {
        /// Surface.
        surface: &'static str,
        /// Why.
        why: String,
    },
}

/// DR-100: the run config from the annotation and inheritance.
#[must_use]
pub fn run_config(step: &Step) -> RunConfig {
    let agents = step.run_with.as_ref().and_then(|rw| {
        rw.text
            .split("agents:")
            .nth(1)
            .map(|a| a.lines().next().unwrap_or_default().trim().to_owned())
    });
    RunConfig {
        surface: step.surface,
        cwd: step.cwd,
        session: step.session,
        chain: step.chain.clone(),
        start: step.start,
        switches: step.switches().to_vec(),
        alts: step
            .alts()
            .iter()
            .map(|a| (a.rule.as_str().to_owned(), a.model, a.effort))
            .collect(),
        agents,
    }
}

fn plan_rule(p: Plan) -> AltRule {
    match p {
        Plan::Pro => AltRule::ProPlan,
        Plan::Max => AltRule::MaxPlan,
    }
}

fn is_chat(step: &Step) -> bool {
    step.surface == Some(Surface::Chat)
}

/// Applies the alternative and the substitutions to one start (DR-101, DR-102).
fn apply_policy(step: &Step, s: Start, p: &ModelPolicy) -> (Start, ValueOrigin) {
    let mut out = s;
    let mut origin = ValueOrigin::Template;
    if let Some(a) = step.alts().iter().find(|a| a.rule == plan_rule(p.plan)) {
        out.model = a.model;
        out.effort = a.effort;
        origin = ValueOrigin::Alternative {
            rule: a.rule.as_str().into(),
        };
    }
    for sub in &p.substitutions {
        if out.model == sub.from {
            out.model = sub.to;
            if let Some(e) = sub.effort
                && !is_chat(step)
            {
                out.effort = e;
            }
            let eff = sub
                .effort
                .map(|e| format!("@{}", e.as_str()))
                .unwrap_or_default();
            origin = ValueOrigin::Substitution {
                rule: format!("{} → {}{eff}", sub.from.as_str(), sub.to.as_str()),
            };
            break; // once, not transitively
        }
    }
    (out, origin)
}

/// The policy start (template + alternative + substitution): the confirmation baseline (DR-106).
///
/// # Errors
/// [`PolicyError::NoStart`].
pub fn policy_start(step: &Step, p: &ModelPolicy) -> Result<Explained<Start>, PolicyError> {
    let s = step
        .start
        .ok_or_else(|| PolicyError::NoStart(step.key.to_string()))?;
    let (v, origin) = apply_policy(step, s, p);
    Ok(Explained {
        value: v,
        origin,
        from: (v != s).then_some(s),
    })
}

/// DR-104: the effective start (override > substitution > alternative > template).
///
/// # Errors
/// [`PolicyError`] when there's no start or the override doesn't fit the surface.
pub fn effective_start(
    step: &Step,
    p: &ModelPolicy,
    ovr: Option<&RunOverride>,
) -> Result<Explained<Start>, PolicyError> {
    let base = policy_start(step, p)?;
    let Some(o) = ovr.filter(|o| o.model.is_some() || o.effort.is_some() || o.mode.is_some())
    else {
        return Ok(base);
    };
    let chat = is_chat(step);
    let surface = if chat { "chat" } else { "cli" };
    if chat && o.mode.is_some() {
        return Err(PolicyError::OverrideDoesntFit {
            surface,
            why: "chat steps have no mode".into(),
        });
    }
    if let Some(e) = o.effort
        && chat != (e == Effort::Adaptive)
    {
        return Err(PolicyError::OverrideDoesntFit {
            surface,
            why: format!("effort {} doesn't fit", e.as_str()),
        });
    }
    let mut v = base.value;
    if let Some(m) = o.model {
        v.model = m;
    }
    if let Some(e) = o.effort {
        v.effort = e;
    }
    if let Some(m) = o.mode {
        v.mode = Some(m);
    }
    let template = step
        .start
        .ok_or_else(|| PolicyError::NoStart(step.key.to_string()))?;
    Ok(Explained {
        value: v,
        origin: ValueOrigin::Override,
        from: (v != template).then_some(template),
    })
}

/// The switches with the alternative and substitutions applied (DR-101: "and every switch").
#[must_use]
pub fn effective_switches(step: &Step, p: &ModelPolicy) -> Vec<Switch> {
    step.switches()
        .iter()
        .map(|w| Switch {
            trigger: w.trigger.clone(),
            to: apply_policy(step, w.to, p).0,
        })
        .collect()
}

/// DR-104: the `SUBSTITUTIONS:` line for a step whose effective start differs from its template.
#[must_use]
pub fn substitution_line(step: &Step, eff: &Explained<Start>) -> Option<String> {
    let template = step.start?;
    if eff.value == template {
        return None;
    }
    let origin = match &eff.origin {
        ValueOrigin::Template => return None,
        ValueOrigin::Alternative { rule } => format!("alternative {rule}"),
        ValueOrigin::Substitution { rule } => format!("substitution {rule}"),
        ValueOrigin::Override => "override".into(),
    };
    Some(format!(
        "{}: {template} → {} ({origin})",
        step.key, eff.value
    ))
}

fn mode_name(m: Option<Mode>) -> &'static str {
    match m {
        Some(Mode::Plan) => "Plan",
        Some(Mode::Default) => "Manual",
        Some(Mode::AcceptEdits) => "Accept edits",
        Some(Mode::Auto) => "Auto",
        Some(Mode::DontAsk) => "dontAsk",
        None => "",
    }
}

/// DR-105: switches are shown as cues; Foreman never sends them as keystrokes. This returns text.
#[must_use]
pub fn switch_cues(step: &Step, p: &ModelPolicy) -> Vec<String> {
    effective_switches(step, p)
        .iter()
        .map(|w| {
            let when = if w.trigger == "plan-approved" {
                "after you approve the plan".to_owned()
            } else {
                format!("on {}", w.trigger)
            };
            format!("Next: {when}, choose {}", mode_name(w.to.mode))
        })
        .collect()
}
