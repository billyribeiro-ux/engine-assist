//! M1-19: run configs and model policy (DR-100–DR-106, AC-F005-01/-02).
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

use common::{blueprint, parse};
use foreman_core::ids::{CwdKind, Effort, Mode, Model, Surface};
use foreman_core::policy::{self as pol, PolicyError, RunOverride, ValueOrigin};
use foreman_core::schema::project::{ModelPolicy, Plan, Substitution};
use foreman_core::template::Session;

fn max() -> ModelPolicy {
    ModelPolicy {
        plan: Plan::Max,
        substitutions: vec![],
    }
}
fn pro() -> ModelPolicy {
    ModelPolicy {
        plan: Plan::Pro,
        substitutions: vec![],
    }
}
/// Foreman's own policy (MASTER-SPEC §5).
fn foreman() -> ModelPolicy {
    ModelPolicy {
        plan: Plan::Max,
        substitutions: vec![Substitution {
            from: Model::Fable,
            to: Model::Opus,
            effort: Some(Effort::Xhigh),
        }],
    }
}

mod policy {
    use super::*;

    /// DR-100.
    #[test]
    fn run_config_from_annotation() {
        let t = parse(&blueprint());
        let c = pol::run_config(t.step("2.2").unwrap());
        assert_eq!(
            (c.surface, c.cwd, c.session),
            (Some(Surface::Cli), Some(CwdKind::Spec), Session::New)
        );
        assert_eq!(c.start.unwrap().to_string(), "fable@high/plan");
        assert_eq!(c.switches[0].to.to_string(), "fable@high/acceptEdits");
        assert_eq!(
            c.alts,
            [("pro-plan".to_owned(), Model::Opus, Effort::Xhigh)]
        );
        assert_eq!(c.agents.as_deref(), Some("all six council agents"));
        let c23 = pol::run_config(t.step("2.3").unwrap());
        assert_eq!(
            (c23.cwd, c23.session, c23.chain.unwrap().as_str()),
            (Some(CwdKind::Spec), Session::Continue, "2.2")
        );
    }

    /// AC-F005-02 / DR-101.
    #[test]
    fn alternative_selection() {
        let t = parse(&blueprint());
        let s = t.step("2.2").unwrap();
        let e = pol::effective_start(s, &pro(), None).unwrap();
        assert_eq!(e.value.to_string(), "opus@xhigh/plan");
        assert_eq!(
            e.origin,
            ValueOrigin::Alternative {
                rule: "pro-plan".into()
            }
        );
        assert_eq!(
            pol::effective_switches(s, &pro())[0].to.to_string(),
            "opus@xhigh/acceptEdits"
        );
        let m = pol::effective_start(s, &max(), None).unwrap();
        assert_eq!(
            (m.value.to_string(), m.origin),
            ("fable@high/plan".into(), ValueOrigin::Template)
        );
        // No alt on the step: the plan changes nothing.
        assert_eq!(
            pol::effective_start(t.step("5.3").unwrap(), &pro(), None)
                .unwrap()
                .value
                .to_string(),
            "opus@high/plan"
        );
    }

    /// AC-F005-03 / DR-102: once, in order, not transitively; chat keeps `adaptive`.
    #[test]
    fn substitution_rules() {
        let t = parse(&blueprint());
        let e = pol::effective_start(t.step("2.2").unwrap(), &foreman(), None).unwrap();
        assert_eq!(e.value.to_string(), "opus@xhigh/plan");
        assert_eq!(
            e.origin,
            ValueOrigin::Substitution {
                rule: "fable → opus@xhigh".into()
            }
        );
        assert_eq!(
            pol::substitution_line(t.step("2.2").unwrap(), &e).unwrap(),
            "2.2: fable@high/plan → opus@xhigh/plan (substitution fable → opus@xhigh)"
        );
        let c47 = pol::effective_start(t.step("4.7").unwrap(), &foreman(), None).unwrap();
        assert_eq!(c47.value.to_string(), "opus@adaptive");
        let chain = ModelPolicy {
            plan: Plan::Max,
            substitutions: vec![
                Substitution {
                    from: Model::Fable,
                    to: Model::Opus,
                    effort: None,
                },
                Substitution {
                    from: Model::Opus,
                    to: Model::Sonnet,
                    effort: None,
                },
            ],
        };
        assert_eq!(
            pol::effective_start(t.step("2.2").unwrap(), &chain, None)
                .unwrap()
                .value
                .model,
            Model::Opus,
            "not transitive"
        );
        assert_eq!(
            pol::effective_start(t.step("5.3").unwrap(), &chain, None)
                .unwrap()
                .value
                .model,
            Model::Sonnet
        );
        // A substitution after the alternative (Pro plan picks opus@xhigh, then opus → sonnet).
        let both = ModelPolicy {
            plan: Plan::Pro,
            substitutions: vec![Substitution {
                from: Model::Opus,
                to: Model::Sonnet,
                effort: Some(Effort::Medium),
            }],
        };
        assert_eq!(
            pol::effective_start(t.step("2.2").unwrap(), &both, None)
                .unwrap()
                .value
                .to_string(),
            "sonnet@medium/plan"
        );
    }

    /// AC-F005-05 / DR-103.
    #[test]
    fn override_scope() {
        let t = parse(&blueprint());
        let o = RunOverride {
            model: Some(Model::Opus),
            effort: Some(Effort::Max),
            mode: None,
        };
        let e = pol::effective_start(t.step("2.4").unwrap(), &max(), Some(&o)).unwrap();
        assert_eq!(
            (e.value.to_string(), e.origin.clone()),
            ("opus@max/acceptEdits".into(), ValueOrigin::Override)
        );
        assert_eq!(
            pol::substitution_line(t.step("2.4").unwrap(), &e).unwrap(),
            "2.4: fable@high/acceptEdits → opus@max/acceptEdits (override)"
        );
        // The override is for one run: without it the template value is back.
        assert_eq!(
            pol::effective_start(t.step("2.4").unwrap(), &max(), None)
                .unwrap()
                .value
                .to_string(),
            "fable@high/acceptEdits"
        );
        // Overrides must fit the surface.
        let chat = t.step("1.1").unwrap();
        assert!(matches!(
            pol::effective_start(
                chat,
                &max(),
                Some(&RunOverride {
                    mode: Some(Mode::Plan),
                    ..Default::default()
                })
            ),
            Err(PolicyError::OverrideDoesntFit { .. })
        ));
        assert!(matches!(
            pol::effective_start(
                chat,
                &max(),
                Some(&RunOverride {
                    effort: Some(Effort::High),
                    ..Default::default()
                })
            ),
            Err(PolicyError::OverrideDoesntFit { .. })
        ));
        assert!(matches!(
            pol::effective_start(
                t.step("2.2").unwrap(),
                &max(),
                Some(&RunOverride {
                    effort: Some(Effort::Adaptive),
                    ..Default::default()
                })
            ),
            Err(PolicyError::OverrideDoesntFit { .. })
        ));
        assert!(
            pol::effective_start(
                chat,
                &max(),
                Some(&RunOverride {
                    model: Some(Model::Sonnet),
                    ..Default::default()
                })
            )
            .is_ok()
        );
    }

    /// AC-F005-01 / DR-104: every combination of template, alternative, substitution, override.
    #[test]
    fn precedence() {
        let t = parse(&blueprint());
        let s = t.step("2.2").unwrap();
        let ovr = RunOverride {
            model: Some(Model::Haiku),
            effort: None,
            mode: None,
        };
        for (alt, sub, o) in itertools_product() {
            let policy = ModelPolicy {
                plan: if alt { Plan::Pro } else { Plan::Max },
                substitutions: if sub {
                    vec![Substitution {
                        from: if alt { Model::Opus } else { Model::Fable },
                        to: Model::Sonnet,
                        effort: Some(Effort::Low),
                    }]
                } else {
                    vec![]
                },
            };
            let e = pol::effective_start(s, &policy, o.then_some(&ovr)).unwrap();
            let (want_model, want_origin) = match (alt, sub, o) {
                (_, _, true) => (Model::Haiku, "override"),
                (_, true, false) => (Model::Sonnet, "substitution"),
                (true, false, false) => (Model::Opus, "alternative"),
                (false, false, false) => (Model::Fable, "template"),
            };
            assert_eq!(e.value.model, want_model, "{alt} {sub} {o}");
            let origin = match e.origin {
                ValueOrigin::Template => "template",
                ValueOrigin::Alternative { .. } => "alternative",
                ValueOrigin::Substitution { .. } => "substitution",
                ValueOrigin::Override => "override",
            };
            assert_eq!(origin, want_origin);
            assert_eq!(e.value.mode, Some(Mode::Plan), "the mode stays");
        }
    }

    fn itertools_product() -> Vec<(bool, bool, bool)> {
        let mut v = Vec::new();
        for a in [false, true] {
            for b in [false, true] {
                for c in [false, true] {
                    v.push((a, b, c));
                }
            }
        }
        v
    }

    /// AC-F005-07 / DR-105: switches are cues (text), never keystrokes.
    #[test]
    fn switches_never_typed() {
        let t = parse(&blueprint());
        assert_eq!(
            pol::switch_cues(t.step("5.1").unwrap(), &max()),
            ["Next: after you approve the plan, choose Accept edits"]
        );
        assert_eq!(
            pol::switch_cues(t.step("2.2").unwrap(), &foreman()),
            ["Next: after you approve the plan, choose Accept edits"]
        );
        assert!(pol::switch_cues(t.step("5.3").unwrap(), &max()).is_empty());
        // No core code turns a switch into terminal input: nothing outside policy reads switches
        // except the template, lint, compat and render (display) modules.
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut users = Vec::new();
        let mut stack = vec![src.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(d).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let text = std::fs::read_to_string(&p).unwrap();
                if text.contains("switches()") || text.contains(".switches") {
                    users.push(p.strip_prefix(&src).unwrap().display().to_string());
                }
            }
        }
        users.sort();
        for u in &users {
            assert!(
                u.starts_with("template/") || u == "policy.rs" || u == "compat.rs",
                "{u} reads switches"
            );
        }
    }
}
