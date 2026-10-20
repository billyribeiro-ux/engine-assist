//! `events/<runId>/<seq>-<event>.json` v1 · class (d) · P2 (`docs/contracts/files.md` §4.8).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Migration, Versioned};

/// `events/…json` v1. `payload` is untrusted hook input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SensorEventFile {
    /// Always 1.
    #[serde(deserialize_with = "super::version_const::check::<_, 1>")]
    pub schema_version: u32,
    /// Per-run token.
    pub token: String,
    /// Hook event name.
    pub event: String,
    /// RFC 3339 UTC.
    pub received_at: String,
    /// stdin was cut.
    pub truncated: bool,
    /// Hook input.
    pub payload: Option<serde_json::Map<String, Value>>,
}

impl Versioned for SensorEventFile {
    const KIND: &'static str = "sensor-event";
    const CURRENT: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}
