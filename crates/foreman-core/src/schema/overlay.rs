//! `.foreman/overlay.json` v1 (`docs/contracts/files.md` §3.2). Every field is a decision field.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Migration, Versioned};
use crate::ids::{CwdKind, Start, StepKey, Surface};

/// A recorded skip (DR-056).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Skip {
    /// 1–200 characters.
    pub reason: String,
    /// RFC 3339 UTC.
    pub at: String,
}

/// `optional` or `core` (added steps only, DR-059).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AddedTag {
    /// Skippable.
    Optional,
    /// Not skippable.
    Core,
}

/// An added step (P2, F-025).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddedStep {
    /// `x-<stage>-<n>`.
    pub id: String,
    /// The stage.
    pub stage: u8,
    /// Placed after this step.
    pub after: StepKey,
    /// Run config copied from this step.
    pub copied_from: StepKey,
    /// Title.
    pub title: String,
    /// Tag.
    pub tag: AddedTag,
    /// Prompt text.
    pub text: String,
    /// Surface.
    pub surface: Surface,
    /// Working directory.
    pub cwd: Option<CwdKind>,
    /// Start.
    pub start: Start,
    /// Declared outputs.
    pub out: Vec<String>,
}

/// An added inspection item (P2, DR-060).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddedItem {
    /// `xi-<stage>-<n>`.
    pub id: String,
    /// The stage.
    pub stage: u8,
    /// Bullet text.
    pub text: String,
    /// Always `attest`.
    pub kind: String,
}

/// `.foreman/overlay.json` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OverlayFile {
    /// Always 1.
    #[serde(deserialize_with = "super::version_const::check::<_, 1>")]
    pub schema_version: u32,
    /// Skips by step key.
    pub skips: BTreeMap<StepKey, Skip>,
    /// Added steps (P2).
    pub added_steps: Vec<AddedStep>,
    /// Added items (P2).
    pub added_items: Vec<AddedItem>,
}

impl OverlayFile {
    /// An empty overlay.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schema_version: 1,
            ..Self::default()
        }
    }
}

impl Versioned for OverlayFile {
    const KIND: &'static str = "overlay";
    const CURRENT: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}
