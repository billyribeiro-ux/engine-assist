//! The Today inbox (F-017 P2, AC-F017-07 to -09; ADR-014; M4-25): everything waiting on Billy
//! across projects, derived on every read from facts Foreman already keeps (sessions, packets,
//! gates, pending outside changes, tamper, copied chat prompts, waits, interrupted runs), never
//! stored. The text of every item is a Foreman template (SR-56): identifiers, counts and
//! Foreman's own validation messages, never prompt, terminal or assistant text.
//!
//! [`sync`] keeps the OS in step with the inbox: the Dock badge counts the first four kinds
//! ([`crate::notify::BADGED`]) and a notification fires once for a new `session-blocked` or
//! `packet-landed` item while Foreman isn't focused. `run()` switches [`schedule`] on; every event
//! that can change the inbox then schedules a sync, and a minute tick covers time passing (a chat
//! prompt crossing ten minutes, a wait date arriving).
//!
//! `step-ready` is in the order of AC-F017-07 but no criterion says when a step becomes "ready"
//! as news (the Next step rows already show every project's next step), so nothing emits it yet.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use foreman_core::schema::progress::StoredState;

use crate::closeout;
use crate::ipc::IpcError;
use crate::ipc::commands::{gates, projects::summaries};
use crate::ipc::dto::app::{InboxItem, InboxKind};
use crate::ipc::dto::gates::GateState;
use crate::ipc::types::{ChainId, ProjectId, RunId, RunStatus, Stage, StepKey, Timestamp};
use crate::notify;
use crate::project::{self, Opened, display_number};
use crate::render::escape_visible;
use crate::state::App;

/// A copied chat prompt with no paste-back after this long is waiting on Billy (AC-F017-08).
pub const CHAT_AWAIT: Duration = Duration::from_secs(10 * 60);

/// How often the background tick syncs (covers time passing with no event).
const TICK: Duration = Duration::from_secs(60);
/// Events arriving within this window share one sync.
const COALESCE: Duration = Duration::from_millis(300);

/// AC-F017-07's order; the two packet kinds share a rank, and so do the two gate kinds.
#[must_use]
pub fn rank(kind: InboxKind) -> u8 {
    match kind {
        InboxKind::SessionBlocked => 0,
        InboxKind::SessionYourTurn => 1,
        InboxKind::PacketLanded | InboxKind::PacketInvalid => 2,
        InboxKind::GateFailed | InboxKind::GateSignable => 3,
        InboxKind::OutsideChange => 4,
        InboxKind::Tamper => 5,
        InboxKind::ChatAwaitingPaste => 6,
        InboxKind::WaitElapsed => 7,
        InboxKind::InterruptedSession => 8,
        InboxKind::StepReady => 9,
    }
}

/// Orders items by [`rank`]; items of one rank keep their project order.
pub fn sort(items: &mut [InboxItem]) {
    items.sort_by_key(|i| rank(i.kind));
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn item(app: &App, p: &Opened, kind: InboxKind, title: String, detail: String) -> InboxItem {
    InboxItem {
        kind,
        project: ProjectId::from_trusted(p.id()),
        step: None,
        stage: None,
        at: Timestamp(app.now()),
        project_name: escape_visible(&p.summary(app).name),
        title,
        detail,
        chain: None,
        run: None,
    }
}

fn step_label(p: &Opened, key: &str) -> String {
    p.step(key).map_or_else(
        |_| key.to_owned(),
        |s| format!("{} {}", display_number(s), s.title),
    )
}

/// A project's own items: its live sessions that need Billy, its current stage's packet and gate,
/// pending outside changes, tamper, copied chat prompts and elapsed waits.
#[must_use]
pub fn project_items(app: &App, p: &Opened) -> Vec<InboxItem> {
    let mut out = crate::signing::tamper_inbox(app, p);
    if p.read_only.is_some() || p.template.is_none() {
        return out;
    }
    out.extend(session_items(app, p));
    out.extend(stage_items(app, p));
    out.extend(change_items(app, p));
    out.extend(chat_items(app, p));
    out.extend(wait_items(app, p));
    out
}

fn session_items(app: &App, p: &Opened) -> Vec<InboxItem> {
    let mut out = Vec::new();
    for r in app.runs().live() {
        if r.project_id != p.id() {
            continue;
        }
        let (kind, title, detail) = match app.runs().observed(&r.run_id) {
            Some(RunStatus::Blocked) => (
                InboxKind::SessionBlocked,
                format!(
                    "Claude is asking for permission in {}",
                    step_label(p, &r.step)
                ),
                format!("Session {} is waiting for your answer.", r.tab_name),
            ),
            Some(RunStatus::Idle) => (
                InboxKind::SessionYourTurn,
                format!(
                    "Claude finished and is waiting in {}",
                    step_label(p, &r.step)
                ),
                format!(
                    "Session {} is idle. Say what comes next, or end it.",
                    r.tab_name
                ),
            ),
            _ => continue,
        };
        let mut i = item(app, p, kind, title, detail);
        i.step = Some(StepKey::from_trusted(r.step.as_str()));
        i.stage = project::stage_of_key(&r.step).map(Stage);
        i.chain = Some(ChainId::from_trusted(r.chain.as_str()));
        i.run = Some(RunId::from_trusted(r.run_id.as_str()));
        out.push(i);
    }
    out
}

/// The current stage's packet and gate: an invalid packet, a failed gate, a packet that is ready
/// to sign, or a valid packet whose gate is still open.
fn stage_items(app: &App, p: &Opened) -> Vec<InboxItem> {
    let Some(n) = p.current_stage().filter(|n| *n <= 17) else {
        return Vec::new();
    };
    let packet = p.packets.get(&n);
    let has_gate = p.progress.gate_runs.contains_key(&n.to_string());
    if packet.is_none() && !has_gate {
        return Vec::new();
    }
    let file = foreman_core::packet::file_name(n);
    let mut out = Vec::new();
    let stage = Some(Stage(n));
    if let Some(pk) = packet {
        let problems = p.packet_problems(n, &pk.text);
        if !problems.is_empty() {
            let why = problems
                .iter()
                .take(2)
                .map(|x| escape_visible(&x.message))
                .collect::<Vec<_>>()
                .join("; ");
            let mut i = item(
                app,
                p,
                InboxKind::PacketInvalid,
                format!(
                    "Stage {n} packet has {}",
                    plural(problems.len(), "problem", "problems")
                ),
                why,
            );
            i.stage = stage;
            return vec![i];
        }
    }
    let Ok(c) = closeout::gather(app, p, n) else {
        return out;
    };
    let v = gates::view(p, &c);
    let failed = v
        .items
        .iter()
        .filter(|x| x.status == crate::ipc::dto::gates::ItemStatus::Fail)
        .count();
    let (kind, title, detail) = if v.state == GateState::Failed {
        (
            InboxKind::GateFailed,
            format!(
                "Stage {n} gate has {}",
                plural(failed.max(1), "failed item", "failed items")
            ),
            "Fix what failed, then run the inspection again.".to_owned(),
        )
    } else if v.signable {
        let passed = v
            .items
            .iter()
            .filter(|x| x.status == crate::ipc::dto::gates::ItemStatus::Pass)
            .count();
        let lessons = if c.facts.lessons_found {
            ", lessons found"
        } else {
            ""
        };
        (
            InboxKind::GateSignable,
            format!("Stage {n} packet is valid and ready to sign"),
            format!("{file}, gate passed {passed} of {}{lessons}", v.items.len()),
        )
    } else if packet.is_some() {
        (
            InboxKind::PacketLanded,
            format!("Stage {n} packet landed"),
            format!("{file} is valid. Finish the close-out to sign it."),
        )
    } else {
        return out;
    };
    let mut i = item(app, p, kind, title, detail);
    i.stage = stage;
    out.push(i);
    out
}

fn change_items(app: &App, p: &Opened) -> Vec<InboxItem> {
    p.progress
        .pending_outside
        .iter()
        .map(|x| {
            let file = x.file.repo_path();
            let keys: Vec<String> = x.diff.iter().map(|e| escape_visible(&e.key_path)).collect();
            let what = if keys.is_empty() {
                "formatting only".to_owned()
            } else {
                keys.join(", ")
            };
            item(
                app,
                p,
                InboxKind::OutsideChange,
                format!("{file} changed outside Foreman"),
                format!("Changed: {what}. It has no effect until you accept it."),
            )
        })
        .collect()
}

/// Chat steps copied at least [`CHAT_AWAIT`] ago whose reply hasn't come back (AC-F017-08). The
/// copy time is the step's stored `copiedAt`, so it survives a restart.
fn chat_items(app: &App, p: &Opened) -> Vec<InboxItem> {
    let now = app.os().now();
    let mut out: Vec<(jiff::Timestamp, InboxItem)> = Vec::new();
    for (key, sp) in &p.progress.steps {
        if sp.state != StoredState::Copied {
            continue;
        }
        let Some(at) = sp
            .copied_at
            .as_deref()
            .and_then(|t| t.parse::<jiff::Timestamp>().ok())
        else {
            continue;
        };
        let waited = now.as_second() - at.as_second();
        if waited < i64::try_from(CHAT_AWAIT.as_secs()).unwrap_or(i64::MAX) {
            continue;
        }
        let Ok(step) = p.step(key.as_str()) else {
            continue;
        };
        let mut i = item(
            app,
            p,
            InboxKind::ChatAwaitingPaste,
            format!("{} is waiting for its reply", step_label(p, key.as_str())),
            format!(
                "Copied {} minutes ago. Paste the reply back (⌘⇧V) or mark the step done (⌘D).",
                waited / 60
            ),
        );
        i.at = Timestamp(crate::os::rfc3339(at));
        i.stage = Some(Stage(step.stage));
        i.step = Some(StepKey::from_trusted(key.as_str()));
        i.run = sp.last_run_id.as_deref().map(RunId::from_trusted);
        out.push((at, i));
    }
    out.sort_by_key(|(at, _)| *at);
    out.into_iter().map(|(_, i)| i).collect()
}

fn wait_items(app: &App, p: &Opened) -> Vec<InboxItem> {
    let today = app.os().today();
    p.progress
        .waits
        .iter()
        .filter(|(key, until)| {
            until.parse::<jiff::civil::Date>().is_ok_and(|d| d <= today)
                && p.step_progress(key.as_str()).state != StoredState::Done
        })
        .map(|(key, until)| {
            let mut i = item(
                app,
                p,
                InboxKind::WaitElapsed,
                format!("The wait before {} is over", step_label(p, key.as_str())),
                format!("It ran until {until}. The step can start now."),
            );
            i.step = Some(StepKey::from_trusted(key.as_str()));
            i.stage = project::stage_of_key(key.as_str()).map(Stage);
            i
        })
        .collect()
}

/// Runs that were running when Foreman stopped and can't be resumed (the ones that can are listed
/// with Resume under Interrupted sessions).
///
/// # Errors
/// `Io`.
pub fn interrupted_items(app: &App) -> Result<Vec<InboxItem>, IpcError> {
    Ok(crate::runs::interrupted(app)?
        .into_iter()
        .filter(|r| !r.resumable)
        .map(|r| InboxItem {
            kind: InboxKind::InterruptedSession,
            project: r.project.clone(),
            step: r.step.clone(),
            stage: r
                .step
                .as_ref()
                .and_then(|s| project::stage_of_key(s.as_str()))
                .map(Stage),
            at: r.started_at.clone(),
            project_name: escape_visible(&r.project_name),
            title: format!(
                "{} {} can't resume",
                r.step.as_ref().map_or("", StepKey::as_str),
                escape_visible(&r.step_title)
            ),
            detail: r.why_not_resumable.clone().unwrap_or_default(),
            chain: Some(r.chain.clone()),
            run: Some(r.run_id.clone()),
        })
        .collect())
}

/// The whole inbox, in order.
///
/// # Errors
/// `Io` (the registry).
pub fn collect(app: &App) -> Result<Vec<InboxItem>, IpcError> {
    let mut items = Vec::new();
    for s in summaries(app)? {
        if let Ok(p) = project::open(app, s.id.as_str()) {
            items.extend(project_items(app, &p));
        }
    }
    items.extend(interrupted_items(app)?);
    sort(&mut items);
    Ok(items)
}

/// What identifies an item while it stays in the inbox (notifications fire once per key).
fn key(i: &InboxItem) -> String {
    format!(
        "{:?}|{}|{}|{}|{}",
        i.kind,
        i.project.as_str(),
        i.stage.map_or(255, |s| s.0),
        i.step.as_ref().map_or("", StepKey::as_str),
        i.run.as_ref().map_or("", RunId::as_str),
    )
}

/// Updates the Dock badge and sends the notifications that are due (AC-F017-09): a notifying item
/// that wasn't in the inbox at the last sync notifies once, and only while Foreman isn't focused;
/// an item that appeared while focused counts as seen. An item that leaves and returns notifies
/// again.
pub fn sync(app: &App) {
    let Ok(items) = collect(app) else {
        return;
    };
    sync_items(app, &items);
}

/// [`sync`] for an inbox already collected.
pub fn sync_items(app: &App, items: &[InboxItem]) {
    let badge = items
        .iter()
        .filter(|i| notify::BADGED.contains(&i.kind))
        .count();
    app.os()
        .set_badge(u32::try_from(badge).ok().filter(|n| *n > 0));
    let focused = app.os().app_focused();
    let live: HashSet<String> = items.iter().map(key).collect();
    let mut due = Vec::new();
    {
        let mut inner = app.inner();
        inner.notified.retain(|k| live.contains(k));
        for i in items {
            if notify::NOTIFYING.contains(&i.kind) && inner.notified.insert(key(i)) && !focused {
                due.push(i);
            }
        }
    }
    for i in due {
        if let Some(n) = notify::for_item(i) {
            // A refused notification (the user turned them off in macOS) is not an app error.
            let _ = app.os().notify(&n);
        }
    }
}

/// Wait steps whose date has come notify once per run of the app, focused or not (AC-F007-09):
/// a wait that elapsed while Foreman was closed notifies at the next launch.
pub fn notify_waits(app: &App, items: &[InboxItem]) {
    let mut due = Vec::new();
    {
        let mut inner = app.inner();
        for i in items.iter().filter(|i| i.kind == InboxKind::WaitElapsed) {
            if inner.notified.insert(key(i)) {
                due.push(i);
            }
        }
    }
    for i in due {
        let step = i.step.as_ref().map_or("", StepKey::as_str);
        let _ = app
            .os()
            .notify(&notify::wait_elapsed(&i.project_name, step));
    }
}

/// Asks for a sync soon (coalesced). Does nothing until `run()` calls [`start`].
pub fn schedule(app: &App) {
    let Some(me) = app.weak().upgrade() else {
        return;
    };
    {
        let mut inner = app.inner();
        if !inner.inbox_sync || inner.inbox_scheduled {
            return;
        }
        inner.inbox_scheduled = true;
    }
    let _ = std::thread::Builder::new()
        .name("fm-inbox-sync".into())
        .spawn(move || {
            std::thread::sleep(COALESCE);
            me.inner().inbox_scheduled = false;
            run_sync(&me);
        });
}

fn run_sync(app: &App) {
    let Ok(items) = collect(app) else {
        return;
    };
    sync_items(app, &items);
    notify_waits(app, &items);
}

/// Switches event-driven syncs on, syncs once now (items already present when Foreman starts
/// count as seen if it is focused) and starts the minute tick.
pub fn start(app: &Arc<App>) {
    app.inner().inbox_sync = true;
    let weak = Arc::downgrade(app);
    let _ = std::thread::Builder::new()
        .name("fm-inbox-tick".into())
        .spawn(move || {
            loop {
                let Some(app) = weak.upgrade() else {
                    return;
                };
                run_sync(&app);
                drop(app);
                std::thread::sleep(TICK);
            }
        });
}
