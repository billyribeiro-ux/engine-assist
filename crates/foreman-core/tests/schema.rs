//! M1-02 / M1-29: schema versioning (ADR-023) and the app-data file contracts (files.md §2–§4).
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

mod schema {
    use foreman_core::schema::{
        self, Header, Loaded, Migration, MigrationError, Versioned, approvals::ApprovalsFile,
        fingerprint::FingerprintCacheFile, lock::LockFile, overlay::OverlayFile,
        progress::ProgressFile, project::ProjectFile, projects::ProjectsFile, run::RunFile,
        run::RunIndexLine, run_settings::RunSettingsFile, sensor_event::SensorEventFile,
        settings::SettingsFile, stage_record::StageRecordFile,
    };
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};

    use super::common;

    fn golden<T: Versioned + std::fmt::Debug>() {
        let text = common::read_fixture(&format!("schema/{}/v1.json", T::KIND));
        let raw: Value = serde_json::from_str(&text).unwrap();
        common::assert_valid(T::KIND, &raw);
        let loaded = schema::load::<T>(text.as_bytes()).unwrap();
        let value = match loaded {
            Loaded::Current(v) => v,
            other => panic!("{}: {other:?}", T::KIND),
        };
        // FILE-R1: the parse–write round trip is byte-identical.
        let written = schema::to_file(&value).unwrap();
        assert_eq!(written, text, "{} round trip", T::KIND);
        insta::assert_snapshot!(format!("current_{}", T::KIND.replace('-', "_")), written);
    }

    /// One case per file (M1-29), each validated against its JSON Schema from files.md and migrated
    /// to the current form (all kinds are at v1, so the chain is the identity; the synthetic kind
    /// below exercises a real chain).
    #[test]
    fn all_versions_migrate() {
        golden::<ProjectFile>();
        golden::<OverlayFile>();
        golden::<LockFile>();
        golden::<StageRecordFile>();
        golden::<SettingsFile>();
        golden::<ProjectsFile>();
        golden::<ProgressFile>();
        golden::<ApprovalsFile>();
        golden::<RunFile>();
        golden::<RunIndexLine>();
        golden::<SensorEventFile>();
        golden::<FingerprintCacheFile>();
        // The run settings file is in Claude Code's format: schema-valid, no schemaVersion.
        let rs = common::read_fixture("schema/run-settings/v1.json");
        let v: Value = serde_json::from_str(&rs).unwrap();
        common::assert_valid("run-settings", &v);
        let parsed: RunSettingsFile = serde_json::from_value(v).unwrap();
        assert_eq!(foreman_core::json::to_canonical(&parsed).unwrap(), rs);

        // A real chain: v1 → v2 → v3.
        let l = schema::migrate::<Synthetic>(json!({"schemaVersion": 1, "a": 5})).unwrap();
        assert_eq!(
            l,
            Loaded::Migrated {
                value: Synthetic {
                    schema_version: 3,
                    b: 5,
                    c: true
                },
                from: 1
            }
        );
        let l = schema::migrate::<Synthetic>(json!({"schemaVersion": 2, "b": 7})).unwrap();
        assert!(matches!(l, Loaded::Migrated { from: 2, .. }));
        let l =
            schema::migrate::<Synthetic>(json!({"schemaVersion": 3, "b": 1, "c": false})).unwrap();
        assert!(matches!(l, Loaded::Current(_)));
        // deny_unknown_fields after migrating.
        assert!(matches!(
            schema::migrate::<Synthetic>(json!({"schemaVersion": 3, "b": 1, "c": false, "x": 1})),
            Err(MigrationError::Invalid { .. })
        ));
        assert_eq!(
            schema::migrate::<Synthetic>(json!({"b": 1})),
            Err(MigrationError::MissingVersion)
        );
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Synthetic {
        schema_version: u32,
        b: u32,
        c: bool,
    }

    fn v1_to_v2(mut v: Value) -> Result<Value, MigrationError> {
        let o = v.as_object_mut().unwrap();
        let a = o.remove("a").unwrap_or(Value::from(0));
        o.insert("b".into(), a);
        Ok(v)
    }

    fn v2_to_v3(mut v: Value) -> Result<Value, MigrationError> {
        v.as_object_mut()
            .unwrap()
            .insert("c".into(), Value::Bool(true));
        Ok(v)
    }

    impl Versioned for Synthetic {
        const KIND: &'static str = "synthetic";
        const CURRENT: u32 = 3;
        const MIGRATIONS: &'static [Migration] = &[v1_to_v2, v2_to_v3];
    }

    #[test]
    fn newer_version_read_only() {
        let mut v: Value =
            serde_json::from_str(&common::read_fixture("schema/project/v1.json")).unwrap();
        v["schemaVersion"] = json!(2);
        let l = schema::migrate::<ProjectFile>(v).unwrap();
        assert_eq!(
            l.read_only_message().as_deref(),
            Some("Created by a newer Foreman (schema 2). Update Foreman to edit this project.")
        );
        assert!(l.value().is_none());
    }

    #[test]
    fn older_build_header_read_banner() {
        // A v5 project.json with fields this build has never heard of.
        let text = r#"{"schemaVersion": 5, "projectId": "3f6a1c2e-8b4d-4e7f-9a1b-2c3d4e5f6a7b",
            "name": "TradeLog", "verifyCommands": [{"id": "x"}], "conditions": {"brandNew": "maybe"}}"#;
        let h = Header::read(text.as_bytes()).unwrap();
        assert_eq!(h.schema_version, 5);
        assert_eq!(
            h.project_id.as_deref(),
            Some("3f6a1c2e-8b4d-4e7f-9a1b-2c3d4e5f6a7b")
        );
        assert_eq!(h.name.as_deref(), Some("TradeLog"));
        let l = schema::load::<ProjectFile>(text.as_bytes()).unwrap();
        assert_eq!(l, Loaded::NewerReadOnly(h));
        assert!(l.read_only_message().unwrap().contains("schema 5"));
    }

    #[test]
    fn read_does_not_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("synthetic.json");
        let old = "{\n  \"a\": 2,\n  \"schemaVersion\": 1\n}\n";
        std::fs::write(&path, old).unwrap();
        let before = std::fs::metadata(&path).unwrap();
        let root =
            foreman_core::store::Root::open(dir.path(), foreman_core::store::RootKind::AppData)
                .unwrap();
        let p = foreman_core::pathpolicy::check_app_data("synthetic.json").unwrap();
        let (bytes, _) = root
            .read(&p, foreman_core::pathpolicy::ReadClass::Decision)
            .unwrap();
        let loaded = schema::load::<Synthetic>(&bytes).unwrap();
        assert!(matches!(loaded, Loaded::Migrated { from: 1, .. }));
        let after = std::fs::metadata(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), old);
        assert_eq!(before.modified().unwrap(), after.modified().unwrap());
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no temp or backup files"
        );
    }

    /// The schema files under tests/schema equal the JSON Schemas in docs/contracts/files.md.
    #[test]
    fn schemas_match_contract() {
        let md =
            std::fs::read_to_string(common::repo_root().join("docs/contracts/files.md")).unwrap();
        let mut n = 0;
        for block in md.split("```json\n").skip(1) {
            let body = block.split("```").next().unwrap();
            let cleaned: String = body
                .lines()
                .map(|l| match l.find(",   ") {
                    Some(i) if l[i..].contains("//") => &l[..=i],
                    _ => l,
                })
                .collect::<Vec<_>>()
                .join("\n");
            let v: Value = serde_json::from_str(&cleaned).unwrap();
            let id = v["$id"].as_str().unwrap();
            let name = id.rsplit(':').next().unwrap().trim_end_matches(".v1");
            let file: Value =
                serde_json::from_str(&common::read_fixture(&format!("schema/{name}.schema.json")))
                    .unwrap();
            assert_eq!(v, file, "{name}");
            n += 1;
        }
        assert_eq!(n, 14);
    }
}
