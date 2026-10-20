//! M1-27 / M1-28: approved copies, outside-change classification, Foreman's own changes
//! (ADR-022, DR-150–DR-153). The commit side is in foreman-sys `tests/decision_commit.rs`.
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

use common::{read_fixture, replace_once};
use foreman_core::decisions::{
    Classified, Decisions, OwnChange, Processed, approved_path, classify_outside_change,
};
use foreman_core::ids::{ConditionId, StepKey, TriState};
use foreman_core::schema::progress::{ChangeKind, DecisionFileKind as K};
use foreman_core::store::{Root, RootKind, sha256_hex};

const ID: &str = "3f6a1c2e-8b4d-4e7f-9a1b-2c3d4e5f6a7b";

fn fixture(k: K) -> String {
    let name = match k {
        K::Project => "project",
        K::Overlay => "overlay",
        K::BlueprintLock => "blueprint-lock",
    };
    read_fixture(&format!("schema/{name}/v1.json"))
}

struct Env {
    project_dir: tempfile::TempDir,
    app_dir: tempfile::TempDir,
    project: Root,
    app: Root,
}

impl Env {
    /// Every decision file written in the repo and approved (as setup leaves it).
    fn new() -> Self {
        let project_dir = tempfile::tempdir().unwrap();
        let app_dir = tempfile::tempdir().unwrap();
        let project = Root::open(project_dir.path(), RootKind::Project).unwrap();
        let app = Root::open_app_data(app_dir.path()).unwrap();
        let e = Self { project_dir, app_dir, project, app };
        for k in K::ALL {
            e.put(k, &fixture(k));
            e.d().approve(k, fixture(k).as_bytes(), true).unwrap();
        }
        e
    }
    fn d(&self) -> Decisions<'_> {
        Decisions { project: &self.project, app: &self.app, project_id: ID }
    }
    /// An outside edit (Claude, an editor).
    fn put(&self, k: K, text: &str) {
        let p = self.project_dir.path().join(k.repo_path());
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    fn on_disk(&self, k: K) -> String {
        std::fs::read_to_string(self.project_dir.path().join(k.repo_path())).unwrap()
    }
    fn approved(&self, k: K) -> String {
        std::fs::read_to_string(self.app_dir.path().join(approved_path(ID, k))).unwrap()
    }
}

mod decisions {
    use super::*;

    /// AC-F022-01 / DR-150: one case per file. The repo edit is pending and has no effect.
    #[test]
    fn outside_edit_has_no_effect_until_accepted() {
        let cases = [
            (K::Project, "\"commercial\": \"no\"", "\"commercial\": \"yes\"", "conditions.commercial"),
            (K::Project, "\"plan\": \"max\"", "\"plan\": \"pro\"", "modelPolicy.plan"),
            (K::Overlay, "\"reason\": \"idea already chosen\"", "\"reason\": \"skip it\"", "skips.0.1.reason"),
            (K::Overlay, "\"kind\": \"attest\",\n      \"stage\": 5", "\"kind\": \"time:7d\",\n      \"stage\": 5", "addedItems"),
            (K::BlueprintLock, "\"kind\": \"attest\"", "\"kind\": \"time:1d\"", "items.fresh-clone.kind"),
            (K::BlueprintLock, "\"blueprintVersion\": \"2.2\"", "\"blueprintVersion\": \"2.3\"", "blueprintVersion"),
        ];
        for (k, from, to, key) in cases {
            let e = Env::new();
            let before_project = e.d().effective_project().unwrap();
            let before_overlay = e.d().effective_overlay().unwrap();
            let before_lock = e.d().effective_lock().unwrap();
            let edited = replace_once(&fixture(k), from, to);
            e.put(k, &edited);
            let p = e.d().process(k, "chg-1", "2026-10-20T10:00:00Z").unwrap();
            let Processed::Pending(pending) = p else { panic!("{k:?} {key}: {p:?}") };
            assert_eq!(pending.file, k);
            assert_eq!(pending.seen_sha256, sha256_hex(edited.as_bytes()));
            assert_eq!(pending.diff.len(), 1, "{:?}", pending.diff);
            assert_eq!(pending.diff[0].key_path, key);
            assert_eq!(pending.diff[0].change, ChangeKind::ValueChanged);
            // The effective configuration is still the approved one.
            assert_eq!(e.d().effective_project().unwrap(), before_project);
            assert_eq!(e.d().effective_overlay().unwrap(), before_overlay);
            assert_eq!(e.d().effective_lock().unwrap(), before_lock);
            assert_eq!(e.approved(k), fixture(k));
            // Nothing was written back to the repo file.
            assert_eq!(e.on_disk(k), edited);
            // Accepting (after the native dialog) makes it effective.
            e.d().approve(k, edited.as_bytes(), false).unwrap();
            assert_eq!(e.d().process(k, "chg-2", "t").unwrap(), Processed::Unchanged);
        }
        // Concretely: commercial stays `no` until accepted.
        let e = Env::new();
        e.put(K::Project, &replace_once(&fixture(K::Project), "\"commercial\": \"no\"", "\"commercial\": \"yes\""));
        assert!(matches!(e.d().process(K::Project, "c", "t").unwrap(), Processed::Pending(_)));
        assert_eq!(e.d().effective_project().unwrap().conditions[&ConditionId::Commercial], TriState::No);
        // A skip added from outside doesn't skip anything.
        e.put(K::Overlay, &replace_once(&fixture(K::Overlay), "\"skips\": {", "\"skips\": {\n    \"0.2\": {\"at\": \"t\", \"reason\": \"x\"},"));
        assert!(matches!(e.d().process(K::Overlay, "c", "t").unwrap(), Processed::Pending(_)));
        assert!(!e.d().effective_overlay().unwrap().skips.contains_key(&StepKey::parse("0.2").unwrap()));
    }

    /// AC-F022-03 / DR-151, plus the read-only cases (AC-F020-05).
    #[test]
    fn variables_only_autoaccept() {
        let e = Env::new();
        let edited = replace_once(&fixture(K::Project), "10 hours a week", "12 hours a week");
        let edited = replace_once(&edited, "https://claude.ai/chat/abc", "https://claude.ai/chat/def");
        let edited = replace_once(&edited, "\"https://claude.ai/project/0199eeb7\"", "\"https://claude.ai/project/0199eeb8\"");
        assert_eq!(classify_outside_change(K::Project, fixture(K::Project).as_bytes(), edited.as_bytes()), Classified::VariablesOnly);
        e.put(K::Project, &edited);
        assert_eq!(e.d().process(K::Project, "c", "t").unwrap(), Processed::VariablesReloaded);
        assert_eq!(e.approved(K::Project), edited);
        assert_eq!(e.d().effective_project().unwrap().variables["BUDGET_TIME"], "$0 cash; 12 hours a week");
        let (a, _) = e.d().approvals().unwrap();
        assert_eq!(a.files.project.approved_sha256.as_deref(), Some(sha256_hex(edited.as_bytes()).as_str()));
        assert_eq!(e.d().process(K::Project, "c", "t").unwrap(), Processed::Unchanged);
        // A new variable is a variables-only change too.
        let added = replace_once(&edited, "\"APP_NAME\": \"Foreman\",", "\"APP_NAME\": \"Foreman\",\n    \"NEW\": \"x\",");
        assert_eq!(classify_outside_change(K::Project, edited.as_bytes(), added.as_bytes()), Classified::VariablesOnly);
        // name and slug are decision fields; so is a mix.
        for (from, to) in [("\"name\": \"Foreman\"", "\"name\": \"Other\""), ("\"slug\": \"foreman\"", "\"slug\": \"other\"")] {
            let c = classify_outside_change(K::Project, edited.as_bytes(), replace_once(&edited, from, to).as_bytes());
            assert!(matches!(c, Classified::Pending(_)), "{c:?}");
        }
        let mixed = replace_once(&added, "\"commercial\": \"no\"", "\"commercial\": \"yes\"");
        let Classified::Pending(d) = classify_outside_change(K::Project, edited.as_bytes(), mixed.as_bytes()) else { panic!() };
        assert_eq!(d.iter().map(|x| x.key_path.as_str()).collect::<Vec<_>>(), ["conditions.commercial", "variables.NEW"]);
        assert_eq!(d[1].change, ChangeKind::Added);
        // Overlay and lock never auto-accept.
        let ov = fixture(K::Overlay).replace("\"schemaVersion\": 1,", "\"schemaVersion\": 1,\n  \"variables\": {},");
        assert!(!matches!(classify_outside_change(K::Overlay, fixture(K::Overlay).as_bytes(), ov.as_bytes()), Classified::VariablesOnly));
        // Unparsable → read-only; nothing is overwritten.
        e.put(K::Overlay, "{ not json");
        let p = e.d().process(K::Overlay, "c", "t").unwrap();
        assert!(matches!(p, Processed::ReadOnly(ref r) if r.starts_with(".foreman/overlay.json")), "{p:?}");
        assert_eq!(e.on_disk(K::Overlay), "{ not json");
        // A newer schema → read-only with the newer-Foreman message.
        e.put(K::BlueprintLock, &fixture(K::BlueprintLock).replace("\"schemaVersion\": 1", "\"schemaVersion\": 9"));
        let p = e.d().process(K::BlueprintLock, "c", "t").unwrap();
        assert!(matches!(p, Processed::ReadOnly(ref r) if r.contains("Created by a newer Foreman (schema 9)")), "{p:?}");
        // Secrets in a diff are redacted (SR-55).
        let diff = foreman_core::decisions::diff(
            &serde_json::json!({"env": {"KEY": "a"}, "x": {"apiKey": "b"}, "plain": 1}),
            &serde_json::json!({"env": {"KEY": "c"}, "x": {"apiKey": "d"}, "plain": 2}),
        );
        assert!(diff[0].after.as_deref().unwrap().starts_with("‹redacted sha256:"));
        assert_eq!(diff[1].after.as_deref(), Some("2"));
        assert!(diff[2].before.as_deref().unwrap().starts_with("‹redacted sha256:"));
        assert!(diff.iter().all(|x| !x.after.as_deref().unwrap_or_default().contains("\"d\"")));
    }

    /// AC-F020-08, -09 / DR-153 (the store side): before Stage 5 the repo file is written for an
    /// immediate commit; from Stage 5 on the change waits, and the drift check doesn't flag it.
    #[test]
    fn pending_from_stage5_not_flagged() {
        let e = Env::new();
        let changed = replace_once(&fixture(K::Project), "\"payments\": \"no\"", "\"payments\": \"yes\"");
        // Before Stage 5 is signed.
        let o = e.d().own_change(K::Project, changed.as_bytes(), false, "t").unwrap();
        assert_eq!(o, OwnChange::CommitNow { path: ".foreman/project.json".into(), sha256: sha256_hex(changed.as_bytes()) });
        assert_eq!((e.on_disk(K::Project), e.approved(K::Project)), (changed.clone(), changed.clone()));
        assert_eq!(e.d().process(K::Project, "c", "t").unwrap(), Processed::Unchanged);
        e.d().mark_committed(K::Project, &sha256_hex(changed.as_bytes())).unwrap();
        // From Stage 5 on: only the approved copy changes; the repo file keeps the committed version.
        let later = replace_once(&changed, "\"public\": \"no\"", "\"public\": \"yes\"");
        let o = e.d().own_change(K::Project, later.as_bytes(), true, "2026-10-21T00:00:00Z").unwrap();
        let OwnChange::Pending(p) = o else { panic!("{o:?}") };
        assert_eq!((p.file, p.sha256.as_str()), (K::Project, sha256_hex(later.as_bytes()).as_str()));
        assert_eq!(e.on_disk(K::Project), changed);
        assert_eq!(e.approved(K::Project), later);
        let (a, _) = e.d().approvals().unwrap();
        assert_eq!(a.files.project.committed_sha256.as_deref(), Some(sha256_hex(changed.as_bytes()).as_str()));
        assert_eq!(a.files.project.approved_sha256.as_deref(), Some(sha256_hex(later.as_bytes()).as_str()));
        // Not flagged as an outside change, and the effective config is Foreman's change.
        assert_eq!(e.d().process(K::Project, "c", "t").unwrap(), Processed::Unchanged);
        assert_eq!(e.d().effective_project().unwrap().conditions[&ConditionId::Public], TriState::Yes);
        // A real outside edit meanwhile is still flagged.
        e.put(K::Project, &replace_once(&changed, "\"server\": \"no\"", "\"server\": \"yes\""));
        assert!(matches!(e.d().process(K::Project, "c", "t").unwrap(), Processed::Pending(_)));
        // An own change never writes over an outside edit waiting for review.
        let e = Env::new();
        let outside = fixture(K::Overlay).replace("idea already chosen", "edited outside");
        e.put(K::Overlay, &outside);
        let new_overlay = fixture(K::Overlay).replace("idea already chosen", "chosen");
        let err = e.d().own_change(K::Overlay, new_overlay.as_bytes(), false, "t").unwrap_err();
        assert!(err.to_string().contains("waiting for review"), "{err}");
        assert_eq!((e.on_disk(K::Overlay), e.approved(K::Overlay)), (outside, fixture(K::Overlay)));
    }
}
