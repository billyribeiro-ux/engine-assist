//! The Claude Code version floor (ADR-017, AC-F008-02): the maximum minimum version over the steps'
//! models and flags and the pinned kit's agent models.

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::ids::{Model, Surface};
use crate::policy;
use crate::schema::project::ModelPolicy;
use crate::template::Template;

/// One requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    /// What needs it.
    pub feature: String,
    /// The minimum version.
    pub min: String,
    /// Where the number comes from.
    pub source: String,
}

/// The floor table (MASTER-SPEC §6, ADR-017).
pub const TABLE: [(&str, &str); 8] = [
    ("Fable 5.1", "2.1.257"),
    ("--permission-prompts", "2.1.259"),
    ("Opus 5.5", "2.1.280"),
    ("--agents file form", "2.1.281"),
    ("Sonnet 5.5", "2.1.284"),
    ("--desktop", "2.1.285"),
    ("Haiku 5.5", "2.1.293"),
    ("--ax-screen-reader", "2.1.181"),
];

fn model_feature(m: Model) -> &'static str {
    match m {
        Model::Fable => "Fable 5.1",
        Model::Opus | Model::Opusplan => "Opus 5.5",
        Model::Sonnet => "Sonnet 5.5",
        Model::Haiku => "Haiku 5.5",
    }
}

fn min_of(feature: &str) -> Option<Version> {
    TABLE
        .iter()
        .find(|(f, _)| *f == feature)
        .and_then(|(_, v)| Version::parse(v).ok())
}

/// Parses the leading semver of `claude -v` output (`2.1.211 (Claude Code)`).
#[must_use]
pub fn parse_version_output(out: &str) -> Option<Version> {
    let first = out.split_whitespace().next()?;
    let first = first.trim_start_matches('v');
    Version::parse(first).ok()
}

/// The floor and the requirements behind it. `kit_agents` are the pinned kit's agent models.
#[must_use]
pub fn floor(t: &Template, kit_agents: &[Model], p: &ModelPolicy) -> (Version, Vec<Requirement>) {
    let mut features: Vec<(String, String)> = Vec::new();
    for s in t.steps.iter().filter(|s| s.runnable()) {
        if let Ok(e) = policy::policy_start(s, p) {
            features.push((
                model_feature(e.value.model).into(),
                format!("step {}", s.key),
            ));
        }
        for w in policy::effective_switches(s, p) {
            features.push((
                model_feature(w.to.model).into(),
                format!("step {} switch", s.key),
            ));
        }
        if s.surface == Some(Surface::Desktop) {
            features.push(("--desktop".into(), format!("step {}", s.key)));
        }
    }
    for m in kit_agents {
        features.push((model_feature(*m).into(), "kit agent".into()));
    }
    let mut reqs: Vec<Requirement> = Vec::new();
    let mut max = Version::new(0, 0, 0);
    for (f, src) in features {
        let Some(v) = min_of(&f) else { continue };
        if !reqs.iter().any(|r| r.feature == f) {
            reqs.push(Requirement {
                feature: f.clone(),
                min: v.to_string(),
                source: src,
            });
        }
        if v > max {
            max = v;
        }
    }
    reqs.sort_by(|a, b| a.feature.cmp(&b.feature));
    (max, reqs)
}

/// The minimum version a feature of [`TABLE`] needs (`"--desktop"`, `"Opus 5.5"`, …).
#[must_use]
pub fn feature_min(feature: &str) -> Option<Version> {
    min_of(feature)
}

/// Whether a found version meets the floor.
#[must_use]
pub fn meets(found: &Version, floor: &Version) -> bool {
    found >= floor
}
