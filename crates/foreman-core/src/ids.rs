//! Identifiers and closed vocabularies shared by every core module (`docs/contracts/ipc.md` §2.1,
//! §2.2; domain rules DR-005, DR-023, DR-027, DR-034).

use std::fmt;

use serde::{Deserialize, Serialize};

/// Why an identifier failed validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {kind}: {value:?}")]
pub struct IdError {
    /// What was being parsed (`step key`, `item id`, …).
    pub kind: &'static str,
    /// The rejected text.
    pub value: String,
}

fn is_digits(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
}

/// `^[0-9]{1,2}\.[0-9]{1,2}[a-z]?$`: a numbered step (`2.3`, `1.3b`).
fn is_numbered_step(s: &str) -> bool {
    let Some((a, b)) = s.split_once('.') else {
        return false;
    };
    let b = b
        .strip_suffix(|c: char| c.is_ascii_lowercase())
        .unwrap_or(b);
    is_digits(a, 1, 2) && is_digits(b, 1, 2)
}

/// True for a template step ID (DR-005): `N.M[a-z]`, `council-setup`, `gate.A|B|C`, `F.2`.
#[must_use]
pub fn is_step_id(s: &str) -> bool {
    is_numbered_step(s) || matches!(s, "council-setup" | "gate.A" | "gate.B" | "gate.C" | "F.2")
}

/// A step key (DR-005): a template step ID, `<stage>:gate.X` / `<stage>:F.2` instances, or an
/// overlay step `x-<stage>-<n>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StepKey(String);

impl StepKey {
    /// Parses and validates a step key.
    ///
    /// # Errors
    /// When the text doesn't match the step-key grammar.
    pub fn parse(s: &str) -> Result<Self, IdError> {
        let ok = is_numbered_step(s)
            || s == "council-setup"
            || s.split_once(':').is_some_and(|(n, rest)| {
                is_digits(n, 1, 2) && matches!(rest, "gate.A" | "gate.B" | "gate.C" | "F.2")
            })
            || s.strip_prefix("x-").is_some_and(|rest| {
                rest.split_once('-')
                    .is_some_and(|(a, b)| is_digits(a, 1, 2) && is_digits(b, 1, 3))
            });
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(IdError {
                kind: "step key",
                value: s.to_owned(),
            })
        }
    }

    /// The instance key of a gate or F.2 template for a stage (`5:gate.A`).
    #[must_use]
    pub fn instance(stage: u8, template_id: &str) -> Self {
        Self(format!("{stage}:{template_id}"))
    }

    /// The key as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StepKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for StepKey {
    type Error = IdError;
    fn try_from(s: String) -> Result<Self, IdError> {
        Self::parse(&s)
    }
}

impl From<StepKey> for String {
    fn from(k: StepKey) -> Self {
        k.0
    }
}

/// True for a template item ID (DR-034): `^[a-z][a-z0-9-]{0,47}$`.
#[must_use]
pub fn is_item_id(s: &str) -> bool {
    let mut b = s.bytes();
    matches!(b.next(), Some(c) if c.is_ascii_lowercase())
        && s.len() <= 48
        && b.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

/// An inspection item ID: a template ID or an overlay item `xi-<stage>-<n>` (DR-060).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ItemId(String);

impl ItemId {
    /// Parses and validates an item ID.
    ///
    /// # Errors
    /// When the text isn't a template or overlay item ID.
    pub fn parse(s: &str) -> Result<Self, IdError> {
        let overlay = s.strip_prefix("xi-").is_some_and(|rest| {
            rest.split_once('-')
                .is_some_and(|(a, b)| is_digits(a, 1, 2) && is_digits(b, 1, 3))
        });
        if is_item_id(s) || overlay {
            Ok(Self(s.to_owned()))
        } else {
            Err(IdError {
                kind: "item id",
                value: s.to_owned(),
            })
        }
    }

    /// The ID as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ItemId {
    type Error = IdError;
    fn try_from(s: String) -> Result<Self, IdError> {
        Self::parse(&s)
    }
}

impl From<ItemId> for String {
    fn from(k: ItemId) -> Self {
        k.0
    }
}

/// True for a lowercase hex SHA-256.
#[must_use]
pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// True for a lowercase hyphenated UUID v4 (`urn:foreman:common#/$defs/uuid`).
#[must_use]
pub fn is_uuid_v4(s: &str) -> bool {
    let b = s.as_bytes();
    s.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            14 => *c == b'4',
            19 => matches!(c, b'8' | b'9' | b'a' | b'b'),
            _ => matches!(c, b'0'..=b'9' | b'a'..=b'f'),
        })
}

/// A git object ID (SHA-1 or SHA-256 hex).
#[must_use]
pub fn is_git_oid(s: &str) -> bool {
    (s.len() == 40 || s.len() == 64) && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The twelve condition IDs (DR-027). Wire form is `snake_case`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionId {
    /// `idea_known`
    IdeaKnown,
    /// `commercial`
    Commercial,
    /// `payments`
    Payments,
    /// `multi_user`
    MultiUser,
    /// `public`
    Public,
    /// `has_ui`
    HasUi,
    /// `others_data`
    OthersData,
    /// `server`
    Server,
    /// `has_database`
    HasDatabase,
    /// `has_uploads`
    HasUploads,
    /// `has_email`
    HasEmail,
    /// `has_integrations`
    HasIntegrations,
}

impl ConditionId {
    /// Every condition, in the order of DR-027.
    pub const ALL: [Self; 12] = [
        Self::IdeaKnown,
        Self::Commercial,
        Self::Payments,
        Self::MultiUser,
        Self::Public,
        Self::HasUi,
        Self::OthersData,
        Self::Server,
        Self::HasDatabase,
        Self::HasUploads,
        Self::HasEmail,
        Self::HasIntegrations,
    ];

    /// The wire / annotation name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IdeaKnown => "idea_known",
            Self::Commercial => "commercial",
            Self::Payments => "payments",
            Self::MultiUser => "multi_user",
            Self::Public => "public",
            Self::HasUi => "has_ui",
            Self::OthersData => "others_data",
            Self::Server => "server",
            Self::HasDatabase => "has_database",
            Self::HasUploads => "has_uploads",
            Self::HasEmail => "has_email",
            Self::HasIntegrations => "has_integrations",
        }
    }

    /// Parses an annotation name.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }

    /// Late conditions are decided at the Stage 4 sign (DR-052).
    #[must_use]
    pub const fn is_late(self) -> bool {
        matches!(
            self,
            Self::HasDatabase | Self::HasUploads | Self::HasEmail | Self::HasIntegrations
        )
    }
}

impl fmt::Display for ConditionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A three-valued condition (DR-051).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TriState {
    /// `yes`
    Yes,
    /// `no`
    No,
    /// `undecided`
    Undecided,
}

impl TriState {
    /// The wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::No => "no",
            Self::Undecided => "undecided",
        }
    }
}

/// Where a step runs (DR-022).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Surface {
    /// Claude chat.
    Chat,
    /// Claude Code CLI.
    Cli,
    /// Claude Code Desktop.
    Desktop,
}

impl Surface {
    /// The annotation name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Cli => "cli",
            Self::Desktop => "desktop",
        }
    }
}

/// A step's working directory (DR-022). `blueprint`/`worktree` are v1.1 (`grammar.v1-1-cwd`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CwdKind {
    /// The project root.
    Project,
    /// The project's `spec/` folder.
    Spec,
}

impl CwdKind {
    /// The annotation name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Spec => "spec",
        }
    }
}

/// Model aliases (DR-023).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Model {
    /// `fable`
    Fable,
    /// `opus`
    Opus,
    /// `sonnet`
    Sonnet,
    /// `haiku`
    Haiku,
    /// `opusplan`
    Opusplan,
}

impl Model {
    /// Every model.
    pub const ALL: [Self; 5] = [
        Self::Fable,
        Self::Opus,
        Self::Sonnet,
        Self::Haiku,
        Self::Opusplan,
    ];

    /// The alias.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fable => "fable",
            Self::Opus => "opus",
            Self::Sonnet => "sonnet",
            Self::Haiku => "haiku",
            Self::Opusplan => "opusplan",
        }
    }

    /// Parses an alias.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == s)
    }
}

/// Effort levels (DR-023): `adaptive` is for chat only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Effort {
    /// `low`
    Low,
    /// `medium`
    Medium,
    /// `high`
    High,
    /// `xhigh`
    Xhigh,
    /// `max`
    Max,
    /// `ultracode`
    Ultracode,
    /// `adaptive` (chat)
    Adaptive,
}

impl Effort {
    /// Every effort.
    pub const ALL: [Self; 7] = [
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Xhigh,
        Self::Max,
        Self::Ultracode,
        Self::Adaptive,
    ];

    /// The annotation name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
            Self::Ultracode => "ultracode",
            Self::Adaptive => "adaptive",
        }
    }

    /// Parses an annotation name.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == s)
    }
}

/// Permission modes (DR-023); the template's `manual` is stored as `Default`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// `plan`
    Plan,
    /// `default` (the blueprint's "Manual")
    Default,
    /// `acceptEdits`
    AcceptEdits,
    /// `auto`
    Auto,
    /// `dontAsk`
    DontAsk,
}

impl Mode {
    /// The `--permission-mode` value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::Auto => "auto",
            Self::DontAsk => "dontAsk",
        }
    }

    /// Parses an annotation mode; `manual` maps to `Default`. `bypassPermissions` is never a mode.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "plan" => Self::Plan,
            "default" | "manual" => Self::Default,
            "acceptEdits" => Self::AcceptEdits,
            "auto" => Self::Auto,
            "dontAsk" => Self::DontAsk,
            _ => return None,
        })
    }
}

/// A run start: model, effort and (CLI/Desktop only) mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    /// The model alias.
    pub model: Model,
    /// The effort.
    pub effort: Effort,
    /// The mode (`None` for chat).
    pub mode: Option<Mode>,
}

impl fmt::Display for Start {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.model.as_str(), self.effort.as_str())?;
        if let Some(m) = self.mode {
            write!(f, "/{}", m.as_str())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_keys() {
        for ok in [
            "2.3",
            "1.3b",
            "council-setup",
            "5:gate.A",
            "2:F.2",
            "x-2-1",
            "17:gate.C",
        ] {
            assert!(StepKey::parse(ok).is_ok(), "{ok}");
        }
        for bad in ["", "2", "2.", "123.1", "gate.A", "F.2", "5:gate.D", "x-2", "2.3B", "../x"] {
            assert!(StepKey::parse(bad).is_err(), "{bad}");
        }
        assert!(is_step_id("gate.A") && is_step_id("F.2") && !is_step_id("5:gate.A"));
    }

    #[test]
    fn item_ids() {
        assert!(ItemId::parse("fresh-clone").is_ok());
        assert!(ItemId::parse("xi-5-1").is_ok());
        assert!(ItemId::parse("Fresh").is_err());
        assert!(ItemId::parse(&"a".repeat(49)).is_err());
    }

    #[test]
    fn uuid_and_hashes() {
        assert!(is_uuid_v4("6f1c2a3b-1d2e-4f50-8a9b-0c1d2e3f4a5b"));
        assert!(!is_uuid_v4("6f1c2a3b-1d2e-3f50-8a9b-0c1d2e3f4a5b"));
        assert!(is_sha256_hex(&"a".repeat(64)) && !is_sha256_hex(&"A".repeat(64)));
        assert!(is_git_oid(&"0".repeat(40)));
    }

    #[test]
    fn vocabularies() {
        assert_eq!(Mode::parse("manual"), Some(Mode::Default));
        assert_eq!(Mode::parse("bypassPermissions"), None);
        assert_eq!(ConditionId::parse("has_ui"), Some(ConditionId::HasUi));
        assert!(ConditionId::HasEmail.is_late() && !ConditionId::Server.is_late());
        let s = Start {
            model: Model::Opus,
            effort: Effort::Xhigh,
            mode: Some(Mode::Plan),
        };
        assert_eq!(s.to_string(), "opus@xhigh/plan");
    }
}
