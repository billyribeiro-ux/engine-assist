//! M2-15 (F-019; AC-F019-04 to AC-F019-07; SR-01, SR-08; T-036): the P1 Doctor rows. A missing,
//! old or logged-out Claude Code and a missing git each fail with what was found and how to fix
//! it; project creation stays possible; only the two subscription methods pass; git has no floor;
//! the run reports its level through `doctor:status`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::project::{ProjectApp, req};
use foreman_app::ipc::commands::{doctor as cmd, projects};
use foreman_app::ipc::dto::doctor::{DoctorCheck, DoctorReport};
use foreman_app::ipc::types::DoctorLevel;
use foreman_core::schema::settings::ToolSource;
use serde_json::{Value, json};

fn p1(p: &ProjectApp) -> DoctorReport {
    cmd::doctor_run(&p.t.app, req(json!({ "scope": "p1" }))).unwrap()
}

fn row<'a>(r: &'a DoctorReport, id: &str) -> &'a DoctorCheck {
    r.checks
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("no {id} row"))
}

fn detail(c: &DoctorCheck) -> String {
    serde_json::to_string(&c.detail).unwrap()
}

fn set_path(p: &ProjectApp, tool: &str, path: &str) {
    let (mut s, stamp, _) = p.t.app.settings().unwrap();
    let t = if tool == "claude" {
        &mut s.tools.claude
    } else {
        &mut s.tools.git
    };
    t.path = Some(path.to_owned());
    t.source = ToolSource::Settings;
    p.t.app.save_settings(&s, &stamp).unwrap();
}

mod doctor {
    use super::*;

    /// AC-F019-04: below the floor shows the version found and the fix; a missing `claude` says
    /// so and project creation still works; a logged-out or wrong-method sign-in fails with the
    /// method found; a missing git fails with its fix.
    #[test]
    fn failures_name_what_was_found_and_the_fix() {
        // Below the floor.
        let old = ProjectApp::new(json!({ "version": "2.1.280 (Claude Code)" }));
        let r = p1(&old);
        let v = row(&r, "version");
        assert_eq!(v.level, DoctorLevel::Failing);
        assert!(v.found.as_deref().unwrap().contains("2.1.280"), "{v:?}");
        assert!(detail(v).contains("claude update"), "{}", detail(v));
        assert!(detail(v).contains("2.1.293"), "{}", detail(v));
        assert_eq!(
            row(&r, "auth").level,
            DoctorLevel::Unknown,
            "needs claude first"
        );
        assert_eq!(r.level, DoctorLevel::Failing);

        // Logged out: the answer names no method.
        let out = ProjectApp::new(json!({ "auth": { "loggedIn": false } }));
        let a = p1(&out);
        let a = row(&a, "auth");
        assert_eq!(a.level, DoctorLevel::Failing, "{a:?}");
        assert!(detail(a).contains("/login"), "{}", detail(a));

        // An API key: found, and refused.
        let key = ProjectApp::new(json!({ "auth": { "loggedIn": true, "authMethod": "api_key" } }));
        let a = p1(&key);
        let a = row(&a, "auth");
        assert_eq!(a.level, DoctorLevel::Failing);
        assert_eq!(a.found.as_deref(), Some("api_key"));
        assert!(detail(a).contains("api_key"), "{}", detail(a));

        // git missing: its fix, and setup is blocked with the reason (AC-F018-05).
        let nogit = ProjectApp::new(json!({}));
        set_path(
            &nogit,
            "git",
            &nogit.root.join("nowhere/git").display().to_string(),
        );
        let r = p1(&nogit);
        let g = row(&r, "git");
        assert_eq!(g.level, DoctorLevel::Failing);
        assert!(detail(g).contains("xcode-select"), "{}", detail(g));

        // claude missing: the row says so, and creating a project stays possible.
        let none = ProjectApp::new(json!({}));
        set_path(
            &none,
            "claude",
            &none.root.join("nowhere/claude").display().to_string(),
        );
        let r = p1(&none);
        let c = row(&r, "path");
        assert_eq!(c.level, DoctorLevel::Failing);
        assert!(detail(c).contains("Settings"), "{}", detail(c));
        let folder = none.root.join("work/after-doctor");
        std::fs::create_dir_all(&folder).unwrap();
        none.t
            .os
            .pick_next(Some(std::fs::canonicalize(&folder).unwrap()));
        let pick = projects::setup_pick_folder(&none.t.app, req(json!({}))).unwrap();
        let pv = projects::setup_preview(
            &none.t.app,
            req(json!({
                "pickId": pick.pick_id, "name": "After", "slug": "after", "variables": {},
                "conditions": {}, "chatProjectUrl": null
            })),
        )
        .unwrap();
        projects::setup_apply(&none.t.app, req(json!({ "previewId": pv.preview_id }))).unwrap();
        assert!(folder.join(".foreman/project.json").is_file());
    }

    /// AC-F019-05: a fake `claude` reporting each method; Doctor passes only `claude.ai` and
    /// `oauth_token`. A non-zero exit never passes.
    #[test]
    fn passes_only_subscription_auth_methods() {
        for (method, ok) in [
            ("claude.ai", true),
            ("oauth_token", true),
            ("api_key", false),
            ("bedrock", false),
            ("vertex", false),
            ("something-new", false),
        ] {
            let p = ProjectApp::new(json!({ "auth": { "loggedIn": true, "authMethod": method } }));
            let r = p1(&p);
            let a = row(&r, "auth");
            assert_eq!(a.level == DoctorLevel::Ok, ok, "{method}: {a:?}");
            assert_eq!(a.found.as_deref(), Some(method));
        }
        let p = ProjectApp::new(
            json!({ "auth": { "loggedIn": true, "authMethod": "claude.ai" }, "authExit": 1 }),
        );
        assert_eq!(row(&p1(&p), "auth").level, DoctorLevel::Failing);
    }

    /// AC-F019-06: git's version is shown whatever it is; no floor is enforced.
    #[test]
    fn git_version_is_shown_without_a_floor() {
        let p = ProjectApp::new(json!({}));
        let r = p1(&p);
        let g = row(&r, "git");
        assert_eq!(g.level, DoctorLevel::Ok, "{g:?}");
        assert!(
            g.found
                .as_deref()
                .unwrap()
                .to_lowercase()
                .contains("git version"),
            "{g:?}"
        );
        assert!(detail(g).contains("No version floor"), "{}", detail(g));
    }

    /// AC-F019-07 (int): a Doctor run reports its level through `doctor:status`, and the next
    /// bootstrap carries it for the sidebar's dot.
    #[test]
    fn run_reports_its_level() {
        let p = ProjectApp::new(json!({}));
        let r = p1(&p);
        let ev: Vec<Value> =
            p.t.os
                .events()
                .iter()
                .filter(|e| e.name() == "doctor:status")
                .map(|e| serde_json::to_value(e.payload()).unwrap())
                .collect();
        assert_eq!(
            ev.last().unwrap()["level"],
            serde_json::to_value(r.level).unwrap()
        );
        let boot = serde_json::to_value(
            foreman_app::ipc::commands::app::app_bootstrap(&p.t.app, req(json!({}))).unwrap(),
        )
        .unwrap();
        assert_eq!(boot["doctor"], serde_json::to_value(r.level).unwrap());
    }
}
