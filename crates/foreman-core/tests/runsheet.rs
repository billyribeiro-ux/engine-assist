//! M4-32: the run sheet (AC-F027-01, -03; NFR-14).
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

use std::collections::{BTreeMap, BTreeSet};

use common::Fixture;
use foreman_core::runsheet::{SheetInput, run_sheet};

fn sheet(f: &Fixture, stage: u8, inputs: &BTreeMap<String, BTreeMap<String, String>>) -> String {
    run_sheet(&SheetInput {
        template: &f.template,
        project: &f.project,
        path: &f.path,
        packets: &f.packets,
        inputs,
        remembered: &BTreeMap::new(),
        optional: &BTreeMap::new(),
        attested_na: &BTreeSet::new(),
        overlay_items: &[],
        stage,
        generated_on: "2026-10-20",
    })
}

mod runsheet {
    use super::*;

    /// AC-F027-01: Stage 2 of TradeLog matches the golden file: each applicable step's resolved
    /// prompt, its exact argv (CLI) or chat chips, the run inputs as filled, and the inspection
    /// items with their kinds.
    #[test]
    fn stage_2_golden() {
        let f = Fixture::tradelog();
        let mut inputs: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for s in f.template.steps.iter().filter(|s| s.stage == 2) {
            let m: BTreeMap<String, String> = s
                .inputs()
                .iter()
                .map(|i| (i.name.clone(), format!("TradeLog value for {}", i.name)))
                .collect();
            if !m.is_empty() {
                inputs.insert(s.key.to_string(), m);
            }
        }
        let md = sheet(&f, 2, &inputs);
        assert!(!md.contains("⟦UNRESOLVED"), "{md}");
        assert!(md.contains("Argv (one argument per line):"));
        assert!(md.contains("--permission-mode\nplan\n"));
        assert!(md.contains("## Inspection items"));
        assert!(md.contains("(attest)"));
        insta::assert_snapshot!(md);
    }

    /// AC-F027-03: unresolved required tokens don't stop the export; each is marked in place and
    /// listed under its step.
    #[test]
    fn unresolved_tokens_marked() {
        let mut f = Fixture::tradelog();
        f.project.variables.remove("APP_NAME");
        f.packets.remove(&1);
        let md = sheet(&f, 2, &BTreeMap::new());
        assert!(md.contains("⟦UNRESOLVED [APP_NAME]⟧"), "{md}");
        assert!(md.contains("- Unresolved (marked ⟦UNRESOLVED …⟧ below):"));
        assert!(md.contains("[APP_NAME]: the project variable is empty"));
        assert!(md.contains("the Stage 1 packet: not signed yet"));
        // A declared run input left empty is marked in the inputs list.
        if f.template.steps.iter().any(|s| s.stage == 2 && !s.inputs().is_empty()) {
            assert!(md.contains("⟦UNRESOLVED (not filled in)⟧"));
        }
        // Every applicable step still has its section.
        for s in f
            .template
            .steps
            .iter()
            .filter(|s| s.stage == 2 && f.path.step(s.key.as_str()).is_some_and(|a| a.on_path()))
        {
            assert!(md.contains(&format!("## {} {}", s.id, s.title)), "{}", s.key);
        }
    }
}
