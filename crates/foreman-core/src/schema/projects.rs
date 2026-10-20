//! `projects.json` v1 · class (f) (`docs/contracts/files.md` §4.2).

use serde::{Deserialize, Serialize};

use super::{Migration, Versioned};

/// One registered project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEntry {
    /// Equals `.foreman/project.json` `projectId`.
    pub project_id: String,
    /// Display name.
    pub name: String,
    /// Canonical absolute root.
    pub path: String,
    /// RFC 3339 UTC.
    pub added_at: String,
    /// Sidebar pin 1–9.
    pub pinned: Option<u8>,
}

/// `projects.json` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectsFile {
    /// Always 1.
    #[serde(deserialize_with = "super::version_const::check::<_, 1>")]
    pub schema_version: u32,
    /// Registered projects (sorted by `projectId`, FILE-R1).
    pub projects: Vec<ProjectEntry>,
}

impl Versioned for ProjectsFile {
    const KIND: &'static str = "projects";
    const CURRENT: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}
