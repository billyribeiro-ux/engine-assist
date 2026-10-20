//! `packets` commands (`docs/contracts/ipc.md` §4.5, F-013): the packet view, normalization and
//! hand-signature confirmation. `packet_repair_prepare` lives in `signing.rs`; the run-log commands
//! (`metrics_*`, `runs_*`) in `runs.rs`.

#![allow(
    clippy::needless_pass_by_value,
    reason = "handlers take requests by value and return the IPC Result"
)]

use std::collections::BTreeSet;

use foreman_core::decisions;
use foreman_core::packet::{self, Signature as PacketSignature};
use foreman_core::pathpolicy::{self, ReadClass, WritePurpose};
use foreman_core::schema;
use foreman_core::schema::approvals::OutsideSignature;

use crate::ipc::IpcError;
use crate::ipc::dto::records::{
    NormalizationNote, PacketFieldView, PacketHeader, PacketView, RawLine, Signature,
};
use crate::ipc::dto::requests::{
    PacketNormalizeApplyReq, PacketSignatureConfirmReq, PacketViewReq,
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

/// The format key (or `header`, or `""` for the whole file) that holds a 1-based line of `text`.
fn field_at(text: &str, keys: &[String], line: u32) -> String {
    let parsed = packet::parse_packet(text, keys);
    if parsed.header.is_some_and(|(_, _, l)| l == line) {
        return "header".to_owned();
    }
    parsed
        .fields
        .iter()
        .rev()
        .find(|f| f.line <= line)
        .map(|f| escape_visible(&f.key))
        .unwrap_or_default()
}

/// DR-131 notes for a file Claude wrote, one per change kind and field (AC-F013-08: shown before
/// anything is written). Empty when normalizing would record nothing.
fn normalization_notes(text: &str, keys: &[String]) -> Vec<NormalizationNote> {
    let n = packet::normalize_packet(text);
    let mut out: Vec<NormalizationNote> = Vec::new();
    for note in &n.notes {
        let mut fields: Vec<String> = note
            .lines
            .iter()
            .map(|l| field_at(&n.text, keys, *l))
            .collect();
        if fields.is_empty() {
            fields.push(String::new());
        }
        fields.dedup();
        for field in fields {
            let x = NormalizationNote {
                field,
                message: note.message.clone(),
            };
            if !out.contains(&x) {
                out.push(x);
            }
        }
    }
    out
}

/// AC-F004-12: a Stage 2 packet signed by hand while an early condition is undecided doesn't
/// count; this is the problem the packet view shows.
fn undecided_problem(p: &Opened, n: u8, line: u32) -> Option<Problem> {
    if n != 2 {
        return None;
    }
    let (_, _, project) = p.parts().ok()?;
    let undecided: Vec<String> = project
        .conditions
        .iter()
        .filter(|(c, v)| !c.is_late() && **v == foreman_core::ids::TriState::Undecided)
        .map(|(c, _)| c.to_string())
        .collect();
    (!undecided.is_empty()).then(|| Problem {
        rule: "packet.undecided-early-condition".to_owned(),
        line: Some(line),
        message: format!("Decide {} before signing Stage 2", undecided.join(", ")),
        fix: None,
    })
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
    let mut problems: Vec<Problem> = all.iter().map(problem).collect();
    if signed
        .as_ref()
        .is_some_and(|s| !s.confirmed && !s.by_foreman)
        && let Some(x) = undecided_problem(p, n, parsed.get("APPROVED BY").map_or(1, |f| f.line))
    {
        problems.push(x);
    }
    let normalization = normalization_notes(&file.text, &keys);
    let errors = all.iter().filter(|x| !x.warning).count()
        + problems
            .iter()
            .filter(|x| x.rule == "packet.undecided-early-condition")
            .count();
    let mut summary = match (errors, &signed) {
        (0, Some(s)) if s.confirmed => "Valid and signed.".to_owned(),
        (0, Some(_)) => "Valid. The signature isn't confirmed yet.".to_owned(),
        (0, None) => "Valid, waiting for your signature.".to_owned(),
        (1, _) => "1 problem.".to_owned(),
        (k, _) => format!("{k} problems."),
    };
    if !normalization.is_empty() {
        summary.push_str(" Normalizing the file fixes its formatting.");
    }
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
        problems,
        normalization,
        signed,
        // M4-22: a signed packet whose working copy differs from its commit (committed → working).
        tamper: crate::signing::tamper_diff(p, n),
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

/// `packet_normalize_apply` (P1): writes the DR-131 normalization of a packet Claude wrote, after
/// Billy reviewed the notes in `packet_view` (AC-F013-08). The file must still be the one he saw
/// (`seenSha256`), and the write is a compare-and-swap against the stamp read now, so an edit in
/// between is a conflict, never overwritten (SR-33). Only the changes §5.2 lists are made.
///
/// # Errors
/// `Stale` (the packet changed since it was shown; the fresh view is in `detail`), `Conflict`
/// (it changed during the write), `NotFound`, `ReadOnly`, `PolicyViolation`, `Io`.
pub fn packet_normalize_apply(
    app: &App,
    req: PacketNormalizeApplyReq,
) -> Result<PacketView, IpcError> {
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let n = req.stage.0;
    if n > 17 {
        return Err(IpcError::not_found("That stage has no packet."));
    }
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
    let normalized = packet::normalize_packet(&file.text);
    if normalized.text == file.text {
        return view_of(&p, n);
    }
    let rel = pathpolicy::check_write(&packet::file_name(n), WritePurpose::Repo)?;
    let stamp = p
        .root()?
        .write(&rel, normalized.text.as_bytes(), &file.stamp)?;
    let bytes = normalized.text.into_bytes();
    p.packets.insert(
        n,
        project::PacketFile {
            sha256: foreman_core::store::sha256_hex(&bytes),
            text: String::from_utf8_lossy(&bytes).into_owned(),
            stamp,
        },
    );
    crate::watch::emit_packet_changed(app, p.id(), n);
    view_of(&p, n)
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
