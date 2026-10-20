//! M2-20: Today's rows and Do next (AC-F017-01 to AC-F017-05, AC-F004-04). Each project's row
//! carries its next step, or says why there is none; nothing runnable is offered for a missing
//! folder.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::project::{ProjectApp, git_identity, req};
use foreman_app::ipc::commands::{app, projects};
use serde_json::{Value, json};

fn today(p: &ProjectApp) -> Value {
    serde_json::to_value(app::today_get(&p.t.app, req(json!({}))).unwrap()).unwrap()
}

mod today {
    use super::*;

    /// AC-F017-01: the row shows the first runnable step on the applicable path with its title and
    /// stage, and Do next picks the same step without launching anything.
    #[test]
    fn row_shows_the_next_step() {
        let p = ProjectApp::new(json!({}));
        let t = today(&p);
        let row = &t["rows"][0];
        assert_eq!(row["project"]["name"], json!("TradeLog"));
        assert_eq!(row["next"]["key"], json!("1.1"), "{row}");
        assert!(row["whyNone"].is_null());
        assert!(row["stageTitle"].is_string());
        assert_eq!(
            row["strip"].as_array().unwrap().len(),
            19,
            "one cell per stage"
        );
        let next = serde_json::to_value(app::do_next(&p.t.app, req(json!({}))).unwrap()).unwrap();
        assert_eq!(next["step"], json!("1.1"), "{next}");
        assert!(p.run_files().is_empty(), "nothing launched");
    }

    /// AC-F017-03: with no project there are no rows and Do next says to start one.
    #[test]
    fn no_projects_no_rows() {
        let p = ProjectApp::new(json!({}));
        p.t.os.answer_next(true);
        projects::project_remove(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        let t = today(&p);
        assert!(t["rows"].as_array().unwrap().is_empty());
        let next = serde_json::to_value(app::do_next(&p.t.app, req(json!({}))).unwrap()).unwrap();
        assert_eq!(next["kind"], json!("nothing"));
    }

    /// AC-F017-04 / AC-F004-04: a project whose next step waits on a decision says
    /// "Decide <condition> to continue" and offers nothing runnable.
    #[test]
    fn undecided_condition_says_what_to_decide() {
        let p = ProjectApp::new(json!({}));
        git_identity(&p);
        projects::condition_set(
            &p.t.app,
            req(json!({
                "projectId": p.id, "condition": "idea_known", "to": "undecided", "note": "not sure"
            })),
        )
        .unwrap();
        let t = today(&p);
        let row = &t["rows"][0];
        assert!(row["next"].is_null(), "{row}");
        assert_eq!(
            row["whyNone"],
            json!("Decide idea_known to continue"),
            "{row}"
        );
        let next = serde_json::to_value(app::do_next(&p.t.app, req(json!({}))).unwrap()).unwrap();
        assert_eq!(next["kind"], json!("nothing"));
        assert_eq!(next["why"], json!("Decide idea_known to continue"));
    }

    /// AC-F017-05: a registered project whose folder is gone shows "Project folder not found" and
    /// offers nothing runnable.
    #[test]
    fn missing_folder_offers_nothing() {
        let p = ProjectApp::new(json!({}));
        let gone = p.root.join("work/moved-away");
        std::fs::rename(&p.project, &gone).unwrap();
        let t = today(&p);
        let row = &t["rows"][0];
        assert!(row["next"].is_null(), "{row}");
        assert!(
            row["whyNone"]
                .as_str()
                .unwrap()
                .contains("Project folder not found"),
            "{row}"
        );
        assert_eq!(row["project"]["readOnly"]["reason"], json!("folderMissing"));
        let next = serde_json::to_value(app::do_next(&p.t.app, req(json!({}))).unwrap()).unwrap();
        assert_eq!(next["kind"], json!("nothing"));
    }
}
