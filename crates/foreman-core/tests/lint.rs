//! M1-15: prose lint and chat chips (DR-040–DR-043, AC-F001-03).
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

use common::{blueprint, parse, replace_once, rules};
use foreman_core::ids::{CwdKind, Effort, Mode, Model};
use foreman_core::template::Session;
use foreman_core::template::lint::{chat_chips as chips, infer};

mod template {
    use super::*;

    /// AC-F001-03: prose that disagrees never changes a value; it lints with the line.
    #[test]
    fn prose_change_lints_only() {
        let bp = blueprint();
        let edited = replace_once(
            &bp,
            "> **RUN WITH** · same session · same chair model · effort unchanged · **Accept edits**\n<!-- fm:step id=2.3",
            "> **RUN WITH** · fresh session · same chair model · effort unchanged · **Accept edits**\n<!-- fm:step id=2.3",
        );
        let (a, b) = (parse(&bp), parse(&edited));
        assert_eq!(b.step("2.3").unwrap().session, Session::Continue);
        let mut b23 = b.step("2.3").unwrap().clone();
        b23.run_with.clone_from(&a.step("2.3").unwrap().run_with);
        assert_eq!(a.step("2.3").unwrap(), &b23, "only the prose differs");
        let d = b
            .diagnostics
            .iter()
            .find(|d| d.rule == "lint.session-disagrees")
            .unwrap();
        assert_eq!(
            d.line,
            b.step("2.3").unwrap().run_with.as_ref().unwrap().line
        );
        assert!(!d.is_error());
        assert!(!rules(&a).contains(&"lint.session-disagrees".to_owned()));
        // The real v2.2 Stages 1–2 and the Gate lint clean.
        let noisy: Vec<_> = a
            .diagnostics
            .iter()
            .filter(|d| d.stage.is_some_and(|s| s <= 3) || d.stage.is_none())
            .collect();
        assert!(noisy.is_empty(), "{noisy:?}");
    }

    /// DR-041: one case per inference rule.
    #[test]
    fn inference_rules() {
        // (1) session
        assert_eq!(
            infer("same session · x", "", None).session,
            Some(Session::Continue)
        );
        assert_eq!(
            infer("same chat", "", None).session,
            Some(Session::Continue)
        );
        for p in [
            "CLI, fresh session",
            "new session",
            "new conversation in the Project",
            "a **brand-new conversation**",
        ] {
            assert_eq!(infer(p, "", None).session, Some(Session::New), "{p}");
        }
        // (2) cwd
        assert_eq!(
            infer("Claude Code CLI in `[app]-spec/` ·", "", None).cwd,
            Some(CwdKind::Spec)
        );
        assert_eq!(
            infer("Claude Code CLI in `spec/` ·", "", None).cwd,
            Some(CwdKind::Spec)
        );
        // (3) modes and switches
        let i = infer("**Plan** mode first, then **Accept edits**", "", None);
        assert_eq!(
            (i.mode, i.switch_to),
            (Some(Mode::Plan), Some(Mode::AcceptEdits))
        );
        let i = infer("`/effort high` · **Plan** → **Accept edits**", "", None);
        assert_eq!(
            (i.mode, i.switch_to),
            (Some(Mode::Plan), Some(Mode::AcceptEdits))
        );
        let i = infer("Shift+Tab → **Plan**", "", None);
        assert_eq!((i.mode, i.switch_to), (Some(Mode::Plan), None));
        assert_eq!(
            infer("**Manual** mode (Shift+Tab)", "", None).mode,
            Some(Mode::Default)
        );
        // (4) model and effort
        let i = infer(
            "same session · `/model opus` · `/effort high` · Shift+Tab → **Plan**",
            "",
            None,
        );
        assert_eq!((i.model, i.effort), (Some(Model::Opus), Some(Effort::High)));
        let i = infer(
            "CLI · **opusplan** (Opus plans) · effort **medium**",
            "",
            None,
        );
        assert_eq!(
            (i.model, i.effort),
            (Some(Model::Opusplan), Some(Effort::Medium))
        );
        assert_eq!(
            infer("**Opus 5.5** · effort adaptive", "", None).effort,
            Some(Effort::Adaptive)
        );
        // (5) alternatives
        for p in [
            "**Fable 5.1** · effort **high** — or **Opus 5.5**",
            "x (or Opus)",
            "**Fable 5.1** if available, otherwise **Opus 5.5**",
        ] {
            assert!(infer(p, "", None).has_alternative, "{p}");
        }
        // (6) outputs, resolved against the cwd
        let i = infer(
            "",
            "x\n-> council/r6-feature-specs.md\n",
            Some(CwdKind::Spec),
        );
        assert_eq!(i.outs, ["spec/council/r6-feature-specs.md"]);
        let i = infer(
            "",
            "All items PASS. Write docs/handoff/stage-[N].md in exactly this format:",
            Some(CwdKind::Project),
        );
        assert_eq!(i.outs, ["docs/handoff/stage-NN.md"]);
        // `bp:1090` agrees with start=opus@high/plan: no lint on 5.2.
        let t = parse(&blueprint());
        assert!(!t.diagnostics.iter().any(|d| d.message.starts_with("5.2:")));
        // A disagreeing model lints.
        let b = parse(&replace_once(
            &blueprint(),
            "<!-- fm:step id=5.3 tag=core session=continue chain=5.1 start=opus@high/plan -->",
            "<!-- fm:step id=5.3 tag=core session=continue chain=5.1 start=sonnet@high/plan -->",
        ));
        assert!(rules(&b).contains(&"lint.model-disagrees".to_owned()));
    }

    /// DR-042: a followup can't change model, effort or mode.
    #[test]
    fn followup_start_lint() {
        let b = parse(&replace_once(
            &blueprint(),
            "<!-- fm:step id=2.6 kind=prompt tag=core",
            "<!-- fm:step id=2.6 kind=followup tag=core",
        ));
        let d = b
            .diagnostics
            .iter()
            .find(|d| d.rule == "lint.followup-changes-start")
            .unwrap();
        assert!(d.message.contains("use kind=prompt to re-arm"));
        assert!(d.message.starts_with("2.6:"));
        // 2.3–2.5 and gate.C equal their chain's live start: clean.
        assert!(!rules(&parse(&blueprint())).contains(&"lint.followup-changes-start".to_owned()));
    }

    /// DR-043: chips come from prose and change nothing.
    #[test]
    fn chat_chips() {
        let t = parse(&blueprint());
        assert_eq!(
            chips(&t, t.step("2.1").unwrap(), "TradeLog"),
            [
                "New conversation in Project TradeLog",
                "Opus 5.5",
                "Research ON"
            ]
        );
        assert_eq!(
            chips(&t, t.step("1.2").unwrap(), "TradeLog"),
            ["Same chat as 1.1 The architect interview", "Opus 5.5"]
        );
        assert_eq!(
            chips(&t, t.step("1.1").unwrap(), "TradeLog"),
            [
                "New conversation in Project TradeLog",
                "Opus 5.5",
                "web search ON"
            ]
        );
    }
}
