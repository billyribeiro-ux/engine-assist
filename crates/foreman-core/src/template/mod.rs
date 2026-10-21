//! The blueprint template (F-001, ADR-005, DR-001–DR-043): one comrak pass with source positions,
//! one walk collecting stages, steps, RUN WITH blockquotes, prompts, inspection lists, HANDOFF notes,
//! packet formats and `fm:` directives; then instances, inheritance, validation and lint. The parser
//! never panics; every problem is a [`Diagnostic`].

use serde::{Deserialize, Serialize};

use crate::diagnostic::Diagnostic;
use crate::ids::{CwdKind, Start, StepKey, Surface};

pub mod cache;
pub mod expr;
pub mod grammar;
pub mod lint;
pub mod render;
mod structure;
mod validate;

pub use expr::{Expr, ExprError, parse_expr};
pub use grammar::{
    Alt, AltRule, ChainRef, InputDecl, InputType, Session, SourcePos, StepAnnotation, StepKind,
    Switch, Tag,
};

/// The parser version (part of the template cache key, ADR-023 §5).
pub const PARSER_VERSION: u32 = 1;
/// The default file name in diagnostics.
pub const FILE: &str = "claude-build-blueprint.md";
/// NFR-24's soft limit.
pub const SOFT_LINE_LIMIT: usize = 20_000;

/// Why a step can't run (derived at parse time; DR-013, DR-035, DR-037).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepStatus {
    /// Annotated and valid.
    Runnable,
    /// No `fm:step`: listed, never runnable.
    NeedsAnnotation,
    /// An unknown key under a newer grammar (DR-035).
    NeedsNewerForeman,
    /// A template error in its stage (or in its gate/F.2 template).
    Invalid,
}

/// A RUN WITH blockquote (prose; lint and chips only, DR-004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunWith {
    /// The text without `>` markers.
    pub text: String,
    /// First line.
    pub line: u32,
}

/// One `text` fence of a prompt, possibly inside an `fm:if` part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptPart {
    /// The fence literal (ends with LF).
    pub text: String,
    /// The line of the fence's first content line.
    pub line: u32,
    /// The `fm:if` expression around it.
    pub cond: Option<(String, Expr)>,
}

/// A step's prompt: a run of `text` fences joined (DR-008).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    /// The parts in order.
    pub parts: Vec<PromptPart>,
}

impl Prompt {
    /// The full text with every part.
    #[must_use]
    pub fn joined(&self) -> String {
        self.parts.iter().map(|p| p.text.as_str()).collect()
    }
}

/// A step (template step or gate/F.2 instance).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    /// Step key (DR-005).
    pub key: StepKey,
    /// Annotation ID (or the heading's ID when unannotated).
    pub id: String,
    /// Stage.
    pub stage: u8,
    /// Title.
    pub title: String,
    /// Heading or paragraph line.
    pub line: u32,
    /// Parse status.
    pub status: StepStatus,
    /// The annotation (`None` when unannotated).
    pub annotation: Option<StepAnnotation>,
    /// Effective kind.
    pub kind: StepKind,
    /// Effective tag.
    pub tag: Tag,
    /// Effective surface (after inheritance).
    pub surface: Option<Surface>,
    /// Effective cwd (after inheritance).
    pub cwd: Option<CwdKind>,
    /// Session.
    pub session: Session,
    /// The chain's start step key (resolved).
    pub chain: Option<StepKey>,
    /// Effective start (after inheritance for `chain=@stage`).
    pub start: Option<Start>,
    /// Instantiated `out` paths.
    pub out: Vec<String>,
    /// `appends`.
    pub appends: Option<String>,
    /// RUN WITH.
    pub run_with: Option<RunWith>,
    /// Prompt.
    pub prompt: Option<Prompt>,
    /// For instances: the template ID (`gate.A`, `F.2`).
    pub template_id: Option<String>,
}

impl Step {
    /// Whether Foreman can run it.
    #[must_use]
    pub fn runnable(&self) -> bool {
        self.status == StepStatus::Runnable
    }

    /// The `when` expression.
    #[must_use]
    pub fn when(&self) -> Option<&Expr> {
        self.annotation
            .as_ref()
            .and_then(|a| a.when.as_ref())
            .map(|(_, e)| e)
    }

    /// Switches.
    #[must_use]
    pub fn switches(&self) -> &[Switch] {
        self.annotation
            .as_ref()
            .map_or(&[], |a| a.switches.as_slice())
    }

    /// Alternatives.
    #[must_use]
    pub fn alts(&self) -> &[Alt] {
        self.annotation.as_ref().map_or(&[], |a| a.alts.as_slice())
    }

    /// Declared run inputs.
    #[must_use]
    pub fn inputs(&self) -> &[InputDecl] {
        self.annotation
            .as_ref()
            .map_or(&[], |a| a.inputs.as_slice())
    }
}

/// A stage (DR-002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    /// 0–18.
    pub number: u8,
    /// Title.
    pub title: String,
    /// Heading line.
    pub line: u32,
    /// Has a `### Stage N gate` (5–17).
    pub has_gate: bool,
}

/// An inspection item (DR-009, DR-014, DR-030).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectionItem {
    /// Item ID (`None` when the list has no `fm:verify`).
    pub id: Option<String>,
    /// Stage.
    pub stage: u8,
    /// The bullet as written (Markdown inline source).
    pub text: String,
    /// Normalized text (DR-014).
    pub normalized: String,
    /// SHA-256 of the normalized text.
    pub text_sha256: String,
    /// Kind as written (`attest` when unannotated).
    pub kind: String,
    /// Line.
    pub line: u32,
}

impl InspectionItem {
    /// The kind Foreman v1 acts on: `auto:` items are attested (DR-031).
    #[must_use]
    pub fn effective_kind(&self) -> &str {
        if self.kind.starts_with("auto:") {
            "attest"
        } else {
            &self.kind
        }
    }
}

/// One required packet key (DR-017).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatKey {
    /// The key, exactly.
    pub key: String,
    /// The placeholder text (first line plus continuation lines).
    pub placeholder: String,
    /// Line.
    pub line: u32,
}

/// A packet format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacketFormat {
    /// The stage (`None` for the generic Gate B format, Stages 5–17).
    pub stage: Option<u8>,
    /// The step whose prompt holds it.
    pub step: String,
    /// The header line as written.
    pub header: String,
    /// Keys in order.
    pub keys: Vec<FormatKey>,
    /// Header line number.
    pub line: u32,
}

/// A HANDOFF note (DR-010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffNote {
    /// Stage.
    pub stage: u8,
    /// Text.
    pub text: String,
    /// Line.
    pub line: u32,
}

/// A `markdown` fence under Council setup (DR-036).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CouncilBlock {
    /// The `name:` line value.
    pub name: String,
    /// The fence literal.
    pub text: String,
    /// Line.
    pub line: u32,
}

/// The parsed template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    /// Cache schema version (always 1).
    pub schema_version: u32,
    /// Parser version.
    pub parser_version: u32,
    /// `fm:grammar v=` (`None` when missing).
    pub grammar_version: Option<u32>,
    /// From the `Version X.Y` line.
    pub blueprint_version: String,
    /// DR-001.
    pub source_sha256: String,
    /// Stages.
    pub stages: Vec<Stage>,
    /// Steps in order, instances included.
    pub steps: Vec<Step>,
    /// Gate templates `gate.A`, `gate.B`, `gate.C`.
    pub gate_templates: Vec<Step>,
    /// The F.2 template.
    pub f2_template: Option<Step>,
    /// Inspection items.
    pub items: Vec<InspectionItem>,
    /// Packet formats.
    pub packet_formats: Vec<PacketFormat>,
    /// HANDOFF notes.
    pub handoffs: Vec<HandoffNote>,
    /// Council setup fences.
    pub council_blocks: Vec<CouncilBlock>,
    /// Diagnostics (errors, warnings, lints).
    pub diagnostics: Vec<Diagnostic>,
}

impl Template {
    /// A step by key.
    #[must_use]
    pub fn step(&self, key: &str) -> Option<&Step> {
        self.steps.iter().find(|s| s.key.as_str() == key)
    }

    /// Errors only.
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.is_error())
    }

    /// The steps of a stage, in order.
    pub fn stage_steps(&self, stage: u8) -> impl Iterator<Item = &Step> {
        self.steps.iter().filter(move |s| s.stage == stage)
    }

    /// The packet format for a stage (the generic Gate B format for 5–17).
    #[must_use]
    pub fn packet_format(&self, stage: u8) -> Option<&PacketFormat> {
        self.packet_formats
            .iter()
            .find(|f| f.stage == Some(stage))
            .or_else(|| {
                (5..=17)
                    .contains(&stage)
                    .then(|| self.packet_formats.iter().find(|f| f.stage.is_none()))
                    .flatten()
            })
    }

    /// Steps whose grammar is newer than this Foreman ("Update Foreman: N steps", DR-035).
    #[must_use]
    pub fn needs_newer_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == StepStatus::NeedsNewerForeman)
            .count()
    }
}

/// BOM removed, CRLF and lone CR → LF (DR-001).
#[must_use]
pub fn normalize_source(src: &str) -> String {
    let s = src.strip_prefix('\u{feff}').unwrap_or(src);
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Tabs expanded to the next multiple of 4 columns (`CommonMark` tab stops).
fn expand_tabs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.split_inclusive('\n') {
        let mut col = 0usize;
        for ch in line.chars() {
            if ch == '\t' {
                let n = 4 - col % 4;
                out.extend(std::iter::repeat_n(' ', n));
                col += n;
            } else {
                out.push(ch);
                col = if ch == '\n' { 0 } else { col + 1 };
            }
        }
    }
    out
}

/// `sourceSha256` (DR-001): over the normalized text with tabs expanded. Text that needs no
/// normalization is hashed in place (NFR-03's cached-load budget).
#[must_use]
pub fn source_sha256(src: &str) -> String {
    let clean = !src.starts_with('\u{feff}') && !src.contains(['\r', '\t']);
    if clean {
        return crate::store::sha256_hex(src.as_bytes());
    }
    crate::store::sha256_hex(expand_tabs(&normalize_source(src)).as_bytes())
}

/// Normalizes one inspection bullet's Markdown (DR-014): inline literal text (code without
/// backticks, link text without URL, emphasis markers dropped, breaks as spaces), Unicode
/// lowercase, whitespace collapsed, trimmed.
#[must_use]
pub fn normalize_item_text(bullet: &str) -> String {
    let arena = comrak::Arena::new();
    let root = comrak::parse_document(
        &arena,
        &expand_tabs(&normalize_source(bullet)),
        &comrak::Options::default(),
    );
    let mut raw = String::new();
    structure::inline_text(root, &mut raw);
    collapse(&raw.to_lowercase())
}

pub(crate) fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// SHA-256 hex of a normalized item text.
#[must_use]
pub fn item_hash(normalized: &str) -> String {
    crate::store::sha256_hex(normalized.as_bytes())
}

/// The stages the first release runs (P1): Stages 1–2, with their `F.2` instances (CR-003,
/// DR-155).
pub const P1_STAGES: std::ops::RangeInclusive<u8> = 1..=2;

/// Where an error sits, for scoping (DR-037).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// In stage N: from the diagnostic's own stage, else from its line.
    Stage(u8),
    /// In the Stage Gate section (gate templates).
    StageGate,
    /// In Appendix F (the F.2 template).
    AppendixF,
    /// In none of those (the preamble, the other appendices).
    Elsewhere,
}

/// An error and where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedError {
    /// Where.
    pub place: Place,
    /// The error.
    pub diagnostic: Diagnostic,
}

/// Parses a blueprint (never panics; problems become diagnostics).
#[must_use]
pub fn parse_template(source: &str) -> Template {
    parse_template_named(source, FILE)
}

/// [`parse_template`] with the file name used in diagnostics.
#[must_use]
pub fn parse_template_named(source: &str, file: &str) -> Template {
    parse_template_p1(source, file).0
}

/// [`parse_template_named`] plus the errors that make a P1 step ([`P1_STAGES`] and their F.2)
/// not runnable, in line order (CR-003): what a pin refuses on.
#[must_use]
pub fn parse_template_p1(source: &str, file: &str) -> (Template, Vec<ScopedError>) {
    let (mut t, extras) = structure::parse(source, file);
    validate::run(&mut t, &extras, file);
    lint::run(&mut t, file);
    validate::finish(&mut t, &extras);
    let p1 = validate::p1_errors(&t, &extras);
    (t, p1)
}
