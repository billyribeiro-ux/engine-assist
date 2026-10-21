//! M1-30: the TradeLog harness (AC-F003-01, -03; EX-M1-03). Runs in `cargo test` (so in
//! `pnpm verify` and CI): every runnable step of Stages 1–2 and their F.2 instances resolves
//! against the real v2.2 blueprint with zero unresolved tokens.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::Fixture;
use foreman_core::ids::{Surface, TriState};
use foreman_core::resolver::{required_inputs, resolve};
use foreman_core::template::{Session, StepKind};

mod harness {
    use super::*;

    #[test]
    fn tradelog_stages_1_2() {
        let mut fx = Fixture::tradelog();
        // The fixture: every condition yes; the blueprint's example variables.
        assert!(fx.project.conditions.values().all(|v| *v == TriState::Yes));
        assert_eq!(fx.template.blueprint_version, "2.2");
        let keys: Vec<String> = fx
            .template
            .steps
            .iter()
            .filter(|s| matches!(s.stage, 1 | 2))
            .filter(|s| s.runnable() && matches!(s.kind, StepKind::Prompt | StepKind::Followup))
            .filter(|s| fx.path.step(s.key.as_str()).is_some_and(|a| a.on_path()))
            .map(|s| s.key.to_string())
            .collect();
        assert!(keys.contains(&"1:F.2".to_owned()) && keys.contains(&"2:F.2".to_owned()));
        assert!(keys.len() >= 12, "{keys:?}");
        let mut problems = Vec::new();
        let mut resolved = 0;
        for key in &keys {
            let step = fx.template.step(key).unwrap().clone();
            // Every CLI step has cwd, session and start (a continued session names its chain).
            if step.surface == Some(Surface::Cli) {
                if step.cwd.is_none() || step.start.is_none() {
                    problems.push(format!("{key}: CLI step without cwd or start"));
                }
                if step.session == Session::Continue && step.chain.is_none() {
                    problems.push(format!("{key}: continued session without a chain"));
                }
            }
            // Every run input the prompt needs is declared on the step.
            let declared: Vec<String> = step.inputs().iter().map(|i| i.name.clone()).collect();
            fx.inputs.clear();
            let needed = required_inputs(&fx.ctx(key));
            for n in &needed {
                if !declared.contains(n) {
                    problems.push(format!("{key}: run input {n} isn't declared"));
                }
            }
            for n in &declared {
                fx.inputs
                    .insert(n.clone(), format!("harness value for {n}"));
            }
            match resolve(&fx.ctx(key)) {
                Ok(p) => {
                    resolved += 1;
                    assert!(!p.text.is_empty(), "{key}");
                    for n in &needed {
                        assert!(
                            p.text.contains(&format!("harness value for {n}")),
                            "{key}: {n}"
                        );
                    }
                }
                Err(u) => problems.push(format!("{key}: unresolved {u:?}")),
            }
        }
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!(resolved, keys.len());
    }
}

/// M4-05: the harness through Stage 5 (AC-F003-04, AC-F006-15, EX-M4-02, NFR-48).
mod harness_p2 {
    use super::*;
    use foreman_core::resolver::tokens::scan;

    /// Every runnable prompt or followup step of Stages 1–5, their F.2 instances and the Stage
    /// Gate prompts (A, B, C) of Stages 1–5 resolves against the real v2.2 blueprint with zero
    /// unresolved tokens, and every token in Stages 3–5 is classified.
    #[test]
    fn tradelog_stages_1_5() {
        let mut fx = Fixture::tradelog();
        let keys: Vec<String> = fx
            .template
            .steps
            .iter()
            .filter(|s| (1..=5).contains(&s.stage))
            .filter(|s| s.runnable() && matches!(s.kind, StepKind::Prompt | StepKind::Followup))
            .filter(|s| fx.path.step(s.key.as_str()).is_some_and(|a| a.on_path()))
            .map(|s| s.key.to_string())
            .collect();
        // The chat stages 1–4 close with their F.2 instance; Stage 5 is the first with the Stage
        // Gate prompts (A, B, C).
        for n in 1..=4 {
            assert!(keys.contains(&format!("{n}:F.2")), "{n}:F.2 in {keys:?}");
        }
        for g in ["A", "B", "C"] {
            assert!(
                keys.contains(&format!("5:gate.{g}")),
                "5:gate.{g} in {keys:?}"
            );
        }
        // Stages 3–5 contribute their own steps, not only the gate and F.2.
        for n in 3..=5u8 {
            let own = fx
                .template
                .steps
                .iter()
                .filter(|s| s.stage == n && s.template_id.is_none() && s.runnable())
                .count();
            assert!(own > 0, "Stage {n} has no runnable steps of its own");
        }
        let mut problems = Vec::new();
        let mut resolved = 0;
        for key in &keys {
            let step = fx.template.step(key).unwrap().clone();
            if step.surface == Some(Surface::Cli) {
                if step.cwd.is_none() || step.start.is_none() {
                    problems.push(format!("{key}: CLI step without cwd or start"));
                }
                if step.session == Session::Continue && step.chain.is_none() {
                    problems.push(format!("{key}: continued session without a chain"));
                }
            }
            let declared: Vec<String> = step.inputs().iter().map(|i| i.name.clone()).collect();
            // AC-F006-15: every token is classified.
            if let Some(p) = &step.prompt {
                let (toks, errs) = scan(&p.joined(), &declared);
                if !errs.is_empty() {
                    problems.push(format!("{key}: scan errors {errs:?}"));
                }
                for t in toks.iter().filter(|t| t.class.is_none()) {
                    problems.push(format!("{key}: unclassified {}", t.text));
                }
            }
            fx.inputs.clear();
            let needed = required_inputs(&fx.ctx(key));
            for n in &needed {
                if !declared.contains(n) {
                    problems.push(format!("{key}: run input {n} isn't declared"));
                }
            }
            for n in &declared {
                fx.inputs
                    .insert(n.clone(), format!("harness value for {n}"));
            }
            match resolve(&fx.ctx(key)) {
                Ok(p) => {
                    resolved += 1;
                    if p.text.is_empty() {
                        problems.push(format!("{key}: empty prompt"));
                    }
                    // No bracket token of a resolvable class is left in the text.
                    let (left, _) = scan(&p.text, &[]);
                    for t in left.iter().filter(|t| {
                        matches!(
                            t.class,
                            Some(
                                foreman_core::resolver::tokens::TokenClass::ProjectVariable
                                    | foreman_core::resolver::tokens::TokenClass::PacketMarker
                                    | foreman_core::resolver::tokens::TokenClass::PathAlias
                            )
                        )
                    }) {
                        problems.push(format!("{key}: {} left in the prompt", t.text));
                    }
                    for n in &needed {
                        if !p.text.contains(&format!("harness value for {n}")) {
                            problems.push(format!("{key}: input {n} not substituted"));
                        }
                    }
                }
                Err(u) => problems.push(format!("{key}: unresolved {u:?}")),
            }
        }
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!(resolved, keys.len());
        // The Stages 1–2 harness is a subset.
        assert!(keys.len() > 30, "{}", keys.len());
    }
}
