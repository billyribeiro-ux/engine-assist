//! Responses of §4.9 (decisions and recovery) and §4.10 (packets and run log); §2.4 types settled
//! in Stage 5.

use serde::{Deserialize, Serialize};

use super::app::{RunSummary, StageView, response};
use crate::ipc::types::{
    ChangeId, DecisionFileKind, Diff, DocNode, IsoDate, PreviewId, Problem, RedactedDiff, RepoPath,
    Sha256, Stage, Surface, TextSpan, Timestamp,
};

/// §2.4 what accepting an outside change does to the applicable path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ChangeEffect {
    ChangesPath,
    NoEffect,
    Reloaded,
}

/// §2.4 a consequence line's glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ConsequenceGlyph {
    Lock,
    Info,
    Check,
}

response!(
    /// §2.4 one consequence line of the outside-change review.
    Consequence {
        glyph: ConsequenceGlyph,
        text: String,
    }
);

response!(
    /// §2.4 the review's context lines (what changed, git, what is approved).
    ChangeContext {
        changed: String,
        git: String,
        approved: String,
    }
);

response!(
    /// `PendingChange` (§4.9) with the review's effect, consequences and context (§2.4).
    PendingChange {
        id: ChangeId,
        file: DecisionFileKind,
        detected_at: Timestamp,
        seen_sha256: Sha256,
        diff: RedactedDiff,
        effect: ChangeEffect,
        consequences: Vec<Consequence>,
        context: ChangeContext,
    }
);

response!(
    /// `RebuildPreview` (§4.9).
    RebuildPreview {
        preview_id: PreviewId,
        stages: Vec<StageView>,
        lost: Vec<String>,
    }
);

response!(
    /// §2.4 `PacketHeader`.
    PacketHeader {
        line: String,
        from_stage: Stage,
        to_stage: Option<Stage>,
    }
);

response!(
    /// §2.4 `PacketFieldView`: one packet field as document nodes (SR-26).
    PacketFieldView {
        key: String,
        label: String,
        value: Vec<DocNode>,
        missing: bool,
        problems: Vec<Problem>,
        note: Option<Vec<TextSpan>>,
    }
);

response!(
    /// §2.4 `NormalizationNote`.
    NormalizationNote { field: String, message: String }
);

response!(
    /// §2.4 `Signature`: a packet's `APPROVED BY`.
    Signature {
        name: String,
        date: IsoDate,
        by_foreman: bool,
        confirmed: bool,
    }
);

response!(
    /// §2.4 one raw packet line for the inspector.
    RawLine {
        line: u32,
        spans: Vec<TextSpan>,
        bad: bool,
    }
);

response!(
    /// `PacketView` (§4.10) with `raw`, `writtenBy` and `summary` (§2.4).
    PacketView {
        path: RepoPath,
        exists: bool,
        sha256: Option<Sha256>,
        header: Option<PacketHeader>,
        fields: Vec<PacketFieldView>,
        problems: Vec<Problem>,
        normalization: Vec<NormalizationNote>,
        signed: Option<Signature>,
        tamper: Option<Diff>,
        raw: Vec<RawLine>,
        written_by: Option<String>,
        summary: String,
    }
);

/// `ExportResult` (§4.10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ExportResult {
    Saved { display_path: String },
    Cancelled,
}

response!(
    /// `SurfaceMetrics` (§4.10).
    SurfaceMetrics {
        surface: Surface,
        median_time_to_start_ms: u32,
        p95_time_to_start_ms: u32,
        median_active_ms_per_step: u32,
    }
);

response!(
    /// `StageEdits` (§4.10).
    StageEdits { stage: Stage, edits: u32 }
);

response!(
    /// §2.4 `BaselineComparison` (AC-SM-04).
    BaselineComparison {
        baseline_median_ms: u32,
        foreman_median_ms: u32,
    }
);

response!(
    /// `MetricsView` (§4.10) with `p95AllMs` (§2.4, NFR-09).
    MetricsView {
        per_surface: Vec<SurfaceMetrics>,
        manual_edits_by_stage: Vec<StageEdits>,
        stages_through_foreman: u32,
        baseline: Option<BaselineComparison>,
        runs_counted: u32,
        p95_all_ms: u32,
    }
);

response!(
    /// `RunsPage` (§4.10) with `total` (§2.4).
    RunsPage {
        runs: Vec<RunSummary>,
        next: Option<String>,
        total: u32,
    }
);
