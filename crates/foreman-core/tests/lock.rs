//! M1-17: the blueprint lock (DR-155–DR-157). The git side is in foreman-sys `tests/lock_restore.rs`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use std::collections::BTreeMap;

use common::{assert_valid, blueprint, parse, replace_once};
use foreman_core::ids::ItemId;
use foreman_core::lock::{
    self as lk, AnnotationProblem, BlueprintState, Fetched, LockError, ReadOnly, Restored,
};
use foreman_core::schema::approvals::ItemKindConfirm;
use foreman_core::schema::lock::LockFile;
use foreman_core::store::{Root, RootKind, sha256_hex};
use foreman_core::template::source_sha256;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const PATH: &str = "claude-build-blueprint.md";

fn clean_state(src: &str) -> BlueprintState {
    let oid = "1111111111111111111111111111111111111111".to_owned();
    BlueprintState {
        bytes: src.as_bytes().to_vec(),
        commit: COMMIT.into(),
        working_oid: oid.clone(),
        head_oid: Some(oid),
    }
}

struct Dirs {
    project: tempfile::TempDir,
    app: tempfile::TempDir,
}

impl Dirs {
    fn new() -> Self {
        Self {
            project: tempfile::tempdir().unwrap(),
            app: tempfile::tempdir().unwrap(),
        }
    }
    fn project(&self) -> Root {
        Root::open(self.project.path(), RootKind::Project).unwrap()
    }
    fn app(&self) -> Root {
        Root::open_app_data(self.app.path()).unwrap()
    }
    fn pin(&self, src: &str, stage1_signed: bool) -> Result<LockFile, LockError> {
        lk::pin(
            &self.project(),
            &self.app(),
            PATH,
            &clean_state(src),
            stage1_signed,
            "2026-10-20T10:00:00Z",
        )
    }
}

fn files_under(p: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(p).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            for f in files_under(&e.path()) {
                out.push(format!("{}/{f}", e.file_name().to_string_lossy()));
            }
        } else {
            out.push(e.file_name().to_string_lossy().into_owned());
        }
    }
    out.sort();
    out
}

mod lock {
    use super::*;

    /// AC-F002-01 / DR-155.
    #[test]
    fn pin_contents() {
        let d = Dirs::new();
        let src = blueprint();
        let lock = d.pin(&src, false).unwrap();
        let t = parse(&src);
        // The lock file: exactly these fields, valid against the contract schema.
        let text =
            std::fs::read_to_string(d.project.path().join(".foreman/blueprint.lock.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_valid("blueprint-lock", &v);
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "blueprintPath",
                "blueprintRepoCommit",
                "blueprintVersion",
                "grammarVersion",
                "items",
                "pinnedAt",
                "schemaVersion",
                "sourceSha256"
            ]
        );
        assert_eq!(lock.blueprint_repo_commit, COMMIT);
        assert_eq!(lock.blueprint_path, PATH);
        assert_eq!(lock.source_sha256, source_sha256(&src));
        assert_eq!(lock.blueprint_version, "2.2");
        assert_eq!(lock.grammar_version, 1);
        assert_eq!(lock.pinned_at, "2026-10-20T10:00:00Z");
        // {textSha256, kind} per item with an ID.
        let ided: Vec<_> = t.items.iter().filter(|i| i.id.is_some()).collect();
        assert_eq!(lock.items.len(), ided.len());
        let fc = &lock.items[&ItemId::parse("fresh-clone").unwrap()];
        assert_eq!(fc.kind, "attest");
        assert_eq!(
            fc.text_sha256,
            ided.iter()
                .find(|i| i.id.as_deref() == Some("fresh-clone"))
                .unwrap()
                .text_sha256
        );
        // The full text is in app data, byte-identical; the repo holds no copy.
        let copy = std::fs::read(d.app.path().join(lk::text_path(&lock.source_sha256))).unwrap();
        assert_eq!(copy, src.as_bytes());
        assert_eq!(
            files_under(d.project.path()),
            [".foreman/blueprint.lock.json"]
        );
        // CRLF and BOM variants pin to the same hash (normalized source).
        let crlf = format!("\u{feff}{}", src.replace('\n', "\r\n"));
        let d2 = Dirs::new();
        assert_eq!(
            d2.pin(&crlf, false).unwrap().source_sha256,
            lock.source_sha256
        );
        assert_ne!(sha256_hex(crlf.as_bytes()), lock.source_sha256);
    }

    /// AC-F002-03.
    #[test]
    fn repin_window() {
        let d = Dirs::new();
        let src = blueprint();
        d.pin(&src, false).unwrap();
        // Before Stage 1 is signed, re-pinning (to an edited blueprint) is allowed.
        let edited = replace_once(
            &src,
            "The ADR reality check has no CONTRADICTED left open",
            "The ADR reality check has nothing CONTRADICTED left open",
        );
        let again = d.pin(&edited, false).unwrap();
        assert_eq!(again.source_sha256, source_sha256(&edited));
        // After Stage 1 is signed, it's refused and nothing changes.
        let before = std::fs::read(d.project.path().join(".foreman/blueprint.lock.json")).unwrap();
        let e = d.pin(&src, true).unwrap_err();
        assert_eq!(e, LockError::RepinClosed);
        assert!(e.to_string().contains("upgrading the pin is v1.1"));
        assert_eq!(
            std::fs::read(d.project.path().join(".foreman/blueprint.lock.json")).unwrap(),
            before
        );
        // The first pin is never "re-pinning".
        assert!(lk::check_repin(None, true).is_ok());
    }

    /// AC-F002-05, -06, -07 / DR-156 (the read-only reasons; real git in foreman-sys).
    #[test]
    fn hash_mismatch_read_only() {
        let d = Dirs::new();
        let src = blueprint();
        let lock = d.pin(&src, false).unwrap();
        // The app-data copy is used when present.
        let (text, from) = lk::restore(&d.app(), &lock, || panic!("git not needed")).unwrap();
        assert_eq!((text.as_str(), from), (src.as_str(), Restored::AppData));
        // A tampered app-data copy → read-only with the reason.
        let copy = d.app.path().join(lk::text_path(&lock.source_sha256));
        std::fs::write(&copy, format!("{src}\nextra")).unwrap();
        let e = lk::restore(&d.app(), &lock, || panic!("not consulted")).unwrap_err();
        assert!(
            matches!(e, ReadOnly::HashMismatch { ref where_, .. } if where_ == "app data"),
            "{e:?}"
        );
        assert!(e.to_string().contains("read-only"));
        // Missing copy, git returns different text → read-only, nothing stored.
        std::fs::remove_file(&copy).unwrap();
        let e = lk::restore(&d.app(), &lock, || Fetched::Blob(b"other".to_vec())).unwrap_err();
        assert!(matches!(e, ReadOnly::HashMismatch { ref where_, .. } if where_ == "git"));
        assert!(!copy.exists());
        // Repo or commit gone.
        let e = lk::restore(&d.app(), &lock, || Fetched::NotFound).unwrap_err();
        assert_eq!(
            e.to_string(),
            format!(
                "Blueprint 2.2 at {COMMIT} not found. Point Foreman at your blueprint repo or re-pin."
            )
        );
        // git missing names git.
        let e = lk::restore(&d.app(), &lock, || Fetched::GitMissing).unwrap_err();
        assert_eq!(e, ReadOnly::GitMissing);
        assert!(e.to_string().contains("git"));
        // The right blob restores and is stored again.
        let (_, from) =
            lk::restore(&d.app(), &lock, || Fetched::Blob(src.clone().into_bytes())).unwrap();
        assert_eq!(from, Restored::Git);
        assert_eq!(std::fs::read(&copy).unwrap(), src.as_bytes());
    }

    /// AC-F002-02 (the refusal itself; the hashes come from real git in foreman-sys).
    #[test]
    fn dirty_blueprint_refused() {
        let d = Dirs::new();
        let mut st = clean_state(&blueprint());
        st.working_oid = "2222222222222222222222222222222222222222".into();
        let e = lk::pin(&d.project(), &d.app(), PATH, &st, false, "t").unwrap_err();
        assert_eq!(
            e.to_string(),
            format!(
                "{PATH} differs from HEAD (working 2222222222222222222222222222222222222222 vs HEAD 1111111111111111111111111111111111111111). Commit or discard the change, then pin"
            )
        );
        st.head_oid = None;
        assert!(matches!(
            lk::pin(&d.project(), &d.app(), PATH, &st, false, "t"),
            Err(LockError::Dirty { head: None, .. })
        ));
        assert!(files_under(d.project.path()).is_empty() && files_under(d.app.path()).is_empty());
    }

    /// The pin's refusal for annotation errors (CR-003, DR-155, AC-F002-02): the problems it
    /// carries, in line order.
    fn refused(d: &Dirs, src: &str) -> Vec<AnnotationProblem> {
        match d.pin(src, false) {
            Err(LockError::AnnotationErrors { problems }) => problems,
            other => panic!("expected AnnotationErrors, got {other:?}"),
        }
    }

    /// 1-based line of `needle`'s first occurrence in `src`.
    fn line_of(src: &str, needle: &str) -> u32 {
        let at = src.find(needle).unwrap();
        u32::try_from(src[..at].matches('\n').count() + 1).unwrap()
    }

    /// CR-003 (DR-155, AC-F002-02): a pin is refused while a step in the P1 scope (Stages 1–2 and
    /// their F.2) is in error; the diagnostics come back, and nothing is written.
    #[test]
    fn pin_refuses_p1_annotation_errors() {
        let src = blueprint();

        // One error in Stage 2: the message names the stage, the step and the line.
        let d = Dirs::new();
        let bad = replace_once(&src, "<!-- fm:step id=2.5 ", "<!-- fm:step id=2.5 bogus=1 ");
        let line = line_of(&bad, "<!-- fm:step id=2.5 ");
        let problems = refused(&d, &bad);
        assert_eq!(problems.len(), 1, "{problems:#?}");
        let p = &problems[0];
        assert_eq!((p.stage, p.step.as_deref()), (Some(2), Some("2.5")));
        assert_eq!(p.diagnostic.rule, "grammar.unknown-key");
        assert_eq!(p.diagnostic.line, line);
        assert!(p.diagnostic.is_error());
        let e = d.pin(&bad, false).unwrap_err();
        assert_eq!(
            e.to_string(),
            format!(
                "Stage 2 step 2.5 has an annotation error on line {line}: unknown key `bogus` (grammar v1). Fix the blueprint, then pin again."
            )
        );
        // Nothing was written, in the project or in app data.
        assert!(files_under(d.project.path()).is_empty() && files_under(d.app.path()).is_empty());

        // Several errors: all of them come back, in line order, and the message lists each.
        let d = Dirs::new();
        let bad = replace_once(
            &replace_once(
                &src,
                "id=2.3 kind=followup tag=core surface=cli session=continue chain=2.2",
                "id=2.3 kind=followup tag=core surface=cli session=continue chain=2.9",
            ),
            "id=1.2 tag=core session=continue chain=1.1",
            "id=1.2 tag=core session=continue chain=1.9",
        );
        let problems = refused(&d, &bad);
        let seen: Vec<(Option<u8>, &str, &str)> = problems
            .iter()
            .map(|p| {
                (
                    p.stage,
                    p.step.as_deref().unwrap(),
                    p.diagnostic.rule.as_str(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                (Some(1), "1.2", "grammar.bad-chain"),
                (Some(1), "1.2", "grammar.missing-surface"),
                (Some(2), "2.3", "grammar.bad-chain"),
                (Some(2), "2.3", "grammar.missing-cwd"),
            ]
        );
        assert!(
            problems
                .windows(2)
                .all(|w| w[0].diagnostic.line <= w[1].diagnostic.line)
        );
        let text = d.pin(&bad, false).unwrap_err().to_string();
        assert!(
            text.starts_with("4 annotation errors in Stages 1–2 and F.2 stop the pin:\n"),
            "{text}"
        );
        assert!(text.contains("\n- Stage 1 step 1.2, line "), "{text}");
        assert!(text.contains("\n- Stage 2 step 2.3, line "), "{text}");
        assert!(
            text.ends_with("\nFix the blueprint, then pin again."),
            "{text}"
        );

        // F.2 (the lessons step) is in the scope too.
        let d = Dirs::new();
        let bad = replace_once(
            &src,
            "chain=@stage appends=docs/lessons.md",
            "chain=@stage appends=/etc/passwd",
        );
        let problems = refused(&d, &bad);
        assert_eq!(problems.len(), 1);
        assert_eq!(
            (problems[0].stage, problems[0].step.as_deref()),
            (None, Some("F.2"))
        );
        assert!(
            d.pin(&bad, false)
                .unwrap_err()
                .to_string()
                .starts_with("Step F.2 has an annotation error on line ")
        );

        // An error under the Stage Gate templates also makes 1:F.2 and 2:F.2 not runnable
        // (DR-037), so it stops the pin.
        let d = Dirs::new();
        let bad = replace_once(
            &src,
            "id=gate.A tag=core surface=cli",
            "id=gate.A tag=bogus surface=cli",
        );
        let problems = refused(&d, &bad);
        assert_eq!(problems[0].step.as_deref(), Some("gate.A"));
        assert_eq!(problems[0].diagnostic.rule, "grammar.unknown-value");

        // An error that makes every step unusable (no grammar line) stops it too.
        let d = Dirs::new();
        let bad = replace_once(&src, "<!-- fm:grammar v=1 -->\n", "");
        let problems = refused(&d, &bad);
        assert!(
            problems
                .iter()
                .any(|p| p.diagnostic.rule == "grammar.missing-version")
        );

        // A re-pin (allowed before Stage 1 is signed) is refused the same way and leaves the
        // existing lock and stored text alone.
        let d = Dirs::new();
        d.pin(&src, false).unwrap();
        let lock_file = d.project.path().join(".foreman/blueprint.lock.json");
        let (before, stored) = (
            std::fs::read(&lock_file).unwrap(),
            files_under(d.app.path()),
        );
        refused(
            &d,
            &replace_once(&src, "<!-- fm:step id=2.5 ", "<!-- fm:step id=2.5 bogus=1 "),
        );
        assert_eq!(std::fs::read(&lock_file).unwrap(), before);
        assert_eq!(files_under(d.app.path()), stored);

        // The other refusals keep their place in the order: closed re-pin, then dirty, then errors.
        let bad = replace_once(&src, "<!-- fm:step id=2.5 ", "<!-- fm:step id=2.5 bogus=1 ");
        let d = Dirs::new();
        d.pin(&src, false).unwrap();
        assert_eq!(d.pin(&bad, true).unwrap_err(), LockError::RepinClosed);
        let d = Dirs::new();
        let mut st = clean_state(&bad);
        st.working_oid = "2222222222222222222222222222222222222222".into();
        let e = lk::pin(&d.project(), &d.app(), PATH, &st, false, "t").unwrap_err();
        assert!(matches!(e, LockError::Dirty { .. }), "{e:?}");
    }

    /// CR-003: errors that leave every P1 step runnable still pin, and those steps stay not
    /// runnable (DR-037, AC-F001-15).
    #[test]
    fn pin_allows_errors_outside_p1() {
        let src = blueprint();
        let bad = replace_once(
            &replace_once(
                &replace_once(&src, "<!-- fm:step id=0.1 ", "<!-- fm:step id=0.1 bogus=1 "),
                "<!-- fm:step id=3.1 ",
                "<!-- fm:step id=3.1 bogus=1 ",
            ),
            "<!-- fm:step id=4.3 tag=core surface=chat session=new",
            "<!-- fm:step id=4.3 tag=core surface=chat session=continue chain=4.9",
        );
        let t = parse(&bad);
        let error_stages: std::collections::BTreeSet<u8> =
            t.errors().filter_map(|e| e.stage).collect();
        assert_eq!(error_stages, [0, 3, 4].into());
        let d = Dirs::new();
        let lock = d.pin(&bad, false).unwrap();
        assert_eq!(lock.source_sha256, source_sha256(&bad));
        assert!(
            d.project
                .path()
                .join(".foreman/blueprint.lock.json")
                .exists()
        );
        // The steps in error are listed and not runnable; the P1 steps are fine.
        for n in [0u8, 3, 4] {
            assert!(
                t.stage_steps(n)
                    .filter(|s| s.annotation.is_some())
                    .all(|s| !s.runnable()),
                "stage {n}"
            );
        }
        for n in [1u8, 2] {
            assert!(
                t.stage_steps(n)
                    .filter(|s| s.annotation.is_some())
                    .all(|s| s.runnable()),
                "stage {n}"
            );
        }
    }

    /// CR-003: a clean P1 pins; warnings (lints) in it never refuse.
    #[test]
    fn pin_clean_p1() {
        let src = blueprint();
        let t = parse(&src);
        assert_eq!(t.errors().count(), 0);
        assert!(
            t.diagnostics.iter().any(|d| !d.is_error()),
            "the fixture carries lint warnings"
        );
        let d = Dirs::new();
        let lock = d.pin(&src, false).unwrap();
        assert_eq!(lock.source_sha256, source_sha256(&src));
        assert_eq!(
            files_under(d.project.path()),
            [".foreman/blueprint.lock.json"]
        );
    }
}

mod template {
    use super::*;

    /// AC-F001-16 / DR-157: swapping two bullets without moving their IDs drifts both.
    #[test]
    fn swapped_bullets_reconfirm() {
        let src = blueprint();
        let lock = lk::lock_for(&parse(&src), COMMIT, PATH, "t");
        let a = "> - A fresh clone, then `install` and `dev`, works with zero manual steps\n";
        let b = "> - CI is green, and an intentional type error, a planted fake secret, and a known-vulnerable dependency each fail it (prove the gates work, then revert)\n";
        let swapped = replace_once(&src, &format!("{a}{b}"), &format!("{b}{a}"));
        let t = parse(&swapped);
        let none = BTreeMap::new();
        assert!(lk::drift(&parse(&src), &lock, &none).is_empty());
        let d = lk::drift(&t, &lock, &none);
        let ids: Vec<&str> = d.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["fresh-clone", "ci-gates"]);
        // Confirming one (native dialog → approvals itemKinds) clears only that one.
        let mut confirmed = BTreeMap::new();
        confirmed.insert(
            ItemId::parse("fresh-clone").unwrap(),
            ItemKindConfirm {
                text_sha256: d[0].current.clone(),
                kind: "attest".into(),
                at: "t".into(),
            },
        );
        let ids: Vec<String> = lk::drift(&t, &lock, &confirmed)
            .into_iter()
            .map(|x| x.id.as_str().to_owned())
            .collect();
        assert_eq!(ids, ["ci-gates"]);
        // A confirmation for an older text doesn't carry over to a new rewording.
        // fresh-clone now holds the CI bullet; rewording it voids that confirmation.
        let reworded = replace_once(
            &swapped,
            "(prove the gates work, then revert)",
            "(prove that the gates work, then revert)",
        );
        assert_eq!(lk::drift(&parse(&reworded), &lock, &confirmed).len(), 2);
    }
}
