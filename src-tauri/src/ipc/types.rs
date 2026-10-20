//! Shared IPC scalars, enums and structured types (`docs/contracts/ipc.md` §2). Exported to TS by
//! ts-rs (ADR-004) into `src/lib/ipc/bindings/`.
//!
//! Request scalars validate while they deserialize: a value that breaks its rule is a typed
//! `Invalid { field: "req", rule }` (IPC-R2) through [`crate::ipc::Req`]. The rule names are fixed
//! text and never echo the value (SR-57).

use std::fmt;

use foreman_core::ids as core_ids;
use foreman_core::resolver::tokens as core_tokens;
use foreman_core::schema::progress as core_progress;
use foreman_core::schema::settings as core_settings;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

/// The marker a scalar's deserializer puts in front of its rule name; [`crate::ipc::Req`] turns it
/// into `Invalid { rule }`.
pub const INVALID_MARKER: &str = "fm-invalid:";

/// Defines a validated string newtype: serializes as the string, deserializes only when `check`
/// accepts it (otherwise `Invalid { rule }`).
macro_rules! validated_string {
    ($(#[$m:meta])* $name:ident, $rule:literal, $check:expr) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
        pub struct $name(String);

        impl $name {
            /// The rule name used in `Invalid` errors.
            pub const RULE: &'static str = $rule;

            /// Validates `s`.
            ///
            /// # Errors
            /// `Invalid { field, rule }` when `s` breaks the rule.
            pub fn parse(field: &str, s: &str) -> Result<Self, crate::ipc::IpcError> {
                let check: fn(&str) -> bool = $check;
                if check(s) {
                    Ok(Self(s.to_owned()))
                } else {
                    Err(crate::ipc::IpcError::invalid(field, $rule))
                }
            }

            /// Wraps a value Foreman produced itself (responses). Never used on request input.
            #[must_use]
            pub fn from_trusted(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            /// The text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// The owned text.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                let check: fn(&str) -> bool = $check;
                if check(&s) {
                    Ok(Self(s))
                } else {
                    Err(D::Error::custom(concat!("fm-invalid:", $rule)))
                }
            }
        }
    };
}

fn uuid(s: &str) -> bool {
    foreman_core::ids::is_uuid_v4(s)
}

/// No C0 (except, when `lf`, a line feed), C1, DEL or bidi/zero-width control (ipc.md §2.1).
fn clean_text(s: &str, lf: bool) -> bool {
    s.chars().all(|c| {
        let cp = u32::from(c);
        !(cp < 0x20 && !(lf && c == '\n'))
            && cp != 0x7f
            && !(0x80..=0x9f).contains(&cp)
            && !foreman_core::pathpolicy::is_bidi(c)
            && !foreman_core::pathpolicy::is_zero_width(c)
    })
}

validated_string!(
    /// `ProjectId(Uuid)`: a lowercase hyphenated UUID v4 (§2.1); `NotFound` if unknown.
    ProjectId, "uuid", uuid
);
validated_string!(
    /// `RunId(Uuid)` (§2.1).
    RunId, "uuid", uuid
);
validated_string!(
    /// `ChainId(Uuid)` (§2.1).
    ChainId, "uuid", uuid
);
validated_string!(
    /// `PlanId(Uuid)`: a one-time launch plan, valid 60 s (IPC-R7).
    PlanId, "uuid", uuid
);
validated_string!(
    /// `PreviewId(Uuid)`: a one-time preview, valid 10 minutes (IPC-R7).
    PreviewId, "uuid", uuid
);
validated_string!(
    /// `ChangeId(Uuid)`: a pending outside change (§4.9).
    ChangeId, "uuid", uuid
);
validated_string!(
    /// `PickId(Uuid)`: a folder picked in the Rust dialog (§4.3).
    PickId, "uuid", uuid
);
validated_string!(
    /// `HitId`: an SR-07 auth-key hit Rust holds (§2.4: the hit's stable SHA-256 over file, key
    /// and value hash, `foreman_core::fingerprint::AuthKeyHit::id`).
    HitId, "sha256", foreman_core::ids::is_sha256_hex
);
validated_string!(
    /// `StepKey` (DR-005).
    StepKey, "stepKey", |s| foreman_core::ids::StepKey::parse(s).is_ok()
);
validated_string!(
    /// `ItemId` (DR-034, DR-060).
    ItemId, "itemId", |s| foreman_core::ids::ItemId::parse(s).is_ok()
);
validated_string!(
    /// `RepoPath`: ≤ 1,024 bytes, no NUL; `pathpolicy::check_read`/`check_write` run in the handler.
    RepoPath, "repoPath", |s| !s.is_empty() && s.len() <= 1024 && !s.contains('\0')
);
validated_string!(
    /// `Sha256`: `^[0-9a-f]{64}$`.
    Sha256, "sha256", foreman_core::ids::is_sha256_hex
);
validated_string!(
    /// `IsoDate`: `YYYY-MM-DD`, a real date.
    IsoDate, "isoDate", |s| s.len() == 10 && s.parse::<jiff::civil::Date>().is_ok()
);
validated_string!(
    /// `ShortText`: ≤ 200 chars, no C0/C1/DEL/bidi controls.
    ShortText, "shortText", |s| s.chars().count() <= 200 && clean_text(s, false)
);
validated_string!(
    /// `Note`: ≤ 4,000 chars, no C0 (except LF), C1, DEL or bidi controls.
    Note, "note", |s| s.chars().count() <= 4000 && clean_text(s, true)
);
validated_string!(
    /// `InputValue`: ≤ 1 MB (> 64 KB is accepted with a warning, AC-F006-14).
    InputValue, "inputValue", |s| s.len() <= 1024 * 1024
);
validated_string!(
    /// `PtyData`: ≤ 64 KB UTF-8 (the client splits larger writes, §6.2).
    PtyData, "ptyData", |s| s.len() <= 64 * 1024
);
validated_string!(
    /// `ClaudeUrl`: `https`, host exactly `claude.ai`, no userinfo, no port (SR-25).
    ClaudeUrl, "claudeUrl", is_claude_url
);
validated_string!(
    /// `BundleId`: `^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+){1,10}$`.
    BundleId, "bundleId", is_bundle_id
);
validated_string!(
    /// `EnvName`: `^[A-Za-z_][A-Za-z0-9_]{0,127}$`.
    EnvName, "envName", is_env_name
);

/// SR-25: `https://claude.ai` or `https://claude.ai/…`; no userinfo, port, whitespace, controls or
/// backslashes; ≤ 2,048 bytes.
#[must_use]
pub fn is_claude_url(s: &str) -> bool {
    s.len() <= 2048
        && (s == "https://claude.ai" || s.starts_with("https://claude.ai/"))
        && !s.contains('\\')
        && s.chars()
            .all(|c| !c.is_whitespace() && !c.is_control() && clean_text(&c.to_string(), false))
}

fn is_bundle_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    (2..=11).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
}

fn is_env_name(s: &str) -> bool {
    let mut b = s.bytes();
    matches!(b.next(), Some(c) if c.is_ascii_alphabetic() || c == b'_')
        && s.len() <= 128
        && b.all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

/// RFC 3339 UTC (responses only; IPC-R4: never a 64-bit integer).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Timestamp(pub String);

/// Monotonic per-project revision (§2.1, IPC-R4: `u32`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Rev(pub u32);

/// Blueprint stage number, `0..=18` (§2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Stage(pub u8);

impl<'de> Deserialize<'de> for Stage {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let n = u8::deserialize(d)?;
        if n <= 18 {
            Ok(Self(n))
        } else {
            Err(D::Error::custom("fm-invalid:stage"))
        }
    }
}

/// `Record<symbol, never>` in TS: the request of a command with no fields (IPC-R1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Empty {}

/// Defines a unit-only enum mirrored from a core enum (same wire form) with `From` both ways.
macro_rules! mirror_enum {
    ($(#[$m:meta])* $name:ident = $cm:ident :: $ct:ident, rename = $ren:literal { $($v:ident),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(rename_all = $ren)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
        pub enum $name { $($v),+ }

        impl From<$cm::$ct> for $name {
            fn from(v: $cm::$ct) -> Self {
                match v { $($cm::$ct::$v => Self::$v),+ }
            }
        }

        impl From<$name> for $cm::$ct {
            fn from(v: $name) -> Self {
                match v { $($name::$v => Self::$v),+ }
            }
        }
    };
}

mirror_enum!(
    /// `yes | no | undecided` (DR-051).
    TriState = core_ids::TriState, rename = "camelCase" { Yes, No, Undecided }
);
mirror_enum!(
    /// The twelve conditions (DR-027); wire form is `snake_case`.
    ConditionId = core_ids::ConditionId, rename = "snake_case" {
        IdeaKnown, Commercial, Payments, MultiUser, Public, HasUi, OthersData, Server,
        HasDatabase, HasUploads, HasEmail, HasIntegrations
    }
);
mirror_enum!(
    /// Where a step runs (DR-022).
    Surface = core_ids::Surface, rename = "camelCase" { Chat, Cli, Desktop }
);
mirror_enum!(
    /// A step's working directory (DR-022).
    CwdKind = core_ids::CwdKind, rename = "camelCase" { Project, Spec }
);
mirror_enum!(
    /// Model aliases (DR-023).
    Model = core_ids::Model, rename = "camelCase" { Fable, Opus, Sonnet, Haiku, Opusplan }
);
mirror_enum!(
    /// Effort levels (DR-023); `adaptive` only for chat.
    Effort = core_ids::Effort, rename = "camelCase" {
        Low, Medium, High, Xhigh, Max, Ultracode, Adaptive
    }
);
mirror_enum!(
    /// Permission modes (DR-023); the template's `manual` is `default`.
    Mode = core_ids::Mode, rename = "camelCase" { Plan, Default, AcceptEdits, Auto, DontAsk }
);
mirror_enum!(
    /// Theme (files.md §4.1).
    Theme = core_settings::Theme, rename = "camelCase" { System, Vellum, Night }
);
mirror_enum!(
    /// A redacted diff entry's change (SR-55).
    ChangeKind = core_progress::ChangeKind, rename = "camelCase" {
        Added, Removed, ValueChanged
    }
);
mirror_enum!(
    /// A decision file (files.md §3.1–§3.3; `blueprintLock` as in `progress.json`).
    DecisionFileKind = core_progress::DecisionFileKind, rename = "camelCase" {
        Project, Overlay, BlueprintLock
    }
);
/// A step's kind as shown (DR-022): the template kinds, plus `gate` (gate instances) and `lessons`
/// (the F.2 lessons step).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum StepKind {
    Prompt,
    Followup,
    Action,
    Manual,
    Wait,
    Repeatable,
    Gate,
    Lessons,
}

impl From<foreman_core::template::StepKind> for StepKind {
    fn from(k: foreman_core::template::StepKind) -> Self {
        use foreman_core::template::StepKind as C;
        match k {
            C::Prompt => Self::Prompt,
            C::Followup => Self::Followup,
            C::Action => Self::Action,
            C::Manual => Self::Manual,
            C::Wait => Self::Wait,
            C::Repeatable => Self::Repeatable,
        }
    }
}
mirror_enum!(
    /// Token classes (DR-082).
    TokenClass = core_tokens::TokenClass, rename = "camelCase" {
        ProjectVariable, PacketMarker, PathAlias, RunInput, OptionalPart, Literal
    }
);
/// An action step's kind (DR-069).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ActionKind {
    WriteBlocks,
    CopyFile,
    Mkdir,
}

impl From<foreman_core::steps::actions::ActionKind> for ActionKind {
    fn from(k: foreman_core::steps::actions::ActionKind) -> Self {
        use foreman_core::steps::actions::ActionKind as C;
        match k {
            C::WriteBlocks => Self::WriteBlocks,
            C::CopyFile => Self::CopyFile,
        }
    }
}

/// A step's tag as shown (ipc.md §2.4): the template tag, or `added` for an overlay step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Tag {
    Core,
    Conditional,
    Optional,
    Added,
}

impl From<foreman_core::template::Tag> for Tag {
    fn from(t: foreman_core::template::Tag) -> Self {
        use foreman_core::template::Tag as C;
        match t {
            C::Core => Self::Core,
            C::Conditional => Self::Conditional,
            C::Optional => Self::Optional,
        }
    }
}

/// How a step was entered (F-017, F-028).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum EntryPoint {
    DoNext,
    Palette,
    StageView,
}

/// Every displayed step state (DR-065; `needsNewerForeman`: DR-035).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum StepState {
    NotStarted,
    Copied,
    Running,
    Done,
    Skipped,
    NotApplicable,
    WaitingUntil,
    Undecided,
    NeedsAnnotation,
    NeedsNewerForeman,
}

impl From<&foreman_core::steps::StepState> for StepState {
    fn from(s: &foreman_core::steps::StepState) -> Self {
        use foreman_core::steps::StepState as C;
        match s {
            C::NotStarted => Self::NotStarted,
            C::Copied => Self::Copied,
            C::Running => Self::Running,
            C::Done => Self::Done,
            C::Skipped(_) => Self::Skipped,
            C::NotApplicable(_) => Self::NotApplicable,
            C::WaitingUntil(_) => Self::WaitingUntil,
            C::Undecided => Self::Undecided,
            C::NeedsAnnotation => Self::NeedsAnnotation,
            C::NeedsNewerForeman => Self::NeedsNewerForeman,
        }
    }
}

/// A run's state (`idle`/`blocked`: P2 sensor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum RunStatus {
    Starting,
    Running,
    Idle,
    Blocked,
    Exited,
    Errored,
    Interrupted,
}

/// Why a project is open read-only (§2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ReadOnlyReason {
    LockMismatch,
    BlueprintMissing,
    DecisionFileInvalid,
    NewerSchema,
    GitMissing,
    FolderMissing,
}

/// Doctor's overall level (§2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum DoctorLevel {
    Ok,
    Warning,
    Failing,
    Unknown,
}

/// An external tool Foreman runs (§2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ToolKind {
    Claude,
    Git,
}

/// What a paste-back holds (`adrSet`, `lessons`: P2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum PasteKind {
    Auto,
    Packet,
    Output,
    AdrSet,
    Lessons,
}

/// What `clipboard_copy` copies (§4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum CopyTarget {
    Prompt,
    Command,
    SessionId,
    FileContents,
    FixRequest,
}

/// `Start { model, effort, mode }`; `mode` is null for chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Start {
    pub model: Model,
    pub effort: Effort,
    pub mode: Option<Mode>,
}

impl From<foreman_core::ids::Start> for Start {
    fn from(s: foreman_core::ids::Start) -> Self {
        Self {
            model: s.model.into(),
            effort: s.effort.into(),
            mode: s.mode.map(Into::into),
        }
    }
}

impl From<Start> for foreman_core::ids::Start {
    fn from(s: Start) -> Self {
        Self {
            model: s.model.into(),
            effort: s.effort.into(),
            mode: s.mode.map(Into::into),
        }
    }
}

/// Where a run-config value came from (F-005 explain on hover).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ValueOrigin {
    Template,
    Alternative { rule: String },
    Substitution { rule: String },
    Override,
}

impl From<&foreman_core::policy::ValueOrigin> for ValueOrigin {
    fn from(o: &foreman_core::policy::ValueOrigin) -> Self {
        use foreman_core::policy::ValueOrigin as C;
        match o {
            C::Template => Self::Template,
            C::Alternative { rule } => Self::Alternative { rule: rule.clone() },
            C::Substitution { rule } => Self::Substitution { rule: rule.clone() },
            C::Override => Self::Override,
        }
    }
}

/// `Explained<T> { value, origin, from }`: a value with where it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Explained<T> {
    pub value: T,
    pub origin: ValueOrigin,
    pub from: Option<T>,
}

/// A per-run override of the next launch (F-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RunOverride {
    pub model: Option<Model>,
    pub effort: Option<Effort>,
    pub mode: Option<Mode>,
}

/// Text crosses IPC only as typed nodes (SR-26); visible escapes are computed in Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum TextSpan {
    Plain { text: String },
    Escape { codepoint: u32, label: String },
    Code { text: String },
    Strong { children: Vec<TextSpan> },
    Emph { children: Vec<TextSpan> },
    Link { text: String, url: String },
}

/// Block-level document nodes (SR-26).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum DocNode {
    Heading {
        level: u8,
        spans: Vec<TextSpan>,
    },
    Paragraph {
        spans: Vec<TextSpan>,
    },
    List {
        ordered: bool,
        items: Vec<Vec<DocNode>>,
    },
    Quote {
        children: Vec<DocNode>,
    },
    CodeBlock {
        info: String,
        text: String,
    },
    Rule,
}

/// One resolver decision in a resolved prompt (DR-080 ff.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum PromptSegment {
    Text {
        spans: Vec<TextSpan>,
    },
    Resolved {
        token: String,
        class: TokenClass,
        text: String,
        note: Option<String>,
    },
    Unresolved {
        token: String,
        class: TokenClass,
        input: Option<String>,
    },
    OptionalPart {
        part_id: String,
        included: bool,
        text: String,
    },
    RemovedPart {
        expr: String,
        reason: String,
    },
}

/// A diff line's operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum DiffOp {
    Ctx,
    Add,
    Del,
}

/// One diff line, as escaped spans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DiffLine {
    pub op: DiffOp,
    pub spans: Vec<TextSpan>,
}

/// A diff hunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DiffHunk {
    pub old_start: u32,
    pub new_start: u32,
    pub lines: Vec<DiffLine>,
}

/// A file diff (SR-26: lines are escaped spans).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Diff {
    pub path: RepoPath,
    pub exists: bool,
    pub hunks: Vec<DiffHunk>,
}

/// One key-level change in a redacted diff (SR-55).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RedactedEntry {
    pub key_path: String,
    pub change: ChangeKind,
    pub before: Option<String>,
    pub after: Option<String>,
}

impl From<&foreman_core::schema::progress::DiffEntry> for RedactedEntry {
    fn from(e: &foreman_core::schema::progress::DiffEntry) -> Self {
        Self {
            key_path: e.key_path.clone(),
            change: e.change.into(),
            before: e.before.clone(),
            after: e.after.clone(),
        }
    }
}

/// A redacted, key-level diff of a config or decision file (SR-55).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RedactedDiff {
    pub source: String,
    pub entries: Vec<RedactedEntry>,
}

/// §2.4 `Applicability`: whether a stage applies, as the stage view shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Applicability {
    Applies,
    NotApplicable { reason: String },
    Undecided { condition: ConditionId },
    NeedsAnnotation,
}

/// `ReadOnly { reason, detail }` (§2.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ReadOnly {
    pub reason: ReadOnlyReason,
    pub detail: String,
}

/// What a problem offers to fix it (`repair`: P2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum FixAction {
    Repair,
    OpenInEditor,
    CopyFixRequest,
}

/// A validation problem (packets, paste-back). `fix` lists every action offered, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Problem {
    pub rule: String,
    pub line: Option<u32>,
    pub message: String,
    pub fix: Option<Vec<FixAction>>,
}

/// Why a step can't run (`Blocker::NotRunnable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum NotRunnableReason {
    NotApplicable,
    Undecided,
    NeedsAnnotation,
    Done,
    NeedsNewerForeman,
    Waiting,
    Running,
    /// A `launch_prepare` on a step that isn't a CLI step (runners, design doc §3.2.3).
    NotCli,
    /// A continuing step whose chain hasn't started yet.
    ChainNotStarted,
}

/// Why a tool path was refused (SR-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ToolPathProblem {
    Relative,
    Missing,
    NotExecutable,
    InProject,
    SymlinkIntoProject,
    WorldWritable,
}

/// Why `claude auth status` blocked (SR-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum AuthProblem {
    WrongMethod { method: String },
    UnknownMethod,
    Unparsable,
    Exit { code: Option<i32> },
    Timeout,
}

/// A feature's version requirement (ADR-017; `foreman_core::compat::Requirement`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Requirement {
    pub feature: String,
    pub min: String,
    pub source: String,
}

/// Why a launch can't go ahead (returned inside `PrepareOutcome`, never thrown; DR-120).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Blocker {
    /// `text` is Foreman's sentence for the row ("Stage 3 doesn't apply (commercial = no)").
    NotRunnable {
        reason: NotRunnableReason,
        text: String,
    },
    ReadOnly(ReadOnly),
    ClaudeMissing {
        path: Option<String>,
        why: ToolPathProblem,
    },
    VersionBelowFloor {
        found: String,
        required: String,
        requirements: Vec<Requirement>,
    },
    ContractFailed {
        feature: String,
        version: String,
    },
    ConfigChanged {
        cwd: CwdKind,
        fingerprint: Sha256,
        diffs: Vec<RedactedDiff>,
    },
    AuthKeyBlocked {
        file: String,
        key: String,
        hit: HitId,
        allowlistable: bool,
    },
    ConfigTooLarge {
        file: String,
        bytes: u32,
    },
    ProjectEnvUnapproved {
        file: String,
        sha256: Sha256,
    },
    AuthFailed {
        why: AuthProblem,
    },
    TranscriptMissing {
        chain: ChainId,
    },
    SignedPacketMissing {
        stage: Stage,
    },
    PacketUnsigned {
        stage: Stage,
    },
    RunInputMissing {
        name: String,
    },
    UnresolvedToken {
        token: String,
    },
    PromptOptionLike {
        first: String,
    },
    PromptControlChar {
        offset: u32,
        codepoint: u32,
    },
    RunSettingsPathUnsafe {
        path: String,
    },
    CwdNotDirectory {
        cwd: CwdKind,
    },
}

/// Why a launch needs Billy's native confirmation (SR-19).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ConfirmReason {
    OverlayStep,
    ModeOrModelChanged { from: Start, to: Start },
    FirstLaunchInProject,
    AutoOrDontAsk { mode: Mode },
    ClaudeVersionChanged { from: String, to: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_validate() {
        assert!(ProjectId::parse("f", "7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11").is_ok());
        assert!(ProjectId::parse("f", "7A1C3E52-0B9D-4C1E-9F2A-5D6E8B0C4A11").is_err());
        assert!(StepKey::parse("f", "2.3").is_ok() && StepKey::parse("f", "../x").is_err());
        assert!(ShortText::parse("f", "fine").is_ok());
        assert!(ShortText::parse("f", "bidi\u{202e}").is_err());
        assert!(ShortText::parse("f", &"x".repeat(201)).is_err());
        assert!(Note::parse("f", "line\nline").is_ok() && Note::parse("f", "a\rb").is_err());
        assert!(ClaudeUrl::parse("f", "https://claude.ai/project/abc").is_ok());
        for bad in [
            "http://claude.ai/x",
            "https://claude.ai.evil.com/",
            "https://claude.ai@evil.com/",
            "https://claude.ai:443/",
            "https://claude.ai/a b",
            "https://claude.ai/\\x",
        ] {
            assert!(ClaudeUrl::parse("f", bad).is_err(), "{bad}");
        }
        assert!(BundleId::parse("f", "com.microsoft.VSCode").is_ok());
        assert!(BundleId::parse("f", "VSCode").is_err());
        assert!(EnvName::parse("f", "HTTPS_PROXY").is_ok() && EnvName::parse("f", "1X").is_err());
        assert!(IsoDate::parse("f", "2026-10-26").is_ok());
        assert!(IsoDate::parse("f", "2026-02-30").is_err());
        let s: Result<Stage, _> = serde_json::from_str("19");
        assert!(s.unwrap_err().to_string().contains("fm-invalid:stage"));
    }
}
