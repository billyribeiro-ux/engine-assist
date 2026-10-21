//! `.foreman/blueprint.lock.json` v1 (`docs/contracts/files.md` §3.3). Every field is a decision field.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Migration, Versioned};
use crate::ids::ItemId;

/// One pinned inspection item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LockedItem {
    /// SHA-256 of the normalized bullet text (DR-014).
    pub text_sha256: String,
    /// The item kind as written (`attest`, `time:7d>attest`, `auto:cmd=x`).
    pub kind: String,
}

/// `.foreman/blueprint.lock.json` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LockFile {
    /// Always 1.
    #[serde(deserialize_with = "super::version_const::check::<_, 1>")]
    pub schema_version: u32,
    /// The blueprint repo commit.
    pub blueprint_repo_commit: String,
    /// The blueprint path in that repo.
    pub blueprint_path: String,
    /// Normalized source hash (DR-001).
    pub source_sha256: String,
    /// `2.2`.
    pub blueprint_version: String,
    /// `fm:grammar v=`.
    pub grammar_version: u32,
    /// RFC 3339 UTC.
    pub pinned_at: String,
    /// Every template inspection item with an ID.
    pub items: BTreeMap<ItemId, LockedItem>,
}

impl Versioned for LockFile {
    const KIND: &'static str = "blueprint-lock";
    const CURRENT: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}
