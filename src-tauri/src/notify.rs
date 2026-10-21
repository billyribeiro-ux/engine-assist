//! Notification templates (SR-56, ADR-014; AC-F007-09, AC-F017-09). Every notification is a fixed
//! Foreman sentence filled with identifiers Foreman already shows (the project's name, a stage or
//! step number). It never carries prompt, packet, terminal or assistant text, and nothing is
//! registered to act on a click, so clicking only brings Foreman to the front.
//!
//! `notify::body_template_snapshot` (src-tauri/tests/notify.rs) pins every template.

use crate::ipc::dto::app::{InboxItem, InboxKind};
use crate::os::Notice;

/// The longest project name a notification repeats.
const NAME_MAX: usize = 40;

/// The inbox kinds that notify when Foreman isn't focused (AC-F017-09).
pub const NOTIFYING: [InboxKind; 2] = [InboxKind::SessionBlocked, InboxKind::PacketLanded];

/// The inbox kinds the Dock badge counts: the first four of AC-F017-07's order.
pub const BADGED: [InboxKind; 4] = [
    InboxKind::SessionBlocked,
    InboxKind::SessionYourTurn,
    InboxKind::PacketLanded,
    InboxKind::PacketInvalid,
];

/// A project name as a notification repeats it: no control characters, collapsed spaces, at most
/// [`NAME_MAX`] characters.
#[must_use]
pub fn name(raw: &str) -> String {
    let flat = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if flat.chars().count() <= NAME_MAX {
        flat
    } else {
        let cut: String = flat.chars().take(NAME_MAX - 1).collect();
        format!("{cut}…")
    }
}

/// The notification for an inbox item, if its kind notifies.
#[must_use]
pub fn for_item(item: &InboxItem) -> Option<Notice> {
    let who = name(&item.project_name);
    match item.kind {
        InboxKind::SessionBlocked => {
            let step = item
                .step
                .as_ref()
                .map_or_else(String::new, |s| format!(" in step {}", s.as_str()));
            Some(Notice {
                title: "Claude is waiting for you".to_owned(),
                body: format!("{who}: a session needs your permission{step}."),
            })
        }
        InboxKind::PacketLanded => {
            let stage = item
                .stage
                .map_or_else(String::new, |s| format!(" Stage {}", s.0));
            Some(Notice {
                title: "A packet landed".to_owned(),
                body: format!("{who}: the{stage} packet is ready to review."),
            })
        }
        _ => None,
    }
}

/// The notification for a wait step whose date has come (AC-F007-09). A wait whose date passed
/// while Foreman was closed fires this at the next launch.
#[must_use]
pub fn wait_elapsed(project_name: &str, step: &str) -> Notice {
    Notice {
        title: "A wait is over".to_owned(),
        body: format!("{}: step {step} can start now.", name(project_name)),
    }
}
