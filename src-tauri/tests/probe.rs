//! M3-18, M4-12 (F-019, ADR-017 as amended, SR-17, NFR-44; T-036, T-037): the probe-folder trust
//! tab, `probe_run`, and the contract probes before a launch on a version change, against
//! fake-claude (which parses `--init-only` like Claude Code 2.1.304 and fires the `SessionStart`
//! marker hook through the real sensor).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::project::{ProjectApp, channel, collect, req};
use common::sensor as rig;
use foreman_app::ipc::commands::{doctor, launch};
use foreman_app::ipc::dto::doctor::ProbeResult;
use foreman_app::ipc::dto::launch::{LaunchProgress, PrepareOutcome};
use foreman_app::ipc::error::IpcErrorCode;
use foreman_app::ipc::types::Blocker;
use serde_json::{Value, json};

fn calls(p: &ProjectApp) -> Vec<Value> {
    std::fs::read_to_string(format!("{}.calls.jsonl", p.record.display()))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn init_only(p: &ProjectApp) -> Vec<Value> {
    calls(p)
        .into_iter()
        .filter(|c| {
            c["argv"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == "--init-only")
        })
        .collect()
}

/// Replaces fake-claude's script (keeps its record file).
fn script(p: &ProjectApp, mut v: Value) {
    v["record"] = json!(p.record);
    std::fs::write(p.root.join("bin/fake.json"), v.to_string()).unwrap();
}

fn probe_dir(p: &ProjectApp) -> PathBuf {
    std::fs::canonicalize(p.t.app_data()).unwrap().join("probe")
}

/// The sensor, and the first-run trust step done (probes before launches are on).
fn probing(v: Value) -> ProjectApp {
    let p = ProjectApp::new(v);
    rig::enable(&p.t.app);
    let (mut s, stamp, _) = p.t.app.settings().unwrap();
    s.probe_folder_trusted = true;
    p.t.app.save_settings(&s, &stamp).unwrap();
    p
}

fn progress() -> (tauri::ipc::Channel<LaunchProgress>, Arc<Mutex<Vec<Value>>>) {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let b = buf.clone();
    let ch = tauri::ipc::Channel::new(move |body| {
        if let tauri::ipc::InvokeResponseBody::Json(j) = body {
            b.lock().unwrap().push(serde_json::from_str(&j).unwrap());
        }
        Ok(())
    });
    (ch, buf)
}

fn prepare(p: &ProjectApp, step: &str) -> PrepareOutcome {
    p.prepare(step)
}

fn contract(p: &ProjectApp) -> Option<foreman_core::schema::settings::Contract> {
    p.t.app.settings().unwrap().0.contract
}

mod probe {
    use super::*;

    /// AC-F019-08 (M3-18, trust-tab case) and AC-F019-17 (M4-12): the trust tab runs interactive
    /// `claude` with no prompt in the fixed `<app-data>/probe/`; `probe_run` runs every probe
    /// there with `--init-only` only, never in a project folder.
    #[test]
    fn runs_only_in_probe_dir() {
        let p = ProjectApp::new(json!({}));
        rig::enable(&p.t.app);
        assert!(!p.t.app.settings().unwrap().0.probe_folder_trusted);
        let (out, buf) = collect();
        let r = doctor::probe_folder_open(&p.t.app, req(json!({})), out).unwrap();
        assert_eq!(r.tab_name, "fm-probe");
        let deadline = Instant::now() + Duration::from_secs(20);
        while !String::from_utf8_lossy(&buf.lock().unwrap()).contains("FAKE-CLAUDE READY") {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        let tab = calls(&p).pop().unwrap();
        assert_eq!(Path::new(tab["cwd"].as_str().unwrap()), probe_dir(&p));
        let argv: Vec<&str> = tab["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();
        assert_eq!(&argv[1..], ["--name", "fm-probe"], "no prompt, no project");
        assert!(p.t.app.settings().unwrap().0.probe_folder_trusted);
        assert!(
            p.t.app.runs().get(r.run_id.as_str()).is_some(),
            "an ordinary terminal tab"
        );
        p.t.app.runs().hangup_all();
        // The contract probes.
        let res = doctor::probe_run(&p.t.app, req(json!({})), channel()).unwrap();
        assert_eq!(res.outcomes.len(), 2);
        assert!(
            res.outcomes.iter().all(|o| o.result == ProbeResult::Pass),
            "{res:?}"
        );
        let probes = init_only(&p);
        assert_eq!(probes.len(), 3, "A, B1 and B2");
        for c in &probes {
            assert_eq!(Path::new(c["cwd"].as_str().unwrap()), probe_dir(&p));
            assert_eq!(c["argv"][1], "--init-only");
            assert!(
                !c["cwd"]
                    .as_str()
                    .unwrap()
                    .starts_with(p.project.to_str().unwrap())
            );
        }
        assert_eq!(contract(&p).unwrap().version, "2.1.293");
    }

    /// AC-F019-17, SR-17: `<app-data>/probe/` is emptied and set to `0700` before each probe run;
    /// anything planted there (a `.claude/` with an API key helper, a `CLAUDE.md`, a symlink out)
    /// is removed without following links.
    #[test]
    fn dir_reset_before_run() {
        let p = ProjectApp::new(json!({}));
        rig::enable(&p.t.app);
        let dir = p.t.app_data().join("probe");
        std::fs::create_dir_all(dir.join(".claude")).unwrap();
        std::fs::write(dir.join(".claude/settings.json"), r#"{"apiKeyHelper":"x"}"#).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "planted").unwrap();
        let outside = p.root.join("outside.txt");
        std::fs::write(&outside, "keep").unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("link")).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        doctor::probe_run(&p.t.app, req(json!({})), channel()).unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["settings.json"]);
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep");
        // The probe's marker events don't outlive the run.
        let leftover: Vec<_> = std::fs::read_dir(p.t.app_data().join("events"))
            .map(|r| r.collect())
            .unwrap_or_default();
        assert!(leftover.is_empty(), "{leftover:?}");
    }

    /// AC-F019-15, AC-F008-21: the first launch after `claude -v` changes runs the probes first
    /// (progress `contract` before `fingerprint`); the next launch on the same version doesn't.
    #[test]
    fn runs_on_version_change() {
        let p = probing(json!({}));
        let (ch, buf) = progress();
        let out = launch::launch_prepare(
            &p.t.app,
            req(
                json!({ "projectId": p.id, "step": "2.2", "entry": "stageView", "mode": "normal" }),
            ),
            ch,
        )
        .unwrap();
        let _ = out;
        assert_eq!(init_only(&p).len(), 3, "probes ran before the first launch");
        let phases: Vec<String> = buf
            .lock()
            .unwrap()
            .iter()
            .map(|v| v["phase"].as_str().unwrap().to_owned())
            .collect();
        let at = |ph: &str| phases.iter().position(|x| x == ph).unwrap();
        assert!(
            at("version") < at("contract") && at("contract") < at("fingerprint"),
            "{phases:?}"
        );
        let c = buf
            .lock()
            .unwrap()
            .iter()
            .find(|v| v["phase"] == "contract")
            .cloned()
            .unwrap();
        assert_eq!(c["outcome"], "pass");
        assert_eq!(contract(&p).unwrap().version, "2.1.293");
        // Same version: no probes.
        let _ = prepare(&p, "2.2");
        assert_eq!(init_only(&p).len(), 3);
        // A new version: probes again, and the contract moves.
        script(&p, json!({ "version": "2.1.305 (Claude Code)" }));
        let _ = prepare(&p, "2.2");
        assert_eq!(init_only(&p).len(), 6);
        assert_eq!(contract(&p).unwrap().version, "2.1.305");
    }

    /// AC-F019-16, SR-17: an inconclusive probe (no marker: hooks didn't fire, as with a trust
    /// prompt) keeps the last verified state, warns, and the launch proceeds.
    #[test]
    fn inconclusive_keeps_state() {
        let p = probing(json!({}));
        let _ = prepare(&p, "2.2");
        let verified = contract(&p).unwrap();
        assert_eq!(verified.version, "2.1.293");
        script(
            &p,
            json!({ "version": "2.1.305 (Claude Code)", "initOnlyHooks": false }),
        );
        let PrepareOutcome::Ready(pv) = prepare(&p, "2.2") else {
            panic!("an inconclusive probe never blocks")
        };
        assert!(
            pv.warnings.iter().any(|w| w.contains("inconclusive")),
            "{:?}",
            pv.warnings
        );
        let after = contract(&p).unwrap();
        assert_eq!(after.version, "2.1.293", "the verified version stays");
        assert_eq!(after.outcomes, verified.outcomes);
        // Doctor shows the probe rows with the kept state.
        let r = doctor::doctor_run(&p.t.app, req(json!({ "scope": "full" }))).unwrap();
        assert_eq!(r.probes.len(), 2);
        assert!(r.probe_note.unwrap().contains("2.1.293"));
        // A run with no sensor at all is inconclusive too, never a pass.
        p.t.app.sensor().set_bundle(None);
        let res = doctor::probe_run(&p.t.app, req(json!({})), channel()).unwrap();
        assert_eq!(res.outcomes[0].result, ProbeResult::Inconclusive);
    }

    /// AC-F019-16, SR-17 (T-036): a flag failure disables only the dependent feature and names
    /// it. `--effort` failing leaves launches running (with a warning); `--name` failing blocks
    /// launches with `contractFailed`.
    #[test]
    fn failure_disables_feature() {
        let p = probing(json!({
            "initOnlyStderr": "Warning: Unknown --effort value 'high' — ignoring it and using the default effort. Valid values: low, medium, high, xhigh, max."
        }));
        let PrepareOutcome::Ready(pv) = prepare(&p, "2.2") else {
            panic!("an effort failure doesn't block")
        };
        assert!(
            pv.warnings.iter().any(|w| w.contains("--effort")),
            "{:?}",
            pv.warnings
        );
        let c = contract(&p).unwrap();
        assert_eq!(c.outcomes.probe_a.flag.as_deref(), Some("--effort"));
        let r = doctor::doctor_run(&p.t.app, req(json!({ "scope": "full" }))).unwrap();
        assert_eq!(r.probes[0].result, ProbeResult::Fail);
        assert!(r.probes[0].detail.as_deref().unwrap().contains("effort"));
        // A new version that refuses --name: launches are blocked, naming the feature.
        script(
            &p,
            json!({ "version": "2.1.306 (Claude Code)", "initOnlyStderr": "error: unknown option '--name'", "initOnlyExit": 1 }),
        );
        let PrepareOutcome::Blocked { blockers, .. } = prepare(&p, "2.2") else {
            panic!("a launch flag failure blocks")
        };
        assert!(
            blockers.iter().any(|b| matches!(
                b,
                Blocker::ContractFailed { feature, .. } if feature == "launch"
            )),
            "{blockers:?}"
        );
    }
}

mod preflight {
    use super::*;

    /// SR-19, AC-F008-21: the first launch after a version change asks "Claude Code updated
    /// X → Y" in the native confirmation; Cancel starts nothing.
    #[test]
    fn confirm_required_for_version_change() {
        let p = probing(json!({}));
        // The last launch saw 2.1.293 (`lastSeenClaudeVersion`).
        let (mut st, stamp, _) = p.t.app.settings().unwrap();
        st.last_seen_claude_version = Some("2.1.293 (Claude Code)".into());
        p.t.app.save_settings(&st, &stamp).unwrap();
        script(&p, json!({ "version": "2.1.305 (Claude Code)" }));
        let out = prepare(&p, "2.2");
        let PrepareOutcome::Ready(pv) = out else {
            panic!("{out:?}")
        };
        let reasons = serde_json::to_string(&pv.confirm).unwrap();
        assert!(reasons.contains("2.1.305"), "{reasons}");
        let before = calls(&p).len();
        p.t.os.answer_next(false);
        let e = launch::launch_start(&p.t.app, req(json!({ "planId": pv.plan_id })), channel())
            .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert!(
            p.t.os.asked().last().unwrap().message.contains("2.1.305"),
            "{:?}",
            p.t.os.asked().last()
        );
        assert_eq!(calls(&p).len(), before, "nothing spawned");
    }
}
