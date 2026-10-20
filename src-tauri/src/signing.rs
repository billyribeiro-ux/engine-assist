//! Signing (F-014; ADR-012, DR-139–DR-146, SR-42–SR-47; tickets M4-19…M4-22): the sign preview,
//! phase 1 (APPROVED BY and the added fields, the stage record, pending decision files) and
//! phase 2 (the plumbing commit with read-back), retrying a pending commit, other-branch signing,
//! the index-refresh retry, and tamper detection and restore.
//!
//! APPROVED BY is written only after Billy confirms the native sign dialog (SR-24a, SR-47). Every
//! git call goes through `foreman_sys::git` (plumbing, fixed `-c` overrides, cleared env,
//! `hash-object --no-filters`, `commit-tree --no-gpg-sign`).

use std::collections::BTreeMap;
use std::path::Path;

use foreman_core::decisions::{self, Decisions};
use foreman_core::gate::{ChainSession, SignFacts};
use foreman_core::git::{self as core_git, GitOp};
use foreman_core::ids::{ConditionId, TriState};
use foreman_core::lessons;
use foreman_core::packet::{self, sign_fields};
use foreman_core::pathpolicy::{
    self, DecisionPath, ReadClass, RepoPath as CoreRepoPath, WritePurpose,
};
use foreman_core::policy;
use foreman_core::rules::{self, Applicability};
use foreman_core::schema::progress::{
    DecisionFileKind, IndexRefreshPending, ItemOutcome, ProgressFile, Signing, SigningState,
};
use foreman_core::schema::run::RunFile;
use foreman_core::schema::stage_record::{
    Branch, GateRecord, GitRecord, HashedPath, PacketRef, StageRecordFile, StepRecord,
};
use foreman_core::steps::StepState as CoreState;
use foreman_core::store::{ReadStamp, Root, app_dirs, sha256_hex};
use foreman_sys::git::commit::{
    CommitError, CommitOpts, CommitOutcome, CommitStep, Reviewed, Target, commit_bytes,
    commit_paths,
};
use foreman_sys::git::{GitOpts, show};

use crate::closeout::{self, Closeout};
use crate::ipc::IpcError;
use crate::ipc::dto::gates::{SignPreview, SignResult};
use crate::ipc::dto::launch::SignProgress;
use crate::ipc::error::StaleWhat;
use crate::ipc::types::{ConditionId as DtoCondition, Diff, PreviewId, RepoPath, Sha256, ToolKind};
use crate::os::Confirm;
use crate::project::{Opened, PacketFile};
use crate::render::escape_visible;
use crate::state::App;

/// The environment names git runs with (SR-45), as the stage record notes them.
pub const GIT_ENV: [&str; 5] = [
    "HOME",
    "PATH",
    "LANG",
    "GIT_TERMINAL_PROMPT",
    "GIT_OPTIONAL_LOCKS",
];

/// Where the signing commit goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Branch2 {
    /// The branch HEAD names.
    Current,
    /// Another branch (`refs/heads/<name>`).
    Other(String),
}

/// A sign preview held for `sign_apply` (IPC-R7: one use, 10 minutes).
#[derive(Debug, Clone)]
pub struct HeldSign {
    /// Project.
    pub project_id: String,
    /// Stage.
    pub stage: u8,
    /// The packet's SHA-256 Billy reviewed.
    pub packet_sha256: String,
    /// DRI name.
    pub name: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Where it lands.
    pub branch: Branch2,
    /// The branch's short name (shown and stored in `progress.json`).
    pub branch_short: String,
    /// The signed packet text.
    pub packet_text: String,
    /// The stage record's bytes.
    pub record: Vec<u8>,
    /// Pending decision files: kind and the approved bytes to commit.
    pub decisions: Vec<(DecisionFileKind, Vec<u8>)>,
    /// The stage's session was live at preview.
    pub chain_busy: bool,
    /// Stage 2's early conditions as they'll be confirmed.
    pub conditions: Vec<(ConditionId, TriState)>,
}

fn nn(n: u8) -> String {
    format!("{n:02}")
}

/// `handoff: stage NN`.
#[must_use]
pub fn commit_message(n: u8) -> String {
    format!("handoff: stage {}", nn(n))
}

fn record_path(n: u8) -> Result<CoreRepoPath, IpcError> {
    let d = DecisionPath::StageRecord(n);
    Ok(pathpolicy::check_write(
        &d.path(),
        WritePurpose::DecisionFile(d),
    )?)
}

fn packet_path(n: u8) -> Result<CoreRepoPath, IpcError> {
    Ok(pathpolicy::check_write(
        &packet::file_name(n),
        WritePurpose::Repo,
    )?)
}

fn attachments_path(n: u8) -> Result<CoreRepoPath, IpcError> {
    Ok(pathpolicy::check_write(
        &format!("docs/handoff/stage-{}-attachments.md", nn(n)),
        WritePurpose::Repo,
    )?)
}

fn decision_rel(kind: DecisionFileKind) -> Result<CoreRepoPath, IpcError> {
    let d = decisions::decision_path(kind);
    Ok(pathpolicy::check_write(
        &d.path(),
        WritePurpose::DecisionFile(d),
    )?)
}

/// `symbolic-ref -q HEAD` (`None` when detached or git can't run).
#[must_use]
pub fn head_ref(app: &App, root: &Path) -> Option<String> {
    let ctx = app.spawn_ctx_now().ok()?;
    let out =
        foreman_sys::git::run(root, &GitOp::SymbolicRefHead, &GitOpts::default(), &ctx).ok()?;
    out.ok().then(|| out.text())
}

fn short(r: &str) -> String {
    r.strip_prefix("refs/heads/").unwrap_or(r).to_owned()
}

/// Every run record of a project's stage (app data `runs/*/run.json`), oldest first.
fn stage_runs(p: &Opened, n: u8) -> Vec<RunFile> {
    let Ok(ids) = app_dirs::list_app_data_dir(&p.app_root, "runs") else {
        return Vec::new();
    };
    let mut v: Vec<RunFile> = ids
        .iter()
        .filter(|id| foreman_core::ids::is_uuid_v4(id))
        .filter_map(|id| crate::runs::read_run(&p.app_root, id).ok())
        .map(|(f, _)| f)
        .filter(|f| f.project_id == p.id() && f.stage == n)
        .collect();
    v.sort_by(|a, b| a.t.wall_start.cmp(&b.t.wall_start));
    v
}

/// The `SKIPPED:` lines (DR-055): this stage's skipped and not-applicable steps, plus every
/// not-applicable stage since the last signed packet.
fn skipped_lines(p: &Opened, n: u8) -> Result<Vec<String>, IpcError> {
    let (t, path, _) = p.parts()?;
    let signed = p.signed();
    let since = (0..n).rev().find(|s| signed.contains(s));
    Ok(rules::skipped_queue(t, path, n, since, &[]))
}

/// The `SUBSTITUTIONS:` lines (DR-104, AC-F005-05): every step of the stage whose effective start
/// differs from its template start (policy), plus each run that used a per-run override.
fn substitution_lines(p: &Opened, n: u8, runs: &[RunFile]) -> Result<Vec<String>, IpcError> {
    let (t, path, project) = p.parts()?;
    let mut out = Vec::new();
    for s in t.stage_steps(n) {
        if !path
            .step(s.key.as_str())
            .is_some_and(Applicability::on_path)
            || !s.runnable()
        {
            continue;
        }
        if let Ok(eff) = policy::policy_start(s, &project.model_policy)
            && let Some(l) = policy::substitution_line(s, &eff)
        {
            out.push(l);
        }
        for r in runs.iter().filter(|r| r.step_id == s.key && r.r#override) {
            if let (Some(tpl), Some(used)) = (s.start, r.intended) {
                let line = format!("{}: {tpl} → {used} (override)", s.key);
                if !out.contains(&line) {
                    out.push(line);
                }
            }
        }
    }
    Ok(out)
}

/// The `ATTESTED:` lines (§5.4) and the stage record's gate entries.
fn attested(c: &Closeout) -> (Vec<String>, Vec<GateRecord>) {
    let mut lines = Vec::new();
    let mut recs = Vec::new();
    for i in &c.items {
        let Some(r) = &i.result else { continue };
        let (word, outcome) = match r.outcome {
            ItemOutcome::Done => ("done", "done"),
            ItemOutcome::NotApplicable => ("not applicable", "notApplicable"),
            ItemOutcome::Pass => ("pass", "pass"),
            ItemOutcome::Fail => continue,
        };
        let text = sign_fields::one_line(&r.text);
        lines.push(if text.starts_with("waited until ") {
            format!("{}: {text}", i.id)
        } else {
            format!("{}: {word}: {text}", i.id)
        });
        recs.push(GateRecord {
            item: i.id.as_str().to_owned(),
            kind: foreman_core::gate::kind_name(i.kind).to_owned(),
            outcome: outcome.to_owned(),
            text: text.chars().take(4000).collect(),
            link: r.link.clone(),
        });
    }
    (lines, recs)
}

fn steps_record(
    app: &App,
    p: &Opened,
    n: u8,
    runs: &[RunFile],
) -> Result<Vec<StepRecord>, IpcError> {
    let (t, _, _) = p.parts()?;
    let mut out = Vec::new();
    for s in t.stage_steps(n) {
        let (state, reason) = match p.display_state(app, s) {
            CoreState::Done => ("done", None),
            CoreState::Skipped(r) => ("skipped", Some(r)),
            CoreState::NotApplicable(r) => ("notApplicable", Some(r)),
            _ => continue,
        };
        let mine: Vec<&RunFile> = runs.iter().filter(|r| r.step_id == s.key).collect();
        let last = mine.last();
        out.push(StepRecord {
            step: s.key.clone(),
            state: state.to_owned(),
            reason: reason.map(|r| r.chars().take(200).collect()),
            run_ids: mine.iter().map(|r| r.run_id.clone()).collect(),
            chain_id: last.and_then(|r| r.chain_id.clone()),
            intended: last.and_then(|r| r.intended),
            observed: last.and_then(|r| r.observed.clone()),
            prompt_sha256s: mine
                .iter()
                .filter_map(|r| r.prompt_sha256.clone())
                .collect(),
            active_ms: mine
                .iter()
                .filter_map(|r| {
                    r.t.exited
                        .zip(r.t.spawned)
                        .map(|(e, s)| e.saturating_sub(s))
                })
                .sum(),
        });
    }
    Ok(out)
}

fn hashed_attachments(p: &Opened, n: u8) -> Result<Option<HashedPath>, IpcError> {
    let root = p.root()?;
    let rel = attachments_path(n)?;
    Ok(match root.read_opt(&rel, ReadClass::Decision)? {
        (Some(b), _) => Some(HashedPath {
            path: rel.to_string(),
            sha256: sha256_hex(&b),
        }),
        (None, _) => None,
    })
}

/// The packet's text with Foreman's fields (DR-139).
fn signed_text(
    p: &Opened,
    n: u8,
    text: &str,
    f: &sign_fields::SignFields,
) -> Result<String, IpcError> {
    let (t, _, _) = p.parts()?;
    let keys: Vec<String> = t
        .packet_format(n)
        .map(|f| f.keys.iter().map(|k| k.key.clone()).collect())
        .unwrap_or_default();
    sign_fields::apply(text, &keys, f)
        .map_err(|e| IpcError::not_allowed("not-signable", e.to_string()))
}

fn valid_name(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty() && n.chars().count() <= 80 && !n.contains(['<', '>', '\n', '\r'])
}

/// The stage-record bytes for a signing (§3.4).
#[allow(clippy::too_many_arguments, reason = "every field of the record")]
fn record_bytes(
    app: &App,
    p: &Opened,
    n: u8,
    name: &str,
    date: &str,
    branch: &Branch2,
    branch_ref: &str,
    packet_sha: &str,
    skipped: &[String],
    substitutions: &[String],
    gate: Vec<GateRecord>,
    runs: &[RunFile],
) -> Result<Vec<u8>, IpcError> {
    let rec = StageRecordFile {
        schema_version: 1,
        stage: n,
        signed_by: name.to_owned(),
        signed_on: date.to_owned(),
        branch: Branch {
            mode: match branch {
                Branch2::Current => "current".into(),
                Branch2::Other(_) => "other".into(),
            },
            r#ref: branch_ref.to_owned(),
        },
        packet: PacketRef {
            path: packet::file_name(n),
            sha256: packet_sha.to_owned(),
            attachments: hashed_attachments(p, n)?,
        },
        steps: steps_record(app, p, n, runs)?,
        gate,
        skipped: skipped.to_vec(),
        substitutions: substitutions.to_vec(),
        git: GitRecord {
            overrides: core_git::OVERRIDES
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            sign_mode: "noGpgSign".into(),
            env: GIT_ENV.iter().map(|s| (*s).to_owned()).collect(),
        },
    };
    Ok(foreman_core::schema::to_file(&rec)
        .map_err(|e| IpcError::io(e.to_string()))?
        .into_bytes())
}

/// What `sign_preview` asks for.
#[derive(Debug, Clone)]
pub struct PreviewInput {
    /// Stage.
    pub stage: u8,
    /// Current or another branch (as requested).
    pub other: Option<String>,
    /// DRI name.
    pub name: String,
    /// Date.
    pub date: String,
}

/// Builds the sign preview and what `sign_apply` needs (AC-F014-01…-03, AC-F004-13, AC-F013-15,
/// AC-F005-05).
///
/// # Errors
/// `NotAllowed("not-signable" | "detached-head")`, `ToolMissing(git)`, `ReadOnly`, `Invalid`.
#[allow(clippy::too_many_lines, reason = "the preview's parts in order")]
pub fn preview(
    app: &App,
    p: &Opened,
    inp: &PreviewInput,
) -> Result<(SignPreview, HeldSign), IpcError> {
    p.writable()?;
    let n = inp.stage;
    let c = closeout::gather(app, p, n)?;
    if !c.sign.git_found {
        return Err(IpcError::tool_missing(ToolKind::Git, None));
    }
    if !c.sign_enabled() {
        return Err(IpcError::not_allowed(
            "not-signable",
            format!("Stage {n} can't be signed yet: {}", c.why_not().join(" ")),
        ));
    }
    if !valid_name(&inp.name) {
        return Err(IpcError::invalid("name", "signatureName"));
    }
    let root = p.root()?;
    let head = head_ref(app, root.path());
    let (branch, branch_ref) = match &inp.other {
        None => {
            let Some(h) = head.clone() else {
                return Err(IpcError::not_allowed(
                    "detached-head",
                    "Check out a branch first. Nothing was signed.",
                ));
            };
            (Branch2::Current, h)
        }
        Some(b) => {
            let full =
                core_git::branch_ref(b).map_err(|_| IpcError::invalid("branch", "branchName"))?;
            if head.as_deref() == Some(full.as_str()) {
                return Err(IpcError::invalid("branch", "currentBranch"));
            }
            (Branch2::Other(full.clone()), full)
        }
    };
    let Some(file) = p.packets.get(&n) else {
        return Err(IpcError::not_allowed(
            "not-signable",
            "The packet doesn't exist yet.",
        ));
    };
    if p.tampered.contains_key(&n) {
        return Err(IpcError::not_allowed(
            "not-signable",
            "The packet differs from its commit.",
        ));
    }
    let runs = stage_runs(p, n);
    let skipped = skipped_lines(p, n)?;
    let substitutions = substitution_lines(p, n, &runs)?;
    let (attested_lines, gate) = attested(&c);
    let fields = sign_fields::SignFields {
        name: inp.name.trim().to_owned(),
        date: inp.date.clone(),
        skipped: skipped.clone(),
        substitutions: substitutions.clone(),
        attested: attested_lines,
    };
    let text = signed_text(p, n, &file.text, &fields)?;
    let signed_sha = sha256_hex(text.as_bytes());
    let record = record_bytes(
        app,
        p,
        n,
        &fields.name,
        &inp.date,
        &branch,
        &branch_ref,
        &signed_sha,
        &skipped,
        &substitutions,
        gate,
        &runs,
    )?;
    // Decision files with pending changes (DR-153, AC-F014-14): their approved bytes.
    let d = Decisions {
        project: root,
        app: &p.app_root,
        project_id: p.id(),
    };
    let mut decisions_v = Vec::new();
    for po in &p.progress.pending_own {
        let (bytes, _) = d
            .approved(po.file)
            .map_err(|e| IpcError::io(e.to_string()))?;
        decisions_v.push((po.file, bytes));
    }
    let mut paths = vec![RepoPath::from_trusted(packet::file_name(n))];
    let mut notes = BTreeMap::new();
    if hashed_attachments(p, n)?.is_some() {
        paths.push(RepoPath::from_trusted(attachments_path(n)?.to_string()));
    }
    let rp = record_path(n)?.to_string();
    paths.push(RepoPath::from_trusted(rp.clone()));
    notes.insert(
        rp,
        "New: the stage record (step states, runs, gate results).".to_owned(),
    );
    for (k, _) in &decisions_v {
        paths.push(RepoPath::from_trusted(k.repo_path()));
        notes.insert(
            k.repo_path().to_owned(),
            "Your accepted change, held since Stage 5 was signed.".to_owned(),
        );
    }
    if let Some((_, _)) = lessons::read(root)? {
        paths.push(RepoPath::from_trusted(lessons::PATH));
        notes.insert(
            lessons::PATH.to_owned(),
            "Committed only if it changed since the last commit.".to_owned(),
        );
    }
    let early: Vec<(ConditionId, TriState)> = p
        .project
        .as_ref()
        .map(|pr| {
            pr.conditions
                .iter()
                .filter(|(c, _)| !c.is_late())
                .map(|(c, v)| (*c, *v))
                .collect()
        })
        .unwrap_or_default();
    let conditions = if n == 2 { early } else { Vec::new() };
    let held = HeldSign {
        project_id: p.id().to_owned(),
        stage: n,
        packet_sha256: file.sha256.clone(),
        name: fields.name.clone(),
        date: inp.date.clone(),
        branch_short: short(&branch_ref),
        branch: branch.clone(),
        packet_text: text.clone(),
        record,
        decisions: decisions_v,
        chain_busy: c.sign.chain == ChainSession::Busy,
        conditions: conditions.clone(),
    };
    let short_hash: String = file.sha256.chars().take(6).collect();
    let pv = SignPreview {
        preview_id: PreviewId::from_trusted(""),
        packet_sha256: Sha256::from_trusted(file.sha256.as_str()),
        short: short_hash,
        added_fields: crate::diff::diff(&packet::file_name(n), Some(&file.text), &text),
        paths,
        confirm_conditions: (n == 2).then(|| {
            conditions
                .iter()
                .map(|(c, _)| DtoCondition::from(*c))
                .collect()
        }),
        chain_busy: held.chain_busy,
        blockers: Vec::new(),
        branch: escape_visible(&held.branch_short),
        commit_message: commit_message(n),
        path_notes: notes,
    };
    Ok((pv, held))
}

fn progress_step(send: &dyn Fn(SignProgress), s: CommitStep<'_>) {
    send(match s {
        CommitStep::HashObject(p) => SignProgress::HashObject {
            path: RepoPath::from_trusted(p),
        },
        CommitStep::WriteTree => SignProgress::WriteTree,
        CommitStep::CommitTree => SignProgress::CommitTree { signed: false },
        CommitStep::UpdateRef => SignProgress::UpdateRef,
        CommitStep::RefreshIndex => SignProgress::RefreshIndex,
        CommitStep::ReadBack(p, ok) => SignProgress::ReadBack {
            path: RepoPath::from_trusted(p),
            ok,
        },
    });
}

/// Test seams for the commit (fault injection only; `run()` never sets them).
#[derive(Default)]
#[allow(missing_debug_implementations, reason = "holds callbacks")]
pub struct Seams<'a> {
    /// Between `commit-tree` and `update-ref`.
    pub before_update_ref: Option<&'a dyn Fn()>,
    /// Changes the committed bytes read back (a hook rewriting the packet).
    pub tamper_readback: Option<&'a foreman_sys::git::commit::ReadbackSeam>,
    /// Index retries.
    pub index_retries: Option<(u32, std::time::Duration)>,
    /// Stop after phase 1 (Foreman killed between the phases, AC-F014-13).
    pub stop_after_phase1: bool,
}

fn sign_confirm(app: &App, h: &HeldSign) -> bool {
    let mut msg = format!(
        "Sign Stage {} as {} on {}?\n\nSigning {} @ {}\nCommit \"{}\" on {}{}.",
        h.stage,
        escape_visible(&h.name),
        h.date,
        packet::file_name(h.stage).trim_start_matches("docs/handoff/"),
        h.packet_sha256.chars().take(6).collect::<String>(),
        commit_message(h.stage),
        escape_visible(&h.branch_short),
        match h.branch {
            Branch2::Current => "",
            Branch2::Other(_) =>
                " (another branch: your checkout, index and working tree stay as they are)",
        }
    );
    if !h.conditions.is_empty() {
        msg.push_str("\n\nConditions (they lock when Stage 2 is signed):");
        for (c, v) in &h.conditions {
            msg.push_str(&format!("\n  {c} = {}", v.as_str()));
        }
    }
    if h.chain_busy {
        msg.push_str(
            "\n\nThis stage's session is still running. Signing doesn't end it; sign anyway only if it's done.",
        );
    }
    msg.push_str("\n\nClaude writing a name never signs a packet. Only this confirmation does.");
    app.os().confirm(&Confirm::new(
        format!("Sign Stage {}?", h.stage),
        msg,
        "Sign",
    ))
}

fn stale_packet(app: &App, p: &Opened, h: &HeldSign) -> IpcError {
    let fresh = preview(
        app,
        p,
        &PreviewInput {
            stage: h.stage,
            other: match &h.branch {
                Branch2::Current => None,
                Branch2::Other(b) => Some(b.clone()),
            },
            name: h.name.clone(),
            date: h.date.clone(),
        },
    )
    .ok()
    .map(|(mut pv, held)| {
        pv.preview_id = PreviewId::from_trusted(hold(app, held));
        pv
    });
    IpcError::stale(
        StaleWhat::ReadStamp,
        "The packet changed since you reviewed it. Review it again.",
        fresh.and_then(|f| serde_json::to_value(f).ok()),
    )
}

/// Holds a sign preview (10 minutes, one use) and returns its ID.
pub fn hold(app: &App, h: HeldSign) -> String {
    app.inner().previews.hold(
        crate::preview::Preview::Sign(Box::new(h)),
        crate::state::PREVIEW_TTL,
    )
}

fn commit_opts<'a>(
    app: &App,
    seams: &'a Seams<'a>,
    step: &'a dyn Fn(CommitStep<'_>),
) -> CommitOpts<'a> {
    CommitOpts {
        app_data: app.paths().app_data.clone(),
        before_update_ref: seams.before_update_ref,
        index_retries: seams.index_retries,
        on_step: Some(step),
        tamper_readback: seams.tamper_readback,
    }
}

/// `sign_apply`: re-read, the native dialog, phase 1, phase 2, the stamp (AC-F014-04…-14).
///
/// # Errors
/// `Expired`, `Cancelled`, `Stale`, `Conflict`, `ToolFailed`, `NotAllowed`.
#[allow(clippy::too_many_lines, reason = "the signing sequence in order")]
pub fn apply(
    app: &App,
    h: HeldSign,
    send: &dyn Fn(SignProgress),
    seams: &Seams<'_>,
) -> Result<SignResult, IpcError> {
    let n = h.stage;
    let mut p = crate::project::open(app, &h.project_id)?;
    p.writable()?;
    send(SignProgress::ReRead);
    // AC-F014-04: the packet must still be what Billy reviewed.
    let now = p.packets.get(&n).map(|f| f.sha256.clone());
    if now.as_deref() != Some(h.packet_sha256.as_str()) || p.tampered.contains_key(&n) {
        return Err(stale_packet(app, &p, &h));
    }
    // Still signable (the gate, lessons, conditions may have moved since the preview).
    let c = closeout::gather(app, &p, n)?;
    let facts = SignFacts {
        chain: ChainSession::BusyConfirmed,
        ..c.sign.clone()
    };
    if c.locked_by.is_some() || !foreman_core::gate::signable(&facts) {
        return Err(IpcError::not_allowed(
            "not-signable",
            format!("Stage {n} can't be signed: {}", c.why_not().join(" ")),
        ));
    }
    if !sign_confirm(app, &h) {
        return Err(IpcError::cancelled());
    }
    let root_path = p.root()?.path().to_path_buf();
    let packet_rel = packet_path(n)?;
    let record_rel = record_path(n)?;
    let ctx = app.spawn_ctx_now()?;
    let step = |s: CommitStep<'_>| progress_step(send, s);
    let opts = commit_opts(app, seams, &step);
    match &h.branch {
        Branch2::Current => {
            // Phase 1: APPROVED BY and the added fields, the stage record, the pending decision
            // files, each by compare-and-swap (SR-33).
            let root = p.root()?;
            let packet_stamp = p.packets[&n].stamp.clone();
            root.write(&packet_rel, h.packet_text.as_bytes(), &packet_stamp)
                .map_err(|e| conflict_or(e, &packet_rel))?;
            send(SignProgress::Write {
                path: RepoPath::from_trusted(packet_rel.to_string()),
            });
            let (_, rstamp) = root.read_opt(&record_rel, ReadClass::Decision)?;
            root.write(&record_rel, &h.record, &rstamp)
                .map_err(|e| conflict_or(e, &record_rel))?;
            send(SignProgress::Write {
                path: RepoPath::from_trusted(record_rel.to_string()),
            });
            for (kind, bytes) in &h.decisions {
                let rel = decision_rel(*kind)?;
                let (cur, st) = root.read_opt(&rel, ReadClass::Decision)?;
                let committed = match *kind {
                    DecisionFileKind::Project => &p.approvals.files.project,
                    DecisionFileKind::Overlay => &p.approvals.files.overlay,
                    DecisionFileKind::BlueprintLock => &p.approvals.files.blueprint_lock,
                }
                .committed_sha256
                .clone();
                // The repo file must still be the last committed version (DR-153); an outside
                // edit waiting for review is never overwritten (SR-33).
                let outside = p.progress.pending_outside.iter().any(|x| x.file == *kind);
                let moved = committed.is_some()
                    && cur.as_deref().map(sha256_hex) != committed
                    && cur.as_deref() != Some(bytes.as_slice());
                if outside || moved {
                    return Err(IpcError::conflict(
                        kind.repo_path(),
                        vec![
                            RepoPath::from_trusted(packet_rel.to_string()),
                            RepoPath::from_trusted(record_rel.to_string()),
                        ],
                    ));
                }
                root.write(&rel, bytes, &st)
                    .map_err(|e| conflict_or(e, &rel))?;
                send(SignProgress::Write {
                    path: RepoPath::from_trusted(rel.to_string()),
                });
            }
            p.progress.signing.insert(
                n.to_string(),
                Signing {
                    state: SigningState::CommitPending,
                    commit: None,
                    branch: h.branch_short.clone(),
                    packet_sha256: sha256_hex(h.packet_text.as_bytes()),
                    at: app.now(),
                },
            );
            p.save_progress(app)?;
            if seams.stop_after_phase1 {
                return Ok(SignResult::CommitPending {
                    stderr_tail: "Stopped after phase 1 (test).".into(),
                });
            }
            let mut expected = vec![
                (packet_rel.to_string(), sha256_hex(h.packet_text.as_bytes())),
                (record_rel.to_string(), sha256_hex(&h.record)),
            ];
            for (k, b) in &h.decisions {
                expected.push((k.repo_path().to_owned(), sha256_hex(b)));
            }
            let reviewed = reviewed_paths(&p, n, &expected)?;
            let out = commit_paths(p.root()?, &reviewed, &commit_message(n), &opts, &ctx);
            let signed_sha = sha256_hex(h.packet_text.as_bytes());
            finish(app, &mut p, n, &h.branch, out, send, &signed_sha, seams)
        }
        Branch2::Other(full) => {
            // Nothing is written to the working tree (AC-F014-09): the commit is built from the
            // bytes Foreman holds.
            let mut files: Vec<(String, Vec<u8>)> = vec![
                (packet_rel.to_string(), h.packet_text.clone().into_bytes()),
                (record_rel.to_string(), h.record.clone()),
            ];
            for (k, b) in &h.decisions {
                files.push((k.repo_path().to_owned(), b.clone()));
            }
            let root = p.root()?;
            for rel in [
                attachments_path(n)?,
                pathpolicy::check_write(lessons::PATH, WritePurpose::Repo)?,
            ] {
                if let (Some(b), _) = root.read_opt(&rel, ReadClass::Decision)? {
                    files.push((rel.to_string(), b));
                }
            }
            let out = commit_bytes(
                &root_path,
                &files,
                &commit_message(n),
                &Target::Other(full.clone()),
                &opts,
                &ctx,
            );
            let signed_sha = sha256_hex(h.packet_text.as_bytes());
            finish(app, &mut p, n, &h.branch, out, send, &signed_sha, seams)
        }
    }
}

fn conflict_or(e: foreman_core::store::StoreError, rel: &CoreRepoPath) -> IpcError {
    match e {
        foreman_core::store::StoreError::Conflict { .. } => {
            IpcError::conflict(rel.as_str(), Vec::new())
        }
        other => IpcError::from(other),
    }
}

/// The paths of a current-branch signing as they are on disk now (re-read; their hashes are what
/// the commit must contain).
fn reviewed_paths(
    p: &Opened,
    n: u8,
    expected: &[(String, String)],
) -> Result<Vec<Reviewed>, IpcError> {
    let root = p.root()?;
    let mut rels = vec![packet_path(n)?, attachments_path(n)?, record_path(n)?];
    for po in &p.progress.pending_own {
        rels.push(decision_rel(po.file)?);
    }
    rels.push(pathpolicy::check_write(lessons::PATH, WritePurpose::Repo)?);
    let mut out = Vec::new();
    for rel in rels {
        let want = expected
            .iter()
            .find(|(path, _)| path == rel.as_str())
            .map(|(_, h)| h.clone());
        match (root.read_opt(&rel, ReadClass::Decision)?, want) {
            // A file Foreman wrote must still hash to what it wrote (commit_paths re-checks).
            ((_, _), Some(h)) => out.push(Reviewed {
                path: rel,
                sha256: h,
            }),
            ((Some(b), _), None) => out.push(Reviewed {
                path: rel,
                sha256: sha256_hex(&b),
            }),
            ((None, _), None) => {}
        }
    }
    Ok(out)
}

/// Phase 2's outcome: the stamp only after read-back passed (DR-145).
#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "one arm per commit outcome"
)]
fn finish(
    app: &App,
    p: &mut Opened,
    n: u8,
    branch: &Branch2,
    out: Result<CommitOutcome, CommitError>,
    send: &dyn Fn(SignProgress),
    signed_sha: &str,
    seams: &Seams<'_>,
) -> Result<SignResult, IpcError> {
    let other = matches!(branch, Branch2::Other(_));
    let key = n.to_string();
    let (commit, branch_ref, pending) = match out {
        Ok(CommitOutcome::Committed {
            commit,
            branch,
            paths,
            index_refresh_pending,
            ..
        }) => {
            if index_refresh_pending {
                p.progress.index_refresh_pending = Some(IndexRefreshPending {
                    commit: commit.clone(),
                    paths,
                    since: app.now(),
                });
            }
            (commit, branch, index_refresh_pending)
        }
        Ok(CommitOutcome::NothingToCommit) => {
            // Everything was already committed (a retry after a commit that landed).
            let tip = match branch {
                Branch2::Current => show::head(p.root()?.path(), &app.spawn_ctx_now()?)
                    .ok()
                    .flatten(),
                Branch2::Other(_) => None,
            };
            let Some(tip) = tip else {
                return pending_result(app, p, n, other, "Nothing differed from the branch tip.");
            };
            (
                tip,
                head_ref(app, p.root()?.path()).unwrap_or_default(),
                false,
            )
        }
        Err(CommitError::ReadbackMismatch { commit, path }) => {
            // The commit landed but holds other bytes: the stage stays commit pending (AC-F014-10).
            let root = p.root()?;
            let ctx = app.spawn_ctx_now()?;
            let committed = show::blob_at(root.path(), &commit, &path, &ctx)
                .ok()
                .flatten()
                .map(|b| match seams.tamper_readback {
                    Some(f) => f(&path, b),
                    None => b,
                })
                .map(|b| String::from_utf8_lossy(&b).into_owned());
            let ours = pathpolicy::check_read(&path, ReadClass::Decision)
                .ok()
                .and_then(|r| root.read_opt(&r, ReadClass::Decision).ok())
                .and_then(|(b, _)| b)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            let diff: Diff =
                crate::diff::diff(&path, Some(&ours), committed.as_deref().unwrap_or(""));
            if !other {
                mark_pending(app, p, n)?;
            }
            return Ok(SignResult::ReadBackMismatch { diffs: vec![diff] });
        }
        Err(CommitError::ChangedSinceReview { path }) => {
            return Err(IpcError::stale(
                StaleWhat::ReadStamp,
                format!(
                    "{} changed since Foreman wrote it. Review it and retry the commit.",
                    escape_visible(&path)
                ),
                None,
            ));
        }
        Err(CommitError::BadBranch(m)) => return Err(IpcError::not_allowed("bad-branch", m)),
        Err(e) => {
            let text = match &e {
                CommitError::Git(foreman_sys::git::GitError::Spawn(
                    foreman_sys::spawn::SpawnError::Timeout(_),
                )) => "git didn't answer within 60 seconds, so Foreman stopped waiting.".to_owned(),
                other_e => other_e.to_string(),
            };
            return pending_result(app, p, n, other, &text);
        }
    };
    // The stamp.
    p.progress.signing.insert(
        key,
        Signing {
            state: if other {
                SigningState::SignedOtherBranch
            } else {
                SigningState::Signed
            },
            commit: Some(commit.clone()),
            branch: short(&branch_ref),
            packet_sha256: signed_sha.to_owned(),
            at: app.now(),
        },
    );
    // Decision files committed in this signing (AC-F014-14, ADR-022 §5).
    {
        let root = p.root()?;
        let d = Decisions {
            project: root,
            app: &p.app_root,
            project_id: &p.entry.project_id,
        };
        for po in &p.progress.pending_own {
            if other {
                let _ = d.mark_committed_on_branch(po.file, &branch_ref, &po.sha256);
            } else {
                let _ = d.mark_committed(po.file, &po.sha256);
            }
        }
        if let Ok((a, _)) = d.approvals() {
            p.approvals = a;
        }
    }
    if !other {
        p.progress.pending_own.clear();
    }
    // SR-53: the stage's sensor events go at its sign.
    for r in stage_runs(p, n) {
        let rel = format!("events/{}", r.run_id);
        if app_dirs::list_app_data_dir(&p.app_root, &rel).is_ok_and(|v| !v.is_empty()) {
            let _ = app_dirs::reset_dir(&p.app_root, &rel);
        }
    }
    p.save_progress(app)?;
    let _ = pending;
    send(SignProgress::Done {
        commit: commit.clone(),
    });
    Ok(SignResult::Signed {
        commit,
        stage: crate::ipc::types::Stage(n),
    })
}

fn mark_pending(app: &App, p: &mut Opened, n: u8) -> Result<(), IpcError> {
    if let Some(s) = p.progress.signing.get_mut(&n.to_string()) {
        s.state = SigningState::CommitPending;
    }
    p.save_progress(app).map(|_| ())
}

fn pending_result(
    app: &App,
    p: &mut Opened,
    n: u8,
    other: bool,
    text: &str,
) -> Result<SignResult, IpcError> {
    if other {
        // Nothing was written for another branch, so there's nothing to retry: sign again.
        return Err(IpcError::tool_failed(
            "git",
            None,
            text.contains("60 seconds"),
            escape_visible(text),
        ));
    }
    mark_pending(app, p, n)?;
    Ok(SignResult::CommitPending {
        stderr_tail: escape_visible(text).chars().take(2000).collect(),
    })
}

/// `sign_retry_commit` (AC-F014-11, -13): phase 2 again for a stage left `signed (commit
/// pending)`, from the files phase 1 wrote. The packet must still hash to what phase 1 wrote.
///
/// # Errors
/// `NotAllowed("not-commit-pending" | "packet-changed")`, `ToolFailed`, `ReadOnly`.
pub fn retry(
    app: &App,
    project_id: &str,
    n: u8,
    send: &dyn Fn(SignProgress),
    seams: &Seams<'_>,
) -> Result<SignResult, IpcError> {
    let mut p = crate::project::open(app, project_id)?;
    p.writable()?;
    let Some(s) = p.progress.signing.get(&n.to_string()).cloned() else {
        return Err(IpcError::not_allowed(
            "not-commit-pending",
            "This stage has no commit waiting.",
        ));
    };
    if s.state != SigningState::CommitPending {
        return Err(IpcError::not_allowed(
            "not-commit-pending",
            "This stage has no commit waiting.",
        ));
    }
    send(SignProgress::ReRead);
    if p.packets.get(&n).map(|f| f.sha256.as_str()) != Some(s.packet_sha256.as_str()) {
        return Err(IpcError::not_allowed(
            "packet-changed",
            "The packet changed since Foreman signed it. Restore it or sign again.",
        ));
    }
    let reviewed = reviewed_paths(&p, n, &[(packet::file_name(n), s.packet_sha256.clone())])?;
    let ctx = app.spawn_ctx_now()?;
    let step = |st: CommitStep<'_>| progress_step(send, st);
    let opts = commit_opts(app, seams, &step);
    let out = commit_paths(p.root()?, &reviewed, &commit_message(n), &opts, &ctx);
    finish(
        app,
        &mut p,
        n,
        &Branch2::Current,
        out,
        send,
        &s.packet_sha256,
        seams,
    )
}

/// On open: another branch's signed packet is read from its commit (AC-F014-09: "the resolver
/// reads the packet from that ref"); a current-branch signed packet whose working file differs
/// from its committed version is a tamper event, and the committed blob is used (DR-146).
pub fn load_signed_packets(app: &App, root: &Root, progress: &ProgressFile, o: &mut Opened) {
    let Ok(ctx) = app.spawn_ctx_now() else { return };
    for (k, s) in &progress.signing {
        let (Ok(n), Some(commit)) = (k.parse::<u8>(), s.commit.as_deref()) else {
            continue;
        };
        let path = packet::file_name(n);
        let working = o.packets.get(&n).cloned();
        let needs = match s.state {
            SigningState::SignedOtherBranch => true,
            SigningState::Signed => working.as_ref().is_none_or(|w| w.sha256 != s.packet_sha256),
            _ => false,
        };
        if !needs {
            continue;
        }
        let Ok(Some(bytes)) = show::blob_at(root.path(), commit, &path, &ctx) else {
            continue;
        };
        let committed = PacketFile {
            sha256: sha256_hex(&bytes),
            text: String::from_utf8_lossy(&bytes).into_owned(),
            stamp: working
                .as_ref()
                .map_or(ReadStamp::Absent, |w| w.stamp.clone()),
        };
        if s.state == SigningState::Signed {
            o.tampered.insert(
                n,
                working.unwrap_or(PacketFile {
                    text: String::new(),
                    sha256: String::new(),
                    stamp: ReadStamp::Absent,
                }),
            );
        }
        o.packets.insert(n, committed);
    }
}

/// The tamper diff of a stage (committed → working), for the packet view and Today.
#[must_use]
pub fn tamper_diff(p: &Opened, n: u8) -> Option<Diff> {
    let w = p.tampered.get(&n)?;
    let c = p.packets.get(&n)?;
    Some(crate::diff::diff(
        &packet::file_name(n),
        Some(&c.text),
        &w.text,
    ))
}

/// Retries a pending real-index refresh (on open; ADR-012 4.7). Clears it on success.
pub fn retry_index_refresh(app: &App, root: &Root, progress: &mut ProgressFile) {
    let Some(pending) = progress.index_refresh_pending.clone() else {
        return;
    };
    let Ok(ctx) = app.spawn_ctx_now() else { return };
    let opts = CommitOpts {
        app_data: app.paths().app_data.clone(),
        ..CommitOpts::default()
    };
    if foreman_sys::git::commit::retry_index_refresh(
        root,
        &pending.commit,
        &pending.paths,
        &opts,
        &ctx,
    )
    .unwrap_or(false)
    {
        progress.index_refresh_pending = None;
    }
}

/// `tamper_restore` (AC-F014-15, SR-46): writes the committed blob back over the working file by
/// compare-and-swap against the working version Billy saw.
///
/// # Errors
/// `Stale` (the file changed again), `Conflict`, `NotAllowed("not-tampered")`.
pub fn restore(app: &App, project_id: &str, n: u8, seen: &str) -> Result<Opened, IpcError> {
    let p = crate::project::open(app, project_id)?;
    p.writable()?;
    let Some(w) = p.tampered.get(&n) else {
        return Err(IpcError::not_allowed(
            "not-tampered",
            "The packet matches its commit.",
        ));
    };
    if w.sha256 != seen {
        return Err(IpcError::stale(
            StaleWhat::ReadStamp,
            "The packet changed again. Review the new difference.",
            None,
        ));
    }
    let committed = p
        .packets
        .get(&n)
        .cloned()
        .ok_or_else(|| IpcError::not_found("No committed packet."))?;
    let rel = packet_path(n)?;
    p.root()?
        .write(&rel, committed.text.as_bytes(), &w.stamp)
        .map_err(|e| conflict_or(e, &rel))?;
    crate::project::open(app, project_id)
}

/// Today's tamper items (AC-F014-15): one per signed packet whose working file differs from its
/// commit, with the changed line count; the diff is on the packet view and `tamper_restore` is one
/// click there.
#[must_use]
pub fn tamper_inbox(app: &App, p: &Opened) -> Vec<crate::ipc::dto::app::InboxItem> {
    use crate::ipc::dto::app::{InboxItem, InboxKind};
    use crate::ipc::types::{ProjectId, Stage, Timestamp};
    let name = p
        .project
        .as_ref()
        .map_or_else(|| p.entry.name.clone(), |x| x.name.clone());
    p.tampered
        .keys()
        .map(|n| {
            let changed = tamper_diff(p, *n).map_or(0, |d| {
                d.hunks
                    .iter()
                    .flat_map(|h| h.lines.iter())
                    .filter(|l| !matches!(l.op, crate::ipc::types::DiffOp::Ctx))
                    .count()
            });
            InboxItem {
                kind: InboxKind::Tamper,
                project: ProjectId::from_trusted(p.id()),
                step: None,
                stage: Some(Stage(*n)),
                at: Timestamp(app.now()),
                project_name: escape_visible(&name),
                title: format!("Signed Stage {n} packet changed on disk"),
                detail: format!(
                    "{} differs from its commit ({changed} lines). Foreman uses the committed version. Review the diff, then restore it.",
                    packet::file_name(*n)
                ),
                chain: None,
                run: None,
            }
        })
        .collect()
}
