//! A read-only pin check (CR-003, DR-155) for the setup preview: the P1 annotation errors that
//! [`crate::lock::pin`] refuses on, found without writing anything.
//!
//! It mirrors the private `lock::problem` and `lock::step_at`; a test pins the two together. The
//! core owner may fold it into `lock.rs`.

use crate::lock::{AnnotationProblem, LockError};
use crate::template::{FILE, Place, ScopedError, Template, parse_template_p1};

/// The errors a pin of `source` would refuse on, as [`LockError::AnnotationErrors`] (whose
/// `Display` is the refusal text).
///
/// # Errors
/// [`LockError::AnnotationErrors`] when a step in Stages 1–2 or F.2 has an annotation error.
pub fn p1_annotation_errors(source: &str) -> Result<(), LockError> {
    let (t, errors) = parse_template_p1(source, FILE);
    if errors.is_empty() {
        return Ok(());
    }
    Err(LockError::AnnotationErrors {
        problems: errors.into_iter().map(|e| problem(&t, e)).collect(),
    })
}

fn problem(t: &Template, e: ScopedError) -> AnnotationProblem {
    let stage = match e.place {
        Place::Stage(n) => Some(n),
        _ => e.diagnostic.stage,
    };
    AnnotationProblem {
        stage,
        step: step_at(t, e.place, e.diagnostic.line),
        diagnostic: e.diagnostic,
    }
}

fn step_at(t: &Template, place: Place, line: u32) -> Option<String> {
    let candidates: Vec<&crate::template::Step> = match place {
        Place::Stage(n) => t
            .steps
            .iter()
            .filter(|s| s.stage == n && s.template_id.is_none())
            .collect(),
        Place::StageGate => t.gate_templates.iter().collect(),
        Place::AppendixF => t.f2_template.iter().collect(),
        Place::Elsewhere => Vec::new(),
    };
    candidates
        .into_iter()
        .filter(|s| s.line <= line)
        .max_by_key(|s| s.line)
        .map(|s| s.id.clone())
}
