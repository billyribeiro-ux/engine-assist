//! Gates and close-out (F-015, DR-070–DR-073, DR-071): the per-stage close-out sequence, the CLI
//! gate state machine, item kinds and attestation rules, the review order, and the signable
//! predicate. Pure: facts (step states, the packet's validity, the lessons file, the chain's
//! session, today's date) are gathered by the app and passed in.

use std::collections::BTreeMap;

use jiff::civil::Date;

use crate::ids::{ConditionId, ItemId};
use crate::schema::progress::{GateState, ItemOutcome, ItemResult};

/// A stage's close-out type (DR-070, MASTER-SPEC §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageType {
    /// Chat stages 0, 1, 3, 4: packet valid → items → `N:F.2` lessons → signable.
    Chat,
    /// Stage 2 (CLI council): 2.6's packet → items → `2:F.2` in the 2.2 chain → signable.
    Council,
    /// Stages 5–17: the gate machine (`notReady → a → results → b → packetValid → c →
    /// lessonsFound → signable`).
    Gated,
}

/// The close-out type of stage `n`.
#[must_use]
pub const fn stage_type(n: u8) -> StageType {
    match n {
        2 => StageType::Council,
        5..=17 => StageType::Gated,
        _ => StageType::Chat,
    }
}

/// The CLI gate sequence, in order (DR-070).
pub const SEQUENCE: [GateState; 8] = [
    GateState::NotReady,
    GateState::A,
    GateState::Results,
    GateState::B,
    GateState::PacketValid,
    GateState::C,
    GateState::LessonsFound,
    GateState::Signable,
];

fn index(s: GateState) -> usize {
    SEQUENCE.iter().position(|x| *x == s).unwrap_or(0)
}

/// A refused gate transition, with the reason shown to Billy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct OutOfOrder(pub String);

/// What has to happen before `to` (the reason an out-of-order move is refused).
#[must_use]
pub fn needs_before(to: GateState) -> &'static str {
    match to {
        GateState::NotReady => "The gate can't go back",
        GateState::A => "Finish the stage's steps, then run Gate A",
        GateState::Results => "Run Gate A first",
        GateState::B => "Resolve every inspection item first",
        GateState::PacketValid => "Run Gate B first",
        GateState::C => "Gate B's packet must be valid first",
        GateState::LessonsFound => "Run Gate C first",
        GateState::Signable => "Gate C must write a lessons entry for this stage first",
    }
}

/// DR-070: the only allowed move is to the next state in [`SEQUENCE`]; every other (from, to)
/// pair, including staying put, going back and skipping ahead, is refused.
///
/// # Errors
/// [`OutOfOrder`] naming what must happen first.
pub fn transition(from: GateState, to: GateState) -> Result<GateState, OutOfOrder> {
    if index(to) == index(from) + 1 {
        Ok(to)
    } else {
        Err(OutOfOrder(needs_before(to).to_owned()))
    }
}

/// Facts the close-out sequence is derived from.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent facts read off the project, each one of DR-070's conditions"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CloseoutFacts {
    /// The stage's own steps are done (Gated: Gate A can run).
    pub steps_done: bool,
    /// Gate A ran (Gated only).
    pub gate_a_ran: bool,
    /// Every applicable item is resolved (DR-073).
    pub items_resolved: bool,
    /// Gate B ran (Gated only).
    pub gate_b_ran: bool,
    /// The stage's packet is valid (DR-133).
    pub packet_valid: bool,
    /// Gate C ran (Gated only).
    pub gate_c_ran: bool,
    /// A lessons entry for the stage exists (DR-074).
    pub lessons_found: bool,
}

/// The gate state the facts reach, walking [`SEQUENCE`] through [`transition`] only: a later
/// fact never counts while an earlier one is missing (Gate B before Gate A's results stays at
/// `a`). For chat and council stages the same walk runs with the gate runs taken as given, so
/// their order is packet valid → items → lessons.
#[must_use]
pub fn derive(kind: StageType, f: &CloseoutFacts) -> GateState {
    let gated = kind == StageType::Gated;
    let mut s = GateState::NotReady;
    for next in SEQUENCE.iter().skip(1) {
        let holds = match next {
            GateState::A => {
                if gated {
                    f.steps_done && f.gate_a_ran
                } else {
                    // Chat and council stages start close-out from a valid packet.
                    f.packet_valid
                }
            }
            GateState::Results => f.items_resolved,
            GateState::B => !gated || f.gate_b_ran,
            GateState::PacketValid => f.packet_valid,
            GateState::C => !gated || f.gate_c_ran,
            GateState::LessonsFound => f.lessons_found,
            GateState::Signable => true,
            GateState::NotReady => false,
        };
        if !holds {
            break;
        }
        match transition(s, *next) {
            Ok(n) => s = n,
            Err(_) => break,
        }
    }
    s
}

/// An item's kind as v1 acts on it (DR-031).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// `attest` (and `auto:…`, which v1 attests).
    Attest,
    /// `time:<n>d`, optionally `>attest`.
    Time {
        /// Days after the stage's first run.
        days: u16,
        /// Attested once available (`time:<n>d>attest`).
        then_attest: bool,
    },
}

/// Parses an item kind as written (DR-031); anything unknown is attested.
#[must_use]
pub fn parse_kind(s: &str) -> ItemKind {
    if let Some(rest) = s.strip_prefix("time:") {
        let (days, then) = match rest.split_once('>') {
            Some((d, t)) => (d, t == "attest"),
            None => (rest, false),
        };
        if let Some(n) = days.strip_suffix('d').and_then(|d| d.parse::<u16>().ok())
            && (1..=365).contains(&n)
        {
            return ItemKind::Time {
                days: n,
                then_attest: then,
            };
        }
    }
    ItemKind::Attest
}

/// The kind's name in the stage record and `approvals` (`attest` or `time`).
#[must_use]
pub const fn kind_name(k: ItemKind) -> &'static str {
    match k {
        ItemKind::Attest => "attest",
        ItemKind::Time { .. } => "time",
    }
}

/// An item's status in the gate view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemStatus {
    /// Gate A (or Billy) recorded a FAIL.
    Fail,
    /// Waits for Billy's attestation (or for its date).
    Waiting,
    /// PASS or attested done.
    Pass,
    /// Attested not applicable (excluded from the gate, DR-072).
    NotApplicable,
}

/// The status of an item with its recorded result.
#[must_use]
pub fn status(result: Option<&ItemResult>) -> ItemStatus {
    match result.map(|r| r.outcome) {
        None => ItemStatus::Waiting,
        Some(ItemOutcome::Fail) => ItemStatus::Fail,
        Some(ItemOutcome::Done | ItemOutcome::Pass) => ItemStatus::Pass,
        Some(ItemOutcome::NotApplicable) => ItemStatus::NotApplicable,
    }
}

/// DR-072: FAIL, then waiting, then PASS/done and not-applicable (collapsed), each group in
/// template order. Returns indexes into `statuses`.
#[must_use]
pub fn order(statuses: &[ItemStatus]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..statuses.len()).collect();
    idx.sort_by_key(|i| {
        let rank = match statuses[*i] {
            ItemStatus::Fail => 0,
            ItemStatus::Waiting => 1,
            ItemStatus::Pass | ItemStatus::NotApplicable => 2,
        };
        (rank, *i)
    });
    idx
}

/// DR-072, AC-F015-09: the items the gate counts (not-applicable items are excluded).
#[must_use]
pub fn counted(statuses: &[ItemStatus]) -> usize {
    statuses
        .iter()
        .filter(|s| **s != ItemStatus::NotApplicable)
        .count()
}

/// DR-073: every applicable item is resolved (PASS, done or not applicable; never FAIL or
/// waiting).
#[must_use]
pub fn all_resolved(statuses: &[ItemStatus]) -> bool {
    statuses
        .iter()
        .all(|s| matches!(s, ItemStatus::Pass | ItemStatus::NotApplicable))
}

/// The minimum Done note length after trimming (DR-073).
pub const MIN_NOTE: usize = 15;
/// On this failure of the same item, the F.4 prompt is shown (DR-073).
pub const F4_AT_FAILURE: u32 = 3;

/// How notes are compared for repetition: lowercased, whitespace runs collapsed, trimmed.
#[must_use]
pub fn normalize_note(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// An attestation Billy asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attest {
    /// Done, with a note and an optional link.
    Done {
        /// How he checked.
        note: String,
        /// A link (optional).
        link: Option<String>,
    },
    /// Not applicable, with a reason.
    NotApplicable {
        /// Why.
        reason: String,
    },
    /// Clear the item's result.
    Clear,
}

/// Why an attestation was refused (`NotAllowed` rule codes, ipc.md §4.11).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttestError {
    /// A Done note under 15 characters.
    #[error("Write at least 15 characters on how you checked.")]
    NoteTooShort,
    /// The same note as another item in this gate.
    #[error("That note repeats another item's. Say how you checked this one.")]
    NoteRepeated,
    /// A `time` item before its date.
    #[error("This item can be attested from {0}.")]
    TimeNotElapsed(Date),
    /// Not applicable without a reason.
    #[error("Say why it doesn't apply.")]
    ReasonRequired,
    /// The item's text drifted and its kind isn't re-confirmed (DR-157).
    #[error("The item text changed. Re-confirm its kind first.")]
    KindNeedsReconfirm,
}

impl AttestError {
    /// The stable rule code.
    #[must_use]
    pub const fn rule(&self) -> &'static str {
        match self {
            Self::NoteTooShort => "note-too-short",
            Self::NoteRepeated => "note-repeated",
            Self::TimeNotElapsed(_) => "time-not-elapsed",
            Self::ReasonRequired => "reason-required",
            Self::KindNeedsReconfirm => "kind-needs-reconfirm",
        }
    }
}

/// The first date a `time:<n>d` item is available: `n` days after the stage's first run.
/// `None` when the stage hasn't run yet (not available at all).
#[must_use]
pub fn available_from(kind: ItemKind, first_run: Option<Date>) -> Option<Date> {
    match kind {
        ItemKind::Attest => Some(Date::MIN),
        ItemKind::Time { days, .. } => {
            first_run.and_then(|d| d.checked_add(jiff::Span::new().days(i64::from(days))).ok())
        }
    }
}

/// What an attestation check needs.
#[derive(Debug, Clone)]
pub struct AttestCtx<'a> {
    /// The item.
    pub item: &'a ItemId,
    /// Its kind.
    pub kind: ItemKind,
    /// The text drifted without a confirmed kind (DR-157).
    pub needs_reconfirm: bool,
    /// The stage's first run (`time` items).
    pub first_run: Option<Date>,
    /// Today (local).
    pub today: Date,
    /// Every recorded result in this gate.
    pub results: &'a BTreeMap<ItemId, ItemResult>,
}

/// DR-073, AC-F015-02/-03, AC-F001-17: checks an attestation before it is recorded.
///
/// # Errors
/// The first rule that refuses it.
pub fn check_attest(c: &AttestCtx<'_>, a: &Attest) -> Result<(), AttestError> {
    if matches!(a, Attest::Clear) {
        return Ok(());
    }
    if c.needs_reconfirm {
        return Err(AttestError::KindNeedsReconfirm);
    }
    if let ItemKind::Time { .. } = c.kind {
        match available_from(c.kind, c.first_run) {
            Some(d) if d <= c.today => {}
            Some(d) => return Err(AttestError::TimeNotElapsed(d)),
            None => {
                return Err(AttestError::TimeNotElapsed(
                    c.today
                        .checked_add(jiff::Span::new().days(1))
                        .unwrap_or(c.today),
                ));
            }
        }
    }
    match a {
        // A plain `time` item is done once its date has come ("waited until <date>").
        Attest::Done { .. }
            if matches!(
                c.kind,
                ItemKind::Time {
                    then_attest: false,
                    ..
                }
            ) =>
        {
            Ok(())
        }
        Attest::Done { note, .. } => {
            if note.trim().chars().count() < MIN_NOTE {
                return Err(AttestError::NoteTooShort);
            }
            let mine = normalize_note(note);
            let repeated = c.results.iter().any(|(id, r)| {
                id != c.item
                    && matches!(r.outcome, ItemOutcome::Done | ItemOutcome::Pass)
                    && normalize_note(&r.text) == mine
            });
            if repeated {
                return Err(AttestError::NoteRepeated);
            }
            Ok(())
        }
        Attest::NotApplicable { reason } => {
            if reason.trim().is_empty() {
                Err(AttestError::ReasonRequired)
            } else {
                Ok(())
            }
        }
        Attest::Clear => Ok(()),
    }
}

/// Records a FAIL for an item and returns its failure count; the F.4 prompt shows at
/// [`F4_AT_FAILURE`] (DR-073).
pub fn record_failure(failures: &mut BTreeMap<String, u32>, item: &ItemId) -> u32 {
    let n = failures.entry(item.as_str().to_owned()).or_insert(0);
    *n = n.saturating_add(1);
    *n
}

/// Whether the item's failure count shows the F.4 prompt instead of another fix chain.
#[must_use]
pub const fn shows_f4(failures: u32) -> bool {
    failures >= F4_AT_FAILURE
}

/// The stage's chain session (DR-071).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainSession {
    /// No live session (idle or ended).
    Idle,
    /// A live session; Billy hasn't confirmed signing anyway.
    Busy,
    /// A live session, and Billy confirmed signing anyway in the native dialog.
    BusyConfirmed,
}

/// Every fact DR-071 needs.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent facts, each one of DR-071's conditions"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignFacts {
    /// The stage.
    pub stage: u8,
    /// The close-out state reached ([`derive`]).
    pub closeout: GateState,
    /// Applicable items still unresolved (FAIL or waiting).
    pub unresolved: Vec<String>,
    /// A lessons entry for the stage exists.
    pub lessons_found: bool,
    /// The chain's session.
    pub chain: ChainSession,
    /// Early conditions still `undecided` (checked for Stage 2).
    pub undecided_early: Vec<ConditionId>,
    /// Late conditions still `undecided` (checked for Stage 4).
    pub undecided_late: Vec<ConditionId>,
    /// The packet is valid (DR-133).
    pub packet_valid: bool,
    /// git was found (AC-F014-03).
    pub git_found: bool,
    /// Already signed.
    pub signed: bool,
}

/// Why a stage can't be signed (each shown as one reason).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignBlocker {
    /// Already signed.
    AlreadySigned,
    /// The close-out isn't complete; what comes next.
    Closeout(String),
    /// Items left.
    Unresolved(Vec<String>),
    /// No lessons entry.
    NoLessons,
    /// The chain's session runs (Billy can confirm in the sign dialog).
    ChainBusy,
    /// Stage 2 with an undecided early condition.
    UndecidedEarly(ConditionId),
    /// Stage 4 with an undecided late condition.
    UndecidedLate(ConditionId),
    /// The packet has problems.
    PacketInvalid,
    /// git wasn't found.
    GitMissing,
}

impl SignBlocker {
    /// The reason as shown.
    #[must_use]
    pub fn text(&self, stage: u8) -> String {
        match self {
            Self::AlreadySigned => format!("Stage {stage} is already signed."),
            Self::Closeout(next) => format!("{next}."),
            Self::Unresolved(items) if items.len() == 1 => {
                "1 inspection item isn't resolved.".to_owned()
            }
            Self::Unresolved(items) => format!("{} inspection items aren't resolved.", items.len()),
            Self::NoLessons => format!(
                "docs/lessons.md has no entry headed \"Stage {stage}\" yet (\"clean stage\" counts)."
            ),
            Self::ChainBusy => {
                "The stage's session is still running. You'll confirm in the sign dialog."
                    .to_owned()
            }
            Self::UndecidedEarly(c) | Self::UndecidedLate(c) => {
                format!("Decide {c} before signing Stage {stage}.")
            }
            Self::PacketInvalid => "The packet has problems. Fix them first.".to_owned(),
            Self::GitMissing => {
                "git wasn't found, so Foreman can't commit. Doctor shows where it looks.".to_owned()
            }
        }
    }

    /// Billy can override it in the native sign dialog (only the busy chain, DR-071).
    #[must_use]
    pub const fn confirmable(&self) -> bool {
        matches!(self, Self::ChainBusy)
    }
}

/// DR-071, AC-F014-01, AC-F015-10, AC-F004-13: every reason the stage can't be signed. The stage
/// is signable only when the list is empty, or holds only [`SignBlocker::ChainBusy`] and Billy
/// confirmed it ([`ChainSession::BusyConfirmed`] never produces it).
#[must_use]
pub fn sign_blockers(f: &SignFacts) -> Vec<SignBlocker> {
    let mut out = Vec::new();
    if f.signed {
        out.push(SignBlocker::AlreadySigned);
    }
    if !f.git_found {
        out.push(SignBlocker::GitMissing);
    }
    if !f.packet_valid {
        out.push(SignBlocker::PacketInvalid);
    }
    if !f.unresolved.is_empty() {
        out.push(SignBlocker::Unresolved(f.unresolved.clone()));
    }
    if !f.lessons_found {
        out.push(SignBlocker::NoLessons);
    }
    if f.closeout != GateState::Signable {
        let next = SEQUENCE
            .get(index(f.closeout) + 1)
            .copied()
            .unwrap_or(GateState::Signable);
        let reason = needs_before(next).to_owned();
        // Don't repeat a reason already listed.
        let dup = matches!(next, GateState::B) && !f.unresolved.is_empty()
            || matches!(next, GateState::C | GateState::A) && !f.packet_valid
            || matches!(next, GateState::Signable) && !f.lessons_found;
        if !dup {
            out.push(SignBlocker::Closeout(reason));
        }
    }
    if f.stage == 2
        && let Some(c) = f.undecided_early.iter().find(|c| !c.is_late())
    {
        out.push(SignBlocker::UndecidedEarly(*c));
    }
    if f.stage == 4
        && let Some(c) = f.undecided_late.iter().find(|c| c.is_late())
    {
        out.push(SignBlocker::UndecidedLate(*c));
    }
    if f.chain == ChainSession::Busy {
        out.push(SignBlocker::ChainBusy);
    }
    out
}

/// True when nothing blocks signing (the sign dialog itself, after its native confirmation).
#[must_use]
pub fn signable(f: &SignFacts) -> bool {
    sign_blockers(f).is_empty()
}

/// True when only confirmable blockers remain (the gate view's Sign button: the busy chain is
/// confirmed in the native sign dialog).
#[must_use]
pub fn sign_enabled(f: &SignFacts) -> bool {
    sign_blockers(f).iter().all(SignBlocker::confirmable)
}

/// A Gate A import proposal for one item (AC-F015-05). Proposals are shown, never applied:
/// each needs Billy's confirmation (`gate_import_confirm`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proposal {
    /// The message says PASS on the item's line.
    Pass(String),
    /// The message says FAIL on the item's line.
    Fail(String),
    /// No line names the item with exactly one of PASS or FAIL.
    Unmatched,
}

fn has_word(line: &str, w: &str) -> bool {
    line.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|t| t.eq_ignore_ascii_case(w))
}

/// Matches each item against the Gate A session's last message by item text: a line naming the
/// item (its ID, or the first words of its normalized text) with exactly one of PASS/FAIL. The
/// matched line is the evidence. Pure; the message is untrusted text and only selects among
/// three outcomes.
#[must_use]
pub fn propose(items: &[(ItemId, String)], message: &str) -> Vec<(ItemId, Proposal)> {
    let lines: Vec<&str> = message.lines().collect();
    items
        .iter()
        .map(|(id, normalized)| {
            let head: String = normalized
                .split_whitespace()
                .take(6)
                .collect::<Vec<_>>()
                .join(" ");
            let names = |l: &str| {
                let low = normalize_note(l);
                low.contains(id.as_str()) || (head.len() >= 12 && low.contains(&head))
            };
            let mut found = None;
            for l in &lines {
                if !names(l) {
                    continue;
                }
                let pass = has_word(l, "pass") || has_word(l, "passed");
                let fail = has_word(l, "fail") || has_word(l, "failed");
                let evidence = l.trim().chars().take(500).collect::<String>();
                found = match (pass, fail) {
                    (true, false) => Some(Proposal::Pass(evidence)),
                    (false, true) => Some(Proposal::Fail(evidence)),
                    _ => continue,
                };
                break;
            }
            (id.clone(), found.unwrap_or(Proposal::Unmatched))
        })
        .collect()
}

/// The run config of Fix this and F.4: Opus 5.5 · high · Plan (then Accept edits, a cue Billy
/// acts on; DR-105). Blueprint v2.2 `bp:2251` (F.4's RUN WITH).
pub const FIX_START: crate::ids::Start = crate::ids::Start {
    model: crate::ids::Model::Opus,
    effort: crate::ids::Effort::High,
    mode: Some(crate::ids::Mode::Plan),
};

/// Fix this (AC-F015-07): a new chain prefilled with the item and its evidence.
#[must_use]
pub fn fix_prompt(stage: u8, item: &str, evidence: &str) -> String {
    format!(
        "This Stage {stage} inspection item failed in Gate A:\n{item}\n\nEvidence:\n{evidence}\n\nFind the cause and plan the smallest fix for this item only. Wait for my approval before you change anything, and don't change anything else."
    )
}

/// The F.4 prompt (blueprint v2.2 `bp:2256`–`bp:2261`, "when the same mistake happens twice"),
/// shown on an item's third failure instead of another fix chain. Its `[L-NNN]` sentence names the
/// item instead, because a FAIL has no lesson ID yet.
#[must_use]
pub fn f4_prompt(item: &str, failures: u32) -> String {
    format!(
        "The inspection item \"{item}\" has now failed {failures} times. Read docs/lessons.md and find the\nroot cause. Propose the smallest permanent fix: a CLAUDE.md rule, an\nagent instruction, a hook, a lint rule, or a CI check. Prefer automated\nenforcement (hooks, lint, CI) over instructions, because instructions\ncan be ignored and checks can't. Implement it after I approve, and\nmark the lesson \"applied\"."
    )
}
