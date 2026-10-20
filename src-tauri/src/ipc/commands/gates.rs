//! `gates` commands (`docs/contracts/ipc.md` §4.11; F-015; M4-16, M4-17): the gate view, item
//! attestation, Gate A's Import results (proposals only, each confirmed by Billy) and Fix this.
//! Every rule is `foreman_core::gate`'s; facts come from [`crate::closeout`].

#![allow(
    clippy::needless_pass_by_value,
    reason = "handlers take requests by value and return the IPC Result"
)]

use foreman_core::gate::{self, Attest, AttestCtx, ItemKind, ItemStatus as CoreStatus, StageType};
use foreman_core::ids::ItemId as CoreItemId;
use foreman_core::sanitize;
use foreman_core::schema::progress::{GateState as CoreGate, ItemOutcome, ItemResult};

use crate::closeout::{self, Closeout, ItemRow};
use crate::ipc::IpcError;
use crate::ipc::dto::gates::{
    CloseoutKey, CloseoutState, CloseoutStep, GateItemView, GateState, GateView, ImportProposal,
    ImportResult, ItemKind as DtoKind, ItemStatus,
};
use crate::ipc::dto::launch::{LaunchProgress, PrepareOutcome};
use crate::ipc::dto::requests::{
    AttestOutcome, GateFixStartReq, GateImportConfirmReq, GateImportResultsReq, GateItemAttestReq,
    GateViewReq, PassFail,
};
use crate::ipc::types::{ItemId, Sha256};
use crate::project::{self, Opened};
use crate::render::{escape_visible, spans};
use crate::state::App;

fn status(s: CoreStatus) -> ItemStatus {
    match s {
        CoreStatus::Fail => ItemStatus::Fail,
        CoreStatus::Waiting => ItemStatus::Waiting,
        CoreStatus::Pass => ItemStatus::Pass,
        CoreStatus::NotApplicable => ItemStatus::NotApplicable,
    }
}

fn item_view(c: &Closeout, i: &ItemRow) -> GateItemView {
    let r = i.result.as_ref();
    let imported = r.is_some_and(|r| matches!(r.outcome, ItemOutcome::Pass | ItemOutcome::Fail));
    GateItemView {
        id: ItemId::from_trusted(i.id.as_str()),
        text: escape_visible(&i.text),
        kind: match i.kind {
            ItemKind::Attest => DtoKind::Attest,
            ItemKind::Time { .. } => DtoKind::Time,
        },
        how: None,
        status: status(i.status),
        evidence: imported.then(|| r.map(|r| spans(&r.text))).flatten(),
        proposed: None,
        note: r
            .filter(|r| matches!(r.outcome, ItemOutcome::Done | ItemOutcome::NotApplicable))
            .map(|r| escape_visible(&r.text)),
        waits_for: match (i.kind, i.status) {
            (ItemKind::Time { .. }, CoreStatus::Waiting) => Some(match i.available_from {
                Some(d) => format!("Available from {d}"),
                None => "Available after the stage's first run".to_owned(),
            }),
            (_, CoreStatus::Waiting) if c.kind == StageType::Gated && !c.facts.gate_a_ran => {
                Some("Run Gate A, then attest".to_owned())
            }
            _ => None,
        },
        artifacts: Vec::new(),
        failures: i.failures,
        needs_reconfirm: i.drift.is_some(),
        session: None,
        text_sha256: Sha256::from_trusted(i.text_sha256.as_str()),
        previous_text: i.previous_text.as_deref().map(escape_visible),
    }
}

fn step(key: CloseoutKey, label: &str, state: CloseoutState, note: Option<&str>) -> CloseoutStep {
    CloseoutStep {
        key,
        label: label.to_owned(),
        state,
        note: note.map(str::to_owned),
    }
}

/// The close-out sequence for the stage's type (DR-070, AC-F015-01, AC-F012-16).
fn closeout_steps(c: &Closeout, signed: bool, pending: bool) -> Vec<CloseoutStep> {
    use CloseoutState as S;
    let idx = |g: CoreGate| gate::SEQUENCE.iter().position(|x| *x == g).unwrap_or(0);
    let at = idx(c.state);
    let failed = c.items.iter().any(|i| i.status == CoreStatus::Fail);
    let waiting = c
        .items
        .iter()
        .filter(|i| i.status == CoreStatus::Waiting)
        .count();
    let state_for = |reached: CoreGate| {
        if at >= idx(reached) {
            S::Done
        } else if at + 1 == idx(reached) {
            S::Now
        } else {
            S::Pending
        }
    };
    let results_label = if failed {
        format!(
            "Results: {} failed",
            c.items
                .iter()
                .filter(|i| i.status == CoreStatus::Fail)
                .count()
        )
    } else if waiting > 0 {
        format!("Results: {waiting} waiting")
    } else {
        "Results: all passed".to_owned()
    };
    let results_state = if failed {
        S::Failed
    } else {
        state_for(CoreGate::Results)
    };
    let lessons_note = format!(
        "Needs an entry headed \"## L-NNN · Stage {} · …\" in docs/lessons.md.",
        c.stage
    );
    let sign_state = if signed {
        S::Done
    } else if c.state == CoreGate::Signable && c.locked_by.is_none() {
        S::Now
    } else {
        S::Locked
    };
    let sign = if pending {
        step(
            CloseoutKey::Sign,
            "Signed (commit pending)",
            S::Failed,
            Some(PENDING_NOTE),
        )
    } else {
        step(
            CloseoutKey::Sign,
            if signed { "Signed" } else { "Sign" },
            sign_state,
            None,
        )
    };
    let mut v = Vec::new();
    match c.kind {
        StageType::Gated => {
            v.push(step(
                CloseoutKey::GateA,
                "Gate A inspection",
                state_for(CoreGate::A),
                Some("Runs in the terminal in its own chain."),
            ));
            v.push(CloseoutStep {
                key: CloseoutKey::Results,
                label: results_label,
                state: results_state,
                note: None,
            });
            v.push(step(
                CloseoutKey::GateB,
                "Gate B packet",
                state_for(CoreGate::PacketValid),
                Some(&format!(
                    "Continues the Gate A chain. Writes {}.",
                    foreman_core::packet::file_name(c.stage)
                )),
            ));
            v.push(step(
                CloseoutKey::GateC,
                "Gate C lessons",
                state_for(CoreGate::LessonsFound),
                Some(&format!("Continues the gate chain. {lessons_note}")),
            ));
        }
        StageType::Chat | StageType::Council => {
            v.push(step(
                CloseoutKey::GateB,
                "Packet valid",
                state_for(CoreGate::A),
                Some(&format!(
                    "{} has no problems.",
                    foreman_core::packet::file_name(c.stage)
                )),
            ));
            v.push(CloseoutStep {
                key: CloseoutKey::Results,
                label: results_label,
                state: results_state,
                note: Some("Attest every inspection item.".to_owned()),
            });
            v.push(step(
                CloseoutKey::GateC,
                "F.2 lessons",
                state_for(CoreGate::LessonsFound),
                Some(&if c.kind == StageType::Council {
                    format!("2:F.2 continues the 2.2 chain. {lessons_note}")
                } else {
                    format!("Paste {}:F.2's lessons back. {lessons_note}", c.stage)
                }),
            ));
        }
    }
    v.push(sign);
    v
}

/// The sign step's note while a signing commit is pending (AC-F014-11, -13).
pub const PENDING_NOTE: &str =
    "APPROVED BY is written, but the commit didn't finish. Retry commit.";

/// The `GateView` of a close-out.
#[must_use]
pub fn view(p: &Opened, c: &Closeout) -> GateView {
    let signed = p.signed().contains(&c.stage);
    let pending = p
        .progress
        .signing
        .get(&c.stage.to_string())
        .is_some_and(|s| s.state == foreman_core::schema::progress::SigningState::CommitPending);
    let statuses: Vec<CoreStatus> = c.items.iter().map(|i| i.status).collect();
    let items = gate::order(&statuses)
        .into_iter()
        .map(|i| item_view(c, &c.items[i]))
        .collect();
    let any_result = c.items.iter().any(|i| i.result.is_some());
    let state = if signed {
        GateState::Signed
    } else if statuses.contains(&CoreStatus::Fail) {
        GateState::Failed
    } else if !statuses.is_empty() && gate::all_resolved(&statuses) {
        GateState::Passed
    } else if any_result || c.facts.gate_a_ran {
        GateState::InProgress
    } else {
        GateState::NotStarted
    };
    let enabled = !signed && c.sign_enabled();
    GateView {
        stage: crate::ipc::types::Stage(c.stage),
        state,
        items,
        closeout: closeout_steps(c, signed, pending),
        signable: enabled,
        why_not: if pending {
            vec![PENDING_NOTE.to_owned()]
        } else if enabled || signed {
            Vec::new()
        } else {
            c.why_not()
        },
        imported_from: None,
    }
}

fn open_closeout(app: &App, id: &str, stage: u8) -> Result<(Opened, Closeout), IpcError> {
    let p = project::open(app, id)?;
    let c = closeout::gather(app, &p, stage)?;
    Ok((p, c))
}

/// Saves a gate run (results, failures and the derived state) and returns the fresh view.
fn save(
    app: &App,
    p: &mut Opened,
    stage: u8,
    run: foreman_core::schema::progress::GateRun,
) -> Result<GateView, IpcError> {
    p.progress.gate_runs.insert(stage.to_string(), run);
    let c = closeout::gather(app, p, stage)?;
    if let Some(r) = p.progress.gate_runs.get_mut(&stage.to_string()) {
        r.state = c.state;
    }
    p.save_progress(app)?;
    Ok(view(p, &c))
}

fn refuse_signed(p: &Opened, stage: u8) -> Result<(), IpcError> {
    if p.signed().contains(&stage) || p.progress.signing.contains_key(&stage.to_string()) {
        return Err(IpcError::not_allowed(
            "stage-signed",
            "This stage is signed; its gate can't change.",
        ));
    }
    Ok(())
}

/// `gate_view` (mutates nothing).
///
/// # Errors
/// `NotFound`, `ReadOnly` without a template.
pub fn gate_view(app: &App, req: GateViewReq) -> Result<GateView, IpcError> {
    let (p, c) = open_closeout(app, req.project_id.as_str(), req.stage.0)?;
    Ok(view(&p, &c))
}

fn check_link(link: Option<&str>) -> Result<Option<String>, IpcError> {
    match link {
        None => Ok(None),
        Some(l) if l.trim().is_empty() => Ok(None),
        Some(l)
            if l.len() <= 2048
                && !l.chars().any(char::is_control)
                && (l.starts_with("https://")
                    || l.starts_with("docs/")
                    || l.starts_with("spec/")) =>
        {
            Ok(Some(l.to_owned()))
        }
        Some(_) => Err(IpcError::invalid("link", "link")),
    }
}

/// `gate_item_attest` (DR-073, AC-F015-02/-03, AC-F001-17): Done with a note, Not applicable
/// with a reason, or Clear.
///
/// # Errors
/// `NotAllowed` (`note-too-short`, `note-repeated`, `time-not-elapsed`, `reason-required`,
/// `kind-needs-reconfirm`, `stage-signed`), `NotFound`, `ReadOnly`.
pub fn gate_item_attest(app: &App, req: GateItemAttestReq) -> Result<GateView, IpcError> {
    let _w = app.writing();
    let n = req.stage.0;
    let (mut p, c) = open_closeout(app, req.project_id.as_str(), n)?;
    p.writable()?;
    refuse_signed(&p, n)?;
    let Some(item) = c.item(req.item.as_str()) else {
        return Err(IpcError::not_found("That item isn't in this stage's gate."));
    };
    let attest = match &req.outcome {
        AttestOutcome::Done(d) => Attest::Done {
            note: d.note.as_str().to_owned(),
            link: check_link(d.link.as_deref())?,
        },
        AttestOutcome::NotApplicable(r) => Attest::NotApplicable {
            reason: r.reason.as_str().to_owned(),
        },
        AttestOutcome::Clear(_) => Attest::Clear,
    };
    let mut run = closeout::gate_run(&p, n);
    let ctx = AttestCtx {
        item: &item.id,
        kind: item.kind,
        needs_reconfirm: item.drift.is_some(),
        first_run: c.first_run,
        today: app.os().today(),
        results: &run.items,
    };
    gate::check_attest(&ctx, &attest)
        .map_err(|e| IpcError::not_allowed(e.rule(), e.to_string()))?;
    let id = item.id.clone();
    match attest {
        Attest::Clear => {
            run.items.remove(&id);
        }
        Attest::Done { note, link } => {
            let text = match (item.kind, item.available_from) {
                (
                    ItemKind::Time {
                        then_attest: false, ..
                    },
                    Some(d),
                ) if note.trim().is_empty() => {
                    format!("waited until {d}")
                }
                _ => note.trim().to_owned(),
            };
            run.items.insert(
                id,
                ItemResult {
                    outcome: ItemOutcome::Done,
                    text,
                    link,
                    at: app.now(),
                },
            );
        }
        Attest::NotApplicable { reason } => {
            run.items.insert(
                id,
                ItemResult {
                    outcome: ItemOutcome::NotApplicable,
                    text: reason.trim().to_owned(),
                    link: None,
                    at: app.now(),
                },
            );
        }
    }
    save(app, &mut p, n, run)
}

/// `gate_import_results` (AC-F015-05): proposes PASS/FAIL per item from the sensor's last message
/// of the Gate A run. Writes nothing; each proposal needs `gate_import_confirm`.
///
/// # Errors
/// `Unsupported` (no sensor for that run), `NotFound` (Gate A hasn't run).
pub fn gate_import_results(
    app: &App,
    req: GateImportResultsReq,
) -> Result<Vec<ImportProposal>, IpcError> {
    let n = req.stage.0;
    let (p, c) = open_closeout(app, req.project_id.as_str(), n)?;
    let key = format!("{n}:gate.A");
    let Some(run_id) = p.step_progress(&key).last_run_id else {
        return Err(IpcError::not_found("Gate A hasn't run for this stage."));
    };
    let no_sensor = || {
        IpcError::unsupported(
            "sensor",
            "no-sensor",
            "Foreman has no session events for Gate A's run, so mark each item yourself.",
        )
    };
    let (file, _) = crate::runs::read_run(&p.app_root, &run_id)?;
    let Some(token) = file.sensor_token else {
        return Err(no_sensor());
    };
    let state = foreman_sys::sensor::ingest(&p.app_root, &run_id, &token, Default::default())
        .map_err(|_| no_sensor())?;
    let Some(message) = state.last_assistant_message else {
        return Err(no_sensor());
    };
    let items: Vec<(CoreItemId, String)> = c
        .items
        .iter()
        .map(|i| {
            (
                i.id.clone(),
                foreman_core::template::normalize_item_text(&i.text),
            )
        })
        .collect();
    Ok(gate::propose(&items, &message)
        .into_iter()
        .map(|(id, prop)| {
            let (proposed, evidence) = match prop {
                gate::Proposal::Pass(e) => (ImportResult::Pass, spans(&e)),
                gate::Proposal::Fail(e) => (ImportResult::Fail, spans(&e)),
                gate::Proposal::Unmatched => (
                    ImportResult::Unmatched,
                    spans("No line in Gate A's last message names this item with PASS or FAIL."),
                ),
            };
            ImportProposal {
                item: ItemId::from_trusted(id.as_str()),
                proposed,
                evidence,
            }
        })
        .collect())
}

/// `gate_import_confirm` (AC-F015-05): Billy's PASS or FAIL for one item, with its evidence. A
/// FAIL counts toward the F.4 prompt (DR-073).
///
/// # Errors
/// `NotFound`, `NotAllowed("stage-signed")`, `ReadOnly`.
pub fn gate_import_confirm(app: &App, req: GateImportConfirmReq) -> Result<GateView, IpcError> {
    let _w = app.writing();
    let n = req.stage.0;
    let (mut p, c) = open_closeout(app, req.project_id.as_str(), n)?;
    p.writable()?;
    refuse_signed(&p, n)?;
    let Some(item) = c.item(req.item.as_str()) else {
        return Err(IpcError::not_found("That item isn't in this stage's gate."));
    };
    let mut run = closeout::gate_run(&p, n);
    let outcome = match req.result {
        PassFail::Pass => ItemOutcome::Pass,
        PassFail::Fail => {
            gate::record_failure(&mut run.failures, &item.id);
            ItemOutcome::Fail
        }
    };
    run.items.insert(
        item.id.clone(),
        ItemResult {
            outcome,
            text: req.evidence.as_str().trim().to_owned(),
            link: None,
            at: app.now(),
        },
    );
    save(app, &mut p, n, run)
}

/// `gate_fix_start` (AC-F015-07): a new CLI chain (Opus 5.5 · high · Plan, then Accept edits)
/// prefilled with the failed item and its evidence; on the item's third failure, the F.4 prompt
/// instead. Prepares only: Billy starts it with `launch_start` (its confirmations apply).
///
/// # Errors
/// `NotAllowed("item-not-failed")`, `NotFound`, `ReadOnly`.
pub fn gate_fix_start(
    app: &App,
    req: GateFixStartReq,
    progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<PrepareOutcome, IpcError> {
    let n = req.stage.0;
    let (mut p, c) = open_closeout(app, req.project_id.as_str(), n)?;
    p.writable()?;
    let Some(item) = c.item(req.item.as_str()) else {
        return Err(IpcError::not_found("That item isn't in this stage's gate."));
    };
    if item.status != CoreStatus::Fail {
        return Err(IpcError::not_allowed(
            "item-not-failed",
            "Fix this is for an item that failed.",
        ));
    }
    // Repo and pasted text is stripped of control and escape sequences before it reaches a
    // session (SR-28).
    let text = sanitize::for_paste(&item.text).0;
    let evidence = sanitize::for_paste(item.result.as_ref().map_or("", |r| r.text.as_str())).0;
    let f4 = gate::shows_f4(item.failures);
    let prompt = if f4 {
        gate::f4_prompt(&text, item.failures)
    } else {
        gate::fix_prompt(n, &text, &evidence)
    };
    // The fix runs where Gate A runs (the project root), outside the gate chain.
    let key = if c.kind == StageType::Gated {
        format!("{n}:gate.A")
    } else {
        let (t, path, _) = p.parts()?;
        foreman_core::rules::packet_producer(t, path, n)
            .map(|s| s.key.to_string())
            .ok_or_else(|| IpcError::not_found("This stage has no CLI step to fix from."))?
    };
    let custom = crate::launch::Custom {
        prompt,
        start: gate::FIX_START,
        resume: None,
        detached: true,
    };
    let send = |m: LaunchProgress| {
        let _ = progress.send(m);
    };
    let mut prepared = crate::launch::prepare_custom(app, &mut p, &key, &custom, &send)?;
    if f4 && let PrepareOutcome::Ready(pv) = &mut prepared.outcome {
        pv.warnings.insert(
            0,
            format!(
                "This item has failed {} times, so this is the F.4 prompt: fix the cause, not the item.",
                item.failures
            ),
        );
    }
    Ok(crate::launch::hold(app, prepared))
}

/// `item_kind_confirm` (DR-157, AC-F001-16/-17, SR-24a): a drifted item (reworded, or bullets
/// swapped without moving IDs) stays blocked for attesting until Billy confirms its kind in a
/// native dialog raised here; the webview only shows the old and new text as context. The
/// confirmation binds the item's current text hash in `approvals` `itemKinds`, so a later
/// rewording needs a new one.
///
/// # Errors
/// `Cancelled`, `Stale` (the bullet changed again; the fresh gate view in `detail`), `NotFound`,
/// `ReadOnly`.
pub fn item_kind_confirm(
    app: &App,
    req: crate::ipc::dto::requests::ItemKindConfirmReq,
) -> Result<GateView, IpcError> {
    use foreman_core::pathpolicy::{self, ReadClass};
    use foreman_core::schema::approvals::ItemKindConfirm;
    let _w = app.writing();
    let mut p = project::open(app, req.project_id.as_str())?;
    p.writable()?;
    let (t, _, _) = p.parts()?;
    let stage = t
        .items
        .iter()
        .find(|i| i.id.as_deref() == Some(req.item.as_str()))
        .map(|i| i.stage)
        .or_else(|| {
            p.overlay
                .added_items
                .iter()
                .find(|a| a.id == req.item.as_str())
                .map(|a| a.stage)
        })
        .ok_or_else(|| IpcError::not_found("That item isn't in this project's blueprint."))?;
    let c = closeout::gather(app, &p, stage)?;
    let Some(item) = c.item(req.item.as_str()).cloned() else {
        return Err(IpcError::not_found(
            "That item isn't in an applicable stage.",
        ));
    };
    if item.text_sha256 != req.text_sha256.as_str() {
        return Err(IpcError::stale(
            crate::ipc::error::StaleWhat::ReadStamp,
            "The item's text changed again. Review it once more.",
            serde_json::to_value(view(&p, &c)).ok(),
        ));
    }
    if item.drift.is_none() {
        return Ok(view(&p, &c));
    }
    let before = item.previous_text.as_deref().map_or_else(
        || "(the pinned text isn't in the blueprint any more)".to_owned(),
        escape_visible,
    );
    let ok = app.os().confirm(&crate::os::Confirm::new(
        format!("Confirm the kind of {}?", item.id),
        format!(
            "The text of inspection item {} changed since the blueprint was pinned.\n\nPinned: {}\nNow: {}\n\nKind: {}. Confirm only if that kind is still right for the new text.",
            item.id,
            before,
            escape_visible(&item.text),
            escape_visible(&item.kind_raw),
        ),
        "Confirm kind",
    ));
    if !ok {
        return Err(IpcError::cancelled());
    }
    let rel = pathpolicy::check_app_data(&foreman_core::decisions::approvals_path(p.id()))?;
    let (_, stamp) = p.app_root.read_opt(&rel, ReadClass::AppState)?;
    let mut a = p.approvals.clone();
    a.item_kinds.insert(
        item.id.clone(),
        ItemKindConfirm {
            text_sha256: item.text_sha256.clone(),
            kind: item.kind_raw.clone(),
            at: app.now(),
        },
    );
    let text = foreman_core::schema::to_file(&a).map_err(|e| IpcError::io(e.to_string()))?;
    p.app_root.write(&rel, text.as_bytes(), &stamp)?;
    p.approvals = a;
    p.save_progress(app)?;
    let c = closeout::gather(app, &p, stage)?;
    Ok(view(&p, &c))
}
