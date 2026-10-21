//! M1-25: action steps with fixed destinations (DR-036, SR-34, AC-F007-02/-03).
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

use common::{blueprint, parse, replace_once};
use foreman_core::steps::actions::{self as act, ActionError};
use foreman_core::store::{Root, RootKind, StoreError};

fn project() -> (tempfile::TempDir, Root) {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("docs/handoff")).unwrap();
    let r = Root::open(d.path(), RootKind::Project).unwrap();
    (d, r)
}

mod actions {
    use super::*;

    #[test]
    fn destinations_fixed() {
        let t = parse(&blueprint());
        let (d, root) = project();
        let plan = act::plan(&t, "council-setup", &root).unwrap();
        let dests: Vec<&str> = plan.writes.iter().map(|w| w.path.as_str()).collect();
        assert_eq!(
            dests,
            [
                "spec/.claude/agents/market-analyst.md",
                "spec/.claude/agents/user-advocate.md",
                "spec/.claude/agents/product-visionary.md",
                "spec/.claude/agents/principal-engineer.md",
                "spec/.claude/agents/growth-strategist.md",
                "spec/.claude/agents/red-team.md",
            ]
        );
        act::apply(&plan, &root, false).unwrap();
        let ma = std::fs::read_to_string(d.path().join("spec/.claude/agents/market-analyst.md"))
            .unwrap();
        assert!(ma.starts_with("---\nname: market-analyst\n"));
        // A fence naming another destination is refused (template error) and never written.
        let evil = parse(&replace_once(
            &blueprint(),
            "name: market-analyst\n",
            "name: ../../evil\n",
        ));
        assert!(
            evil.diagnostics
                .iter()
                .any(|x| x.rule == "grammar.unknown-action-target")
        );
        let (d2, root2) = project();
        let plan = act::plan(&evil, "council-setup", &root2).unwrap();
        assert_eq!(plan.refused, ["../../evil"]);
        act::apply(&plan, &root2, false).unwrap();
        assert!(!d2.path().join("evil").exists() && !d2.path().join("spec/evil.md").exists());
        // 1.4b copies the Stage 1 packet (plus attachments) to the constant destination.
        std::fs::write(
            d.path().join("docs/handoff/stage-01.md"),
            "=== HANDOFF PACKET · Stage 1 → Stage 2 ===\n",
        )
        .unwrap();
        std::fs::write(
            d.path().join("docs/handoff/stage-01-attachments.md"),
            "PRESS RELEASE\n",
        )
        .unwrap();
        let p14 = act::plan(&t, "1.4b", &root).unwrap();
        assert_eq!(p14.writes[0].path.as_str(), act::APP_BRIEF);
        act::apply(&p14, &root, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(d.path().join("spec/01-app-brief.md")).unwrap(),
            "=== HANDOFF PACKET · Stage 1 → Stage 2 ===\n\nPRESS RELEASE\n"
        );
        assert!(matches!(
            act::plan(&t, "2.2", &root),
            Err(ActionError::Unknown(_))
        ));
        // AC-F007-03: an existing destination is listed and written only after confirmation; one
        // that changed since it was read is refused.
        std::fs::write(
            d.path().join("docs/handoff/stage-01.md"),
            "=== HANDOFF PACKET · Stage 1 → Stage 2 ===\nPROJECT: TradeLog\n",
        )
        .unwrap();
        let again = act::plan(&t, "1.4b", &root).unwrap();
        assert!(again.writes[0].existing.is_some());
        assert!(matches!(
            act::apply(&again, &root, false),
            Err(ActionError::NeedsConfirmation(_))
        ));
        std::fs::write(
            d.path().join("spec/01-app-brief.md"),
            "edited by Billy meanwhile\n",
        )
        .unwrap();
        assert!(matches!(
            act::apply(&again, &root, true),
            Err(ActionError::Store(StoreError::Conflict { .. }))
        ));
        assert_eq!(
            std::fs::read_to_string(d.path().join("spec/01-app-brief.md")).unwrap(),
            "edited by Billy meanwhile\n"
        );
        let fresh_plan = act::plan(&t, "1.4b", &root).unwrap();
        act::apply(&fresh_plan, &root, true).unwrap();
        assert!(
            std::fs::read_to_string(d.path().join("spec/01-app-brief.md"))
                .unwrap()
                .contains("PROJECT: TradeLog")
        );
    }
}
