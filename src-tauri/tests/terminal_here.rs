//! M4-25 / M4-33: `terminal_open_here` (AC-F014-11, AC-XC-28, SR-25a): Terminal opens by bundle ID
//! at the project root, through the OS seam, and an unknown project or a failing `open` is a typed
//! error.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::project::{ProjectApp, req};
use foreman_app::ipc::IpcErrorCode;
use foreman_app::ipc::commands::files;
use serde_json::json;

mod terminal_here {
    use super::*;

    /// The project root is what Terminal opens at, once per request, and nothing else is typed.
    #[test]
    fn opens_terminal_at_the_project_root() {
        let p = ProjectApp::new(json!({}));
        files::terminal_open_here(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        assert_eq!(p.t.os.terminals(), std::slice::from_ref(&p.project));
    }

    #[test]
    fn unknown_project_is_not_found() {
        let p = ProjectApp::new(json!({}));
        let e = files::terminal_open_here(
            &p.t.app,
            req(json!({ "projectId": "9b5c1e4a-73f6-4d0a-9a2c-4e83b2d6f093" })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotFound);
        assert!(p.t.os.terminals().is_empty());
    }

    #[test]
    fn a_failing_open_is_tool_failed() {
        let p = ProjectApp::new(json!({}));
        p.t.os.fail_opens(true);
        let e = files::terminal_open_here(&p.t.app, req(json!({ "projectId": p.id }))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::ToolFailed);
    }
}
