//! `packets` commands (`docs/contracts/ipc.md` §4).

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "handlers take requests by value and return the IPC Result"
)]

use std::collections::BTreeSet;

use foreman_core::decisions;
use foreman_core::packet::{self, Signature as PacketSignature};
use foreman_core::pathpolicy::{self, ReadClass};
use foreman_core::schema;
use foreman_core::schema::approvals::OutsideSignature;

use crate::ipc::IpcError;
use crate::ipc::dto::launch::{LaunchProgress, PrepareOutcome};
use crate::ipc::dto::records::{
    ExportResult, MetricsView, PacketFieldView, PacketHeader, PacketView, RawLine, RunsPage,
    Signature,
};
use crate::ipc::dto::requests::{
    MetricsExportReq, MetricsViewReq, PacketNormalizeApplyReq, PacketRepairPrepareReq,
    PacketSignatureConfirmReq, PacketViewReq, RunsClearReq, RunsSearchReq,
};
use crate::ipc::error::StaleWhat;
use crate::ipc::types::{FixAction, IsoDate, Problem, RepoPath, Sha256, Stage, TextSpan};
use crate::os::Confirm;
use crate::project::{self, Opened};
use crate::render::{doc_nodes, escape_visible, spans};
use crate::state::App;

fn title_case(key: &str) -> String {
    let lower = key.to_lowercase();
    let mut c = lower.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// The `PacketView` of a stage's packet (SR-26: every value as typed nodes).
///
/// # Errors
/// `NotFound` (not a packet stage), `ReadOnly` without a template.
pub fn view_of(p: &Opened, n: u8) -> Result<PacketView, IpcError> {
    if n > 17 {
        return Err(IpcError::not_found("That stage has no packet."));
    }
    let path = RepoPath::from_trusted(packet::file_name(n));
    let Some(file) = p.packets.get(&n) else {
        return Ok(PacketView {
            path,
            exists: false,
            sha256: None,
            header: None,
            fields: Vec::new(),
            problems: Vec::new(),
            normalization: Vec::new(),
            signed: None,
            tamper: None,
            raw: Vec::new(),
            written_by: None,
            summary: "Not written yet.".to_owned(),
        });
    };
    let (t, path_, _) = p.parts()?;
    let fmt = t.packet_format(n);
    let keys: Vec<String> = fmt
        .map(|f| f.keys.iter().map(|k| k.key.clone()).collect())
        .unwrap_or_default();
    let parsed = packet::parse_packet(&file.text, &keys);
    let all = fmt.map_or_else(Vec::new, |f| {
        packet::validate_packet(
            &file.text,
            f,
            &packet::PacketCtx {
                stage: n,
                next_stage: path_.next_applicable_stage(n),
                lock_blueprint_version: p.lock.as_ref().map(|l| l.blueprint_version.clone()),
                has_skips: false,
            },
        )
    });
    let problem = |x: &packet::Problem| Problem {
        rule: x.rule.clone(),
        line: Some(x.line),
        message: escape_visible(&x.message),
        fix: Some(vec![FixAction::OpenInEditor, FixAction::CopyFixRequest]),
    };
    let mut fields: Vec<PacketFieldView> = keys
        .iter()
        .map(|k| {
            let f = parsed.get(k);
            let line = f.map(|f| f.line);
            PacketFieldView {
                key: k.clone(),
                label: title_case(k),
                value: f.map(|f| doc_nodes(&f.value)).unwrap_or_default(),
                missing: f.is_none(),
                problems: all
                    .iter()
                    .filter(|x| line.is_some_and(|l| x.line == l))
                    .map(problem)
                    .collect(),
                note: None,
            }
        })
        .collect();
    for f in parsed.fields.iter().filter(|f| !keys.contains(&f.key)) {
        fields.push(PacketFieldView {
            key: escape_visible(&f.key),
            label: title_case(&escape_visible(&f.key)),
            value: doc_nodes(&f.value),
            missing: false,
            problems: Vec::new(),
            note: Some(vec![TextSpan::Plain {
                text: "Not in this stage's packet format.".to_owned(),
            }]),
        });
    }
    let bad: BTreeSet<u32> = all.iter().filter(|x| !x.warning).map(|x| x.line).collect();
    let raw = file
        .text
        .lines()
        .enumerate()
        .map(|(i, l)| {
            let line = u32::try_from(i + 1).unwrap_or(u32::MAX);
            RawLine {
                line,
                spans: spans(l),
                bad: bad.contains(&line),
            }
        })
        .collect();
    let by_foreman = p.progress.signing.contains_key(&n.to_string());
    let confirmed = p.signed().contains(&n);
    let signed = match p.packet_signature(n) {
        Some(PacketSignature::Signed { name, date, .. }) => Some(Signature {
            name: escape_visible(&name),
            date: IsoDate::from_trusted(date),
            by_foreman,
            confirmed,
        }),
        _ => None,
    };
    let errors = all.iter().filter(|x| !x.warning).count();
    let summary = match (errors, &signed) {
        (0, Some(s)) if s.confirmed => "Valid and signed.".to_owned(),
        (0, Some(_)) => "Valid. The signature isn't confirmed yet.".to_owned(),
        (0, None) => "Valid, waiting for your signature.".to_owned(),
        (1, _) => "1 problem.".to_owned(),
        (k, _) => format!("{k} problems."),
    };
    Ok(PacketView {
        path,
        exists: true,
        sha256: Some(Sha256::from_trusted(file.sha256.as_str())),
        header: parsed.header.and_then(|(from, to, line)| {
            (from <= 18).then(|| PacketHeader {
                line: escape_visible(
                    file.text
                        .lines()
                        .nth(usize::try_from(line.saturating_sub(1)).unwrap_or(0))
                        .unwrap_or_default(),
                ),
                from_stage: Stage(from),
                to_stage: (to <= 18).then_some(Stage(to)),
            })
        }),
        fields,
        problems: all.iter().map(problem).collect(),
        normalization: Vec::new(),
        signed,
        // The tamper diff is P2 (M4-22).
        tamper: None,
        raw,
        written_by: None,
        summary,
    })
}

/// `packet_view` (P1, mutates nothing).
///
/// # Errors
/// `NotFound`.
pub fn packet_view(app: &App, req: PacketViewReq) -> Result<PacketView, IpcError> {
    let p = project::open(app, req.project_id.as_str())?;
    view_of(&p, req.stage.0)
}

/// `packet_normalize_apply`: not built yet (M3-21: the DR-131 normalizer; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-21`).
pub fn packet_normalize_apply(
    _app: &App,
    _req: PacketNormalizeApplyReq,
) -> Result<PacketView, IpcError> {
    Err(IpcError::not_yet_available(
        "packet_normalize_apply",
        "M3-21",
    ))
}

/// `packet_signature_confirm` (P1): a packet whose `APPROVED BY` is `<name> on <date>` but wasn't
/// written by Foreman's signing counts as signed only after this native confirmation, which
/// records the packet hash, so any later change voids it (DR-135, SR-47).
///
/// # Errors
/// `Cancelled`, `Stale` (packet changed), `NotAllowed("not-signed-form" |
/// "undecided-early-condition")`, `NotFound`, `ReadOnly`.
pub fn packet_signature_confirm(
    app: &App,
    req: PacketSignatureConfirmReq,
) -> Result<PacketView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let n = req.stage.0;
    let Some(file) = p.packets.get(&n).cloned() else {
        return Err(IpcError::not_found("That packet doesn't exist yet."));
    };
    if file.sha256 != req.seen_sha256.as_str() {
        return Err(IpcError::stale(
            StaleWhat::ReadStamp,
            "The packet changed since you reviewed it.",
            serde_json::to_value(view_of(&p, n)?).ok(),
        ));
    }
    let Some(PacketSignature::Signed { name, date, .. }) = p.packet_signature(n) else {
        return Err(IpcError::not_allowed(
            "not-signed-form",
            "APPROVED BY isn't `<name> on <YYYY-MM-DD>`.",
        ));
    };
    if n == 2
        && p.parts()?
            .2
            .conditions
            .iter()
            .any(|(c, v)| !c.is_late() && *v == foreman_core::ids::TriState::Undecided)
    {
        return Err(IpcError::not_allowed(
            "undecided-early-condition",
            "Decide every early condition before Stage 2 is signed.",
        ));
    }
    let ok = app.os().confirm(&Confirm::new(
        format!("Did you sign Stage {n} yourself?"),
        format!(
            "{} says APPROVED BY: {} on {}.\nClaude writing a name never signs a packet. Confirm only if you wrote it.",
            packet::file_name(n),
            escape_visible(&name),
            escape_visible(&date)
        ),
        "I signed it",
    ));
    if !ok {
        return Err(IpcError::cancelled());
    }
    let rel = pathpolicy::check_app_data(&decisions::approvals_path(p.id()))?;
    let (_, stamp) = p.app_root.read_opt(&rel, ReadClass::AppState)?;
    let mut a = p.approvals.clone();
    a.outside_signatures.insert(
        n.to_string(),
        OutsideSignature {
            packet_sha256: file.sha256.clone(),
            signed_by: name,
            signed_on: date,
            at: app.now(),
        },
    );
    let text = schema::to_file(&a).map_err(|e| IpcError::io(e.to_string()))?;
    p.app_root.write(&rel, text.as_bytes(), &stamp)?;
    p.approvals = a;
    p.save_progress(app)?;
    view_of(&p, n)
}

/// `packet_repair_prepare`: not built yet (M4-23; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-23`).
pub fn packet_repair_prepare(
    _app: &App,
    _req: PacketRepairPrepareReq,
    _progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<PrepareOutcome, IpcError> {
    Err(IpcError::not_yet_available(
        "packet_repair_prepare",
        "M4-23",
    ))
}

/// `metrics_export`: not built yet (M3-26; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M3-26`).
pub fn metrics_export(_app: &App, _req: MetricsExportReq) -> Result<ExportResult, IpcError> {
    Err(IpcError::not_yet_available("metrics_export", "M3-26"))
}

/// `metrics_view`: not built yet (M4-03; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-03`).
pub fn metrics_view(_app: &App, _req: MetricsViewReq) -> Result<MetricsView, IpcError> {
    Err(IpcError::not_yet_available("metrics_view", "M4-03"))
}

/// `runs_clear`: not built yet (M4-02; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-02`).
pub fn runs_clear(_app: &App, _req: RunsClearReq) -> Result<(), IpcError> {
    Err(IpcError::not_yet_available("runs_clear", "M4-02"))
}

/// `runs_search`: not built yet (M4-03; IPC-R12).
///
/// # Errors
/// `Unsupported` (`notYetAvailable:M4-03`).
pub fn runs_search(_app: &App, _req: RunsSearchReq) -> Result<RunsPage, IpcError> {
    Err(IpcError::not_yet_available("runs_search", "M4-03"))
}
