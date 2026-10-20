//! Responses of §4.2 (first run, Doctor, Template health, probes); §2.4 types settled in Stage 5.

use serde::{Deserialize, Serialize};

use super::app::response;
use crate::ipc::types::{DoctorLevel, PreviewId, Stage, TextSpan, Timestamp};

/// §2.4 a Doctor check's group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum CheckGroup {
    Claude,
    Toolchain,
}

/// §2.4 an action a Doctor row offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum CheckAction {
    CaptureLoginPath,
}

response!(
    /// §2.4 `DoctorCheck`: one row of the Doctor screen.
    DoctorCheck {
        id: String,
        group: CheckGroup,
        name: String,
        level: DoctorLevel,
        summary: Option<String>,
        detail: Vec<TextSpan>,
        found: Option<String>,
        found_mono: bool,
        action: Option<CheckAction>,
    }
);

response!(
    /// §2.4 `CapabilityRow`: one feature against this Claude Code (ADR-017).
    CapabilityRow {
        feature: String,
        code: bool,
        minimum: String,
        found: bool,
        blocks: Option<String>,
    }
);

/// §2.4 a probe's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ProbeResult {
    Pass,
    Fail,
    Inconclusive,
}

response!(
    /// §2.4 `ProbeOutcome`.
    ProbeOutcome {
        probe: Vec<TextSpan>,
        result: ProbeResult,
        text: String,
        detail: Option<String>,
    }
);

response!(
    /// `DoctorReport` (§4.2) with the probe table (§2.4).
    DoctorReport {
        level: DoctorLevel,
        checks: Vec<DoctorCheck>,
        capabilities: Vec<CapabilityRow>,
        probes: Vec<ProbeOutcome>,
        probe_note: Option<String>,
        ran_at: Timestamp,
    }
);

response!(
    /// §2.4 `PathEntryCheck`: one captured login-PATH entry (SR-02).
    PathEntryCheck {
        entry: String,
        ok: bool,
        why: Option<String>,
    }
);

response!(
    /// `LoginPathPreview` (§4.2).
    LoginPathPreview {
        preview_id: PreviewId,
        shell: String,
        entries: Vec<PathEntryCheck>,
    }
);

response!(
    /// `ContractResult` (§4.2).
    ContractResult {
        version: String,
        verified_at: Timestamp,
        outcomes: Vec<ProbeOutcome>,
    }
);

response!(
    /// §2.4 `Counts`.
    Counts {
        stages: u32,
        runnable_steps: u32,
        items: u32,
        errors: u32,
        warnings: u32,
    }
);

/// §2.4 a stage's health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum StageHealthStatus {
    Runnable,
    Warning,
    Partial,
    NeedsAnnotation,
}

response!(
    /// §2.4 `StageHealth`.
    StageHealth {
        stages: String,
        title: String,
        status: StageHealthStatus,
        text: String,
        steps: String,
    }
);

/// A diagnostic's severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Severity {
    Error,
    Warning,
}

response!(
    /// `Diagnostic` (§4.2): `message` as escaped spans, with the stage it belongs to (§2.4).
    Diagnostic {
        severity: Severity,
        rule: String,
        file: String,
        line: u32,
        column: u32,
        message: Vec<TextSpan>,
        stage: Stage,
    }
);

response!(
    /// `TemplateHealth` (§4.2) with `commit` and `file` (§2.4).
    TemplateHealth {
        version: String,
        grammar_version: u32,
        counts: Counts,
        stages: Vec<StageHealth>,
        diagnostics: Vec<Diagnostic>,
        commit: String,
        file: String,
    }
);
