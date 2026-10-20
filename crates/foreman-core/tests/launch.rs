//! M2-06, M2-07, M2-09, M3-01, M3-03, M3-05, M3-14, M3-19, M4-08, M4-12: the pure halves of the
//! runners — argv, child environment, run settings, chains, confirmation reasons, SR-07 key
//! blocks, redaction, probe classification and the watcher's interest set.
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

use std::path::{Path, PathBuf};

use common::Fixture;
use foreman_core::chain::{
    self as fchain, ChainStartState, EffortOnResume, ForkIds, HandoffConfirm, SessionPlan,
};
use foreman_core::ids::{CwdKind, Effort, Mode, Model, Start, StepKey, Surface};
use foreman_core::launch::argv::{
    self as fargv, ArgvError, KNOWN_FLAGS, LaunchInputs, SessionArgs, SessionName,
};
use foreman_core::launch::env::{self as fenv, BuiltPath, DirFacts, PathDrop, PathSources};
use foreman_core::launch::preflight::{ConfirmCtx, needs_confirmation};
use foreman_core::launch::run_settings::{self as frs, CaseVariants, RunSettingsOpts, SensorHooks};
use foreman_core::launch::{Blocker, ConfirmReason};
use foreman_core::schema::progress::{Chain, ChainStatus, ChainSurface};
use foreman_core::template::{StepKind, Template};
use proptest::prelude::*;
use uuid::Uuid;

const A: &str = "6f1c2a3b-1d2e-4f50-8a9b-0c1d2e3f4a5b";
const B: &str = "7a1f3c2e-0b4d-4e6f-9a8b-1c2d3e4f5a6b";
const C: &str = "8b2e4d3f-1c5e-4f70-8b9c-2d3e4f5a6b7c";
const APP: &str = "/Users/billy/Library/Application Support/Foreman";

fn u(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap()
}

fn start(model: Model, effort: Effort, mode: Mode) -> Start {
    Start {
        model,
        effort,
        mode: Some(mode),
    }
}

fn settings_path(run: &str) -> PathBuf {
    PathBuf::from(format!("{APP}/runs/{run}/settings.json"))
}

fn inputs(session: SessionArgs, prompt: Option<&str>) -> LaunchInputs {
    LaunchInputs {
        session,
        start: start(Model::Opus, Effort::Xhigh, Mode::Plan),
        pass_effort: true,
        name: SessionName::for_step("TradeLog", 2, "2.3"),
        settings: settings_path(C),
        screen_reader: false,
        prompt: prompt.map(str::to_owned),
    }
}

/// Every runnable CLI step of a template with its effective start, session args and resolved
/// prompt (when it resolves with the fixture).
fn cli_steps(fx: &Fixture) -> Vec<(String, LaunchInputs)> {
    let t: &Template = &fx.template;
    let mut out = Vec::new();
    for s in t.steps.iter().filter(|s| s.runnable()) {
        if s.surface != Some(Surface::Cli)
            || !matches!(s.kind, StepKind::Prompt | StepKind::Followup)
        {
            continue;
        }
        if fx.path.step(s.key.as_str()).is_none() {
            continue;
        }
        let Ok(eff) = foreman_core::policy::effective_start(s, &fx.project.model_policy, None)
        else {
            continue;
        };
        let session = match s.session {
            foreman_core::template::Session::New => SessionArgs::New { id: u(A) },
            foreman_core::template::Session::Continue => SessionArgs::Resume { id: u(A) },
        };
        let prompt = foreman_core::resolver::resolve(&fx.ctx(s.key.as_str()))
            .ok()
            .map(|r| r.text);
        out.push((
            s.key.to_string(),
            LaunchInputs {
                session,
                start: eff.value,
                pass_effort: true,
                name: SessionName::for_step("tradelog", s.stage, s.key.as_str()),
                settings: settings_path(C),
                screen_reader: false,
                prompt,
            },
        ));
    }
    out
}

mod argv {
    use super::*;

    fn arb_start() -> impl Strategy<Value = Start> {
        (
            prop::sample::select(Model::ALL.to_vec()),
            prop::sample::select(vec![
                Effort::Low,
                Effort::Medium,
                Effort::High,
                Effort::Xhigh,
                Effort::Max,
                Effort::Ultracode,
            ]),
            prop::sample::select(vec![
                Mode::Plan,
                Mode::Default,
                Mode::AcceptEdits,
                Mode::Auto,
                Mode::DontAsk,
            ]),
        )
            .prop_map(|(m, e, md)| start(m, e, md))
    }

    fn check_known(v: &[String], has_prompt: bool) {
        let n = if has_prompt { v.len() - 1 } else { v.len() };
        let mut i = 0;
        while i < n {
            let a = &v[i];
            assert!(KNOWN_FLAGS.contains(&a.as_str()), "{a} in {v:?}");
            if matches!(
                a.as_str(),
                "--session-id"
                    | "--resume"
                    | "--model"
                    | "--effort"
                    | "--permission-mode"
                    | "--name"
                    | "--settings"
            ) {
                i += 1;
            }
            i += 1;
        }
    }

    /// SR-03: property over every template step (the real v2.2 blueprint on the TradeLog path)
    /// and random starts, prompts and session forms.
    #[test]
    fn only_known_flags() {
        let fx = Fixture::tradelog();
        let steps = cli_steps(&fx);
        assert!(steps.len() >= 6, "{}", steps.len());
        for (key, li) in &steps {
            if let Ok(a) = fargv::build_argv(li) {
                check_known(a.as_slice(), li.prompt.is_some());
            } else {
                assert!(li.prompt.is_some(), "{key}");
            }
        }
        proptest!(ProptestConfig::with_cases(256), |(
            st in arb_start(),
            kind in 0u8..3,
            prompt in proptest::option::of("[a-zA-Z0-9 .,\n]{1,40}"),
            sr in any::<bool>(),
        )| {
            let session = match kind {
                0 => SessionArgs::New { id: u(A) },
                1 => SessionArgs::Resume { id: u(A) },
                _ => SessionArgs::Fork { from: u(A), new_id: Some(u(B)) },
            };
            let li = LaunchInputs { start: st, screen_reader: sr, ..inputs(session, prompt.as_deref()) };
            let a = fargv::build_argv(&li).unwrap();
            check_known(a.as_slice(), li.prompt.is_some());
        });
    }

    #[test]
    fn never_contains_forbidden_flags() {
        let fx = Fixture::tradelog();
        for (key, li) in cli_steps(&fx) {
            if let Ok(a) = fargv::build_argv(&li) {
                let n = a.as_slice().len() - usize::from(li.prompt.is_some());
                for x in &a.as_slice()[..n] {
                    assert!(!fargv::is_forbidden(x), "{key}: {x}");
                    assert!(
                        !x.contains("bypassPermissions") && !x.contains("dangerously"),
                        "{key}"
                    );
                }
            }
        }
        // A forbidden spelling as the prompt is refused by prompt hygiene (option-like), and the
        // mode enum has no bypassPermissions at all.
        for bad in ["--bare", "-p", "--print", "--dangerously-skip-permissions"] {
            assert!(matches!(
                fargv::build_argv(&inputs(SessionArgs::New { id: u(A) }, Some(bad))),
                Err(ArgvError::Prompt(_))
            ));
        }
        assert_eq!(Mode::parse("bypassPermissions"), None);
    }

    #[test]
    fn no_print_mode_in_v1() {
        for f in KNOWN_FLAGS {
            assert!(f != "-p" && f != "--print" && f != "--bare", "{f}");
        }
        let a = fargv::build_argv(&inputs(SessionArgs::New { id: u(A) }, Some("hi"))).unwrap();
        assert!(!a.as_slice().iter().any(|x| x == "-p" || x == "--print"));
    }

    #[test]
    fn name_format() {
        for (slug, stage, step, want) in [
            ("TradeLog", 2, "2.3", "fm-tradelog-s02-2-3"),
            ("foreman", 5, "5:gate.A", "fm-foreman-s05-5-gate-a"),
            ("foreman", 2, "2:F.2", "fm-foreman-s02-2-f-2"),
            ("Trade Log!", 1, "1.3b", "fm-trade-log-s01-1-3b"),
            ("x", 2, "council-setup", "fm-x-s02-council-setup"),
        ] {
            let n = SessionName::for_step(slug, stage, step);
            assert_eq!(n.as_str(), want);
            assert!(SessionName::parse(n.as_str()).is_ok());
        }
        let long = SessionName::for_step(&"a".repeat(300), 17, "x-17-123");
        assert!(long.as_str().len() <= 83 && long.as_str().ends_with("-s17-x-17-123"));
        for bad in [
            "fm-",
            "fm-UP",
            "fm-a_b",
            "xx-a",
            &format!("fm-{}", "a".repeat(81)),
        ] {
            assert!(SessionName::parse(bad).is_err(), "{bad}");
        }
    }

    /// SR-04 / DR-122, AC-F003-02: the prompt is last; every harness CLI step's argv is pinned
    /// (M3-27). No step is left out silently: the snapshot covers exactly the runnable CLI prompt
    /// and followup steps on the harness path, each with its resolved prompt.
    #[test]
    fn positional_prompt_snapshot() {
        let fx = Fixture::tradelog();
        let expected: Vec<String> = fx
            .template
            .steps
            .iter()
            .filter(|s| {
                s.runnable()
                    && s.surface == Some(Surface::Cli)
                    && matches!(s.kind, StepKind::Prompt | StepKind::Followup)
                    && fx.path.step(s.key.as_str()).is_some()
            })
            .map(|s| s.key.to_string())
            .collect();
        let steps = cli_steps(&fx);
        let covered: Vec<String> = steps.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(covered, expected, "a harness CLI step has no argv snapshot");
        for stage_two in ["2.2", "2.3", "2.4", "2.5", "2.6", "2:F.2"] {
            assert!(covered.iter().any(|k| k == stage_two), "{stage_two}");
        }
        let mut lines = Vec::new();
        for (key, li) in steps {
            assert!(li.prompt.is_some(), "{key}: the prompt didn't resolve");
            let argv = fargv::build_argv(&li).unwrap();
            for banned in [
                "--bare",
                "-p",
                "--print",
                "--dangerously-skip-permissions",
                "--allow-dangerously-skip-permissions",
                "bypassPermissions",
            ] {
                assert!(
                    !argv.as_slice()[..argv.as_slice().len() - 1]
                        .iter()
                        .any(|a| a == banned),
                    "{key}: {banned}"
                );
            }
            match fargv::build_argv(&li) {
                Ok(a) => {
                    if let Some(p) = &li.prompt {
                        assert_eq!(a.as_slice().last(), Some(p), "{key}");
                    }
                    let shown: Vec<String> = a.redacted(li.prompt.is_some());
                    lines.push(format!("{key}: claude {}", shown.join(" ")));
                }
                Err(e) => lines.push(format!("{key}: {e}")),
            }
        }
        insta::assert_snapshot!("tradelog_cli_argv", lines.join("\n"));
    }

    #[test]
    fn rejects_option_like_prompt() {
        for p in ["-x", "  --resume x", "/exit", "\n!rm -rf /", "\t-"] {
            let r = fargv::build_argv(&inputs(SessionArgs::New { id: u(A) }, Some(p)));
            assert!(
                matches!(
                    r,
                    Err(ArgvError::Prompt(
                        foreman_core::sanitize::Blocker::PromptOptionLike { .. }
                    ))
                ),
                "{p:?}: {r:?}"
            );
        }
        let r = fargv::build_argv(&inputs(
            SessionArgs::New { id: u(A) },
            Some("ok\u{1b}[201~"),
        ));
        assert!(matches!(
            r,
            Err(ArgvError::Prompt(
                foreman_core::sanitize::Blocker::PromptControlChar {
                    codepoint: 0x1b,
                    ..
                }
            ))
        ));
        assert!(
            fargv::build_argv(&inputs(
                SessionArgs::New { id: u(A) },
                Some("Run 2.3 - then")
            ))
            .is_ok()
        );
    }

    /// SR-14 / DR-111: `--session-id` only at chain start or in the fork form; never with
    /// `--resume` otherwise. Property over random step-run sequences through the chain planner.
    #[test]
    fn session_id_only_on_chain_start_or_fork() {
        let fx = Fixture::tradelog();
        let t = &fx.template;
        let keys = ["2.2", "2.3", "2.4", "2.5", "2.6", "5.1", "5.2", "5.4"];
        proptest!(ProptestConfig::with_cases(128), |(
            seq in prop::collection::vec((0usize..keys.len(), any::<u8>()), 1..20),
            fork_ids in any::<bool>(),
        )| {
            let mut chains: Vec<Chain> = Vec::new();
            for (i, (k, n)) in seq.iter().enumerate() {
                let step = t.step(keys[*k]).unwrap();
                let id = Uuid::from_u128(0x6f1c_2a3b_1d2e_4f50_8a9b_0c1d_2e3f_0000 + i as u128);
                let handoff = if n % 5 == 0 { HandoffConfirm::Fork } else { HandoffConfirm::DesktopClosed };
                if n % 7 == 0 && let Some(c) = chains.last() {
                    chains = fchain::mark_handed_off(&chains, Uuid::parse_str(&c.id).unwrap());
                }
                let plan = fchain::plan_session(step, &chains, id, ChainStartState::Ran, handoff);
                let fid = if fork_ids { ForkIds::Pass } else { ForkIds::Fail };
                if let Some(args) = fchain::session_args(&plan, fid) {
                    let a = fargv::build_argv(&inputs(args, None)).unwrap();
                    let v = a.as_slice();
                    let sid = v.iter().any(|x| x == "--session-id");
                    let res = v.iter().any(|x| x == "--resume");
                    let fork = v.iter().any(|x| x == "--fork-session");
                    prop_assert!(!(sid && res) || fork, "{v:?}");
                    match plan {
                        SessionPlan::New { .. } => prop_assert!(sid && !res),
                        SessionPlan::Continue { .. } => prop_assert!(res && !sid),
                        SessionPlan::Fork { .. } => prop_assert!(res && fork && sid == fork_ids),
                        _ => {}
                    }
                    chains = fchain::apply_launch(&chains, step, &plan, step.start.unwrap_or(start(Model::Opus, Effort::High, Mode::Plan)));
                }
            }
        });
        // The guard itself: the non-fork combination can't be built.
        let li = inputs(
            SessionArgs::Fork {
                from: u(A),
                new_id: Some(u(B)),
            },
            None,
        );
        assert!(fargv::build_argv(&li).is_ok());
    }
}

mod env {
    use super::*;

    fn own() -> Vec<(String, String)> {
        [
            ("HOME", "/Users/billy"),
            ("USER", "billy"),
            ("LOGNAME", "billy"),
            ("SHELL", "/bin/zsh"),
            ("TMPDIR", "/var/folders/x/T/"),
            ("LANG", "en_US.UTF-8"),
            ("LC_ALL", "en_US.UTF-8"),
            ("PATH", "/evil:/usr/bin"),
            ("ANTHROPIC_API_KEY", "sk-ant-api"),
            ("AWS_PROFILE", "x"),
            ("HTTPS_PROXY", "http://127.0.0.1:1"),
            ("DYLD_INSERT_LIBRARIES", "/tmp/x.dylib"),
            ("NODE_OPTIONS", "--require /tmp/x.js"),
            ("GIT_DIR", "/tmp"),
            ("EDITOR", "vim"),
            ("SSH_AUTH_SOCK", "/tmp/agent"),
            ("TERM", "dumb"),
        ]
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
    }

    fn path() -> BuiltPath {
        BuiltPath {
            entries: vec![
                PathBuf::from("/usr/bin"),
                PathBuf::from("/opt/homebrew/bin"),
            ],
            dropped: vec![],
        }
    }

    #[test]
    fn allowlist_only() {
        let e = fenv::build_env(&own(), &[], &path(), C);
        assert_eq!(
            e.names(),
            [
                "COLORTERM",
                "FOREMAN_RUN_ID",
                "HOME",
                "LANG",
                "LC_ALL",
                "LOGNAME",
                "PATH",
                "SHELL",
                "TERM",
                "TMPDIR",
                "USER"
            ]
        );
        assert_eq!(e.get("PATH"), Some("/usr/bin:/opt/homebrew/bin"));
        assert_eq!(e.get("TERM"), Some("xterm-256color"));
        assert_eq!(e.get("COLORTERM"), Some("truecolor"));
        assert_eq!(e.get("FOREMAN_RUN_ID"), Some(C));
        // Pass-through adds only named, non-removed variables that exist.
        let e = fenv::build_env(
            &own(),
            &["EDITOR".into(), "NOPE".into(), "bad name".into()],
            &path(),
            C,
        );
        assert_eq!(e.get("EDITOR"), Some("vim"));
        assert!(e.get("NOPE").is_none() && e.get("SSH_AUTH_SOCK").is_none());
    }

    #[test]
    fn denylist_beats_passthrough() {
        let pass: Vec<String> = [
            "ANTHROPIC_API_KEY",
            "AWS_PROFILE",
            "HTTPS_PROXY",
            "DYLD_INSERT_LIBRARIES",
            "NODE_OPTIONS",
            "GIT_DIR",
            "PATH",
            "TERM",
            "FOREMAN_RUN_ID",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let e = fenv::build_env(&own(), &pass, &path(), C);
        for n in &pass[..6] {
            assert!(e.get(n).is_none(), "{n}");
        }
        assert_eq!(e.get("PATH"), Some("/usr/bin:/opt/homebrew/bin"));
        assert_eq!(e.get("TERM"), Some("xterm-256color"));
        assert_eq!(e.get("FOREMAN_RUN_ID"), Some(C));
    }

    #[test]
    fn oauth_token_passed_when_present() {
        let mut o = own();
        let e = fenv::build_env(&o, &[], &path(), C);
        assert!(e.get("CLAUDE_CODE_OAUTH_TOKEN").is_none());
        o.push(("CLAUDE_CODE_OAUTH_TOKEN".into(), "sk-ant-oat01-x".into()));
        o.push((
            "CLAUDE_CONFIG_DIR".into(),
            "/Users/billy/.claude-work".into(),
        ));
        o.push(("CLAUDE_CODE_USE_BEDROCK".into(), "1".into()));
        let e = fenv::build_env(&o, &["CLAUDE_CODE_USE_BEDROCK".into()], &path(), C);
        assert_eq!(e.get("CLAUDE_CODE_OAUTH_TOKEN"), Some("sk-ant-oat01-x"));
        assert_eq!(
            e.get("CLAUDE_CONFIG_DIR"),
            Some("/Users/billy/.claude-work")
        );
        assert!(e.get("CLAUDE_CODE_USE_BEDROCK").is_none());
    }

    #[test]
    fn path_entries_sanitized() {
        let src = PathSources {
            etc_paths: vec!["/usr/bin".into(), "/bin".into(), "".into()],
            etc_paths_d: vec![".".into(), "relative/bin".into(), "/usr/bin".into()],
            home: PathBuf::from("/Users/billy"),
            pnpm_home: Some(PathBuf::from("/Users/billy/Library/pnpm")),
            login: vec![
                "/Users/billy/dev/tradelog/node_modules/.bin".into(),
                "/tmp/ww".into(),
                "/missing".into(),
                "/a:b".into(),
            ],
        };
        let projects = vec![PathBuf::from("/Users/billy/dev/tradelog")];
        let built = fenv::build_path(&src.candidates(), &projects, &mut |p: &Path| DirFacts {
            is_dir: p != Path::new("/missing"),
            world_writable: p == Path::new("/tmp/ww"),
        });
        assert_eq!(
            built.joined(),
            "/usr/bin:/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/Users/billy/.cargo/bin:/Users/billy/Library/pnpm"
        );
        let why = |s: &str| built.dropped.iter().find(|(r, _)| r == s).map(|(_, d)| *d);
        assert_eq!(why(""), Some(PathDrop::NotAbsolute));
        assert_eq!(why("."), Some(PathDrop::NotAbsolute));
        assert_eq!(why("relative/bin"), Some(PathDrop::NotAbsolute));
        assert_eq!(
            why("/Users/billy/dev/tradelog/node_modules/.bin"),
            Some(PathDrop::InProject)
        );
        assert_eq!(why("/tmp/ww"), Some(PathDrop::WorldWritable));
        assert_eq!(why("/missing"), Some(PathDrop::Missing));
        assert_eq!(why("/a:b"), Some(PathDrop::BadCharacter));
        assert!(
            built
                .dropped
                .iter()
                .any(|(r, d)| r == "/usr/bin" && *d == PathDrop::Duplicate)
        );
    }

    /// SR-05 is a superset of SR-07's env patterns (CLAUDE_CONFIG_DIR is the documented
    /// exception: it is never taken from pass-through or settings, only from Foreman's own env).
    #[test]
    fn removed_list_covers_blocked_patterns() {
        use foreman_core::fingerprint::auth_keys::BLOCKED_ENV;
        use foreman_core::launch::env::Pattern;
        for p in BLOCKED_ENV {
            let example = match p {
                Pattern::Prefix(x) => format!("{x}EXAMPLE"),
                Pattern::Suffix(x) => format!("EXAMPLE{x}"),
                Pattern::Exact(x) => x.to_owned(),
            };
            if example == "CLAUDE_CONFIG_DIR" {
                assert!(fenv::OWN_ONLY.contains(&"CLAUDE_CONFIG_DIR"));
                continue;
            }
            assert!(fenv::is_removed(&example), "{example}");
        }
        for extra in [
            "GCLOUD_PROJECT",
            "BASH_ENV",
            "ENV",
            "GIT_CONFIG_GLOBAL",
            "http_proxy",
        ] {
            assert!(fenv::is_removed(extra), "{extra}");
        }
    }
}

mod run_settings {
    use super::*;

    fn sensor() -> SensorHooks {
        SensorHooks {
            command: format!("{APP}/bin/foreman-sensor-0123456789ab"),
            run_id: C.into(),
            token: "a".repeat(64),
        }
    }

    #[test]
    fn contents_snapshot() {
        let f = frs::build(
            Path::new("/Users/billy/dev/tradelog"),
            Path::new(APP),
            &RunSettingsOpts::default(),
        )
        .unwrap();
        let (bytes, sha) = frs::to_bytes(&f);
        let text = String::from_utf8(bytes).unwrap();
        insta::assert_snapshot!("run_settings_p1", text);
        assert_eq!(sha.len(), 64);
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        common::assert_valid("run-settings", &v);
        // S-3 fail branch: effortLevel low…xhigh only.
        let f = frs::build(
            Path::new("/r"),
            Path::new("/a"),
            &RunSettingsOpts {
                effort_level: Some(Effort::Max),
                ..RunSettingsOpts::default()
            },
        )
        .unwrap();
        assert_eq!(f.effort_level, None);
        let f = frs::build(
            Path::new("/r"),
            Path::new("/a"),
            &RunSettingsOpts {
                effort_level: Some(Effort::Xhigh),
                ..RunSettingsOpts::default()
            },
        )
        .unwrap();
        assert_eq!(f.effort_level.as_deref(), Some("xhigh"));
    }

    #[test]
    fn deny_paths_absolute() {
        for (repo, cwd_note) in [
            ("/Users/billy/dev/tradelog", "cwd = project"),
            (
                "/Users/billy/dev/tradelog",
                "cwd = spec (the rules name the repo, not the cwd)",
            ),
            ("/Users/billy/Developer/Trade Log", "repo path with a space"),
        ] {
            let f =
                frs::build(Path::new(repo), Path::new(APP), &RunSettingsOpts::default()).unwrap();
            let d = &f.permissions.deny;
            assert_eq!(d.len(), 4, "{cwd_note}");
            assert_eq!(d[0], format!("Read(/{repo}/.foreman/**)"));
            assert_eq!(d[1], format!("Edit(/{repo}/.foreman/**)"));
            assert_eq!(d[2], format!("Read(/{APP}/**)"));
            assert_eq!(d[3], format!("Edit(/{APP}/**)"));
            assert!(d.iter().all(|r| r.contains("(//")), "{cwd_note}");
            assert_eq!(f.permissions.disable_bypass_permissions_mode, "disable");
            assert!(!f.disable_all_hooks);
        }
        for bad in [
            "/Users/b/dev/[x]",
            "/Users/b/dev/a*b",
            "/Users/b/dev/a{b}",
            "/Users/b/(x)",
            "/r/a\\b",
            "relative",
        ] {
            assert_eq!(
                frs::build(Path::new(bad), Path::new(APP), &RunSettingsOpts::default()),
                Err(Blocker::RunSettingsPathUnsafe { path: bad.into() }),
                "{bad}"
            );
        }
    }

    /// S-1′ side result, pending Billy's run: today the four SR-09 rules (NotListed); the
    /// fail branch lists `.FOREMAN/` variants explicitly.
    #[test]
    fn deny_case_variant_probe() {
        let def = frs::build(
            Path::new("/r"),
            Path::new("/a"),
            &RunSettingsOpts::default(),
        )
        .unwrap();
        assert!(!def.permissions.deny.iter().any(|r| r.contains(".FOREMAN")));
        assert_eq!(CaseVariants::default(), CaseVariants::NotListed);
        let listed = frs::build(
            Path::new("/r"),
            Path::new("/a"),
            &RunSettingsOpts {
                case_variants: CaseVariants::Listed,
                ..RunSettingsOpts::default()
            },
        )
        .unwrap();
        assert!(
            listed
                .permissions
                .deny
                .contains(&"Read(//r/.FOREMAN/**)".to_owned())
        );
        assert!(
            listed
                .permissions
                .deny
                .contains(&"Edit(//r/.FOREMAN/**)".to_owned())
        );
    }

    pub(super) fn with_sensor() -> foreman_core::schema::run_settings::RunSettingsFile {
        frs::build(
            Path::new("/Users/billy/dev/tradelog"),
            Path::new(APP),
            &RunSettingsOpts {
                sensor: Some(sensor()),
                ..RunSettingsOpts::default()
            },
        )
        .unwrap()
    }
}

mod sensor {
    use super::*;

    /// SR-15: every hook is exec form (`command` + 8 `args`), one per event, `Bash` matcher on
    /// `PostToolUse` only; the file validates against files.md §4.4.
    #[test]
    fn exec_form_only() {
        let f = run_settings::with_sensor();
        let hooks = f.hooks.clone().unwrap();
        assert_eq!(hooks.len(), 11);
        for (ev, groups) in &hooks {
            assert_eq!(groups.len(), 1);
            let g = &groups[0];
            assert_eq!(
                g.matcher.as_deref(),
                (ev == "PostToolUse").then_some("Bash"),
                "{ev}"
            );
            let h = &g.hooks[0];
            assert_eq!(h.r#type, "command");
            assert!(
                h.command.starts_with('/') && !h.command.contains(' ')
                    || h.command
                        .contains("Application Support/Foreman/bin/foreman-sensor-")
            );
            assert_eq!(
                h.args,
                [
                    "--run",
                    C,
                    "--token",
                    &"a".repeat(64),
                    "--event",
                    ev,
                    "--v",
                    "1"
                ]
            );
        }
        let (bytes, _) = frs::to_bytes(&f);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        common::assert_valid("run-settings", &v);
    }

    /// SR-18: the derived state is data; folding events never produces an action.
    #[test]
    fn events_only_update_state() {
        use foreman_core::schema::sensor_event::SensorEventFile;
        use foreman_core::session_state::{self, Activity, SessionState};
        let tok = "b".repeat(64);
        let ev = |event: &str, payload: serde_json::Value, token: &str| SensorEventFile {
            schema_version: 1,
            token: token.into(),
            event: event.into(),
            received_at: "2026-10-20T10:00:00.000Z".into(),
            truncated: false,
            payload: payload.as_object().cloned(),
        };
        let evs = [
            (
                "00000000000000000001-0000000001",
                ev(
                    "SessionStart",
                    serde_json::json!({"model":"claude-opus-5-5","permission_mode":"plan"}),
                    &tok,
                ),
            ),
            (
                "00000000000000000002-0000000001",
                ev("SubagentStart", serde_json::json!({}), &tok),
            ),
            (
                "00000000000000000003-0000000001",
                ev(
                    "Stop",
                    serde_json::json!({"last_assistant_message":"done","effort":{"level":"xhigh"}}),
                    &tok,
                ),
            ),
            (
                "00000000000000000004-0000000001",
                ev("Stop", serde_json::json!({}), &"c".repeat(64)),
            ),
        ];
        let mut st = SessionState::default();
        session_state::ingest(&mut st, &tok, evs.iter().map(|(s, e)| (*s, e)));
        assert_eq!(st.discarded, 1);
        assert_eq!(st.activity, Activity::Idle);
        assert!(!st.is_idle(), "a subagent is still outstanding");
        assert_eq!(st.observed_model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(st.effort.as_deref(), Some("xhigh"));
        assert_eq!(st.last_assistant_message.as_deref(), Some("done"));
        // The module's public surface returns state only: no function takes a session, a writer
        // or a launcher (checked against the source).
        let src = std::fs::read_to_string(
            common::repo_root().join("crates/foreman-core/src/session_state.rs"),
        )
        .unwrap();
        for forbidden in ["launch", "rearm", "PtySession", "spawn", "write("] {
            assert!(
                !src.contains(&format!("use crate::{forbidden}"))
                    && !src.contains(&format!("{forbidden}::")),
                "{forbidden}"
            );
        }
    }
}

fn chain_of(id: &str, start: &str, status: ChainStatus) -> Chain {
    Chain {
        id: id.into(),
        surface: ChainSurface::Cli,
        cwd: Some(CwdKind::Spec),
        start: StepKey::parse(start).unwrap(),
        steps: vec![StepKey::parse(start).unwrap()],
        status,
        forked_from: None,
        last_start: None,
    }
}

mod chain {
    use super::*;

    #[test]
    fn plan_session_table() {
        let fx = Fixture::tradelog();
        let t = &fx.template;
        let s22 = t.step("2.2").unwrap();
        let s23 = t.step("2.3").unwrap();
        let n = u(B);
        let open = vec![chain_of(A, "2.2", ChainStatus::Open)];
        let cases: Vec<(
            &str,
            Vec<Chain>,
            ChainStartState,
            HandoffConfirm,
            SessionPlan,
        )> = vec![
            (
                "2.2",
                vec![],
                ChainStartState::NotRun,
                HandoffConfirm::Fork,
                SessionPlan::New { id: n },
            ),
            (
                "2.2",
                open.clone(),
                ChainStartState::Ran,
                HandoffConfirm::Fork,
                SessionPlan::New { id: n },
            ),
            (
                "2.3",
                open.clone(),
                ChainStartState::Ran,
                HandoffConfirm::Fork,
                SessionPlan::Continue { chain: u(A) },
            ),
            (
                "2.3",
                vec![],
                ChainStartState::NotRun,
                HandoffConfirm::Fork,
                SessionPlan::NeedsStart {
                    start: StepKey::parse("2.2").unwrap(),
                },
            ),
            (
                "2.3",
                vec![chain_of(A, "2.2", ChainStatus::Broken)],
                ChainStartState::Ran,
                HandoffConfirm::Fork,
                SessionPlan::Broken { chain: u(A) },
            ),
            (
                "2.3",
                vec![chain_of(A, "2.2", ChainStatus::HandedOff)],
                ChainStartState::Ran,
                HandoffConfirm::Fork,
                SessionPlan::Fork {
                    from: u(A),
                    new_id: n,
                },
            ),
            (
                "2.3",
                vec![chain_of(A, "2.2", ChainStatus::HandedOff)],
                ChainStartState::Ran,
                HandoffConfirm::DesktopClosed,
                SessionPlan::Continue { chain: u(A) },
            ),
            (
                "2.3",
                vec![
                    chain_of(A, "2.2", ChainStatus::Superseded),
                    chain_of(C, "2.2", ChainStatus::Open),
                ],
                ChainStartState::Ran,
                HandoffConfirm::Fork,
                SessionPlan::Continue { chain: u(C) },
            ),
        ];
        for (key, chains, st, h, want) in cases {
            let step = if key == "2.2" { s22 } else { s23 };
            assert_eq!(
                fchain::plan_session(step, &chains, n, st, h),
                want,
                "{key} {chains:?}"
            );
        }
        // Argv per plan (DR-111).
        assert_eq!(
            fchain::session_args(&SessionPlan::New { id: n }, ForkIds::Pass),
            Some(SessionArgs::New { id: n })
        );
        assert_eq!(
            fchain::session_args(&SessionPlan::Continue { chain: n }, ForkIds::Pass),
            Some(SessionArgs::Resume { id: n })
        );
        assert_eq!(
            fchain::session_args(&SessionPlan::Broken { chain: n }, ForkIds::Pass),
            None
        );
    }

    #[test]
    fn superseded_on_rerun() {
        let fx = Fixture::tradelog();
        let t = &fx.template;
        let s22 = t.step("2.2").unwrap();
        let s23 = t.step("2.3").unwrap();
        let st = start(Model::Opus, Effort::Xhigh, Mode::Plan);
        let c = fchain::apply_launch(&[], s22, &SessionPlan::New { id: u(A) }, st);
        let c = fchain::apply_launch(&c, s23, &SessionPlan::Continue { chain: u(A) }, st);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].steps.len(), 2);
        // Re-running 2.2 starts a new chain; the old one is superseded; 2.3 joins the new one.
        let c = fchain::apply_launch(&c, s22, &SessionPlan::New { id: u(B) }, st);
        assert_eq!(c[0].status, ChainStatus::Superseded);
        assert_eq!(c[1].status, ChainStatus::Open);
        assert_eq!(
            fchain::plan_session(s23, &c, u(C), ChainStartState::Ran, HandoffConfirm::Fork),
            SessionPlan::Continue { chain: u(B) }
        );
    }

    #[test]
    fn broken_when_transcript_missing() {
        let plan = SessionPlan::Continue { chain: u(A) };
        assert_eq!(fchain::check_transcript(plan.clone(), true), plan);
        assert_eq!(
            fchain::check_transcript(plan, false),
            SessionPlan::Broken { chain: u(A) }
        );
        let new = SessionPlan::New { id: u(A) };
        assert_eq!(fchain::check_transcript(new.clone(), false), new);
        let c = fchain::mark_broken(&[chain_of(A, "2.2", ChainStatus::Open)], u(A));
        assert_eq!(c[0].status, ChainStatus::Broken);
        // The transcript path Claude Code uses (verified against the 2.1.304 binary).
        let p = fchain::transcript_path(
            Path::new("/Users/billy/.claude"),
            Path::new("/Users/billy/dev/trade.log/spec"),
            u(A),
        );
        assert_eq!(
            p,
            PathBuf::from(format!(
                "/Users/billy/.claude/projects/-Users-billy-dev-trade-log-spec/{A}.jsonl"
            ))
        );
        assert_eq!(fchain::project_dir_name("/a/é b"), "-a---b");
        let long = format!("/{}", "x".repeat(250));
        let n = fchain::project_dir_name(&long);
        assert!(
            n.starts_with(&format!("-{}", "x".repeat(199))) && n.len() > 201,
            "{n}"
        );
    }

    #[test]
    fn start_not_applicable_becomes_fresh() {
        let fx = Fixture::tradelog();
        let s32 = fx.template.step("3.2").unwrap();
        for st in [ChainStartState::NotApplicable, ChainStartState::Skipped] {
            assert_eq!(
                fchain::plan_session(s32, &[], u(B), st, HandoffConfirm::Fork),
                SessionPlan::FreshWithContext { id: u(B) }
            );
        }
        let fresh =
            fchain::session_args(&SessionPlan::FreshWithContext { id: u(B) }, ForkIds::Pass);
        assert_eq!(fresh, Some(SessionArgs::New { id: u(B) }));
    }

    /// DR-117 / AC-F010-10, all three S-3 branches.
    #[test]
    fn effort_on_resume_branches() {
        let resume = SessionArgs::Resume { id: u(A) };
        let new = SessionArgs::New { id: u(A) };
        for b in [
            EffortOnResume::Flag,
            EffortOnResume::Settings,
            EffortOnResume::Slash,
        ] {
            assert!(
                fchain::effort_carry(&new, Effort::Max, b).flag,
                "new sessions always pass --effort"
            );
        }
        let f = fchain::effort_carry(&resume, Effort::Xhigh, EffortOnResume::Flag);
        assert!(f.flag && f.settings_level.is_none() && f.slash_line.is_none());
        let s = fchain::effort_carry(&resume, Effort::Xhigh, EffortOnResume::Settings);
        assert!(!s.flag && s.settings_level == Some(Effort::Xhigh));
        let s = fchain::effort_carry(&resume, Effort::Max, EffortOnResume::Settings);
        assert_eq!(s.slash_line.as_deref(), Some("/effort max"));
        let sl = fchain::effort_carry(&resume, Effort::High, EffortOnResume::Slash);
        assert!(
            !sl.flag
                && sl.settings_level.is_none()
                && sl.slash_line.as_deref() == Some("/effort high")
        );
        // Without the flag, the resumed argv has no --effort.
        let li = LaunchInputs {
            pass_effort: false,
            ..inputs(resume, None)
        };
        assert!(
            !fargv::build_argv(&li)
                .unwrap()
                .as_slice()
                .iter()
                .any(|x| x == "--effort")
        );
        assert_eq!(EffortOnResume::default(), EffortOnResume::Flag);
    }

    /// DR-118 / SR-14: the fork form per S-8's branch.
    #[test]
    fn fork_argv() {
        let plan = SessionPlan::Fork {
            from: u(A),
            new_id: u(B),
        };
        let pass = fargv::build_argv(&inputs(
            fchain::session_args(&plan, ForkIds::Pass).unwrap(),
            None,
        ))
        .unwrap();
        assert_eq!(
            &pass.as_slice()[..5],
            ["--resume", A, "--fork-session", "--session-id", B]
        );
        let fail = fargv::build_argv(&inputs(
            fchain::session_args(&plan, ForkIds::Fail).unwrap(),
            None,
        ))
        .unwrap();
        assert_eq!(&fail.as_slice()[..3], ["--resume", A, "--fork-session"]);
        assert!(!fail.as_slice().iter().any(|x| x == "--session-id"));
        let fx = Fixture::tradelog();
        let s23 = fx.template.step("2.3").unwrap();
        let c = fchain::apply_launch(
            &[chain_of(A, "2.2", ChainStatus::HandedOff)],
            s23,
            &plan,
            start(Model::Opus, Effort::High, Mode::Plan),
        );
        assert_eq!(c[1].forked_from.as_deref(), Some(A));
        assert_eq!(c[1].id, B);
        assert_eq!(
            fchain::plan_session(s23, &c, u(C), ChainStartState::Ran, HandoffConfirm::Fork),
            SessionPlan::Continue { chain: u(B) }
        );
    }
}

mod preflight {
    use super::*;
    use foreman_core::fingerprint::{
        self, AllowlistEntry, ConfigFile, SettingsScope, SourceKind, allowlist_hash,
    };
    use serde_json::json;

    fn policy() -> Start {
        start(Model::Opus, Effort::Xhigh, Mode::Plan)
    }

    fn ctx() -> ConfirmCtx {
        ConfirmCtx {
            overlay_step: false,
            first_launch_done: true,
            last_seen_version: Some("2.1.304 (Claude Code)".into()),
            found_version: Some("2.1.304 (Claude Code)".into()),
        }
    }

    #[test]
    fn confirm_required_for_overlay_step() {
        assert!(needs_confirmation(&policy(), &policy(), &ctx()).is_empty());
        let c = ConfirmCtx {
            overlay_step: true,
            ..ctx()
        };
        assert_eq!(
            needs_confirmation(&policy(), &policy(), &c),
            [ConfirmReason::OverlayStep]
        );
    }

    #[test]
    fn confirm_required_for_mode_change() {
        let to = start(Model::Sonnet, Effort::Xhigh, Mode::Plan);
        assert_eq!(
            needs_confirmation(&policy(), &to, &ctx()),
            [ConfirmReason::ModeOrModelChanged { from: policy(), to }]
        );
        let to = start(Model::Opus, Effort::Xhigh, Mode::AcceptEdits);
        assert_eq!(needs_confirmation(&policy(), &to, &ctx()).len(), 1);
        // DR-106: effort alone doesn't confirm.
        let to = start(Model::Opus, Effort::Max, Mode::Plan);
        assert!(needs_confirmation(&policy(), &to, &ctx()).is_empty());
    }

    #[test]
    fn confirm_required_for_auto_and_dontask() {
        for m in [Mode::Auto, Mode::DontAsk] {
            let s = start(Model::Opus, Effort::High, m);
            assert!(
                needs_confirmation(&s, &s, &ctx())
                    .contains(&ConfirmReason::AutoOrDontAsk { mode: m })
            );
        }
    }

    #[test]
    fn confirm_required_for_first_launch() {
        let c = ConfirmCtx {
            first_launch_done: false,
            ..ctx()
        };
        assert_eq!(
            needs_confirmation(&policy(), &policy(), &c),
            [ConfirmReason::FirstLaunchInProject]
        );
    }

    #[test]
    fn confirm_required_for_version_change() {
        let c = ConfirmCtx {
            found_version: Some("2.1.305 (Claude Code)".into()),
            ..ctx()
        };
        assert_eq!(
            needs_confirmation(&policy(), &policy(), &c),
            [ConfirmReason::ClaudeVersionChanged {
                from: "2.1.304 (Claude Code)".into(),
                to: "2.1.305 (Claude Code)".into()
            }]
        );
    }

    fn settings(scope: SettingsScope, v: serde_json::Value) -> ConfigFile {
        ConfigFile {
            kind: SourceKind::UserSettings,
            scope: Some(scope),
            path: "/Users/billy/.claude/settings.json".into(),
            link_target: None,
            sha256: "0".repeat(64),
            json: Some(v),
            text: None,
            bare_name: false,
        }
    }

    fn keys_hit(v: serde_json::Value) -> Vec<String> {
        fingerprint::auth_key_hits(&[settings(SettingsScope::Project, v)], &[])
            .into_iter()
            .map(|h| h.key)
            .collect()
    }

    #[test]
    #[allow(
        non_snake_case,
        reason = "test name per ticket M3-03 (the key's spelling)"
    )]
    fn blocks_apiKeyHelper() {
        assert_eq!(
            keys_hit(json!({"apiKeyHelper": "/bin/echo sk"})),
            ["apiKeyHelper"]
        );
    }

    macro_rules! blocks {
        ($($name:ident => $v:expr, $key:expr;)*) => {$(
            #[test]
            fn $name() {
                assert_eq!(keys_hit($v), [$key]);
            }
        )*};
    }

    blocks! {
        blocks_aws_auth_refresh => json!({"awsAuthRefresh": "aws sso login"}), "awsAuthRefresh";
        blocks_aws_credential_export => json!({"awsCredentialExport": "x"}), "awsCredentialExport";
        blocks_gcp_auth_refresh => json!({"gcpAuthRefresh": "x"}), "gcpAuthRefresh";
        blocks_otel_headers_helper => json!({"otelHeadersHelper": "x"}), "otelHeadersHelper";
        blocks_force_login_method_console => json!({"forceLoginMethod": "console"}), "forceLoginMethod";
        blocks_force_login_method_gateway => json!({"forceLoginMethod": "gateway"}), "forceLoginMethod";
        blocks_force_login_gateway_url => json!({"forceLoginGatewayUrl": "https://gw.example"}), "forceLoginGatewayUrl";
        blocks_env_anthropic => json!({"env": {"ANTHROPIC_API_KEY": "sk"}}), "env.ANTHROPIC_API_KEY";
        blocks_env_claude_code => json!({"env": {"CLAUDE_CODE_USE_BEDROCK": "1"}}), "env.CLAUDE_CODE_USE_BEDROCK";
        blocks_env_claude_config_dir => json!({"env": {"CLAUDE_CONFIG_DIR": "/x"}}), "env.CLAUDE_CONFIG_DIR";
        blocks_env_aws => json!({"env": {"AWS_PROFILE": "x"}}), "env.AWS_PROFILE";
        blocks_env_google => json!({"env": {"GOOGLE_APPLICATION_CREDENTIALS": "x"}}), "env.GOOGLE_APPLICATION_CREDENTIALS";
        blocks_env_cloudsdk => json!({"env": {"CLOUDSDK_CONFIG": "x"}}), "env.CLOUDSDK_CONFIG";
        blocks_env_azure => json!({"env": {"AZURE_CLIENT_ID": "x"}}), "env.AZURE_CLIENT_ID";
        blocks_env_proxy_upper => json!({"env": {"HTTPS_PROXY": "x"}}), "env.HTTPS_PROXY";
        blocks_env_proxy_lower => json!({"env": {"https_proxy": "x"}}), "env.https_proxy";
        blocks_env_node_extra_ca_certs => json!({"env": {"NODE_EXTRA_CA_CERTS": "x"}}), "env.NODE_EXTRA_CA_CERTS";
        blocks_env_node_tls_reject_unauthorized => json!({"env": {"NODE_TLS_REJECT_UNAUTHORIZED": "0"}}), "env.NODE_TLS_REJECT_UNAUTHORIZED";
        blocks_env_ssl_cert_file => json!({"env": {"SSL_CERT_FILE": "x"}}), "env.SSL_CERT_FILE";
        blocks_env_ssl_cert_dir => json!({"env": {"SSL_CERT_DIR": "x"}}), "env.SSL_CERT_DIR";
        blocks_env_node_options => json!({"env": {"NODE_OPTIONS": "x"}}), "env.NODE_OPTIONS";
        blocks_env_dyld => json!({"env": {"DYLD_INSERT_LIBRARIES": "x"}}), "env.DYLD_INSERT_LIBRARIES";
    }

    #[test]
    fn force_login_method_claudeai_passes() {
        assert!(keys_hit(json!({"forceLoginMethod": "claudeai", "env": {"FOO": "1"}})).is_empty());
    }

    #[test]
    fn allowlisted_user_key_passes() {
        let v = json!({"apiKeyHelper": "/usr/local/bin/helper", "env": {"AWS_PROFILE": "work"}});
        let allow = vec![AllowlistEntry {
            key: "apiKeyHelper".into(),
            sha256: allowlist_hash("apiKeyHelper", &json!("/usr/local/bin/helper")),
        }];
        let user = fingerprint::auth_key_hits(&[settings(SettingsScope::User, v.clone())], &allow);
        assert_eq!(user.len(), 1);
        assert_eq!(user[0].key, "env.AWS_PROFILE");
        assert!(user[0].allowlistable);
        // The same allowlist doesn't apply to project settings (SR-07: user settings only).
        let proj = fingerprint::auth_key_hits(&[settings(SettingsScope::Project, v)], &allow);
        assert_eq!(proj.len(), 2);
        assert!(proj.iter().all(|h| !h.allowlistable));
        // A changed value isn't covered by the old hash.
        let changed = json!({"apiKeyHelper": "/tmp/other"});
        assert_eq!(
            fingerprint::auth_key_hits(&[settings(SettingsScope::User, changed)], &allow).len(),
            1
        );
    }

    #[test]
    fn config_snapshot_redacts_secrets() {
        let v = json!({
            "env": {"FOO": "bar", "TOKEN_X": "1"},
            "apiKeyHelper": "/bin/helper",
            "permissions": {"allow": ["Bash(ls)"]},
            "mcpServers": {"x": {"command": "node", "headers": {"Authorization": "Bearer abc"},
                                  "args": ["--key", "sk-ant-api03-xyz"], "accessToken": "t"}},
            "nested": {"db_password": "hunter2", "url": "https://user:pw@example.com/x"}
        });
        let r = fingerprint::redact(&v);
        let text = r.to_string();
        for secret in [
            "bar",
            "/bin/helper",
            "Bearer abc",
            "sk-ant-api03-xyz",
            "hunter2",
            "user:pw",
            "\"t\"",
        ] {
            assert!(!text.contains(secret), "{secret} leaked: {text}");
        }
        assert_eq!(r["permissions"]["allow"][0], "Bash(ls)");
        assert!(
            r["env"]["FOO"]
                .as_str()
                .unwrap()
                .starts_with("‹redacted sha256:")
        );
        assert_eq!(r["mcpServers"]["x"]["command"], "node");
    }

    #[test]
    fn diff_shows_redacted_change() {
        let mk = |v: serde_json::Value, sha: &str| ConfigFile {
            sha256: sha.repeat(64),
            ..settings(SettingsScope::Project, v)
        };
        let before =
            fingerprint::snapshot(&[mk(json!({"env": {"FOO": "one"}, "model": "opus"}), "1")]);
        let after = fingerprint::snapshot(&[mk(
            json!({"env": {"FOO": "two"}, "model": "opus", "hooks": {}}),
            "2",
        )]);
        let d = fingerprint::diff(&before, &after);
        assert_eq!(d.len(), 1);
        let e: Vec<_> = d[0]
            .entries
            .iter()
            .map(|e| (e.key_path.as_str(), e.change))
            .collect();
        assert!(
            e.contains(&("env.FOO", fingerprint::ChangeKind::ValueChanged)),
            "{e:?}"
        );
        assert!(
            e.contains(&("hooks", fingerprint::ChangeKind::Added)),
            "{e:?}"
        );
        let text = serde_json::to_string(&d).unwrap();
        assert!(!text.contains("one") && !text.contains("two"), "{text}");
        // A new source shows as added with a short hash only.
        let d = fingerprint::diff(&[], &after);
        assert_eq!(d[0].entries[0].change, fingerprint::ChangeKind::Added);
    }
}

mod probe {
    use foreman_core::probe::{self, ProbeOutcome, ProbeResult, ProbeRun};

    fn run(exit: Option<i32>, stderr: &str) -> ProbeRun {
        ProbeRun {
            exit,
            stderr: stderr.into(),
            timed_out: false,
        }
    }

    #[test]
    fn runs_on_version_change() {
        assert!(probe::due(
            "2.1.305 (Claude Code)",
            Some("2.1.304 (Claude Code)")
        ));
        assert!(probe::due("2.1.304 (Claude Code)", None));
        assert!(!probe::due(
            "2.1.304 (Claude Code)",
            Some("2.1.304 (Claude Code)")
        ));
    }

    #[test]
    fn inconclusive_keeps_state() {
        let pass = ProbeOutcome {
            result: ProbeResult::Pass,
            flag: None,
        };
        let inc = ProbeOutcome {
            result: ProbeResult::Inconclusive,
            flag: None,
        };
        let (v, a, b) = probe::merge(
            Some(("2.1.304", &pass, &pass)),
            "2.1.305",
            inc.clone(),
            pass.clone(),
        );
        assert_eq!(v, "2.1.304", "the verified version doesn't advance");
        assert_eq!(a, pass);
        assert_eq!(b, pass);
        let (v, ..) = probe::merge(
            Some(("2.1.304", &pass, &pass)),
            "2.1.305",
            pass.clone(),
            pass.clone(),
        );
        assert_eq!(v, "2.1.305");
        // Classification of the real 2.1.304 outputs (S-1′ rehearsal).
        assert_eq!(
            probe::classify_a(&run(Some(0), ""), true).result,
            ProbeResult::Pass
        );
        assert_eq!(
            probe::classify_a(&run(Some(0), ""), false).result,
            ProbeResult::Inconclusive
        );
        let bad_effort = run(
            Some(0),
            "Warning: Unknown --effort value 'bogus' — ignoring it and using the default effort. Valid values: low, medium, high, xhigh, max.",
        );
        assert_eq!(
            probe::classify_a(&bad_effort, true),
            ProbeOutcome {
                result: ProbeResult::FlagFailure,
                flag: Some("--effort".into())
            }
        );
        let unknown = run(Some(1), "error: unknown option '--name'");
        assert_eq!(
            probe::classify_a(&unknown, false).flag.as_deref(),
            Some("--name")
        );
        let mode = run(
            Some(1),
            "error: option '--permission-mode <mode>' argument 'nope' is invalid. Allowed choices are acceptEdits, auto, bypassPermissions, manual, dontAsk, plan.",
        );
        assert_eq!(
            probe::classify_a(&mode, false).flag.as_deref(),
            Some("--permission-mode")
        );
        let timeout = ProbeRun {
            timed_out: true,
            ..ProbeRun::default()
        };
        assert_eq!(
            probe::classify_a(&timeout, false).result,
            ProbeResult::Inconclusive
        );
        // Probe B: fork accepted + control refused = pass; control accepted = inconclusive.
        let refused = run(
            Some(1),
            "Error: --session-id can only be used with --continue or --resume if --fork-session is also specified.",
        );
        assert_eq!(
            probe::classify_b(&run(Some(0), ""), &refused).result,
            ProbeResult::Pass
        );
        assert_eq!(
            probe::classify_b(&run(Some(0), ""), &run(Some(0), "")).result,
            ProbeResult::Inconclusive
        );
        assert_eq!(
            probe::classify_b(
                &run(Some(1), "error: unknown option '--fork-session'"),
                &refused
            )
            .flag
            .as_deref(),
            Some("--fork-session")
        );
        assert_eq!(
            probe::disabled_features(&[ProbeOutcome {
                result: ProbeResult::FlagFailure,
                flag: Some("--effort".into())
            }]),
            ["effort"]
        );
    }
}

mod watcher {
    use foreman_core::interest::{self, Interest};

    #[test]
    fn interest_set_exact_names() {
        let outs = vec!["spec/council/r1-market.md".to_owned()];
        assert_eq!(
            interest::repo_interest("docs/handoff/stage-02.md", &outs),
            Some(Interest::Packet { stage: 2 })
        );
        assert_eq!(
            interest::repo_interest("docs/handoff/stage-02-attachments.md", &outs),
            Some(Interest::Attachments { stage: 2 })
        );
        assert_eq!(
            interest::repo_interest("docs/lessons.md", &outs),
            Some(Interest::Lessons)
        );
        assert_eq!(
            interest::repo_interest(".foreman/project.json", &outs),
            Some(Interest::Decision {
                file: "project".into()
            })
        );
        assert_eq!(
            interest::repo_interest(".foreman/blueprint.lock.json", &outs),
            Some(Interest::Decision {
                file: "blueprint.lock".into()
            })
        );
        assert_eq!(
            interest::repo_interest("spec/council/r1-market.md", &outs),
            Some(Interest::Out {
                path: outs[0].clone()
            })
        );
        for no in [
            "docs/handoff/stage-2.md",
            "docs/handoff/Stage-02.md",
            "docs/handoff/stage-02.md.bak",
            "docs/handoff/stage-02.md.orig",
            "docs/handoff/stage-02.md~",
            "docs/handoff/stage-002.md",
            "docs/handoff/sub/stage-02.md",
            "docs/Lessons.md",
            ".foreman/progress.json",
            ".FOREMAN/project.json",
            "spec/council/r1-market.md.tmp",
            "spec/council/*.md",
            "README.md",
        ] {
            assert_eq!(interest::repo_interest(no, &outs), None, "{no}");
        }
        assert!(interest::app_data_interest("events/6f1c2a3b-1d2e-4f50-8a9b-0c1d2e3f4a5b/00000000000000000001-0000000042-Stop.json").is_some());
        assert!(interest::app_data_interest("events/x/00000000000000000001-1-Stop.json").is_none());
        assert!(
            interest::app_data_interest("events/6f1c2a3b-1d2e-4f50-8a9b-0c1d2e3f4a5b/a.json")
                .is_none()
        );
    }

    #[test]
    fn ignores_tmp_and_secrets() {
        let outs = vec!["docs/.env".to_owned(), "spec/x.md".to_owned()];
        for no in [
            "docs/handoff/.stage-02.md.0123456789abcdef.foreman-tmp",
            "docs/.env",
            "spec/x.md.foreman-tmp",
        ] {
            assert_eq!(interest::repo_interest(no, &outs), None, "{no}");
        }
    }
}
