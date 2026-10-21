//! M2-23: redacted config snapshots end to end (SR-55, AC-F008-23, T-070). A secret planted in
//! the user's Claude settings reaches neither the diff the webview is shown, nor the snapshot kept
//! in app data, nor any other file Foreman writes; the snapshot files are `0600`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use common::project::{ProjectApp, channel, req};
use foreman_app::ipc::commands::launch;
use foreman_app::ipc::dto::launch::PrepareOutcome;
use serde_json::json;

/// The planted values, in two generations.
fn secrets(gen_: &str) -> [String; 5] {
    [
        format!("hunter2-env-value-{gen_}"),
        format!("Bearer abc123-header-value-{gen_}"),
        format!("ghp_0123456789abcdefghijklmnopqrstuvwxyz{gen_}"),
        format!("my-password-value-{gen_}"),
        format!("extra-key-value-{gen_}"),
    ]
}

fn settings_with(gen_: &str, model: &str) -> String {
    let s = secrets(gen_);
    json!({
        "env": { "MY_API_THING": s[0] },
        "apiKeyExtra": s[4],
        "model": model,
        "permissions": { "allow": ["Bash(ls)"] },
        "mcp": { "headers": { "Authorization": s[1] }, "note": s[2] },
        "db": { "password": s[3] }
    })
    .to_string()
}

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let md = std::fs::symlink_metadata(&p).unwrap();
        if md.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

mod preflight {
    use super::*;

    fn prepare_raw(p: &ProjectApp) -> PrepareOutcome {
        launch::launch_prepare(
            &p.t.app,
            req(
                json!({ "projectId": p.id, "step": "2.2", "entry": "stageView", "mode": "normal" }),
            ),
            channel(),
        )
        .unwrap()
    }

    /// A change to the user's settings shows a redacted diff that needs one acknowledgement; the
    /// diff, the stored snapshot and every file in app data hold markers, never the values; the
    /// snapshot files are `0600`.
    #[test]
    fn snapshot_files_are_redacted_and_private() {
        let p = ProjectApp::new(json!({}));
        let claude = p.t.home().join(".claude");
        std::fs::create_dir_all(&claude).unwrap();
        let file = claude.join("settings.json");

        // Baseline: the first scan is acknowledged (as Billy would), which stores the snapshot.
        std::fs::write(&file, settings_with("one", "opus")).unwrap();
        assert!(matches!(p.prepare("2.2"), PrepareOutcome::Ready(_)));

        // The secrets change, and so does a plain value.
        std::fs::write(&file, settings_with("two", "sonnet")).unwrap();
        let PrepareOutcome::Blocked { blockers, .. } = prepare_raw(&p) else {
            panic!("not blocked after a settings change");
        };
        let shown = serde_json::to_string(&blockers).unwrap();
        assert!(shown.contains("configChanged"), "{shown}");
        for s in secrets("one").iter().chain(secrets("two").iter()) {
            assert!(
                !shown.contains(s.as_str()),
                "{s} reached the webview: {shown}"
            );
        }
        assert!(shown.contains("‹redacted sha256:"), "{shown}");
        assert!(shown.contains("env.MY_API_THING"), "{shown}");
        assert!(
            shown.contains("\"valueChanged\"") || shown.contains("valueChanged"),
            "{shown}"
        );
        // The plain change is readable.
        assert!(
            shown.contains("opus") && shown.contains("sonnet"),
            "{shown}"
        );

        // Acknowledge, which stores the new snapshot.
        assert!(matches!(p.prepare("2.2"), PrepareOutcome::Ready(_)));

        let mut files = Vec::new();
        walk(&p.t.app_data(), &mut files);
        assert!(!files.is_empty());
        let mut markers = 0;
        for f in &files {
            let bytes = std::fs::read(f).unwrap();
            let text = String::from_utf8_lossy(&bytes);
            for s in secrets("one").iter().chain(secrets("two").iter()) {
                assert!(
                    !text.contains(s.as_str()),
                    "{s} leaked into {}",
                    f.display()
                );
            }
            if text.contains("‹redacted sha256:") {
                markers += 1;
                assert_eq!(
                    std::fs::metadata(f).unwrap().permissions().mode() & 0o777,
                    0o600,
                    "{}",
                    f.display()
                );
            }
        }
        assert!(markers >= 1, "no snapshot file holds a redaction marker");
    }
}
