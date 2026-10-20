//! Shared helpers for the core's integration tests.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "shared test helpers; each test binary uses a subset"
)]

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
    let errs: Vec<String> = val
        .iter_errors(v)
        .map(|e| format!("{e} at {}", e.instance_path()))
        .collect();
    assert!(errs.is_empty(), "{kind}: {errs:#?}");
}

/// The annotated v2.2 blueprint fixture (M1-01).
pub fn blueprint() -> String {
    read_fixture("fixtures/blueprint/claude-build-blueprint-v2.2.md")
}

/// The v2.1 baseline from docs/spec-input (never edited).
pub fn blueprint_v21() -> String {
    std::fs::read_to_string(repo_root().join("docs/spec-input/claude-build-blueprint.md")).unwrap()
}

/// Parses text as a template.
pub fn parse(s: &str) -> foreman_core::template::Template {
    foreman_core::template::parse_template(s)
}

/// The rule codes of a template's diagnostics.
pub fn rules(t: &foreman_core::template::Template) -> Vec<String> {
    t.diagnostics.iter().map(|d| d.rule.clone()).collect()
}

/// A minimal annotated blueprint: title, grammar line, then `body`.
pub fn mini(body: &str) -> String {
    format!("# Mini\n<!-- fm:grammar v=1 -->\n\nVersion 2.2 · test\n\n{body}")
}

/// Replaces exactly one occurrence (panics otherwise) — for fixture edits of the real blueprint.
pub fn replace_once(s: &str, old: &str, new: &str) -> String {
    assert_eq!(s.matches(old).count(), 1, "anchor {old:?}");
    s.replacen(old, new, 1)
}

/// The TradeLog harness project (all conditions yes, the blueprint's example variables with a
/// concrete BUDGET_TIME, DR-083).
pub fn tradelog_project() -> foreman_core::schema::project::ProjectFile {
    serde_json::from_str(&read_fixture("fixtures/tradelog/project.json")).unwrap()
}

/// The synthetic signed packets for Stages 1–4.
pub fn tradelog_packets() -> std::collections::BTreeMap<u8, foreman_core::resolver::PacketText> {
    (1..=4u8)
        .map(|n| {
            let text = read_fixture(&format!("fixtures/tradelog/handoff/stage-{n:02}.md"));
            (n, foreman_core::resolver::PacketText { text, signed: true })
        })
        .collect()
}

/// Owned inputs for a `ResolveCtx`.
pub struct Fixture {
    pub template: foreman_core::template::Template,
    pub project: foreman_core::schema::project::ProjectFile,
    pub path: foreman_core::rules::ApplicablePath,
    pub packets: std::collections::BTreeMap<u8, foreman_core::resolver::PacketText>,
    pub inputs: std::collections::BTreeMap<String, String>,
    pub remembered: std::collections::BTreeMap<String, String>,
    pub optional: std::collections::BTreeMap<String, bool>,
    pub attested_na: std::collections::BTreeSet<String>,
    pub overlay_items: Vec<foreman_core::schema::overlay::AddedItem>,
}

impl Fixture {
    /// TradeLog on the real v2.2 blueprint.
    pub fn tradelog() -> Self {
        let template = parse(&blueprint());
        let project = tradelog_project();
        Self::with(template, project)
    }

    /// Any template and project.
    pub fn with(
        template: foreman_core::template::Template,
        project: foreman_core::schema::project::ProjectFile,
    ) -> Self {
        let path = foreman_core::rules::applicable_path(
            &template,
            &project.conditions,
            &foreman_core::schema::overlay::OverlayFile::empty(),
        );
        Self {
            template,
            project,
            path,
            packets: tradelog_packets(),
            inputs: std::collections::BTreeMap::new(),
            remembered: std::collections::BTreeMap::new(),
            optional: std::collections::BTreeMap::new(),
            attested_na: std::collections::BTreeSet::new(),
            overlay_items: Vec::new(),
        }
    }

    /// Recomputes the path after a condition change.
    pub fn repath(&mut self) {
        self.path = foreman_core::rules::applicable_path(
            &self.template,
            &self.project.conditions,
            &foreman_core::schema::overlay::OverlayFile::empty(),
        );
    }

    /// The context for a step.
    pub fn ctx<'a>(&'a self, key: &str) -> foreman_core::resolver::ResolveCtx<'a> {
        foreman_core::resolver::ResolveCtx {
            template: &self.template,
            step: self
                .template
                .step(key)
                .unwrap_or_else(|| panic!("step {key}")),
            project: &self.project,
            path: &self.path,
            packets: &self.packets,
            inputs: &self.inputs,
            remembered: &self.remembered,
            optional: &self.optional,
            attested_na: &self.attested_na,
            overlay_items: &self.overlay_items,
        }
    }

    /// Resolves a step or panics with what's missing.
    pub fn resolve(&self, key: &str) -> foreman_core::resolver::ResolvedPrompt {
        foreman_core::resolver::resolve(&self.ctx(key)).unwrap_or_else(|e| panic!("{key}: {e:?}"))
    }
}
