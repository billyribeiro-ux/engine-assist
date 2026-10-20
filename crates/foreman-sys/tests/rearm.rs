//! M3-13, M3-15: re-arm against fake-claude in a real PTY (ADR-008, DR-114–DR-116, SR-13,
//! SR-18): Ctrl-C, 150 ms, `/exit`, wait, re-prepare, relaunch with `--resume`; no signal ever.
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

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::Collect;
use foreman_core::chain::{EffortCarry, SessionPlan};
use foreman_core::launch::LaunchProgress;
use foreman_core::launch::blocker::EndingStep;
use foreman_core::rearm::{IdleSource, UserAction};
use foreman_sys::launch::{LaunchPlan, PrepareOutcome};
use foreman_sys::pty::{self as fpty, PtySession, Sink};
use foreman_sys::rearm::{self as rr, Hooks, RearmOutcome, RearmUi, Timings};
use foreman_sys::spawn::{Program, SpawnCtx};
use uuid::Uuid;

const SID: &str = "6f1c2a3b-1d2e-4f50-8a9b-0c1d2e3f4a5b";

struct Rig {
    _dir: tempfile::TempDir,
    root: PathBuf,
    record: PathBuf,
    ctx: SpawnCtx,
}

fn rig(script: serde_json::Value) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let root = common::canon(dir.path());
    let record = root.join("rec.json");
    let mut cfg = script;
    cfg["record"] = serde_json::json!(record);
    let claude = common::install_fake(&root.join("bin"), "claude", &cfg);
    std::fs::create_dir_all(root.join("home")).unwrap();
    std::fs::create_dir_all(root.join("repo")).unwrap();
    Rig {
        ctx: SpawnCtx {
            home: root.join("home"),
            app_data: root.join("app"),
            projects: vec![root.join("repo")],
            settings: vec![(Program::Claude, claude)],
        },
        _dir: dir,
        root,
        record,
    }
}

fn plan(r: &Rig, args: &[&str], prompt: Option<&str>) -> LaunchPlan {
    let run = Uuid::new_v4();
    let mut argv: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    if let Some(p) = prompt {
        argv.push(p.to_owned());
    }
    LaunchPlan {
        run_id: run,
        program: PathBuf::new(),
        argv_redacted: argv.clone(),
        argv,
        env: common::child_env(&r.root.join("home"), run),
        cwd: r.root.join("repo"),
        run_settings_path: r.root.join(format!("app/runs/{run}/settings.json")),
        run_settings: vec![],
        run_settings_sha256: String::new(),
        confirm: vec![],
        session: SessionPlan::Continue {
            chain: Uuid::parse_str(SID).unwrap(),
        },
        effort: EffortCarry::default(),
        claude_version: "2.1.304 (Claude Code)".into(),
        fingerprint: String::new(),
        tab_name: "fm-tradelog-s02-2-3".into(),
        prompt_sha256: prompt.map(|p| foreman_core::store::sha256_hex(p.as_bytes())),
        warnings: vec![],
    }
}

fn start_old(r: &Rig, sink: &Arc<Collect>) -> PtySession {
    let run = Uuid::new_v4();
    let env = common::child_env(&r.root.join("home"), run);
    let s = PtySession::spawn(
        &r.ctx,
        &[
            "--session-id".into(),
            SID.into(),
            "--model".into(),
            "opus".into(),
        ],
        &env.0,
        &r.root.join("repo"),
        fpty::size(100, 30).unwrap(),
        Arc::clone(sink) as Arc<dyn Sink>,
    )
    .unwrap();
    sink.wait_for("FAKE-CLAUDE READY", 10);
    s
}

#[derive(Default)]
struct Ui {
    idle: bool,
    busy: bool,
    log: Mutex<Vec<(String, Instant)>>,
}

impl RearmUi for Ui {
    fn confirm_idle(&self) -> bool {
        self.log
            .lock()
            .unwrap()
            .push(("idleDialog".into(), Instant::now()));
        self.idle
    }
    fn confirm_busy(&self, tasks: u32) -> bool {
        self.log
            .lock()
            .unwrap()
            .push((format!("busyDialog {tasks}"), Instant::now()));
        self.busy
    }
    fn end_yourself(&self) {
        self.log
            .lock()
            .unwrap()
            .push(("endYourself".into(), Instant::now()));
    }
    fn progress(&self, p: LaunchProgress) {
        let k = match p {
            LaunchProgress::Ending {
                step: EndingStep::CtrlC,
            } => "ctrlC",
            LaunchProgress::Ending {
                step: EndingStep::Exit,
            } => "exit",
            LaunchProgress::Ending {
                step: EndingStep::Waiting,
            } => "waiting",
            LaunchProgress::Exited { .. } => "exited",
            LaunchProgress::Spawning => "spawning",
            _ => "other",
        };
        self.log.lock().unwrap().push((k.into(), Instant::now()));
    }
}

impl Ui {
    fn kinds(&self) -> Vec<String> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .map(|(k, _)| k.clone())
            .collect()
    }
    fn at(&self, k: &str) -> Instant {
        self.log
            .lock()
            .unwrap()
            .iter()
            .find(|(x, _)| x == k)
            .unwrap()
            .1
    }
}

fn resume_plan(r: &Rig) -> LaunchPlan {
    plan(
        r,
        &[
            "--resume",
            SID,
            "--model",
            "sonnet",
            "--permission-mode",
            "acceptEdits",
        ],
        Some("Now 2.3."),
    )
}

mod rearm {
    use super::*;

    /// DR-115: Ctrl-C, then 150 ms, then `/exit\r`, then the exit; then the divider, then the
    /// relaunch with `--resume` and the new step's flags.
    #[test]
    fn ctrl_c_then_exit_order() {
        let r = rig(serde_json::json!({}));
        let old_sink = Arc::new(Collect::default());
        let old = start_old(&r, &old_sink);
        old.write(b"draft text in the box").unwrap();
        let confirmed = resume_plan(&r);
        let ui = Ui {
            idle: true,
            ..Ui::default()
        };
        let order = Mutex::new(Vec::<&str>::new());
        let new_sink = Arc::new(Collect::default());
        let mut prepare = || {
            order.lock().unwrap().push("prepare");
            PrepareOutcome::Ready(Box::new(resume_plan(&r)))
        };
        let mut archive = || order.lock().unwrap().push("archive");
        let mut relaunch = |p: &LaunchPlan| {
            order.lock().unwrap().push("spawn");
            PtySession::spawn(
                &r.ctx,
                &p.argv,
                &p.env.0,
                &p.cwd,
                fpty::size(100, 30).unwrap(),
                Arc::clone(&new_sink) as Arc<dyn Sink>,
            )
            .map_err(|e| e.to_string())
        };
        let out = rr::rearm_start(
            UserAction::from_ipc_invoke(),
            Some(&old),
            IdleSource::Billy,
            &confirmed,
            &ui,
            &mut Hooks {
                prepare_again: &mut prepare,
                archive: &mut archive,
                relaunch: &mut relaunch,
            },
        )
        .unwrap();
        let RearmOutcome::Relaunched { session, .. } = out else {
            panic!("{out:?}")
        };
        assert_eq!(*order.lock().unwrap(), ["prepare", "archive", "spawn"]);
        assert_eq!(
            ui.kinds()[..5],
            ["idleDialog", "ctrlC", "exit", "waiting", "exited"]
        );
        assert!(ui.at("exit") - ui.at("ctrlC") >= Duration::from_millis(150));
        // The old session ended by /exit (code 0), not by a signal.
        assert_eq!(old.wait_exit(Duration::ZERO).unwrap().code, Some(0));
        let log = common::tui_log(&r.record);
        let seq: Vec<String> = log
            .iter()
            .filter_map(|e| match e["type"].as_str() {
                Some("ctrlC") => Some("ctrlC".into()),
                Some("submit") => Some(format!("submit {}", e["text"].as_str().unwrap())),
                _ => None,
            })
            .collect();
        assert_eq!(
            seq[..2],
            ["ctrlC".to_owned(), "submit /exit".to_owned()],
            "{seq:?}"
        );
        new_sink.wait_for("mode=resume model=sonnet", 10);
        new_sink.wait_for("ECHO: Now 2.3.", 10);
        session.hangup();
    }

    /// SR-13: no ending without Billy's idle assertion; on a timeout nothing is signalled and
    /// Billy is asked to end the session himself.
    #[test]
    fn no_kill_without_idle_or_confirm() {
        let r = rig(serde_json::json!({ "tui": { "ignoreExit": true } }));
        let sink = Arc::new(Collect::default());
        let old = start_old(&r, &sink);
        let confirmed = resume_plan(&r);
        let mut never = || -> PrepareOutcome { panic!("prepared without an exit") };
        let mut no_archive = || panic!("archived without an exit");
        let mut no_spawn = |_: &LaunchPlan| -> Result<PtySession, String> { panic!("spawned") };
        // Cancel: nothing is written.
        let ui = Ui::default();
        let out = rr::rearm_start(
            UserAction::from_ipc_invoke(),
            Some(&old),
            IdleSource::Billy,
            &confirmed,
            &ui,
            &mut Hooks {
                prepare_again: &mut never,
                archive: &mut no_archive,
                relaunch: &mut no_spawn,
            },
        )
        .unwrap();
        assert!(matches!(out, RearmOutcome::Cancelled));
        assert_eq!(ui.kinds(), ["idleDialog"]);
        assert!(
            !common::tui_log(&r.record)
                .iter()
                .any(|e| e["type"] == "ctrlC")
        );
        // P2 busy: Wait sends nothing either.
        let ui = Ui::default();
        let out = rr::rearm_start(
            UserAction::from_ipc_invoke(),
            Some(&old),
            IdleSource::Sensor {
                idle: false,
                tasks: 2,
            },
            &confirmed,
            &ui,
            &mut Hooks {
                prepare_again: &mut never,
                archive: &mut no_archive,
                relaunch: &mut no_spawn,
            },
        )
        .unwrap();
        assert!(matches!(out, RearmOutcome::Cancelled));
        assert_eq!(ui.kinds(), ["busyDialog 2"]);
        // Confirmed, but Claude ignores /exit: after the wait, "end it yourself" — still alive.
        let ui = Ui {
            idle: true,
            ..Ui::default()
        };
        let out = rr::rearm_start_with(
            UserAction::from_ipc_invoke(),
            Some(&old),
            IdleSource::Billy,
            &confirmed,
            &ui,
            &mut Hooks {
                prepare_again: &mut never,
                archive: &mut no_archive,
                relaunch: &mut no_spawn,
            },
            Timings {
                pause: Duration::from_millis(150),
                exit_wait: Duration::from_secs(1),
            },
        )
        .unwrap();
        assert!(matches!(out, RearmOutcome::EndYourself));
        assert!(ui.kinds().contains(&"endYourself".to_owned()));
        std::thread::sleep(Duration::from_millis(300));
        assert!(old.is_live(), "no signal was sent");
        // Continue while it still runs: the same dialog again, nothing sent.
        let ui = Ui::default();
        let out = rr::rearm_continue(
            UserAction::from_ipc_invoke(),
            Some(&old),
            &confirmed,
            &ui,
            &mut Hooks {
                prepare_again: &mut never,
                archive: &mut no_archive,
                relaunch: &mut no_spawn,
            },
        )
        .unwrap();
        assert!(matches!(out, RearmOutcome::EndYourself));
        assert!(old.is_live());
        // Billy ends it himself; Continue relaunches.
        old.write(b"\x04").ok();
        old.hangup();
        assert!(old.wait_exit(Duration::from_secs(5)).is_some());
        let new_sink = Arc::new(Collect::default());
        let mut prepare = || PrepareOutcome::Ready(Box::new(resume_plan(&r)));
        let mut archive = || {};
        let mut relaunch = |p: &LaunchPlan| {
            PtySession::spawn(
                &r.ctx,
                &p.argv,
                &p.env.0,
                &p.cwd,
                fpty::size(80, 24).unwrap(),
                Arc::clone(&new_sink) as Arc<dyn Sink>,
            )
            .map_err(|e| e.to_string())
        };
        let out = rr::rearm_continue(
            UserAction::from_ipc_invoke(),
            Some(&old),
            &confirmed,
            &Ui::default(),
            &mut Hooks {
                prepare_again: &mut prepare,
                archive: &mut archive,
                relaunch: &mut relaunch,
            },
        )
        .unwrap();
        let RearmOutcome::Relaunched { session, .. } = out else {
            panic!("{out:?}")
        };
        new_sink.wait_for("mode=resume", 10);
        session.hangup();
        // Structural: the re-arm driver has no path to a signal.
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rearm.rs"),
        )
        .unwrap();
        for s in [".terminate(", ".hangup(", "kill", "Signal::"] {
            assert!(!src.contains(s), "rearm.rs mentions {s}");
        }
    }

    /// SR-18 / DR-114: only an IPC command handler constructs `UserAction`; the watcher and the
    /// sensor ingest can't reach re-arm, launch or PTY input.
    #[test]
    fn only_from_user_action() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut offenders = Vec::new();
        for dir in [
            "crates/foreman-core/src",
            "crates/foreman-sys/src",
            "crates/foreman-sensor/src",
            "src-tauri/src",
        ] {
            let mut stack = vec![root.join(dir)];
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                        continue;
                    }
                    let text = std::fs::read_to_string(&p).unwrap_or_default();
                    let allowed = p.ends_with("foreman-core/src/rearm.rs")
                        || p.to_string_lossy().contains("src-tauri/src/ipc/commands/");
                    if text.contains("from_ipc_invoke(") && !allowed {
                        offenders.push(p.display().to_string());
                    }
                }
            }
        }
        assert!(offenders.is_empty(), "{offenders:?}");
        for f in ["src/watch.rs", "src/sensor.rs"] {
            let src =
                std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(f))
                    .unwrap();
            for s in ["rearm", "launch::", "PtySession", "spawn::", ".write("] {
                assert!(!src.contains(s), "{f} reaches {s}");
            }
        }
    }

    /// DR-116: if the re-prepared plan differs from the confirmed one, nothing is spawned.
    #[test]
    fn stale_plan_not_spawned() {
        let r = rig(serde_json::json!({}));
        let sink = Arc::new(Collect::default());
        let old = start_old(&r, &sink);
        let confirmed = resume_plan(&r);
        let mut prepare = || {
            PrepareOutcome::Ready(Box::new(plan(
                &r,
                &[
                    "--resume",
                    SID,
                    "--model",
                    "sonnet",
                    "--permission-mode",
                    "acceptEdits",
                ],
                Some("Now 2.3, changed."),
            )))
        };
        let mut archive = || panic!("archived a stale plan");
        let mut relaunch =
            |_: &LaunchPlan| -> Result<PtySession, String> { panic!("spawned a stale plan") };
        let out = rr::rearm_start(
            UserAction::from_ipc_invoke(),
            Some(&old),
            IdleSource::Billy,
            &confirmed,
            &Ui {
                idle: true,
                ..Ui::default()
            },
            &mut Hooks {
                prepare_again: &mut prepare,
                archive: &mut archive,
                relaunch: &mut relaunch,
            },
        )
        .unwrap();
        let RearmOutcome::Stale(fresh) = out else {
            panic!("{out:?}")
        };
        assert!(matches!(*fresh, PrepareOutcome::Ready(_)));
        // A blocked re-prepare is stale too.
        let sink = Arc::new(Collect::default());
        let old = start_old(&r, &sink);
        let mut blocked = || PrepareOutcome::Blocked {
            blockers: vec![],
            warnings: vec![],
        };
        let out = rr::rearm_start(
            UserAction::from_ipc_invoke(),
            Some(&old),
            IdleSource::Sensor {
                idle: true,
                tasks: 0,
            },
            &confirmed,
            &Ui::default(),
            &mut Hooks {
                prepare_again: &mut blocked,
                archive: &mut archive,
                relaunch: &mut relaunch,
            },
        )
        .unwrap();
        assert!(matches!(out, RearmOutcome::Stale(_)));
    }
}
