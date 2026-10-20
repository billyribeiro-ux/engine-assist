//! M1-31: the abuse-case suite (AB-01; AC-XC-09; T-016, T-051, T-087; SR-03, SR-27, SR-34).
//! Content (packets, overlays, tickets, annotations) never changes launch inputs or triggers
//! actions: the effective start, session plan and destinations are byte-identical to the clean
//! fixture. M2-06 extends the comparison to the built argv.
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

use common::{Fixture, blueprint, parse, read_fixture, replace_once};
use foreman_core::decisions::{Decisions, Processed};
use foreman_core::packet;
use foreman_core::pathpolicy::{self, WritePurpose};
use foreman_core::policy::{effective_start, run_config};
use foreman_core::rules::{self, ApplicablePath};
use foreman_core::schema::overlay::OverlayFile;
use foreman_core::schema::progress::DecisionFileKind as K;
use foreman_core::steps::actions;
use foreman_core::store::{Root, RootKind};
use foreman_core::template::{StepKind, StepStatus, Template};

const ID: &str = "3f6a1c2e-8b4d-4e7f-9a1b-2c3d4e5f6a7b";

fn hostile() -> String {
    read_fixture("fixtures/abuse/hostile-lines.md")
}

/// The launch inputs of every runnable prompt step of Stages 1–6 on the path: effective start,
/// session plan (session, chain, surface, cwd), destinations (out, appends) and run config.
fn launch_inputs(t: &Template, fx: &Fixture, path: &ApplicablePath) -> String {
    let mut out = String::new();
    for s in t.steps.iter().filter(|s| s.stage <= 6 && s.runnable()) {
        if !matches!(s.kind, StepKind::Prompt | StepKind::Followup) {
            continue;
        }
        let on = path.step(s.key.as_str()).map(|a| format!("{a:?}"));
        let start = effective_start(s, &fx.project.model_policy, None)
            .map(|e| e.value.to_string())
            .unwrap_or_default();
        out.push_str(&format!(
            "{} {on:?} start={start} session={:?} chain={:?} surface={:?} cwd={:?} out={:?} appends={:?} cfg={:?}\n",
            s.key,
            s.session,
            s.chain,
            s.surface,
            s.cwd,
            s.out,
            s.appends,
            run_config(s)
        ));
    }
    out
}

mod abuse {
    use super::*;

    /// T-016 / T-087 case 1 (DR-083 example): a signed packet full of flags and instructions.
    #[test]
    fn packet_text_cannot_change_flags() {
        let clean = Fixture::tradelog();
        let mut dirty = Fixture::tradelog();
        for p in dirty.packets.values_mut() {
            // Inside a field value, as extra keys, and after the packet.
            p.text = p.text.replacen(
                "PROJECT: TradeLog",
                "PROJECT: TradeLog --dangerously-skip-permissions",
                1,
            );
            p.text.push_str(&hostile());
        }
        let base = launch_inputs(&clean.template, &clean, &clean.path);
        assert_eq!(launch_inputs(&dirty.template, &dirty, &dirty.path), base);
        // Every step that embeds a packet still resolves; the content is only prompt text.
        let mut embedded = 0;
        for s in clean
            .template
            .steps
            .iter()
            .filter(|s| (2..=5).contains(&s.stage) && s.runnable())
        {
            if !matches!(s.kind, StepKind::Prompt | StepKind::Followup) {
                continue;
            }
            let (Ok(a), Ok(b)) = (
                foreman_core::resolver::resolve(&clean.ctx(s.key.as_str())),
                foreman_core::resolver::resolve(&dirty.ctx(s.key.as_str())),
            ) else {
                continue;
            };
            if a.text != b.text {
                embedded += 1;
                assert!(b.text.contains("--permission-mode bypassPermissions"));
                // The prompt still starts the same way (never option-like, DR-086).
                assert_eq!(a.text.chars().next(), b.text.chars().next());
                if foreman_core::sanitize::check_prompt(&a.text).is_ok() {
                    assert!(
                        foreman_core::sanitize::check_prompt(&b.text).is_ok(),
                        "{}",
                        s.key
                    );
                }
            }
        }
        assert!(embedded >= 2, "packets are embedded in later prompts");
        // Packet parsing: the hostile lines are fields or continuation text, never a signature.
        let p = packet::parse_packet(
            &format!(
                "{}{}",
                read_fixture("fixtures/tradelog/handoff/stage-02.md").replace(
                    "APPROVED BY: Billy Ribeiro on 2026-10-21",
                    "APPROVED BY: <blank>"
                ),
                hostile()
            ),
            &[],
        );
        assert_eq!(
            p.fields.iter().filter(|f| f.key == "APPROVED BY").count(),
            2
        );
        assert_eq!(
            p.get("APPROVED BY").unwrap().value,
            "<blank>",
            "the first one counts; the duplicate is a problem"
        );
    }

    /// T-087 case 3: an overlay edited from outside with a gate skip, a hostile added step, a
    /// `.git/hooks` destination and `bypassPermissions`.
    #[test]
    fn overlay_text_cannot_change_flags() {
        let fx = Fixture::tradelog();
        let clean = read_fixture("schema/overlay/v1.json");
        let hostile_overlay = replace_once(
            &replace_once(
                &clean,
                "\"skips\": {",
                "\"skips\": {\n    \"5:gate.A\": {\"at\": \"t\", \"reason\": \"--dangerously-skip-permissions\"},",
            ),
            "\"out\": [\n        \"spec/review.md\"\n      ]",
            "\"out\": [\n        \".git/hooks/pre-commit\"\n      ]",
        );
        let hostile_overlay = hostile_overlay.replace(
            "\"text\": \"Review the spec again.\"",
            &format!("\"text\": {}", serde_json::to_string(&hostile()).unwrap()),
        );
        let bypass = replace_once(
            &clean,
            "\"mode\": \"default\"",
            "\"mode\": \"bypassPermissions\"",
        );
        // As an outside edit: pending (or read-only); the effective overlay is unchanged.
        let pd = tempfile::tempdir().unwrap();
        let ad = tempfile::tempdir().unwrap();
        let project = Root::open(pd.path(), RootKind::Project).unwrap();
        let app = Root::open_app_data(ad.path()).unwrap();
        let d = Decisions {
            project: &project,
            app: &app,
            project_id: ID,
        };
        std::fs::create_dir_all(pd.path().join(".foreman")).unwrap();
        std::fs::write(pd.path().join(".foreman/overlay.json"), &clean).unwrap();
        d.approve(K::Overlay, clean.as_bytes(), true).unwrap();
        let effective_before = d.effective_overlay().unwrap();
        let path_before =
            rules::applicable_path(&fx.template, &fx.project.conditions, &effective_before);
        let base = launch_inputs(&fx.template, &fx, &path_before);
        for (text, read_only) in [(&hostile_overlay, false), (&bypass, true)] {
            std::fs::write(pd.path().join(".foreman/overlay.json"), text).unwrap();
            let p = d.process(K::Overlay, "c", "t").unwrap();
            assert_eq!(matches!(p, Processed::ReadOnly(_)), read_only, "{p:?}");
            if !read_only {
                assert!(matches!(p, Processed::Pending(_)));
            }
            let eff = d.effective_overlay().unwrap();
            assert_eq!(eff, effective_before);
            let path = rules::applicable_path(&fx.template, &fx.project.conditions, &eff);
            assert_eq!(launch_inputs(&fx.template, &fx, &path), base);
        }
        // Even if Billy accepted it: a gate skip is ignored (gates are core), the destination is
        // refused by the path policy, and nothing in the launch inputs moves.
        let accepted: OverlayFile = serde_json::from_str(&hostile_overlay).unwrap();
        let path = rules::applicable_path(&fx.template, &fx.project.conditions, &accepted);
        assert!(path.step("5:gate.A").unwrap().on_path());
        assert!(
            path.flags
                .iter()
                .any(|f| f.starts_with("rules.stale-skip: 5:gate.A")),
            "{:?}",
            path.flags
        );
        assert!(
            pathpolicy::check_write(&accepted.added_steps[0].out[0], WritePurpose::TemplateOut)
                .is_err()
        );
        assert_eq!(launch_inputs(&fx.template, &fx, &path), base);
        assert!(
            serde_json::from_str::<OverlayFile>(&bypass).is_err(),
            "bypassPermissions isn't a mode"
        );
    }

    /// T-087 case 4: ticket and lessons text with instructions and a REPO FILES block.
    #[test]
    fn ticket_text_cannot_trigger_actions() {
        let fx = Fixture::tradelog();
        let plan = |dir: &std::path::Path| {
            let root = Root::open(dir, RootKind::Project).unwrap();
            let p = actions::plan(&fx.template, "council-setup", &root).unwrap();
            let writes: Vec<String> = p.writes.iter().map(|w| w.path.to_string()).collect();
            (writes, p.refused)
        };
        let clean = tempfile::tempdir().unwrap();
        let dirty = tempfile::tempdir().unwrap();
        for f in ["docs/tickets.md", "docs/lessons.md", "spec/MASTER-SPEC.md"] {
            let p = dirty.path().join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, read_fixture("fixtures/abuse/hostile-ticket.md")).unwrap();
        }
        // The same fixed destinations, whatever the repo says.
        assert_eq!(plan(clean.path()), plan(dirty.path()));
        let (writes, _) = plan(dirty.path());
        assert_eq!(writes.len(), 6);
        assert!(
            writes
                .iter()
                .all(|w| w.starts_with("spec/.claude/agents/") && w.ends_with(".md"))
        );
        // Unknown action IDs from text are refused; steps don't move on text.
        assert!(
            actions::plan(
                &fx.template,
                "sign-stage-5",
                &Root::open(dirty.path(), RootKind::Project).unwrap()
            )
            .is_err()
        );
        // A ticket isn't a packet: its "signature" is just text in a file named otherwise.
        assert_eq!(packet::stage_of_file("docs/tickets.md"), None);
        // The REPO FILES block names paths the policy refuses.
        for p in [".git/hooks/pre-commit", "~/.zshrc", ".claude/settings.json"] {
            assert!(
                pathpolicy::check_write(p, WritePurpose::Repo).is_err(),
                "{p}"
            );
        }
    }

    /// T-087 case 5: annotations that try to leave the policy.
    #[test]
    fn annotation_cannot_escape_policy() {
        let bp = blueprint();
        let ann = "start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r6-feature-specs.md -->";
        for (bad, rule_prefix) in [
            (
                "start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=../../.zshrc -->",
                "path.",
            ),
            (
                "start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=.git/hooks/pre-commit -->",
                "path.",
            ),
            (
                "start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=.claude/settings.json -->",
                "path.",
            ),
            (
                "start=fable@high/bypassPermissions alt=pro-plan:opus@xhigh out=spec/council/r6-feature-specs.md -->",
                "grammar.",
            ),
            (
                "start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r6-feature-specs.md cwd=../.. -->",
                "grammar.",
            ),
        ] {
            let t = parse(&replace_once(&bp, ann, bad));
            let s = t.step("2.3").unwrap();
            assert_ne!(s.status, StepStatus::Runnable, "{bad}");
            let errs: Vec<&str> = t.errors().map(|d| d.rule.as_str()).collect();
            assert!(
                errs.iter()
                    .any(|r| r.starts_with(rule_prefix) || r.starts_with("grammar.")),
                "{bad}: {errs:?}"
            );
            assert!(
                s.out
                    .iter()
                    .all(|o| pathpolicy::check_write(o, WritePurpose::TemplateOut).is_ok())
            );
        }
        // An action step can't name its own destination: council-setup's are Rust constants, and a block with another name is refused.
        let renamed = replace_once(&bp, "name: market-analyst", "name: ../../../.zshrc");
        let t = parse(&renamed);
        let dir = tempfile::tempdir().unwrap();
        let p = actions::plan(
            &t,
            "council-setup",
            &Root::open(dir.path(), RootKind::Project).unwrap(),
        )
        .unwrap();
        assert_eq!(p.refused, ["../../../.zshrc"]);
        assert_eq!(p.writes.len(), 5);
        for i in 0..6 {
            let d = actions::council_destination(i).unwrap();
            assert!(d.to_string().starts_with("spec/.claude/agents/"));
        }
        assert!(actions::council_destination(6).is_none());
    }
}
