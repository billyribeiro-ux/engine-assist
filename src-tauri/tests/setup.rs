//! M2-17: project setup (AC-F018-02 to AC-F018-07; T-041, T-051, SR-24, SR-44). The review lists
//! every write and the apply makes exactly those; existing files are never overwritten; git init
//! is the fixed invocation and a missing git blocks setup; unsafe folders are refused before any
//! write; an existing `.foreman/` is registered as it is, or read-only when its schema is newer.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs and spawn git (design doc §3.2)"
)]

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::project::{ProjectApp, req};
use foreman_app::ipc::commands::projects::{self, KIT_TEMPLATES};
use foreman_app::ipc::dto::app::SetupPreview;
use foreman_app::ipc::{IpcError, IpcErrorCode};
use foreman_core::schema::settings::ToolSource;
use serde_json::{Value, json};

/// Every file and folder under `dir` (relative, `/`-separated) except `.git/`, with each file's
/// bytes (a folder is `None`).
fn tree(dir: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Option<Vec<u8>>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let rel = e
                .path()
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if rel == ".git" {
                continue;
            }
            if e.file_type().unwrap().is_dir() {
                out.insert(rel, None);
                walk(base, &e.path(), out);
            } else {
                out.insert(rel, Some(std::fs::read(e.path()).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn git(dir: &Path, args: &[&str]) {
    let st = Command::new("/usr/bin/git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .status()
        .unwrap();
    assert!(st.success(), "git {args:?}");
}

fn new_folder(p: &ProjectApp, name: &str) -> PathBuf {
    let f = p.root.join("work").join(name);
    std::fs::create_dir_all(&f).unwrap();
    std::fs::canonicalize(f).unwrap()
}

/// Picks `folder` through the scripted dialog and previews setup for it.
fn preview(p: &ProjectApp, folder: &Path, name: &str) -> Result<SetupPreview, IpcError> {
    p.t.os.pick_next(Some(folder.to_path_buf()));
    let pick = projects::setup_pick_folder(&p.t.app, req(json!({})))?;
    projects::setup_preview(
        &p.t.app,
        req(json!({
            "pickId": pick.pick_id,
            "name": name,
            "slug": name.to_lowercase(),
            "variables": {},
            "conditions": { "idea_known": "yes" },
            "chatProjectUrl": null
        })),
    )
}

fn apply(p: &ProjectApp, pv: &SetupPreview) -> Result<Value, IpcError> {
    projects::setup_apply(&p.t.app, req(json!({ "previewId": pv.preview_id })))
        .map(|s| serde_json::to_value(s).unwrap())
}

/// The planned writes as `(path, action)`.
fn plan(pv: &SetupPreview) -> Vec<(String, String)> {
    pv.writes
        .iter()
        .map(|w| {
            let v = serde_json::to_value(w).unwrap();
            (
                v["path"].as_str().unwrap().to_owned(),
                v["action"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn action_of(pv: &SetupPreview, path: &str) -> String {
    plan(pv)
        .into_iter()
        .find(|(p, _)| p == path)
        .unwrap_or_else(|| panic!("{path} is not in the review"))
        .1
}

mod setup {
    use super::*;

    /// AC-F018-02, AC-F018-03: the review lists every file Foreman will create, leave alone or
    /// refuse, and the folder afterwards holds exactly what was listed: the `.foreman/` decision
    /// files, the five folders, the lessons header and the kit's templates; no council agents, no
    /// commit.
    #[test]
    fn review_lists_every_write_and_apply_matches() {
        let p = ProjectApp::new(json!({}));
        let folder = new_folder(&p, "fresh");
        let before = tree(&folder);
        let pv = preview(&p, &folder, "Fresh").unwrap();
        assert_eq!(tree(&folder), before, "the review writes nothing");
        assert!(pv.git_init, "not a repository yet");
        let planned = plan(&pv);
        for must in [
            ".foreman/project.json",
            ".foreman/overlay.json",
            ".foreman/blueprint.lock.json",
            "docs/handoff/",
            "docs/templates/",
            "docs/change-requests/",
            "docs/postmortems/",
            "spec/",
            "docs/lessons.md",
            "docs/templates/handoff-packet.md",
            "docs/templates/change-request.md",
            "docs/templates/postmortem.md",
            ".git/",
        ] {
            assert!(
                planned.iter().any(|(p, _)| p == must),
                "{must} is listed: {planned:?}"
            );
        }
        assert!(
            planned.iter().all(|(_, a)| a == "create"),
            "a fresh folder only creates: {planned:?}"
        );

        apply(&p, &pv).unwrap();
        let after = tree(&folder);
        // Everything listed exists; nothing unlisted was written.
        let listed: Vec<String> = planned
            .iter()
            .filter(|(p, _)| p != ".git/")
            .map(|(p, _)| p.trim_end_matches('/').to_owned())
            .collect();
        for l in &listed {
            assert!(after.contains_key(l), "{l} was written: {after:?}");
        }
        for (path, content) in &after {
            let covered = listed.iter().any(|l| l == path)
                || listed.iter().any(|l| l.starts_with(&format!("{path}/")));
            assert!(covered, "{path} was written but not listed");
            if content.is_none() && !path.starts_with("docs") && path != ".foreman" {
                assert_eq!(path, "spec");
            }
        }
        assert!(folder.join(".git").is_dir(), "git init ran");
        // The lessons header and the kit templates.
        let lessons = std::fs::read_to_string(folder.join("docs/lessons.md")).unwrap();
        assert!(lessons.starts_with("# Lessons — Fresh"), "{lessons}");
        for (path, text) in KIT_TEMPLATES {
            assert_eq!(std::fs::read_to_string(folder.join(path)).unwrap(), text);
        }
        // No council agents; nothing committed.
        assert_eq!(
            std::fs::read_dir(folder.join("spec")).unwrap().count(),
            0,
            "spec/ is empty: Foreman writes no council agents"
        );
        let head = std::fs::read_to_string(folder.join(".git/HEAD")).unwrap();
        let branch = head.trim().trim_start_matches("ref: ").to_owned();
        assert!(
            !folder.join(".git").join(&branch).exists(),
            "no commit: {branch} has no ref"
        );
        let objects = Command::new("/usr/bin/git")
            .args(["rev-list", "--all", "--count"])
            .current_dir(&folder)
            .env("HOME", &folder)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&objects.stdout).trim(), "0");
        // The review is single-use.
        let e = apply(&p, &pv).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Expired, "{e:?}");
    }

    /// AC-F018-04, T-051: a listed file that exists is never overwritten. The lessons file and a
    /// template equal to the kit's are left alone; a template that differs is refused, and
    /// nothing is written until it is moved aside.
    #[test]
    fn existing_files_are_never_overwritten() {
        let p = ProjectApp::new(json!({}));
        let folder = new_folder(&p, "existing");
        std::fs::create_dir_all(folder.join("docs/templates")).unwrap();
        std::fs::write(folder.join("docs/lessons.md"), "# My own lessons\n").unwrap();
        let (kit_path, kit_text) = KIT_TEMPLATES[0];
        std::fs::write(folder.join(kit_path), kit_text).unwrap();
        let (cr_path, _) = KIT_TEMPLATES[1];
        std::fs::write(folder.join(cr_path), "my change request template\n").unwrap();
        let before = tree(&folder);

        let pv = preview(&p, &folder, "Existing").unwrap();
        assert_eq!(action_of(&pv, "docs/lessons.md"), "leaveAlone");
        assert_eq!(action_of(&pv, kit_path), "leaveAlone");
        assert_eq!(action_of(&pv, cr_path), "refuse");
        let refused = serde_json::to_value(
            pv.writes
                .iter()
                .find(|w| serde_json::to_value(w).unwrap()["path"] == cr_path)
                .unwrap(),
        )
        .unwrap();
        assert!(
            refused["reason"]
                .as_str()
                .unwrap()
                .contains("Move it aside"),
            "{refused}"
        );
        // Apply refuses and writes nothing.
        let e = apply(&p, &pv).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed, "{e:?}");
        assert_eq!(tree(&folder), before, "nothing written");
        assert!(!folder.join(".git").exists(), "not even git init");

        // Moved aside, setup goes ahead and still leaves Billy's files as they were.
        std::fs::rename(folder.join(cr_path), folder.join("old-cr.md")).unwrap();
        let pv = preview(&p, &folder, "Existing").unwrap();
        assert_eq!(action_of(&pv, cr_path), "create");
        apply(&p, &pv).unwrap();
        assert_eq!(
            std::fs::read_to_string(folder.join("docs/lessons.md")).unwrap(),
            "# My own lessons\n"
        );
        assert_eq!(
            std::fs::read_to_string(folder.join(kit_path)).unwrap(),
            kit_text
        );
        assert_eq!(
            std::fs::read_to_string(folder.join("old-cr.md")).unwrap(),
            "my change request template\n"
        );
    }

    /// AC-F018-05, SR-44: a folder that isn't a git repo gets `git init`; one that is isn't
    /// touched; with git missing, setup is blocked with the reason and nothing is written.
    #[test]
    fn git_init_and_missing_git() {
        let p = ProjectApp::new(json!({}));
        // Not a repository: git init is in the review and runs.
        let plain = new_folder(&p, "plain");
        let pv = preview(&p, &plain, "Plain").unwrap();
        assert!(pv.git_init);
        assert_eq!(action_of(&pv, ".git/"), "create");
        apply(&p, &pv).unwrap();
        assert!(plain.join(".git/HEAD").is_file());
        // Already a repository: left alone, its HEAD untouched.
        let repo = new_folder(&p, "repo");
        git(&repo, &["init", "-q", "-b", "trunk"]);
        let head = std::fs::read(repo.join(".git/HEAD")).unwrap();
        let pv = preview(&p, &repo, "Repo").unwrap();
        assert!(!pv.git_init);
        assert_eq!(action_of(&pv, ".git/"), "leaveAlone");
        apply(&p, &pv).unwrap();
        assert_eq!(std::fs::read(repo.join(".git/HEAD")).unwrap(), head);
        // A folder inside an enclosing repository is not its own: setup offers git init rather
        // than act on the enclosing one (SR-44).
        let outer = new_folder(&p, "outer");
        git(&outer, &["init", "-q"]);
        let inner = outer.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        let pv = preview(&p, &inner, "Inner").unwrap();
        assert!(pv.git_init, "an enclosing repo doesn't count");

        // git missing: blocked, with the reason, before anything is written.
        let (mut s, stamp, _) = p.t.app.settings().unwrap();
        s.tools.git.path = Some(p.root.join("nowhere/git").display().to_string());
        s.tools.git.source = ToolSource::Settings;
        p.t.app.save_settings(&s, &stamp).unwrap();
        let blocked = new_folder(&p, "blocked");
        let before = tree(&blocked);
        p.t.os.pick_next(Some(blocked.clone()));
        let e = projects::setup_pick_folder(&p.t.app, req(json!({}))).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::ToolMissing, "{e:?}");
        assert!(!e.message.is_empty(), "the reason is given");
        assert_eq!(tree(&blocked), before);
        assert!(!blocked.join(".git").exists());
    }

    /// AC-F018-06, SR-24: a folder inside another registered project, one that holds a project,
    /// the blueprint repo itself, or one Foreman can't write is refused before any write, with
    /// the reason.
    #[test]
    fn unsafe_folders_are_refused_before_any_write() {
        let p = ProjectApp::new(json!({}));
        let inside = p.project.join("nested");
        std::fs::create_dir_all(&inside).unwrap();
        let blueprint = std::fs::canonicalize(p.root.join("blueprint")).unwrap();
        let mut cases: Vec<(PathBuf, &str)> = vec![
            (inside, "inside another Foreman project"),
            (
                p.project.parent().unwrap().to_path_buf(),
                "contains another Foreman project",
            ),
            (blueprint, "blueprint repo"),
        ];
        // A read-only folder; root can write anywhere, so this case needs a normal user.
        if !rustix::process::geteuid().is_root() {
            let ro = new_folder(&p, "readonly");
            let mut perm = std::fs::metadata(&ro).unwrap().permissions();
            std::os::unix::fs::PermissionsExt::set_mode(&mut perm, 0o555);
            std::fs::set_permissions(&ro, perm).unwrap();
            cases.push((ro, "can't write"));
        }
        for (folder, why) in cases {
            let before = tree(&folder);
            // The pick itself says why (the form shows it inline) ...
            p.t.os.pick_next(Some(folder.clone()));
            let pick = projects::setup_pick_folder(&p.t.app, req(json!({}))).unwrap();
            let pick = serde_json::to_value(pick).unwrap();
            let reason = pick["refusal"].as_str().unwrap_or_default().to_owned();
            assert!(reason.contains(why), "{folder:?}: {reason}");
            // ... and the review refuses it.
            let e = projects::setup_preview(
                &p.t.app,
                req(json!({
                    "pickId": pick["pickId"], "name": "No", "slug": "no", "variables": {},
                    "conditions": {}, "chatProjectUrl": null
                })),
            )
            .unwrap_err();
            assert_eq!(e.code, IpcErrorCode::NotAllowed, "{e:?}");
            assert!(e.message.contains(why), "{}", e.message);
            assert_eq!(tree(&folder), before, "{folder:?} untouched");
            assert!(!folder.join(".foreman").exists());
        }
    }

    /// AC-F018-07: a folder whose `.foreman/` has a supported schema is registered as it is,
    /// writing no file; one with a newer schema registers and opens read-only.
    #[test]
    fn existing_foreman_folder_is_registered_without_writes() {
        let p = ProjectApp::new(json!({}));
        let before = tree(&p.project);
        // Forget the project (its folder stays), then set it up again from the same folder.
        p.t.os.answer_next(true);
        projects::project_remove(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        assert!(projects::summaries(&p.t.app).unwrap().is_empty());
        let pv = preview(&p, &p.project, "TradeLog").unwrap();
        assert_eq!(action_of(&pv, ".foreman/"), "leaveAlone", "{:?}", plan(&pv));
        let summary = apply(&p, &pv).unwrap();
        assert_eq!(summary["id"], json!(p.id), "the same project comes back");
        assert_eq!(tree(&p.project), before, "no file written or changed");
        let view = serde_json::to_value(
            projects::project_open(&p.t.app, req(json!({ "projectId": p.id }))).unwrap(),
        )
        .unwrap();
        assert!(view["readOnly"].is_null(), "{}", view["readOnly"]);

        // A newer schema: registered, read-only, nothing written.
        p.t.os.answer_next(true);
        projects::project_remove(&p.t.app, req(json!({ "projectId": p.id }))).unwrap();
        let file = p.project.join(".foreman/project.json");
        let text = std::fs::read_to_string(&file).unwrap();
        let mut v: Value = serde_json::from_str(&text).unwrap();
        v["schemaVersion"] = json!(99);
        std::fs::write(&file, serde_json::to_string_pretty(&v).unwrap()).unwrap();
        let before = tree(&p.project);
        p.t.os.pick_next(Some(p.project.clone()));
        let pick = projects::setup_pick_folder(&p.t.app, req(json!({}))).unwrap();
        assert_eq!(
            serde_json::to_value(&pick).unwrap()["hasForeman"],
            json!("newer")
        );
        let pv = projects::setup_preview(
            &p.t.app,
            req(json!({
                "pickId": pick.pick_id, "name": "TradeLog", "slug": "tradelog",
                "variables": {}, "conditions": {}, "chatProjectUrl": null
            })),
        )
        .unwrap();
        assert_eq!(action_of(&pv, ".foreman/"), "leaveAlone");
        let summary = apply(&p, &pv).unwrap();
        assert_eq!(summary["readOnly"]["reason"], json!("newerSchema"));
        assert_eq!(tree(&p.project), before, "a newer file is never rewritten");
        let view = serde_json::to_value(
            projects::project_open(&p.t.app, req(json!({ "projectId": p.id }))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            view["readOnly"]["reason"],
            json!("newerSchema"),
            "{}",
            view["readOnly"]
        );
    }
}
