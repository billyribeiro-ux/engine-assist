//! M4-13, M4-14 (F-019; SR-01, SR-02, SR-10; T-015, T-019, T-027, T-038): the full Doctor and the
//! consented login-shell PATH capture, against a real project, fake tools at the fixed candidate
//! paths under the test home, and the real `/bin/bash` for the one shell.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::path::{Path, PathBuf};

use common::project::{ProjectApp, fake_claude, req};
use foreman_app::ipc::commands::doctor as cmd;
use foreman_app::ipc::dto::doctor::{CheckAction, DoctorCheck, DoctorReport};
use foreman_app::ipc::error::IpcErrorCode;
use foreman_app::ipc::types::DoctorLevel;
use serde_json::{Value, json};

fn full(p: &ProjectApp) -> DoctorReport {
    cmd::doctor_run(&p.t.app, req(json!({ "scope": "full" }))).unwrap()
}

fn row<'a>(r: &'a DoctorReport, id: &str) -> &'a DoctorCheck {
    r.checks.iter().find(|c| c.id == id).unwrap_or_else(|| {
        panic!(
            "no {id} row: {:?}",
            r.checks.iter().map(|c| &c.id).collect::<Vec<_>>()
        )
    })
}

fn text(c: &DoctorCheck) -> String {
    serde_json::to_string(&c.detail).unwrap()
}

/// fake-claude copied to `<home>/<rel>` with its own record file (it answers `--version`).
fn fake_tool(home: &Path, rel: &str) -> PathBuf {
    let exe = home.join(rel);
    let dir = exe.parent().unwrap();
    std::fs::create_dir_all(dir).unwrap();
    std::fs::copy(fake_claude(), &exe).unwrap();
    let record = dir.join("rec.json");
    std::fs::write(
        dir.join("fake.json"),
        json!({ "record": record, "version": "9.9.9 (fake)" }).to_string(),
    )
    .unwrap();
    record
}

fn bash_listed() -> bool {
    std::fs::read_to_string("/etc/shells").is_ok_and(|s| s.lines().any(|l| l.trim() == "/bin/bash"))
}

mod spawn {
    use super::*;

    /// AC-F019-11, SR-01 (T-038): the toolchain probes run from the fixed candidate paths with
    /// `--version` only, their cwd in app data (never a project), and their versions show.
    #[test]
    fn helper_cwd_is_app_data() {
        let p = ProjectApp::new(json!({}));
        let cargo = fake_tool(&p.t.home(), ".cargo/bin/cargo");
        let pnpm = fake_tool(&p.t.home(), "Library/pnpm/pnpm");
        let r = full(&p);
        for (id, rec) in [("cargo", &cargo), ("pnpm", &pnpm)] {
            let c = row(&r, id);
            assert_eq!(c.level, DoctorLevel::Ok, "{c:?}");
            assert_eq!(c.found.as_deref(), Some("9.9.9 (fake)"));
            let v: Value = serde_json::from_str(&std::fs::read_to_string(rec).unwrap()).unwrap();
            assert_eq!(v["argv"].as_array().unwrap().len(), 2, "{v}");
            assert_eq!(v["argv"][1], "--version");
            assert_eq!(
                Path::new(v["cwd"].as_str().unwrap()),
                std::fs::canonicalize(p.t.app_data()).unwrap()
            );
        }
        // Not found at a fixed path: a row that says where Foreman looks, never a PATH search.
        let node = row(&r, "node");
        if node.found.is_none() {
            assert!(
                text(node).contains("/opt/homebrew/bin/node"),
                "{}",
                text(node)
            );
        }
    }
}

mod doctor_rows {
    use super::*;

    /// AC-F019-09…-11, -14: every row of the full Doctor; user settings are read, never written
    /// (SR-10).
    #[test]
    fn full_doctor_rows() {
        let p = ProjectApp::new(json!({}));
        let claude = p.t.home().join(".claude");
        std::fs::create_dir_all(claude.join("agents")).unwrap();
        let user = json!({
            "cleanupPeriodDays": 20,
            "disableAllHooks": true,
            "permissions": { "allow": ["Bash(*)", "Bash(python3 *)", "Bash(cargo test *)", "Bash(npx *)"] }
        })
        .to_string();
        std::fs::write(claude.join("settings.json"), &user).unwrap();
        std::fs::write(
            claude.join("agents/explore.md"),
            "---\nname: Explore\n---\n",
        )
        .unwrap();
        let r = full(&p);
        for id in [
            "path",
            "version",
            "auth",
            "sensor",
            "probeFolder",
            "cleanup",
            "hooks",
            "allow",
            "appendixA",
            "git",
            "node",
            "pnpm",
            "cargo",
            "loginPath",
        ] {
            row(&r, id);
        }
        let cleanup = row(&r, "cleanup");
        assert_eq!(cleanup.level, DoctorLevel::Warning);
        assert_eq!(cleanup.found.as_deref(), Some("20 days"));
        assert!(
            text(cleanup).contains("90"),
            "a recommendation: {}",
            text(cleanup)
        );
        assert_eq!(row(&r, "hooks").level, DoctorLevel::Warning);
        let allow = row(&r, "allow");
        assert_eq!(allow.level, DoctorLevel::Warning);
        let t = text(allow);
        assert!(
            t.contains("Bash(*)") && t.contains("Bash(python3 *)") && t.contains("Bash(npx *)"),
            "{t}"
        );
        assert!(!t.contains("cargo test"), "{t}");
        let a = row(&r, "appendixA");
        assert_eq!(a.level, DoctorLevel::Warning);
        assert!(
            text(a).contains("~/.claude/agents/test-runner.md"),
            "{}",
            text(a)
        );
        assert!(!text(a).contains("explore.md"), "{}", text(a));
        assert_eq!(
            row(&r, "loginPath").action,
            Some(CheckAction::CaptureLoginPath)
        );
        // The PATH launches get is shown (AC-F019-14).
        assert!(row(&r, "launchPath").found.is_some());
        // No sensor in this build: said so, and probes are off with the reason.
        assert_eq!(row(&r, "sensor").level, DoctorLevel::Unknown);
        assert!(
            r.probe_note.as_deref().unwrap().contains("sensor"),
            "{:?}",
            r.probe_note
        );
        // Capability table: feature → minimum → found → what it blocks.
        assert_eq!(r.capabilities.len(), foreman_core::compat::TABLE.len());
        assert!(
            r.capabilities.iter().all(|c| c.found),
            "fake-claude 2.1.293 meets every floor"
        );
        // Read only.
        assert_eq!(
            std::fs::read_to_string(claude.join("settings.json")).unwrap(),
            user
        );
        // P1 scope stays small.
        let p1 = cmd::doctor_run(&p.t.app, req(json!({ "scope": "p1" }))).unwrap();
        assert!(
            p1.checks
                .iter()
                .all(|c| !matches!(c.id.as_str(), "cleanup" | "sensor" | "node"))
        );
    }

    /// AC-F019-09: a feature the found version lacks names the steps it blocks.
    #[test]
    fn capability_names_blocked_steps() {
        let p = ProjectApp::new(json!({ "version": "2.1.282 (Claude Code)" }));
        let r = full(&p);
        let sonnet = r
            .capabilities
            .iter()
            .find(|c| c.feature == "Sonnet 5.5")
            .unwrap();
        assert!(!sonnet.found);
        let fable = r
            .capabilities
            .iter()
            .find(|c| c.feature == "Fable 5.1")
            .unwrap();
        assert!(fable.found);
        let haiku = r
            .capabilities
            .iter()
            .find(|c| c.feature == "Haiku 5.5")
            .unwrap();
        assert!(!haiku.found);
        let blocks = r
            .capabilities
            .iter()
            .filter(|c| !c.found)
            .filter_map(|c| c.blocks.clone())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(blocks.contains("TradeLog"), "{blocks}");
        assert_eq!(row(&r, "version").level, DoctorLevel::Failing);
    }
}

mod doctor {
    use super::*;

    /// AC-F019-12, SR-02 (T-015, T-019): the one shell runs only after Billy consents in a native
    /// dialog, only if `$SHELL` is listed in `/etc/shells`, with the fixed argv; the output is
    /// shown and stored only after a second native confirmation.
    #[test]
    fn login_path_requires_consent() {
        let p = ProjectApp::new(json!({}));
        let home = p.t.home();
        // A login shell reads ~/.bash_profile: it leaves a marker when it runs.
        let marker = home.join("shell-ran");
        std::fs::write(
            home.join(".bash_profile"),
            format!(
                "echo ran >> '{}'\nexport PATH=\"/usr/bin:/bin:relative:{}:/does/not/exist\"\n",
                marker.display(),
                p.project.display()
            ),
        )
        .unwrap();
        // Not listed: refused before any dialog or spawn.
        let asked = p.t.os.asked().len();
        let e = cmd::capture_login_path_with(&p.t.app, Some("/tmp/evil-shell")).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        let e = cmd::capture_login_path_with(&p.t.app, None).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        assert_eq!(
            p.t.os.asked().len(),
            asked,
            "no dialog for an unlisted shell"
        );
        if !bash_listed() {
            eprintln!("/bin/bash isn't in /etc/shells here; the consent half needs it");
            return;
        }
        // Cancel in the native dialog: no shell starts.
        p.t.os.answer_next(false);
        let e = cmd::capture_login_path_with(&p.t.app, Some("/bin/bash")).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert_eq!(p.t.os.asked().len(), asked + 1);
        assert!(p.t.os.asked().last().unwrap().message.contains("-l -c"));
        assert!(!marker.exists(), "Cancel ran nothing");
        // Consent: it runs once; the result is shown with each entry's verdict; nothing stored.
        let pv = cmd::capture_login_path_with(&p.t.app, Some("/bin/bash")).unwrap();
        assert_eq!(std::fs::read_to_string(&marker).unwrap().lines().count(), 1);
        let kept: Vec<&str> = pv
            .entries
            .iter()
            .filter(|e| e.ok)
            .map(|e| e.entry.as_str())
            .collect();
        assert_eq!(kept, ["/usr/bin", "/bin"]);
        let dropped: Vec<&str> = pv
            .entries
            .iter()
            .filter(|e| !e.ok)
            .map(|e| e.entry.as_str())
            .collect();
        assert_eq!(dropped.len(), 3, "{:?}", pv.entries);
        assert!(p.t.app.settings().unwrap().0.login_path.is_none());
        // The second dialog: Cancel stores nothing; the preview is used up.
        p.t.os.answer_next(false);
        let e = cmd::doctor_login_path_apply(&p.t.app, req(json!({ "previewId": pv.preview_id })))
            .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        assert!(p.t.app.settings().unwrap().0.login_path.is_none());
        let e = cmd::doctor_login_path_apply(&p.t.app, req(json!({ "previewId": pv.preview_id })))
            .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Expired);
        // Again, and OK: stored, and shown in Settings and Doctor.
        let pv = cmd::capture_login_path_with(&p.t.app, Some("/bin/bash")).unwrap();
        let v = cmd::doctor_login_path_apply(&p.t.app, req(json!({ "previewId": pv.preview_id })))
            .unwrap();
        assert_eq!(v.login_path.unwrap().entries, ["/usr/bin", "/bin"]);
        let s = p.t.app.settings().unwrap().0.login_path.unwrap();
        assert_eq!(s.shell, "/bin/bash");
        assert!(text(row(&full(&p), "loginPath")).contains("/usr/bin:/bin"));
    }

    /// SR-02: the shell's output is data. A PATH full of shell syntax is split on `:` and checked,
    /// never run; nothing it names is executed.
    #[test]
    fn login_path_output_is_data() {
        let projects = vec![PathBuf::from("/work/p")];
        let (kept, checks) = cmd::parse_login_path(
            b"/usr/bin:$(touch /tmp/x):/work/p/bin:`id`:/usr/bin:\n",
            &projects,
            &mut |p| foreman_core::launch::env::DirFacts {
                is_dir: p == Path::new("/usr/bin") || p.starts_with("/work"),
                world_writable: false,
                canonical: None,
            },
        );
        assert_eq!(kept, ["/usr/bin"]);
        assert_eq!(checks.len(), 6);
        assert!(checks.iter().filter(|c| !c.ok).all(|c| c.why.is_some()));
    }
}
