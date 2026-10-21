//! Schema versioning (ADR-023, FILE-R2, FILE-R3). Every Foreman JSON file has an integer
//! `schemaVersion`. Reading migrates older files in memory through a pure chain (never writing
//! back), opens newer files read-only after a tolerant header read, and parses the current form
//! with `deny_unknown_fields`.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::json::{self, JsonError};

pub mod approvals;
pub mod fingerprint;
pub mod lock;
pub mod overlay;
pub mod progress;
pub mod project;
pub mod projects;
pub mod run;
pub mod run_settings;
pub mod sensor_event;
pub mod settings;
pub mod stage_record;

/// One forward migration `v<N>` → `v<N+1>` over raw JSON.
pub type Migration = fn(Value) -> Result<Value, MigrationError>;

/// A file kind with a versioned schema.
pub trait Versioned: Serialize + DeserializeOwned {
    /// The kind's name (`project`, `progress`, …), also its fixture folder.
    const KIND: &'static str;
    /// The current schema version.
    const CURRENT: u32;
    /// `MIGRATIONS[i]` turns `v(i+1)` into `v(i+2)`; its length is `CURRENT - 1`. It only grows.
    const MIGRATIONS: &'static [Migration];
}

/// Why a file couldn't be loaded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MigrationError {
    /// Not JSON, too deep, or not an object.
    #[error(transparent)]
    Json(#[from] JsonError),
    /// `schemaVersion` missing or not a positive integer.
    #[error("missing or invalid schemaVersion")]
    MissingVersion,
    /// A migration step failed.
    #[error("migration v{from} → v{} failed: {message}", from + 1)]
    Step {
        /// The version migrated from.
        from: u32,
        /// Why.
        message: String,
    },
    /// The migrated form doesn't fit the current schema (`deny_unknown_fields`).
    #[error("invalid {kind} file: {message}")]
    Invalid {
        /// The file kind.
        kind: &'static str,
        /// Why.
        message: String,
    },
}

/// The identity of a file read by the tolerant header read (ADR-023 §3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Header {
    /// The declared `schemaVersion`.
    pub schema_version: u32,
    /// `projectId`, if present and a string.
    pub project_id: Option<String>,
    /// `name`, if present and a string.
    pub name: Option<String>,
}

impl Header {
    /// Reads `schemaVersion`, `projectId` and `name`, ignoring everything else.
    ///
    /// # Errors
    /// When the bytes aren't a JSON object with a positive integer `schemaVersion`.
    pub fn read(bytes: &[u8]) -> Result<Self, MigrationError> {
        let v = json::parse_value(bytes)?;
        Self::of(&v)
    }

    fn of(v: &Value) -> Result<Self, MigrationError> {
        let obj = v.as_object().ok_or(MigrationError::MissingVersion)?;
        let schema_version = obj
            .get("schemaVersion")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n >= 1)
            .ok_or(MigrationError::MissingVersion)?;
        let text = |k: &str| obj.get(k).and_then(Value::as_str).map(str::to_owned);
        Ok(Self {
            schema_version,
            project_id: text("projectId"),
            name: text("name"),
        })
    }
}

/// The result of loading a versioned file.
#[derive(Debug, Clone, PartialEq)]
pub enum Loaded<T> {
    /// Already current.
    Current(T),
    /// Migrated in memory from an older version; nothing was written.
    Migrated {
        /// The migrated value.
        value: T,
        /// The version on disk.
        from: u32,
    },
    /// Written by a newer Foreman: read-only, with only the tolerant header read.
    NewerReadOnly(Header),
}

impl<T> Loaded<T> {
    /// The value, unless the file is newer than this build.
    pub fn value(self) -> Option<T> {
        match self {
            Self::Current(v) | Self::Migrated { value: v, .. } => Some(v),
            Self::NewerReadOnly(_) => None,
        }
    }

    /// The read-only banner for a newer file (ADR-023, FILE-R3).
    #[must_use]
    pub fn read_only_message(&self) -> Option<String> {
        match self {
            Self::NewerReadOnly(h) => Some(newer_message(h.schema_version)),
            _ => None,
        }
    }
}

/// "Created by a newer Foreman (schema N). Update Foreman to edit this project."
#[must_use]
pub fn newer_message(version: u32) -> String {
    format!("Created by a newer Foreman (schema {version}). Update Foreman to edit this project.")
}

/// Migrates a raw value to the current form of `T` and parses it.
///
/// # Errors
/// See [`MigrationError`].
pub fn migrate<T: Versioned>(raw: Value) -> Result<Loaded<T>, MigrationError> {
    let header = Header::of(&raw)?;
    if header.schema_version > T::CURRENT {
        return Ok(Loaded::NewerReadOnly(header));
    }
    let from = header.schema_version;
    let mut v = raw;
    for (i, step) in T::MIGRATIONS
        .iter()
        .enumerate()
        .skip(usize::try_from(from - 1).unwrap_or(usize::MAX))
    {
        v = step(v).map_err(|e| match e {
            MigrationError::Step { .. } => e,
            other => MigrationError::Step {
                from: u32::try_from(i + 1).unwrap_or(u32::MAX),
                message: other.to_string(),
            },
        })?;
        if let Some(obj) = v.as_object_mut() {
            obj.insert(
                "schemaVersion".into(),
                Value::from(u32::try_from(i + 2).unwrap_or(u32::MAX)),
            );
        }
    }
    let value: T = serde_json::from_value(v).map_err(|e| MigrationError::Invalid {
        kind: T::KIND,
        message: e.to_string(),
    })?;
    Ok(if from == T::CURRENT {
        Loaded::Current(value)
    } else {
        Loaded::Migrated { value, from }
    })
}

/// Parses file bytes and migrates them (a pure function: reading never rewrites, ADR-023 §3).
///
/// # Errors
/// See [`MigrationError`].
pub fn load<T: Versioned>(bytes: &[u8]) -> Result<Loaded<T>, MigrationError> {
    migrate::<T>(json::parse_value(bytes)?)
}

/// Serializes the current form canonically (FILE-R1).
///
/// # Errors
/// When the value can't be represented as JSON.
pub fn to_file<T: Versioned>(v: &T) -> Result<String, JsonError> {
    json::to_canonical(v)
}

/// Serde helper: `schemaVersion` must equal the kind's current version.
pub mod version_const {
    /// Deserializes and checks `schemaVersion == N`.
    ///
    /// # Errors
    /// When the number differs.
    pub fn check<'de, D: serde::Deserializer<'de>, const N: u32>(d: D) -> Result<u32, D::Error> {
        let v: u32 = serde::Deserialize::deserialize(d)?;
        if v == N {
            Ok(v)
        } else {
            Err(serde::de::Error::custom(format!(
                "schemaVersion {v}, expected {N}"
            )))
        }
    }
}
