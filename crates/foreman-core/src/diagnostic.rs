//! Diagnostics: every problem the core finds carries a stable rule code, a file, a 1-based line
//! and a 1-based byte column (DR-015, DR-037). Nothing in the core panics on bad input.

use serde::{Deserialize, Serialize};

/// Error or warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// Blocks what it's attached to (a stage, a template, a file).
    Error,
    /// Shown only (`lint.*`, `grammar.auto-needs-v1-1`, …).
    Warning,
}

/// One problem with its position.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostic {
    /// The file the problem is in (as named by the caller, e.g. `claude-build-blueprint.md`).
    pub file: String,
    /// 1-based line.
    pub line: u32,
    /// 1-based byte column.
    pub column: u32,
    /// Stable rule code (`grammar.bad-chain`, `structure.depth-limit`, …).
    pub rule: String,
    /// Human-readable message.
    pub message: String,
    /// Error or warning.
    pub severity: Severity,
    /// The stage the problem belongs to, when it belongs to one (DR-037 scoping).
    pub stage: Option<u8>,
}

impl Diagnostic {
    /// An error.
    #[must_use]
    pub fn error(
        file: &str,
        line: usize,
        column: usize,
        rule: &str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            file: file.to_owned(),
            line: clamp(line),
            column: clamp(column),
            rule: rule.to_owned(),
            message: message.into(),
            severity: Severity::Error,
            stage: None,
        }
    }

    /// A warning.
    #[must_use]
    pub fn warning(
        file: &str,
        line: usize,
        column: usize,
        rule: &str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::error(file, line, column, rule, message)
        }
    }

    /// Attaches the stage.
    #[must_use]
    pub fn in_stage(mut self, stage: Option<u8>) -> Self {
        self.stage = stage;
        self
    }

    /// True for errors.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

fn clamp(n: usize) -> u32 {
    u32::try_from(n.max(1)).unwrap_or(u32::MAX)
}
