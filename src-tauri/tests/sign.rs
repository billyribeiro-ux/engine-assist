//! M4-19…M4-22: signing against a real project repo: the preview and its gating, the native sign
//! dialog, phase 1 and the plumbing commit (pathspec only, untracked files, read-back, a moving
//! ref, a held index.lock), commit pending and retry, another branch, the hostile-repo fixture
//! (HR-05, HR-06: no hook, filter or gpg program runs), and tamper detection and restore.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs and run git (design doc §3.2)"
)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use common::project::{ProjectApp, channel, req};
use foreman_app::ipc::IpcError;
use foreman_app::ipc::commands::{app as app_cmd, gates, packets, signing};
use foreman_app::ipc::dto::gates::{SignPreview, SignResult};
use foreman_app::ipc::dto::launch::SignProgress;
use foreman_app::ipc::error::{IpcErrorCode, IpcErrorDetail};
use foreman_app::project;
use foreman_app::signing::Seams;
use foreman_core::schema::progress::{DecisionFileKind, SigningState};
use serde_json::{Value, json};

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("/usr/bin/git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgSign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn identity(p: &ProjectApp) {
    std::fs::write(
        p.t.home().join(".gitconfig"),
        "[user]\n\tname = Billy Ribeiro\n\temail = billy@example.com\n",
    )
    .unwrap();
}

/// A valid Stage 1 packet from the pinned template's format (every key filled, unsigned).
fn stage1_packet(p: &ProjectApp) -> String {
    let o = project::open(&p.t.app, &p.id).unwrap();
    let f = o
        .template
        .as_ref()
        .unwrap()
        .packet_format(1)
        .unwrap()
        .clone();
    let mut s = String::from("=== HANDOFF PACKET · Stage 1 → Stage 2 ===\n");
    for k in &f.keys {
        let v = match k.key.as_str() {
            "APPROVED BY" => "<blank>".to_owned(),
            "OPEN ITEMS" => "none".to_owned(),
            other => format!("the {} for TradeLog", other.to_lowercase()),
        };
        s.push_str(&format!("{}: {v}\n", k.key));
    }
    s
}

/// The decision files committed and marked so (what setup's own commit does when git has an
/// identity; the test app's home has none during setup).
fn committed_baseline(p: &ProjectApp) {
    git(&p.project, &["add", ".foreman"]);
    git(&p.project, &["commit", "-q", "-m", "foreman: setup"]);
    let o = project::open(&p.t.app, &p.id).unwrap();
    let root = o.root().unwrap();
    let d = foreman_core::decisions::Decisions {
        project: root,
        app: &o.app_root,
        project_id: &p.id,
    };
    for k in DecisionFileKind::ALL {
        let b = std::fs::read(p.project.join(k.repo_path())).unwrap();
        d.mark_committed(k, &foreman_core::store::sha256_hex(&b))
            .unwrap();
    }
}

fn try_head(p: &ProjectApp) -> Option<String> {
    Command::new("/usr/bin/git")
        .args(["rev-parse", "--verify", "-q", "HEAD"])
        .current_dir(&p.project)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
}

/// Stage 1 ready to sign: a valid packet, every item attested, a lessons entry.
fn ready(p: &ProjectApp) {
    identity(p);
    if try_head(p).is_none() {
        committed_baseline(p);
    }
    let dir = p.project.join("docs/handoff");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("stage-01.md"), stage1_packet(p)).unwrap();
    let lessons = p.project.join("docs/lessons.md");
    let mut text = std::fs::read_to_string(&lessons).unwrap_or_else(|_| "# Lessons\n\n".into());
    text.push_str("\n## L-001 · Stage 1 · 2026-10-20 · clean stage\nSTATUS: applied\n");
    std::fs::write(&lessons, text).unwrap();
    let v = gates::gate_view(&p.t.app, req(json!({ "projectId": p.id, "stage": 1 }))).unwrap();
    for (i, it) in v.items.iter().enumerate() {
        gates::gate_item_attest(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "item": it.id, "outcome": { "kind": "done", "note": format!("checked item number {i} by hand"), "link": null } })),
        )
        .unwrap();
    }
    let v = gates::gate_view(&p.t.app, req(json!({ "projectId": p.id, "stage": 1 }))).unwrap();
    assert!(v.signable, "not signable: {:?}", v.why_not);
}

fn preview(p: &ProjectApp, branch: Value) -> Result<SignPreview, IpcError> {
    signing::sign_preview(
        &p.t.app,
        req(
            json!({ "projectId": p.id, "stage": 1, "branch": branch, "name": "Billy Ribeiro", "date": "2026-10-20" }),
        ),
    )
}

fn current() -> Value {
    json!({ "kind": "current" })
}

fn collect() -> (tauri::ipc::Channel<SignProgress>, Arc<Mutex<Vec<String>>>) {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let b = buf.clone();
    let ch = tauri::ipc::Channel::new(move |body| {
        if let tauri::ipc::InvokeResponseBody::Json(j) = body {
            b.lock().unwrap().push(j);
        }
        Ok(())
    });
    (ch, buf)
}

fn apply_with(p: &ProjectApp, pv: &SignPreview, seams: &Seams<'_>) -> Result<SignResult, IpcError> {
    signing::sign_apply_with(
        &p.t.app,
        req(json!({ "previewId": pv.preview_id })),
        channel(),
        seams,
    )
}

fn apply(p: &ProjectApp, pv: &SignPreview) -> Result<SignResult, IpcError> {
    apply_with(p, pv, &Seams::default())
}

fn head(p: &ProjectApp) -> String {
    git(&p.project, &["rev-parse", "HEAD"])
}

fn markers(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|r| {
            r.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// HR-05/HR-06 from `scripts/make-hostile-repo.mjs`: the hostile `.git/config` (hooksPath,
/// fsmonitor, filter driver, gpg program with `commit.gpgSign=true`, …), the `.hooks/*` (a
/// lint-staged-style pre-commit, post-commit, reference-transaction) and `.gitattributes`.
fn make_hostile(p: &ProjectApp) -> PathBuf {
    let d = p.root.join("hostile");
    let st = Command::new("node")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/make-hostile-repo.mjs"))
        .arg(&d)
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "{}",
        String::from_utf8_lossy(&st.stderr)
    );
    let src = d.join("repo");
    let cfg = std::fs::read_to_string(src.join(".git/config")).unwrap();
    // Everything after the first [core] block is the hostile part.
    let hostile = &cfg[cfg[1..].find("\n[").unwrap() + 2..];
    let mut mine = std::fs::read_to_string(p.project.join(".git/config")).unwrap();
    mine.push_str(hostile);
    std::fs::write(p.project.join(".git/config"), mine).unwrap();
    std::fs::create_dir_all(p.project.join(".hooks")).unwrap();
    for h in ["pre-commit", "post-commit", "reference-transaction"] {
        std::fs::copy(src.join(".hooks").join(h), p.project.join(".hooks").join(h)).unwrap();
    }
    std::fs::copy(src.join(".gitattributes"), p.project.join(".gitattributes")).unwrap();
    let m = p.project.join("markers");
    std::fs::create_dir_all(&m).unwrap();
    // Positive control: plain git status in this repo fires the fsmonitor/filter markers…
    m
}

mod sign {
    use super::*;

    /// AC-F014-01/-02, AC-F013-15: the preview shows name, date, branch, the added fields as a
    /// diff, the content hash and the paths; nothing is written until apply.
    #[test]
    fn preview_shows_fields() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let before = std::fs::read_to_string(p.project.join("docs/handoff/stage-01.md")).unwrap();
        let pv = preview(&p, current()).unwrap();
        assert_eq!(pv.short.len(), 6);
        assert_eq!(pv.commit_message, "handoff: stage 01");
        let added = serde_json::to_string(&pv.added_fields).unwrap();
        assert!(
            added.contains("APPROVED BY: Billy Ribeiro on 2026-10-20"),
            "{added}"
        );
        assert!(
            added.contains("SKIPPED:") && added.contains("ATTESTED:"),
            "{added}"
        );
        assert!(added.contains("SUBSTITUTIONS"), "{added}");
        let paths: Vec<&str> = pv.paths.iter().map(|x| x.as_str()).collect();
        assert!(paths.contains(&"docs/handoff/stage-01.md"));
        assert!(paths.contains(&".foreman/stages/stage-01.json"));
        assert!(
            pv.confirm_conditions.is_none(),
            "only Stage 2 confirms conditions"
        );
        assert_eq!(
            std::fs::read_to_string(p.project.join("docs/handoff/stage-01.md")).unwrap(),
            before
        );
        // A name that isn't a signature form.
        let e = signing::sign_preview(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "branch": current(), "name": "<Claude>", "date": "2026-10-20" })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Invalid);
        // Not signable: stage 2 (locked by stage 1, items waiting).
        let e = signing::sign_preview(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 2, "branch": current(), "name": "Billy Ribeiro", "date": "2026-10-20" })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
    }

    /// AC-F014-03: no git → Sign is disabled and the reason names git.
    #[test]
    fn git_missing_disables_sign() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let (mut s, stamp, _) = p.t.app.settings().unwrap();
        let fake = p.root.join("bin/git");
        std::fs::write(&fake, "not a program").unwrap();
        s.tools.git.path = Some(fake.display().to_string());
        s.tools.git.source = foreman_core::schema::settings::ToolSource::Settings;
        p.t.app.save_settings(&s, &stamp).unwrap();
        let v = gates::gate_view(&p.t.app, req(json!({ "projectId": p.id, "stage": 1 }))).unwrap();
        assert!(!v.signable);
        assert!(
            v.why_not.iter().any(|w| w.contains("git")),
            "{:?}",
            v.why_not
        );
        let e = preview(&p, current()).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::ToolMissing);
    }

    /// AC-F014-04: the packet changed after Billy reviewed it → refused, re-rendered (a fresh
    /// preview in the Stale detail), nothing written or committed.
    #[test]
    fn refuses_changed_file() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let pv = preview(&p, current()).unwrap();
        let path = p.project.join("docs/handoff/stage-01.md");
        let mut t = std::fs::read_to_string(&path).unwrap();
        t = t.replace(
            "the project for TradeLog",
            "the project for TradeLog, edited",
        );
        std::fs::write(&path, &t).unwrap();
        let asked = p.t.os.asked().len();
        let e = apply(&p, &pv).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Stale);
        let Some(IpcErrorDetail::Stale { fresh: Some(f), .. }) = &e.detail else {
            panic!("{e:?}")
        };
        assert_ne!(f["packetSha256"], json!(pv.packet_sha256));
        assert_eq!(p.t.os.asked().len(), asked, "no dialog for a stale packet");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), t);
        assert!(!p.project.join(".foreman/stages/stage-01.json").exists());
    }

    /// AC-F014-05/-06, EX-M4-03: unrelated staged and unstaged changes survive and aren't
    /// committed; the commit holds exactly the packet, the record and lessons; the real index
    /// is refreshed for those paths only; plumbing steps in order.
    #[test]
    fn pathspec_only() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let d = &p.project;
        std::fs::write(d.join("README.md"), "base\n").unwrap();
        git(d, &["add", "README.md"]);
        git(d, &["commit", "-q", "-m", "base"]);
        std::fs::write(d.join("README.md"), "staged\n").unwrap();
        git(d, &["add", "README.md"]);
        std::fs::write(d.join("README.md"), "staged then unstaged\n").unwrap();
        std::fs::write(d.join("notes.txt"), "untracked\n").unwrap();
        let base = head(&p);
        let pv = preview(&p, current()).unwrap();
        let (ch, buf) = collect();
        let r =
            signing::sign_apply(&p.t.app, req(json!({ "previewId": pv.preview_id })), ch).unwrap();
        let SignResult::Signed { commit, .. } = r else {
            panic!("{r:?}")
        };
        assert_eq!(head(&p), commit);
        assert_eq!(git(d, &["rev-parse", "HEAD~1"]), base);
        assert_eq!(git(d, &["log", "-1", "--format=%s"]), "handoff: stage 01");
        let files = git(
            d,
            &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
        );
        let mut files: Vec<&str> = files.lines().collect();
        files.sort_unstable();
        assert_eq!(
            files,
            [
                ".foreman/stages/stage-01.json",
                "docs/handoff/stage-01.md",
                "docs/lessons.md"
            ]
        );
        // Unrelated changes are as they were: README staged + unstaged, notes untracked.
        assert_eq!(
            std::fs::read_to_string(d.join("README.md")).unwrap(),
            "staged then unstaged\n"
        );
        assert_eq!(git(d, &["show", ":README.md"]), "staged");
        let status = git(d, &["status", "--porcelain"]);
        assert!(status.contains("MM README.md"), "{status}");
        assert!(status.contains("?? notes.txt"), "{status}");
        assert!(
            !status.contains("stage-01"),
            "signed paths are clean: {status}"
        );
        // Phase 1: APPROVED BY in place, the record, progress.
        let packet = std::fs::read_to_string(d.join("docs/handoff/stage-01.md")).unwrap();
        assert_eq!(
            packet.lines().nth(2),
            Some("APPROVED BY: Billy Ribeiro on 2026-10-20")
        );
        let rec: Value = serde_json::from_str(
            &std::fs::read_to_string(d.join(".foreman/stages/stage-01.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(rec["signedBy"], "Billy Ribeiro");
        assert_eq!(rec["git"]["signMode"], "noGpgSign");
        assert_eq!(
            rec["packet"]["sha256"],
            json!(foreman_core::store::sha256_hex(packet.as_bytes()))
        );
        let o = project::open(&p.t.app, &p.id).unwrap();
        let s = &o.progress.signing["1"];
        assert_eq!(s.state, SigningState::Signed);
        assert_eq!(s.commit.as_deref(), Some(commit.as_str()));
        assert!(o.signed().contains(&1));
        assert!(o.progress.index_refresh_pending.is_none());
        // Progress in plumbing order.
        let steps: Vec<String> = buf
            .lock()
            .unwrap()
            .iter()
            .map(|j| {
                serde_json::from_str::<Value>(j.as_str()).unwrap()["step"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        let pos = |s: &str| steps.iter().position(|x| x == s).unwrap();
        assert!(pos("reRead") < pos("write"));
        assert!(pos("write") < pos("hashObject"));
        assert!(pos("hashObject") < pos("writeTree"));
        assert!(pos("writeTree") < pos("commitTree"));
        assert!(pos("commitTree") < pos("updateRef"));
        assert!(pos("updateRef") < pos("readBack"));
        assert_eq!(steps.last().map(String::as_str), Some("done"));
        // The gate shows signed; a second sign is refused.
        let v = gates::gate_view(&p.t.app, req(json!({ "projectId": p.id, "stage": 1 }))).unwrap();
        assert!(!v.signable);
        assert_eq!(
            preview(&p, current()).unwrap_err().code,
            IpcErrorCode::NotAllowed
        );
    }

    /// AC-F014-08: a new untracked packet and stage record are both committed.
    #[test]
    fn untracked_new_packet() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let st = git(&p.project, &["status", "--porcelain", "--", "docs/handoff"]);
        assert!(st.contains("??"), "untracked before: {st}");
        let pv = preview(&p, current()).unwrap();
        let SignResult::Signed { commit, .. } = apply(&p, &pv).unwrap() else {
            panic!()
        };
        let tree = git(&p.project, &["ls-tree", "-r", "--name-only", &commit]);
        assert!(tree.contains("docs/handoff/stage-01.md"), "{tree}");
        assert!(tree.contains(".foreman/stages/stage-01.json"), "{tree}");
    }

    /// AC-F014-10, AC-F014-11: read-back finds different bytes in the commit (a hook rewrote
    /// the packet) → "Your repo's commit hooks changed the packet" with the diff; the stage stays
    /// commit pending; Retry commit stamps it once the commit is right.
    #[test]
    fn readback_detects_mismatch() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let pv = preview(&p, current()).unwrap();
        let rewrite = |path: &str, b: Vec<u8>| {
            if path.ends_with("stage-01.md") {
                let mut b = b;
                b.extend_from_slice(b"Formatted by a hook\n");
                b
            } else {
                b
            }
        };
        let seams = Seams {
            tamper_readback: Some(&rewrite),
            ..Seams::default()
        };
        let r = apply_with(&p, &pv, &seams).unwrap();
        let SignResult::ReadBackMismatch { diffs } = r else {
            panic!("{r:?}")
        };
        assert!(
            serde_json::to_string(&diffs)
                .unwrap()
                .contains("Formatted by a hook")
        );
        let o = project::open(&p.t.app, &p.id).unwrap();
        assert_eq!(o.progress.signing["1"].state, SigningState::CommitPending);
        assert!(!o.signed().contains(&1), "no stamp before read-back passes");
        let v = gates::gate_view(&p.t.app, req(json!({ "projectId": p.id, "stage": 1 }))).unwrap();
        assert_eq!(v.closeout.last().unwrap().label, "Signed (commit pending)");
        // Retry: the commit is already right; read-back passes and the stamp lands.
        let r = signing::sign_retry_commit(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1 })),
            channel(),
        )
        .unwrap();
        assert!(matches!(r, SignResult::Signed { .. }), "{r:?}");
        assert!(
            project::open(&p.t.app, &p.id)
                .unwrap()
                .signed()
                .contains(&1)
        );
    }

    /// The ref moved between commit-tree and update-ref: nothing is overwritten; commit pending
    /// with the reason; Retry commit lands on the new tip.
    #[test]
    fn concurrent_ref_move_fails() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let pv = preview(&p, current()).unwrap();
        let d = p.project.clone();
        let mover = move || {
            std::fs::write(d.join("src.rs"), "// Claude committed meanwhile\n").unwrap();
            git(&d, &["add", "src.rs"]);
            git(&d, &["commit", "-q", "-m", "meanwhile"]);
        };
        let seams = Seams {
            before_update_ref: Some(&mover),
            ..Seams::default()
        };
        let r = apply_with(&p, &pv, &seams).unwrap();
        let SignResult::CommitPending { stderr_tail } = r else {
            panic!("{r:?}")
        };
        assert!(stderr_tail.contains("moved"), "{stderr_tail}");
        assert_eq!(git(&p.project, &["log", "-1", "--format=%s"]), "meanwhile");
        let r = signing::sign_retry_commit(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1 })),
            channel(),
        )
        .unwrap();
        let SignResult::Signed { commit, .. } = r else {
            panic!("{r:?}")
        };
        assert_eq!(
            git(&p.project, &["rev-parse", &format!("{commit}~1")]),
            git(&p.project, &["rev-parse", "HEAD~1"])
        );
        assert_eq!(
            git(&p.project, &["log", "-2", "--format=%s"]),
            "handoff: stage 01\nmeanwhile"
        );
        // Not commit pending any more.
        let e = signing::sign_retry_commit(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1 })),
            channel(),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
    }

    /// AC-F014-06: a held `index.lock` during the real-index refresh: retried, then the commit
    /// stands, `indexRefreshPending` is recorded; the retry on the next open clears it.
    #[test]
    fn index_locked_refresh_retried() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let pv = preview(&p, current()).unwrap();
        let lock = p.project.join(".git/index.lock");
        std::fs::write(&lock, "").unwrap();
        let seams = Seams {
            index_retries: Some((5, std::time::Duration::from_millis(10))),
            ..Seams::default()
        };
        let r = apply_with(&p, &pv, &seams).unwrap();
        let SignResult::Signed { commit, .. } = r else {
            panic!("{r:?}")
        };
        let o = project::open(&p.t.app, &p.id).unwrap();
        let pend = o.progress.index_refresh_pending.clone().unwrap();
        assert_eq!(pend.commit, commit);
        assert!(pend.paths.contains(&"docs/handoff/stage-01.md".to_owned()));
        // Still locked: project_open retries and keeps it.
        projects_open(&p);
        assert!(
            project::open(&p.t.app, &p.id)
                .unwrap()
                .progress
                .index_refresh_pending
                .is_some()
        );
        std::fs::remove_file(&lock).unwrap();
        projects_open(&p);
        assert!(
            project::open(&p.t.app, &p.id)
                .unwrap()
                .progress
                .index_refresh_pending
                .is_none()
        );
        let status = git(&p.project, &["status", "--porcelain", "--", "docs/handoff"]);
        assert!(status.is_empty(), "{status}");
    }

    /// AC-F014-13: Foreman killed between phase 1 and phase 2 → after the restart the stage shows
    /// `signed (commit pending)` with Retry commit; the retry commits and stamps.
    #[test]
    fn killed_between_phases_retry() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let pv = preview(&p, current()).unwrap();
        let seams = Seams {
            stop_after_phase1: true,
            ..Seams::default()
        };
        let base = try_head(&p);
        apply_with(&p, &pv, &seams).unwrap();
        assert_eq!(try_head(&p), base);
        // The restart.
        let again = foreman_app::state::App::new(
            Arc::new(foreman_app::os::ScriptedOs::default()),
            foreman_app::state::Paths {
                app_data: p.t.app_data(),
                home: p.t.home(),
            },
        );
        let v = gates::gate_view(&again, req(json!({ "projectId": p.id, "stage": 1 }))).unwrap();
        assert_eq!(v.closeout.last().unwrap().label, "Signed (commit pending)");
        let r = signing::sign_retry_commit(
            &again,
            req(json!({ "projectId": p.id, "stage": 1 })),
            channel(),
        )
        .unwrap();
        assert!(matches!(r, SignResult::Signed { .. }), "{r:?}");
    }

    /// AC-F014-09, AC-F014-16, SR-43: signing on another branch commits on that branch's tip;
    /// HEAD, the index and the working tree are unchanged; the stage reads "signed on <branch>
    /// (not merged)" and the packet is read from the commit. HR-06's reference-transaction hook
    /// doesn't run.
    #[test]
    fn other_branch_leaves_worktree_head_and_index() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let d = p.project.clone();
        std::fs::write(d.join("README.md"), "base\n").unwrap();
        git(&d, &["add", "README.md"]);
        git(&d, &["commit", "-q", "-m", "base"]);
        let m = make_hostile(&p);
        let head0 = head(&p);
        let index0 = std::fs::read(d.join(".git/index")).unwrap();
        let packet0 = std::fs::read_to_string(d.join("docs/handoff/stage-01.md")).unwrap();
        let e = preview(&p, json!({ "kind": "other", "branch": "../evil" })).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Invalid);
        let pv = preview(&p, json!({ "kind": "other", "branch": "handoff/stage-01" })).unwrap();
        assert_eq!(pv.branch, "handoff/stage-01");
        let r = apply(&p, &pv).unwrap();
        let SignResult::Signed { commit, .. } = r else {
            panic!("{r:?}")
        };
        assert_eq!(
            git(&d, &["rev-parse", "refs/heads/handoff/stage-01"]),
            commit
        );
        assert_eq!(git(&d, &["rev-parse", &format!("{commit}~1")]), head0);
        assert_eq!(head(&p), head0, "HEAD unchanged");
        assert_eq!(
            std::fs::read(d.join(".git/index")).unwrap(),
            index0,
            "index unchanged"
        );
        assert_eq!(
            std::fs::read_to_string(d.join("docs/handoff/stage-01.md")).unwrap(),
            packet0
        );
        assert!(
            !d.join(".foreman/stages/stage-01.json").exists(),
            "nothing written"
        );
        let blob = git(&d, &["show", &format!("{commit}:docs/handoff/stage-01.md")]);
        assert!(blob.contains("APPROVED BY: Billy Ribeiro on 2026-10-20"));
        // Signed on that branch (not merged); the resolver reads the committed packet.
        let o = project::open(&p.t.app, &p.id).unwrap();
        assert_eq!(
            o.progress.signing["1"].state,
            SigningState::SignedOtherBranch
        );
        assert!(o.signed().contains(&1));
        assert!(
            o.packets[&1]
                .text
                .contains("APPROVED BY: Billy Ribeiro on 2026-10-20")
        );
        assert!(o.tampered.is_empty());
        let view = o.view(&p.t.app, String::new());
        let s1 = view.stages.iter().find(|s| s.stage.0 == 1).unwrap();
        let info = s1.signed.as_ref().unwrap();
        assert_eq!(info.branch, "handoff/stage-01");
        assert!(!info.merged);
        assert!(
            markers(&m).is_empty(),
            "a hostile program ran: {:?}",
            markers(&m)
        );
    }

    /// AC-F014-15, DR-146, SR-46: a signed packet changed on disk → tamper (Today item, the
    /// resolver uses the committed version); Restore writes the committed blob back by CAS.
    #[test]
    fn tamper_detected() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let pv = preview(&p, current()).unwrap();
        apply(&p, &pv).unwrap();
        let path = p.project.join("docs/handoff/stage-01.md");
        let signed = std::fs::read_to_string(&path).unwrap();
        let edited = signed.replace("APPROVED BY: Billy Ribeiro", "APPROVED BY: Mallory");
        std::fs::write(&path, &edited).unwrap();
        let o = project::open(&p.t.app, &p.id).unwrap();
        assert!(o.tampered.contains_key(&1));
        assert_eq!(
            o.packets[&1].text, signed,
            "the resolver uses the committed version"
        );
        let diff =
            serde_json::to_string(&foreman_app::signing::tamper_diff(&o, 1).unwrap()).unwrap();
        assert!(diff.contains("Mallory"), "{diff}");
        // Today shows it.
        let today = app_cmd::today_get(&p.t.app, req(json!({}))).unwrap();
        assert!(
            today
                .inbox
                .iter()
                .any(|i| serde_json::to_value(i.kind).unwrap() == "tamper"),
            "{:?}",
            today.inbox
        );
        // Restore with a stale hash: refused.
        let e = signing::tamper_restore(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "seenSha256": "0".repeat(64) })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Stale);
        let seen = foreman_core::store::sha256_hex(edited.as_bytes());
        let v = signing::tamper_restore(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "seenSha256": seen })),
        )
        .unwrap();
        assert!(v.tamper.is_none());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), signed);
        assert!(project::open(&p.t.app, &p.id).unwrap().tampered.is_empty());
        let e = signing::tamper_restore(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "seenSha256": seen })),
        )
        .unwrap_err();
        assert_eq!(e.code, IpcErrorCode::NotAllowed);
    }

    /// Code review C1: `project_open` announces a tampered packet once per version. The refetch
    /// the event causes opens the project again and stays quiet; a new edit, or tampering again
    /// after a restore, is announced again.
    #[test]
    fn tamper_event_once_per_version() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let pv = preview(&p, current()).unwrap();
        apply(&p, &pv).unwrap();
        let tampers = || {
            p.t.os
                .events()
                .iter()
                .filter(|e| e.name() == "tamper:detected")
                .count()
        };
        let path = p.project.join("docs/handoff/stage-01.md");
        let signed = std::fs::read_to_string(&path).unwrap();
        let edited = signed.replace("APPROVED BY: Billy Ribeiro", "APPROVED BY: Mallory");
        std::fs::write(&path, &edited).unwrap();
        for _ in 0..3 {
            projects_open(&p);
        }
        assert_eq!(tampers(), 1, "one event for one tampered version");
        std::fs::write(&path, format!("{edited}more\n")).unwrap();
        projects_open(&p);
        projects_open(&p);
        assert_eq!(tampers(), 2, "a new version is announced");
        std::fs::write(&path, &signed).unwrap();
        projects_open(&p);
        std::fs::write(&path, &edited).unwrap();
        projects_open(&p);
        assert_eq!(tampers(), 3, "tampered again after a restore");
    }

    /// Code review W3: the sign dialog is shown without the write lock (PTY exit handlers, the
    /// watcher and decision refreshes keep working while Billy reads it), and the packet is
    /// checked again under the lock after it: a packet changed while the dialog was open is
    /// `Stale` with a fresh preview, and nothing is written.
    #[test]
    fn dialog_without_lock_rechecks_after() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let pv = preview(&p, current()).unwrap();
        let app = Arc::clone(&p.t.app);
        let path = p.project.join("docs/handoff/stage-01.md");
        let held = Arc::new(Mutex::new(None));
        let h = Arc::clone(&held);
        let file = path.clone();
        p.t.os.while_dialog_open(Some(Box::new(move |_| {
            *h.lock().unwrap() = Some(app.write_lock_held());
            let t = std::fs::read_to_string(&file).unwrap();
            std::fs::write(&file, format!("{t}\nEdited while the dialog was open.\n")).unwrap();
        })));
        let before_head = try_head(&p);
        let e = apply(&p, &pv).unwrap_err();
        p.t.os.while_dialog_open(None);
        assert_eq!(*held.lock().unwrap(), Some(false), "the lock was held");
        assert_eq!(e.code, IpcErrorCode::Stale, "{e:?}");
        assert!(
            matches!(
                &e.detail,
                Some(IpcErrorDetail::Stale { fresh: Some(_), .. })
            ),
            "{e:?}"
        );
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .contains("APPROVED BY: Billy Ribeiro"),
            "nothing was signed"
        );
        assert_eq!(try_head(&p), before_head, "nothing committed");
    }

    /// IPC-R10 (code review W6): a conflict in phase 1 names the file and what phase 1 already
    /// wrote, so the next open doesn't take Foreman's own APPROVED BY for an outside signature
    /// without telling Billy.
    #[test]
    fn conflict_reports_written_targets() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let pv = preview(&p, current()).unwrap();
        let record = p.project.join(".foreman/stages/stage-01.json");
        let r = record.clone();
        let between = move || {
            std::fs::create_dir_all(r.parent().unwrap()).unwrap();
            std::fs::write(&r, "{\"written\":\"by someone else\"}\n").unwrap();
        };
        let seams = Seams {
            after_packet_write: Some(&between),
            ..Seams::default()
        };
        let e = apply_with(&p, &pv, &seams).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Conflict, "{e:?}");
        match &e.detail {
            Some(IpcErrorDetail::Conflict { path, written }) => {
                assert_eq!(path.as_str(), ".foreman/stages/stage-01.json");
                let w: Vec<&str> = written.iter().map(|x| x.as_str()).collect();
                assert_eq!(w, ["docs/handoff/stage-01.md"]);
            }
            other => panic!("{other:?}"),
        }
    }

    /// M4-22 (AC-F014-15): `packet_view` of a tampered signed packet carries the tamper diff
    /// (committed → working); the fields are the committed text's, and the diff is gone once the
    /// committed text is back.
    #[test]
    fn packet_view_shows_the_tamper_diff() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let pv = preview(&p, current()).unwrap();
        apply(&p, &pv).unwrap();
        let view = || {
            packets::packet_view(&p.t.app, req(json!({ "projectId": p.id, "stage": 1 }))).unwrap()
        };
        assert!(view().tamper.is_none(), "signed and untouched");
        let path = p.project.join("docs/handoff/stage-01.md");
        let signed = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            signed.replace("APPROVED BY: Billy Ribeiro", "APPROVED BY: Mallory"),
        )
        .unwrap();
        let v = view();
        let tamper = v.tamper.expect("tamper diff");
        assert_eq!(tamper.path.as_str(), "docs/handoff/stage-01.md");
        let hunks = serde_json::to_string(&tamper.hunks).unwrap();
        assert!(
            hunks.contains("Mallory") && hunks.contains("\"op\":\"del\""),
            "{hunks}"
        );
        assert!(
            !serde_json::to_string(&v.fields)
                .unwrap()
                .contains("Mallory"),
            "the view shows the committed packet"
        );
        std::fs::write(&path, &signed).unwrap();
        assert!(view().tamper.is_none());
    }
}

fn projects_open(p: &ProjectApp) {
    foreman_app::ipc::commands::projects::project_open(&p.t.app, req(json!({ "projectId": p.id })))
        .unwrap();
}

mod git {
    use super::*;

    /// AC-F014-07, AC-F014-12, EX-M4-03, HR-05/HR-06: in a repo with a lint-staged-style
    /// pre-commit, a filter driver on `*.md`, fsmonitor and a repo-scope gpg program with
    /// `commit.gpgSign=true`, signing runs no hook, filter or gpg program; the commit isn't
    /// GPG-signed; read-back passes; the stage record notes the overrides.
    #[test]
    fn repo_gpg_program_never_runs() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let m = make_hostile(&p);
        // Positive control on a copy: plain `git status` fires the hostile config.
        let pv = preview(&p, current()).unwrap();
        let r = apply(&p, &pv).unwrap();
        let SignResult::Signed { commit, .. } = r else {
            panic!("{r:?}")
        };
        assert!(
            markers(&m).is_empty(),
            "a hostile program ran: {:?}",
            markers(&m)
        );
        let raw = git(&p.project, &["cat-file", "commit", &commit]);
        assert!(!raw.contains("gpgsig"), "{raw}");
        let rec: Value = serde_json::from_str(
            &std::fs::read_to_string(p.project.join(".foreman/stages/stage-01.json")).unwrap(),
        )
        .unwrap();
        let o: Vec<&str> = rec["git"]["overrides"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(o.contains(&"core.hooksPath=/dev/null") && o.contains(&"commit.gpgSign=false"));
        // Our own control: git status (outside Foreman) would run the hostile fsmonitor/filter.
        let _ = Command::new("/usr/bin/git")
            .args(["status", "--porcelain"])
            .current_dir(&p.project)
            .env("HOME", &p.project)
            .output()
            .unwrap();
        assert!(
            !markers(&m).is_empty(),
            "positive control: the fixture's config is live"
        );
    }

    /// AC-F014-12 (signing case): a git that hangs ends at the timeout with a clear message, and
    /// the stage stays commit pending.
    #[test]
    fn timeout() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let pv = preview(&p, current()).unwrap();
        // After the preview, git becomes a program that never answers commit-tree.
        let bin = p.root.join("slowgit");
        std::fs::create_dir_all(&bin).unwrap();
        let fake = bin.join("git");
        std::fs::write(
            &fake,
            "#!/bin/sh\nfor a in \"$@\"; do [ \"$a\" = commit-tree ] && exec sleep 600; done\nexec /usr/bin/git \"$@\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (mut s, stamp, _) = p.t.app.settings().unwrap();
        s.tools.git.path = Some(fake.display().to_string());
        s.tools.git.source = foreman_core::schema::settings::ToolSource::Settings;
        p.t.app.save_settings(&s, &stamp).unwrap();
        let t0 = std::time::Instant::now();
        let r = apply(&p, &pv).unwrap();
        let SignResult::CommitPending { stderr_tail } = r else {
            panic!("{r:?}")
        };
        assert!(stderr_tail.contains("60 seconds"), "{stderr_tail}");
        assert!(t0.elapsed() < std::time::Duration::from_secs(120));
    }
}

mod decisions {
    use super::*;

    /// Code review W2: the watcher's re-classification (`refresh_decisions`) uses the same
    /// classifier as opening the project: after a signing on another branch and a further own
    /// change, checking that branch out records no false pending review.
    #[test]
    fn watcher_path_other_branch_no_false_drift() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let d = p.project.clone();
        std::fs::write(d.join("README.md"), "base\n").unwrap();
        git(&d, &["add", "README.md"]);
        git(&d, &["commit", "-q", "-m", "base"]);
        let own = |name: &str| {
            let mut o = project::open(&p.t.app, &p.id).unwrap();
            let mut pf = o.project.clone().unwrap();
            pf.name = name.into();
            let bytes = foreman_core::schema::to_file(&pf).unwrap();
            {
                let root = o.root().unwrap();
                let dd = foreman_core::decisions::Decisions {
                    project: root,
                    app: &o.app_root,
                    project_id: &p.id,
                };
                let ch = dd
                    .own_change(
                        DecisionFileKind::Project,
                        bytes.as_bytes(),
                        true,
                        "2026-10-20T00:00:00Z",
                    )
                    .unwrap();
                let foreman_core::decisions::OwnChange::Pending(po) = ch else {
                    panic!()
                };
                o.progress.pending_own.retain(|x| x.file != po.file);
                o.progress.pending_own.push(po);
            }
            o.save_progress(&p.t.app).unwrap();
        };
        own("TradeLog Pro");
        let pv = preview(&p, json!({ "kind": "other", "branch": "handoff/stage-01" })).unwrap();
        let SignResult::Signed { .. } = apply(&p, &pv).unwrap() else {
            panic!()
        };
        // A further own change after the signing: the approved copy moves past the branch's.
        own("TradeLog Max");
        git(&d, &["stash", "-u", "-q"]);
        git(&d, &["checkout", "-q", "handoff/stage-01"]);
        let new = project::refresh_decisions(&p.t.app, &p.id).unwrap();
        assert_eq!(new, 0, "the watcher recorded a false pending review");
        let o = project::open(&p.t.app, &p.id).unwrap();
        assert!(
            o.progress.pending_outside.is_empty(),
            "{:?}",
            o.progress.pending_outside
        );
    }

    /// ADR-022 §5, AC-F014-14: a decision change pending after Stage 5 rides in the next signing
    /// commit; signed on another branch, `committedOnOtherBranches` records it, so checking that
    /// branch out raises no false pending change.
    #[test]
    fn other_branch_sign_no_false_drift() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let d = p.project.clone();
        std::fs::write(d.join("README.md"), "base\n").unwrap();
        git(&d, &["add", "README.md"]);
        git(&d, &["commit", "-q", "-m", "base"]);
        // A pending own change (as from Stage 5 on): approved copy ahead of the repo file.
        let mut o = project::open(&p.t.app, &p.id).unwrap();
        let mut pf = o.project.clone().unwrap();
        pf.name = "TradeLog Pro".into();
        let bytes = foreman_core::schema::to_file(&pf).unwrap();
        {
            let root = o.root().unwrap();
            let dd = foreman_core::decisions::Decisions {
                project: root,
                app: &o.app_root,
                project_id: &p.id,
            };
            let ch = dd
                .own_change(
                    DecisionFileKind::Project,
                    bytes.as_bytes(),
                    true,
                    "2026-10-20T00:00:00Z",
                )
                .unwrap();
            let foreman_core::decisions::OwnChange::Pending(po) = ch else {
                panic!()
            };
            o.progress.pending_own.push(po);
        }
        o.save_progress(&p.t.app).unwrap();
        let repo_before = std::fs::read_to_string(d.join(".foreman/project.json")).unwrap();
        let pv = preview(&p, json!({ "kind": "other", "branch": "handoff/stage-01" })).unwrap();
        assert!(
            pv.paths
                .iter()
                .any(|x| x.as_str() == ".foreman/project.json")
        );
        let SignResult::Signed { commit, .. } = apply(&p, &pv).unwrap() else {
            panic!()
        };
        let committed = git(&d, &["show", &format!("{commit}:.foreman/project.json")]);
        assert!(committed.contains("TradeLog Pro"));
        assert_eq!(
            std::fs::read_to_string(d.join(".foreman/project.json")).unwrap(),
            repo_before
        );
        let o = project::open(&p.t.app, &p.id).unwrap();
        assert_eq!(
            o.approvals.files.project.committed_on_other_branches["refs/heads/handoff/stage-01"],
            foreman_core::store::sha256_hex(bytes.as_bytes())
        );
        assert!(o.progress.pending_outside.is_empty());
        // Check the signed branch out: the repo file is now that branch's version → no drift.
        git(&d, &["stash", "-u", "-q"]);
        git(&d, &["checkout", "-q", "handoff/stage-01"]);
        let o = project::open(&p.t.app, &p.id).unwrap();
        assert!(
            o.progress.pending_outside.is_empty(),
            "{:?}",
            o.progress.pending_outside
        );
        assert!(o.read_only.is_none(), "{:?}", o.read_only);
    }

    /// AC-F014-14: on the current branch, the pending decision change is written and committed
    /// in the signing commit, and stops being pending.
    #[test]
    fn pending_decisions_ride_in_signing_commit() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let mut o = project::open(&p.t.app, &p.id).unwrap();
        let mut pf = o.project.clone().unwrap();
        pf.name = "TradeLog Pro".into();
        let bytes = foreman_core::schema::to_file(&pf).unwrap();
        {
            let root = o.root().unwrap();
            let dd = foreman_core::decisions::Decisions {
                project: root,
                app: &o.app_root,
                project_id: &p.id,
            };
            let foreman_core::decisions::OwnChange::Pending(po) = dd
                .own_change(
                    DecisionFileKind::Project,
                    bytes.as_bytes(),
                    true,
                    "2026-10-20T00:00:00Z",
                )
                .unwrap()
            else {
                panic!()
            };
            o.progress.pending_own.push(po);
        }
        o.save_progress(&p.t.app).unwrap();
        let pv = preview(&p, current()).unwrap();
        let SignResult::Signed { commit, .. } = apply(&p, &pv).unwrap() else {
            panic!()
        };
        let files = git(
            &p.project,
            &["diff-tree", "--no-commit-id", "--name-only", "-r", &commit],
        );
        assert!(files.contains(".foreman/project.json"), "{files}");
        assert_eq!(
            std::fs::read_to_string(p.project.join(".foreman/project.json")).unwrap(),
            bytes
        );
        let o = project::open(&p.t.app, &p.id).unwrap();
        assert!(o.progress.pending_own.is_empty());
        assert!(o.progress.pending_outside.is_empty());
        assert_eq!(
            o.approvals.files.project.committed_sha256,
            Some(foreman_core::store::sha256_hex(bytes.as_bytes()))
        );
    }
}

mod packet {
    use super::*;

    /// SR-47, DR-135 (P2: shown as unverified): Claude writing a signed-form APPROVED BY doesn't
    /// sign; the stage counts as signed only after the native confirmation, and a later change
    /// voids it. Foreman's own signing counts without it.
    #[test]
    fn outside_signature_needs_confirmation() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let path = p.project.join("docs/handoff/stage-01.md");
        let t = std::fs::read_to_string(&path).unwrap().replace(
            "APPROVED BY: <blank>",
            "APPROVED BY: Billy Ribeiro on 2026-10-20",
        );
        std::fs::write(&path, &t).unwrap();
        let v =
            packets::packet_view(&p.t.app, req(json!({ "projectId": p.id, "stage": 1 }))).unwrap();
        let sig = v.signed.unwrap();
        assert!(!sig.confirmed && !sig.by_foreman);
        assert!(
            !project::open(&p.t.app, &p.id)
                .unwrap()
                .signed()
                .contains(&1)
        );
        // Billy confirms in the native dialog.
        let sha = foreman_core::store::sha256_hex(t.as_bytes());
        packets::packet_signature_confirm(
            &p.t.app,
            req(json!({ "projectId": p.id, "stage": 1, "seenSha256": sha })),
        )
        .unwrap();
        assert!(
            project::open(&p.t.app, &p.id)
                .unwrap()
                .signed()
                .contains(&1)
        );
        // A later change voids it.
        std::fs::write(&path, format!("{t}\nEXTRA: x\n")).unwrap();
        assert!(
            !project::open(&p.t.app, &p.id)
                .unwrap()
                .signed()
                .contains(&1)
        );
    }
}

mod ipc {
    use super::*;

    /// SR-24a, SR-47: APPROVED BY is written only after Billy confirms the native sign dialog
    /// (Foreman's text: name, date, "Signing stage-01.md @ <hash>", branch); Cancel writes and
    /// commits nothing; the preview can't carry an answer.
    #[test]
    fn security_confirmations_are_native() {
        let p = ProjectApp::new(json!({}));
        ready(&p);
        let base = try_head(&p);
        let before = std::fs::read_to_string(p.project.join("docs/handoff/stage-01.md")).unwrap();
        let pv = preview(&p, current()).unwrap();
        let asked = p.t.os.asked().len();
        p.t.os.answer_next(false);
        let e = apply(&p, &pv).unwrap_err();
        assert_eq!(e.code, IpcErrorCode::Cancelled);
        let shown = p.t.os.asked();
        assert_eq!(shown.len(), asked + 1);
        let c = shown.last().unwrap();
        assert_eq!(c.title, "Sign Stage 1?");
        assert!(c.message.contains("Billy Ribeiro") && c.message.contains("2026-10-20"));
        assert!(
            c.message
                .contains(&format!("Signing stage-01.md @ {}", pv.short)),
            "{}",
            c.message
        );
        assert_eq!(c.ok, "Sign");
        assert_eq!(
            std::fs::read_to_string(p.project.join("docs/handoff/stage-01.md")).unwrap(),
            before
        );
        assert!(!p.project.join(".foreman/stages/stage-01.json").exists());
        assert_eq!(try_head(&p), base);
        assert!(
            project::open(&p.t.app, &p.id)
                .unwrap()
                .progress
                .signing
                .is_empty()
        );
        // The preview is one-use.
        assert_eq!(apply(&p, &pv).unwrap_err().code, IpcErrorCode::Expired);
        // An extra field (an "approved" flag) is refused by the request type (IPC-R1).
        let bad: Result<foreman_app::ipc::dto::requests::SignApplyReq, _> =
            serde_json::from_value(json!({ "previewId": pv.preview_id, "confirmed": true }));
        assert!(bad.is_err());
        let _ = IpcErrorDetail::NotAllowed {
            rule: String::new(),
        };
    }
}
