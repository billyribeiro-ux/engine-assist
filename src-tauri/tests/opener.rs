//! M2-13: opening a repo file in Billy's editor and revealing it (AC-XC-28, AC-F029-07 backend,
//! SR-25a, T-044). One Rust function opens files: the path passes the read policy and the walk
//! (no symlink, regular file), the extension is `.md`, `.json` or `.txt`, and the command is
//! `/usr/bin/open -b <editor bundle id> <absolute path>`, never the default app.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::os::unix::fs::symlink;

use common::project::{ProjectApp, req};
use foreman_app::ipc::IpcErrorCode;
use foreman_app::ipc::commands::{files, settings};
use serde_json::json;

fn set_editor(p: &ProjectApp, bundle: &str) {
    let (_, _, _) = p.t.app.settings().unwrap();
    let v = settings::settings_get(&p.t.app, req(json!({}))).unwrap();
    let rev = serde_json::to_value(&v).unwrap()["rev"].clone();
    settings::settings_update(
        &p.t.app,
        req(json!({
            "expectRev": rev,
            "patch": {
                "driName": null, "editor": { "kind": "set", "bundleId": bundle }, "theme": null,
                "terminal": null, "envPassthrough": null, "globalVariables": null,
                "resetTool": null, "clearLoginPath": null, "removeAllowlistEntry": null,
                "screenReaderMode": null, "osDisplay": null
            }
        })),
    )
    .unwrap();
}

fn open(p: &ProjectApp, path: &str) -> Result<(), foreman_app::ipc::IpcError> {
    files::file_open(&p.t.app, req(json!({ "projectId": p.id, "path": path })))
}

mod opener {
    use super::*;

    /// AC-XC-28: `-b <editor bundle id> <absolute path>`, exactly; Reveal is `-R <absolute path>`;
    /// with no editor set the answer is `NotAllowed("no-editor-set")` and nothing opens.
    #[test]
    fn uses_editor_bundle_id() {
        let p = ProjectApp::new(json!({}));
        std::fs::create_dir_all(p.project.join("docs")).unwrap();
        std::fs::write(p.project.join("docs/notes.md"), "# notes\n").unwrap();
        std::fs::write(p.project.join("docs/data.json"), "{}\n").unwrap();
        std::fs::write(p.project.join("docs/plain.TXT"), "x\n").unwrap();

        // No editor yet: disabled with a reason, nothing opens (AC-F029-07).
        let e = open(&p, "docs/notes.md").unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
        assert!(p.t.os.open_requests().is_empty());

        set_editor(&p, "com.microsoft.VSCode");
        let root = std::fs::canonicalize(&p.project).unwrap();
        open(&p, "docs/notes.md").unwrap();
        open(&p, "docs/data.json").unwrap();
        open(&p, "docs/plain.TXT").unwrap();
        let abs = |f: &str| root.join(f).display().to_string();
        assert_eq!(
            p.t.os.open_requests(),
            vec![
                vec![
                    "-b".to_owned(),
                    "com.microsoft.VSCode".to_owned(),
                    abs("docs/notes.md")
                ],
                vec![
                    "-b".to_owned(),
                    "com.microsoft.VSCode".to_owned(),
                    abs("docs/data.json")
                ],
                vec![
                    "-b".to_owned(),
                    "com.microsoft.VSCode".to_owned(),
                    abs("docs/plain.TXT")
                ],
            ]
        );

        // Reveal: `-R <absolute path>`; the editor isn't involved.
        files::file_reveal(
            &p.t.app,
            req(json!({ "projectId": p.id, "path": "docs/notes.md" })),
        )
        .unwrap();
        assert_eq!(
            p.t.os.open_requests().last().unwrap(),
            &vec!["-R".to_owned(), abs("docs/notes.md")]
        );

        // A failing `open` is a typed error.
        p.t.os.fail_opens(true);
        let e = open(&p, "docs/notes.md").unwrap_err();
        assert_eq!(e.code, IpcErrorCode::ToolFailed);
    }

    /// AC-XC-28, T-044: executable and application types never open; a `.md` symlink to an `.app`
    /// (or anywhere) is refused; nothing outside the read policy opens; nothing reaches `open`.
    #[test]
    fn open_file_refuses_executable_types() {
        let p = ProjectApp::new(json!({}));
        set_editor(&p, "com.microsoft.VSCode");
        let docs = p.project.join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        for name in [
            "run.sh",
            "run.command",
            "tool.py",
            "page.html",
            "x.app",
            "x.pkg",
            "x.dmg",
            "x.scpt",
            "x.workflow",
            "x.js",
            "x.exe",
            "x.webloc",
            "x.inetloc",
            "x.terminal",
            "x.plist",
            "x.md.sh",
            "noext",
            ".md",
        ] {
            std::fs::write(docs.join(name), "x\n").unwrap();
            let e = open(&p, &format!("docs/{name}")).unwrap_err();
            assert_eq!(e.code, IpcErrorCode::PolicyViolation, "{name}");
        }
        // An application bundle folder named like a document.
        std::fs::create_dir_all(docs.join("Evil.app/Contents/MacOS")).unwrap();
        std::fs::write(docs.join("Evil.app/Contents/MacOS/evil"), "#!/bin/sh\n").unwrap();
        assert!(open(&p, "docs/Evil.app/Contents/MacOS/evil").is_err());
        // A `.md` that is a symlink to an `.app`, to an executable, and to a file elsewhere.
        symlink(docs.join("Evil.app"), docs.join("link.md")).unwrap();
        symlink(docs.join("run.sh"), docs.join("exec.md")).unwrap();
        let outside = p.t.dir.path().join("secret.md");
        std::fs::write(&outside, "secret\n").unwrap();
        symlink(&outside, docs.join("out.md")).unwrap();
        for name in ["link.md", "exec.md", "out.md"] {
            let e = open(&p, &format!("docs/{name}")).unwrap_err();
            assert_eq!(e.code, IpcErrorCode::PolicyViolation, "{name}");
        }
        // A symlinked folder on the way, a path that climbs out, absolute paths, secrets.
        symlink(p.t.dir.path(), docs.join("up")).unwrap();
        for path in [
            "docs/up/secret.md",
            "../secret.md",
            "docs/../../secret.md",
            "/etc/hosts.txt",
            ".env.txt",
            "docs/id_rsa.md",
        ] {
            assert!(open(&p, path).is_err(), "{path}");
        }
        // Reveal goes through the same checks.
        let e = files::file_reveal(
            &p.t.app,
            req(json!({ "projectId": p.id, "path": "docs/link.md" })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::PolicyViolation);
        assert!(
            p.t.os.open_requests().is_empty(),
            "nothing reached /usr/bin/open: {:?}",
            p.t.os.open_requests()
        );
        // The positive control: the same folder's plain `.md` opens.
        std::fs::write(docs.join("ok.md"), "# ok\n").unwrap();
        open(&p, "docs/ok.md").unwrap();
        assert_eq!(p.t.os.open_requests().len(), 1);
    }
}
