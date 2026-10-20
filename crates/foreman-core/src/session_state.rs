//! Derived session state from sensor events (F-011, P2, ADR-009, SR-15, SR-18).
//!
//! Ingest accepts an event only with the run's token, reads only the hook-input fields named in
//! ADR-009, and folds events (ordered by `seq`) into [`SessionState`]. The result is displayed and
//! enables buttons; nothing here, and nothing that reads it, starts an action (SR-18,
//! `sensor::events_only_update_state`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::schema::sensor_event::SensorEventFile;

/// Why a session is blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockedOn {
    /// A permission prompt.
    Permission,
    /// Waiting for input (an idle notification after a question).
    Input,
    /// Plan approval.
    PlanApproval,
}

/// The coarse activity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum Activity {
    /// No events yet.
    Unknown,
    /// Working on a turn.
    Working,
    /// The last event was `Stop` and nothing is outstanding.
    Idle,
    /// Blocked.
    Blocked {
        /// On what.
        on: BlockedOn,
    },
    /// `SessionEnd`.
    Ended {
        /// The reason, when the payload had one.
        reason: Option<String>,
    },
}

/// What the UI shows for a live run (P2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    /// Activity.
    pub activity: Activity,
    /// Live `permission_mode` (the mode guard warns on a mismatch; never acts).
    pub permission_mode: Option<String>,
    /// Live `effort.level`.
    pub effort: Option<String>,
    /// Outstanding subagents.
    pub subagents: u32,
    /// Outstanding tasks.
    pub tasks: u32,
    /// Background Bash commands started (no end event exists; cleared at `Stop` only when the
    /// payload says none run, and at `SessionEnd`).
    pub background_shells: u32,
    /// `last_assistant_message` of the last `Stop`/`SubagentStop` (≤ 64 KB kept).
    pub last_assistant_message: Option<String>,
    /// The model `SessionStart` reported.
    pub observed_model: Option<String>,
    /// The highest `seq` folded in.
    pub last_seq: Option<String>,
    /// Events discarded for a wrong token (logged by the caller).
    pub discarded: u32,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            activity: Activity::Unknown,
            permission_mode: None,
            effort: None,
            subagents: 0,
            tasks: 0,
            background_shells: 0,
            last_assistant_message: None,
            observed_model: None,
            last_seq: None,
            discarded: 0,
        }
    }
}

impl SessionState {
    /// Outstanding background work (subagents, tasks, background shells).
    #[must_use]
    pub fn outstanding(&self) -> u32 {
        self.subagents + self.tasks + self.background_shells
    }

    /// ADR-008 P2 idle: last event `Stop` and nothing outstanding.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.activity == Activity::Idle && self.outstanding() == 0
    }
}

/// Constant-time equality for tokens (no early exit on the first differing byte).
#[must_use]
pub fn token_matches(expected: &str, got: &str) -> bool {
    let (a, b) = (expected.as_bytes(), got.as_bytes());
    if a.len() != b.len() || a.is_empty() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn s(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str).map(str::to_owned)
}

const MAX_MESSAGE: usize = 64 * 1024;

/// Folds one accepted event into the state. `seq` is the file-name sequence.
pub fn apply(st: &mut SessionState, seq: &str, ev: &SensorEventFile) {
    let p = ev.payload.as_ref();
    let get = |k: &str| p.and_then(|m| m.get(k));
    if let Some(m) = s(get("permission_mode")) {
        st.permission_mode = Some(m);
    }
    if let Some(e) = get("effort")
        .and_then(|e| e.get("level"))
        .and_then(Value::as_str)
    {
        st.effort = Some(e.to_owned());
    }
    match ev.event.as_str() {
        "SessionStart" => {
            st.activity = Activity::Working;
            if let Some(m) = s(get("model")) {
                st.observed_model = Some(m);
            }
        }
        "Stop" => {
            st.activity = Activity::Idle;
            st.last_assistant_message = s(get("last_assistant_message")).map(|mut t| {
                if t.len() > MAX_MESSAGE {
                    let mut cut = MAX_MESSAGE;
                    while !t.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    t.truncate(cut);
                }
                t
            });
        }
        "StopFailure" => st.activity = Activity::Idle,
        "PermissionRequest" => {
            st.activity = Activity::Blocked {
                on: if get("tool_name").and_then(Value::as_str) == Some("ExitPlanMode") {
                    BlockedOn::PlanApproval
                } else {
                    BlockedOn::Permission
                },
            };
        }
        "Notification" => {
            let kind = s(get("notification_type")).unwrap_or_default();
            if kind == "permission_prompt" {
                st.activity = Activity::Blocked {
                    on: BlockedOn::Permission,
                };
            } else if kind == "idle_prompt" || kind == "elicitation_dialog" {
                st.activity = Activity::Blocked {
                    on: BlockedOn::Input,
                };
            }
        }
        "SubagentStart" => {
            st.subagents += 1;
            st.activity = Activity::Working;
        }
        "SubagentStop" => st.subagents = st.subagents.saturating_sub(1),
        "TaskCreated" => st.tasks += 1,
        "TaskCompleted" => st.tasks = st.tasks.saturating_sub(1),
        "PostToolUse" => {
            if get("tool_input")
                .and_then(|t| t.get("run_in_background"))
                .and_then(Value::as_bool)
                == Some(true)
            {
                st.background_shells += 1;
            }
            if !matches!(st.activity, Activity::Ended { .. }) {
                st.activity = Activity::Working;
            }
        }
        "SessionEnd" => {
            st.activity = Activity::Ended {
                reason: s(get("reason")),
            };
            st.subagents = 0;
            st.tasks = 0;
            st.background_shells = 0;
        }
        _ => {}
    }
    st.last_seq = Some(seq.to_owned());
}

/// Ingests events in `seq` order: wrong-token events are counted and dropped; the rest are folded.
pub fn ingest<'a>(
    st: &mut SessionState,
    token: &str,
    events: impl IntoIterator<Item = (&'a str, &'a SensorEventFile)>,
) {
    let mut v: Vec<(&str, &SensorEventFile)> = events.into_iter().collect();
    v.sort_by(|a, b| a.0.cmp(b.0));
    for (seq, ev) in v {
        if st.last_seq.as_deref().is_some_and(|l| seq <= l) {
            continue;
        }
        if token_matches(token, &ev.token) {
            apply(st, seq, ev);
        } else {
            st.discarded += 1;
        }
    }
}

/// `true` when the file name is an event file: `<20 digits>-<digits>-<Event>.json`.
#[must_use]
pub fn event_file_seq(name: &str) -> Option<&str> {
    let stem = name.strip_suffix(".json")?;
    let mut parts = stem.splitn(3, '-');
    let a = parts.next()?;
    let b = parts.next()?;
    let ev = parts.next()?;
    let ok = a.len() == 20
        && a.bytes().all(|c| c.is_ascii_digit())
        && !b.is_empty()
        && b.bytes().all(|c| c.is_ascii_digit())
        && (1..=40).contains(&ev.len())
        && ev.bytes().all(|c| c.is_ascii_alphabetic());
    ok.then(|| &stem[..a.len() + 1 + b.len()])
}
