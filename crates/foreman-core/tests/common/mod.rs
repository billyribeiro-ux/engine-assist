//! Shared helpers for the core's integration tests.
#![allow(dead_code, clippy::unwrap_used, clippy::panic, clippy::disallowed_methods, reason = "shared test helpers; each test binary uses a subset")]

use std::path::{Path, PathBuf};

/// The crate's `tests/` directory.
pub fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

/// Reads a file under `tests/`.
pub fn read_fixture(rel: &str) -> String {
    std::fs::read_to_string(tests_dir().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The repository root.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A JSON Schema validator for one file kind, with `urn:foreman:common` registered.
pub fn schema_validator(kind: &str) -> jsonschema::Validator {
    let common: serde_json::Value =
        serde_json::from_str(&read_fixture("schema/common.schema.json")).unwrap();
    let schema: serde_json::Value =
        serde_json::from_str(&read_fixture(&format!("schema/{kind}.schema.json"))).unwrap();
    let registry = jsonschema::Registry::new()
        .add("urn:foreman:common", common)
        .unwrap()
        .prepare()
        .unwrap();
    jsonschema::options()
        .with_registry(&registry)
        .build(&schema)
        .unwrap()
}

/// Panics with every schema error.
pub fn assert_valid(kind: &str, v: &serde_json::Value) {
    let val = schema_validator(kind);
    let errs: Vec<String> = val.iter_errors(v).map(|e| format!("{e} at {}", e.instance_path())).collect();
    assert!(errs.is_empty(), "{kind}: {errs:#?}");
}
