//! A stage's close-out as the app sees it (F-015, F-016, F-014; DR-070–DR-074): the inspection
//! items with their recorded results and drift, the close-out facts, the gate state, the lessons
//! requirement and the signing facts. Gathered fresh for every gate and sign command (derived,
//! never stored, DR-050); `foreman_core::gate` decides.

use foreman_core::gate::{
    self, ChainSession, CloseoutFacts, ItemKind, ItemStatus, SignFacts, StageType,
};
use foreman_core::ids::{ItemId, TriState};
use foreman_core::lessons;
use foreman_core::lock::{self, Drift};
use foreman_core::schema::progress::{GateRun, GateState, ItemResult, StoredState};
use foreman_core::steps::StepState as CoreState;
use jiff::civil::Date;

use crate::ipc::IpcError;
use crate::project::{Opened, stage_of_key};
use crate::state::App;

/// One inspection item.
#[derive(Debug, Clone)]
pub struct ItemRow {
    /// Item ID.
    pub id: ItemId,
    /// The bullet as written.
    pub text: String,
    /// The kind as written.
    pub kind_raw: String,
    /// The kind v1 acts on.
    pub kind: ItemKind,
    /// The current text hash (DR-014).
    pub text_sha256: String,
    /// The recorded result.
    pub result: Option<ItemResult>,
    /// Its status.
    pub status: ItemStatus,
    /// The text drifted from the lock and its kind isn't re-confirmed (DR-157).
    pub drift: Option<Drift>,
    /// For a drifted item: the bullet that had the locked text, when another item has it now
    /// (bullets swapped without moving IDs).
    pub previous_text: Option<String>,
    /// FAILs recorded so far.
    pub failures: u32,
    /// First date a `time` item can be attested.
    pub available_from: Option<Date>,
    /// Added in the overlay (`xi-<stage>-<n>`).
    pub added: bool,
}

/// A stage's close-out.
#[derive(Debug, Clone)]
pub struct Closeout {
    /// Stage.
    pub stage: u8,
    /// Close-out type.
    pub kind: StageType,
    /// Items in template order (overlay items after).
    pub items: Vec<ItemRow>,
    /// The facts.
    pub facts: CloseoutFacts,
    /// The state they reach.
    pub state: GateState,
    /// The signing facts (the chain as it is now, not yet confirmed).
    pub sign: SignFacts,
    /// `docs/lessons.md`'s requirement for the stage (`None`: file missing).
    pub lessons: Option<lessons::Status>,
    /// The stage's first run (`time` items start counting there).
    pub first_run: Option<Date>,
    /// The previous applicable stage isn't signed yet.
    pub locked_by: Option<u8>,
}

/// `progress.json`'s gate run for a stage (empty when none).
#[must_use]
pub fn gate_run(p: &Opened, n: u8) -> GateRun {
    p.progress
        .gate_runs
        .get(&n.to_string())
        .cloned()
        .unwrap_or_else(|| GateRun {
            state: GateState::NotReady,
            items: std::collections::BTreeMap::new(),
            failures: std::collections::BTreeMap::new(),
        })
}

fn step_done(p: &Opened, key: &str) -> bool {
    p.step_progress(key).state == StoredState::Done
}

/// A live session of the stage in Foreman's run table.
#[must_use]
pub fn chain_busy(app: &App, p: &Opened, n: u8) -> bool {
    app.runs()
        .live()
        .iter()
        .any(|r| r.project_id == p.id() && stage_of_key(&r.step) == Some(n))
}

/// Reads `docs/lessons.md` (`None` when missing or unreadable).
#[must_use]
pub fn lessons_text(p: &Opened) -> Option<String> {
    let root = p.root.as_ref()?;
    lessons::read(root).ok().flatten().map(|(t, _)| t)
}

/// Gathers the close-out of stage `n`.
///
/// # Errors
/// `NotFound` (no such stage), `ReadOnly` without a template.
#[allow(
    clippy::too_many_lines,
    reason = "one gathering pass in DR-070's order"
)]
pub fn gather(app: &App, p: &Opened, n: u8) -> Result<Closeout, IpcError> {
    let (t, path, project) = p.parts()?;
    if n > 17 || !t.stages.iter().any(|s| s.number == n) {
        return Err(IpcError::not_found("That stage has no close-out."));
    }
    let kind = gate::stage_type(n);
    let run = gate_run(p, n);
    let drift = p
        .lock
        .as_ref()
        .map(|l| lock::drift(t, l, &p.approvals.item_kinds))
        .unwrap_or_default();
    let first_run = t
        .stage_steps(n)
        .filter_map(|s| p.step_progress(s.key.as_str()).started_at)
        .filter_map(|ts| ts.get(..10).and_then(|d| d.parse::<Date>().ok()))
        .min();
    let mut items = Vec::new();
    if path.stage_applies(n) {
        for i in t.items.iter().filter(|i| i.stage == n) {
            let Some(id) = i.id.as_deref().and_then(|x| ItemId::parse(x).ok()) else {
                continue;
            };
            let k = gate::parse_kind(i.effective_kind());
            let d = drift.iter().find(|d| d.id == id).cloned();
            let previous_text = d.as_ref().and_then(|d| {
                let locked = d.locked.as_deref()?;
                t.items
                    .iter()
                    .find(|o| o.text_sha256 == locked)
                    .map(|o| o.text.clone())
            });
            let result = run.items.get(&id).cloned();
            items.push(ItemRow {
                status: gate::status(result.as_ref()),
                failures: run.failures.get(id.as_str()).copied().unwrap_or(0),
                available_from: match k {
                    ItemKind::Time { .. } => gate::available_from(k, first_run),
                    ItemKind::Attest => None,
                },
                id,
                text: i.text.clone(),
                kind_raw: i.kind.clone(),
                kind: k,
                text_sha256: i.text_sha256.clone(),
                result,
                drift: d,
                previous_text,
                added: false,
            });
        }
        for a in p.overlay.added_items.iter().filter(|a| a.stage == n) {
            let Ok(id) = ItemId::parse(&a.id) else {
                continue;
            };
            let result = run.items.get(&id).cloned();
            items.push(ItemRow {
                status: gate::status(result.as_ref()),
                failures: run.failures.get(id.as_str()).copied().unwrap_or(0),
                available_from: None,
                id,
                text: a.text.clone(),
                kind_raw: "attest".into(),
                kind: ItemKind::Attest,
                text_sha256: foreman_core::template::item_hash(
                    &foreman_core::template::normalize_item_text(&a.text),
                ),
                result,
                drift: None,
                previous_text: None,
                added: true,
            });
        }
    }
    let statuses: Vec<ItemStatus> = items.iter().map(|i| i.status).collect();
    let own_steps_done = t
        .stage_steps(n)
        .filter(|s| {
            !s.template_id
                .as_deref()
                .is_some_and(|x| x.starts_with("gate.") || x == "F.2")
        })
        .all(|s| {
            matches!(
                p.display_state(app, s),
                CoreState::Done | CoreState::Skipped(_) | CoreState::NotApplicable(_)
            )
        });
    let packet_valid = p
        .packets
        .get(&n)
        .is_some_and(|f| p.packet_problems(n, &f.text).is_empty());
    let lessons = lessons_text(p).map(|text| lessons::status(&text, n));
    let facts = CloseoutFacts {
        steps_done: own_steps_done,
        gate_a_ran: step_done(p, &format!("{n}:gate.A")),
        items_resolved: gate::all_resolved(&statuses),
        gate_b_ran: step_done(p, &format!("{n}:gate.B")),
        packet_valid,
        gate_c_ran: step_done(p, &format!("{n}:gate.C")),
        lessons_found: lessons.as_ref().is_some_and(|l| l.met),
    };
    let state = gate::derive(kind, &facts);
    let git_found = app
        .spawn_ctx_now()
        .is_ok_and(|ctx| foreman_sys::git::resolve(&ctx).is_ok());
    let undecided = |late: bool| {
        project
            .conditions
            .iter()
            .filter(|(c, v)| c.is_late() == late && **v == TriState::Undecided)
            .map(|(c, _)| *c)
            .collect::<Vec<_>>()
    };
    let signed = p.signed().contains(&n) || p.progress.signing.contains_key(&n.to_string());
    let sign = SignFacts {
        stage: n,
        closeout: state,
        unresolved: items
            .iter()
            .filter(|i| matches!(i.status, ItemStatus::Fail | ItemStatus::Waiting))
            .map(|i| i.id.as_str().to_owned())
            .collect(),
        lessons_found: facts.lessons_found,
        chain: if chain_busy(app, p, n) {
            ChainSession::Busy
        } else {
            ChainSession::Idle
        },
        undecided_early: undecided(false),
        undecided_late: undecided(true),
        packet_valid,
        git_found,
        signed,
    };
    let locked_by = path
        .previous_applicable_stage(n)
        .filter(|prev| !p.signed().contains(prev));
    Ok(Closeout {
        stage: n,
        kind,
        items,
        facts,
        state,
        sign,
        lessons,
        first_run,
        locked_by,
    })
}

impl Closeout {
    /// Every reason Sign is disabled (the busy chain is confirmed in the sign dialog, so it isn't
    /// one); the locked stage first.
    #[must_use]
    pub fn why_not(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .locked_by
            .map(|s| format!("Stage {s} must be signed first."))
            .into_iter()
            .collect();
        v.extend(
            gate::sign_blockers(&self.sign)
                .iter()
                .filter(|b| !b.confirmable())
                .map(|b| b.text(self.stage)),
        );
        v
    }

    /// Sign is enabled (only a busy chain may remain; the native dialog confirms it).
    #[must_use]
    pub fn sign_enabled(&self) -> bool {
        self.locked_by.is_none() && gate::sign_enabled(&self.sign)
    }

    /// An item by ID.
    #[must_use]
    pub fn item(&self, id: &str) -> Option<&ItemRow> {
        self.items.iter().find(|i| i.id.as_str() == id)
    }
}
