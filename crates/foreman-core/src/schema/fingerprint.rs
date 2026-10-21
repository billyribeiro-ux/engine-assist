//! `cache/fingerprints/<cwdHash>.json` v1 · class (e) (`docs/contracts/files.md` §4.9).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Migration, Versioned};

/// One redacted source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotEntry {
    /// Source kind.
    pub source: String,
    /// Path.
    pub path: String,
    /// Hash.
    pub sha256: String,
    /// Redacted JSON or null.
    pub redacted: Value,
}

/// The acknowledged fingerprint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Acked {
    /// Fingerprint.
    pub fingerprint: String,
    /// RFC 3339 UTC.
    pub at: String,
    /// Snapshot.
    pub snapshot: Vec<SnapshotEntry>,
}

/// One stat-cache entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatEntry {
    /// Path.
    pub path: String,
    /// Device.
    pub dev: u64,
    /// Inode.
    pub ino: u64,
    /// Size.
    pub size: u64,
    /// Modification time (ns).
    pub mtime_ns: i64,
    /// Hash.
    pub sha256: String,
}

/// The auth cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthCache {
    /// Fingerprint.
    pub fingerprint: String,
    /// `claude -v`.
    pub claude_version: String,
    /// Env key hash.
    pub env_key: String,
    /// Auth method.
    pub auth_method: Option<String>,
    /// Exit code.
    pub exit: i32,
    /// RFC 3339 UTC.
    pub at: String,
}

/// `cache/fingerprints/<cwdHash>.json` v1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FingerprintCacheFile {
    /// Always 1.
    #[serde(deserialize_with = "super::version_const::check::<_, 1>")]
    pub schema_version: u32,
    /// Project.
    pub project_id: String,
    /// Canonical cwd.
    pub cwd: String,
    /// Acknowledged state.
    pub acked: Option<Acked>,
    /// Stat cache.
    pub stat: Vec<StatEntry>,
    /// Auth cache.
    pub auth: Option<AuthCache>,
}

impl Versioned for FingerprintCacheFile {
    const KIND: &'static str = "fingerprint-cache";
    const CURRENT: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}
