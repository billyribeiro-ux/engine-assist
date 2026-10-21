//! M2-27 (NFR-01, NFR-02): the commands behind cold start and a project switch log how long they
//! took, in names and numbers only, so the run log can report them (targets are enforced in M5-09).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::project::{ProjectApp, req};
use foreman_app::ipc::commands::{app, projects};
use foreman_app::ipc::run_caught;
use serde_json::json;

mod perf {
    use super::*;

    /// NFR-01 / NFR-02: `app_bootstrap` logs `ms` and `since_start_ms`; `project_open` logs `ms`.
    #[test]
    fn startup_and_project_switch_are_timed_in_the_run_log() {
        foreman_app::logging::mark_process_start();
        foreman_app::logging::install_subscriber();
        let p = ProjectApp::new(json!({}));
        let app_data = p.t.app_data();
        foreman_app::logging::attach(&app_data).unwrap();
        run_caught(&p.t.app, req(json!({})), app::app_bootstrap).unwrap();
        run_caught(
            &p.t.app,
            req(json!({ "projectId": p.id })),
            projects::project_open,
        )
        .unwrap();
        foreman_app::logging::detach();
        let log = std::fs::read_to_string(app_data.join("logs/foreman.log")).unwrap();
        let line = |needle: &str| {
            log.lines()
                .find(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("no {needle:?} line in the log:\n{log}"))
                .to_owned()
        };
        let number = |l: &str, key: &str| -> u64 {
            l.split(' ')
                .find_map(|w| w.strip_prefix(&format!("{key}=")))
                .unwrap_or_else(|| panic!("no {key} in {l}"))
                .parse()
                .unwrap()
        };
        let boot = line("perf: startup");
        assert!(boot.contains("command=app_bootstrap"), "{boot}");
        assert!(number(&boot, "ms") < 5_000, "{boot}");
        assert!(
            number(&boot, "since_start_ms") >= number(&boot, "ms"),
            "{boot}"
        );
        let open = line("perf: project switch");
        assert!(open.contains("command=project_open"), "{open}");
        assert!(number(&open, "ms") < 5_000, "{open}");
    }
}
