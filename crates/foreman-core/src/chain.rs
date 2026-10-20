//! Session chains (F-010, ADR-008, DR-110–DR-118, SR-14).
//!
//! [`plan_session`] is pure: it turns a step and the stored chains into a [`SessionPlan`]; the
//! transcript check is a fact passed to [`check_transcript`]; [`apply_launch`] records a launch in
//! the chain list (new chains, `superseded`, forks). Argv comes from [`session_args`] with the S-8
//! and S-3 branches as explicit inputs.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::ids::{Effort, Start, StepKey};
use crate::launch::argv::SessionArgs;
use crate::schema::progress::{Chain, ChainStatus, ChainSurface};
use crate::template::{Session, Step};

/// What a launch does with sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionPlan {
    /// A chain start: a new session with Foreman's UUID.
    New {
        /// The new UUID.
        id: Uuid,
    },
    /// Continue the chain with `--resume`.
    Continue {
        /// The chain.
        chain: Uuid,
    },
    /// The chain was handed off to Desktop: fork it (DR-118).
    Fork {
        /// The handed-off chain.
        from: Uuid,
        /// Foreman's ID for the fork.
        new_id: Uuid,
    },
    /// The chain's transcript is gone (DR-113): offer "Start a fresh chain with context".
    Broken {
        /// The chain.
        chain: Uuid,
    },
    /// The chain start doesn't apply or was skipped (DR-113): a new session whose prompt starts
    /// with the last signed packet and the declared inputs.
    FreshWithContext {
        /// The new UUID.
        id: Uuid,
    },
    /// The chain start hasn't run yet.
    NeedsStart {
        /// The chain start step.
        start: StepKey,
    },
}

/// What happened to the chain start step (for DR-113's "doesn't apply" rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainStartState {
    /// It ran (a chain may exist).
    Ran,
    /// Not applicable on this path.
    NotApplicable,
    /// Skipped by Billy.
    Skipped,
    /// Not run yet.
    NotRun,
}

/// Whether the Desktop handoff's fork may be skipped because Billy confirmed Desktop is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HandoffConfirm {
    /// Not confirmed: fork.
    #[default]
    Fork,
    /// Billy confirmed Desktop is closed: continue the chain.
    DesktopClosed,
}

fn uuid_of(c: &Chain) -> Option<Uuid> {
    Uuid::parse_str(&c.id).ok()
}

/// The live chain for a chain start: the last one with that start that isn't superseded.
#[must_use]
pub fn chain_for<'a>(chains: &'a [Chain], start: &StepKey) -> Option<&'a Chain> {
    chains
        .iter()
        .rev()
        .find(|c| &c.start == start && c.status != ChainStatus::Superseded)
}

/// DR-110: the session plan for a step. `new_id` is the UUID a new session or fork gets.
#[must_use]
pub fn plan_session(
    step: &Step,
    chains: &[Chain],
    new_id: Uuid,
    start_state: ChainStartState,
    handoff: HandoffConfirm,
) -> SessionPlan {
    if step.session == Session::New {
        return SessionPlan::New { id: new_id };
    }
    let start = step.chain.clone().unwrap_or_else(|| step.key.clone());
    match (chain_for(chains, &start), start_state) {
        (_, ChainStartState::NotApplicable | ChainStartState::Skipped) => {
            SessionPlan::FreshWithContext { id: new_id }
        }
        (None, _) => SessionPlan::NeedsStart { start },
        (Some(c), _) => {
            let Some(id) = uuid_of(c) else {
                return SessionPlan::Broken { chain: Uuid::nil() };
            };
            match c.status {
                ChainStatus::Broken => SessionPlan::Broken { chain: id },
                ChainStatus::HandedOff if handoff == HandoffConfirm::Fork => {
                    SessionPlan::Fork { from: id, new_id }
                }
                _ => SessionPlan::Continue { chain: id },
            }
        }
    }
}

/// DR-113: a continuing plan whose transcript is missing becomes [`SessionPlan::Broken`].
#[must_use]
pub fn check_transcript(plan: SessionPlan, transcript_exists: bool) -> SessionPlan {
    match plan {
        SessionPlan::Continue { chain } | SessionPlan::Fork { from: chain, .. }
            if !transcript_exists =>
        {
            SessionPlan::Broken { chain }
        }
        p => p,
    }
}

/// Claude Code's project folder name for a cwd: every UTF-16 code unit that isn't an ASCII letter
/// or digit becomes `-`; a name over 200 characters is cut to 200 and suffixed with `-` and the
/// base-36 absolute value of the Java-style 32-bit string hash. Read from the Claude Code 2.1.304
/// binary on 2026-10-20 (`replace(/[^a-zA-Z0-9]/g,"-")`, `_ye=200`, `cce`), as ADR-008 asks
/// ("encoding verified in M3").
#[must_use]
pub fn project_dir_name(cwd: &str) -> String {
    let units: Vec<u16> = cwd.encode_utf16().collect();
    let name: String = units
        .iter()
        .map(|&u| {
            let c = char::from_u32(u32::from(u)).unwrap_or('-');
            if c.is_ascii_alphanumeric() { c } else { '-' }
        })
        .collect();
    if name.len() <= 200 {
        return name;
    }
    let mut h: i32 = 0;
    for &u in &units {
        h = h.wrapping_shl(5).wrapping_sub(h).wrapping_add(i32::from(u));
    }
    format!("{}-{}", &name[..200], to_base36(h.unsigned_abs()))
}

fn to_base36(mut n: u32) -> String {
    const D: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut v = Vec::new();
    while n > 0 {
        v.push(D[(n % 36) as usize]);
        n /= 36;
    }
    v.reverse();
    String::from_utf8(v).unwrap_or_default()
}

/// ADR-008: `<CLAUDE_CONFIG_DIR or ~/.claude>/projects/<project dir>/<id>.jsonl`.
#[must_use]
pub fn transcript_path(config_dir: &Path, cwd: &Path, id: Uuid) -> PathBuf {
    config_dir
        .join("projects")
        .join(project_dir_name(&cwd.display().to_string()))
        .join(format!("{}.jsonl", id.hyphenated()))
}

/// S-8's fork-ID branch (pending Billy's run; `Pass` is the specified behavior).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ForkIds {
    /// `--session-id <new>` is honored with `--resume <id> --fork-session`.
    #[default]
    Pass,
    /// The fork's ID is "the only new transcript after launch".
    Fail,
}

/// DR-111: argv session addressing for a plan.
#[must_use]
pub fn session_args(plan: &SessionPlan, fork_ids: ForkIds) -> Option<SessionArgs> {
    Some(match plan {
        SessionPlan::New { id } | SessionPlan::FreshWithContext { id } => {
            SessionArgs::New { id: *id }
        }
        SessionPlan::Continue { chain } => SessionArgs::Resume { id: *chain },
        SessionPlan::Fork { from, new_id } => SessionArgs::Fork {
            from: *from,
            new_id: (fork_ids == ForkIds::Pass).then_some(*new_id),
        },
        SessionPlan::Broken { .. } | SessionPlan::NeedsStart { .. } => return None,
    })
}

/// S-3's branch for effort on resume (DR-117; pending Billy's run, `Flag` is the pass branch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EffortOnResume {
    /// `--effort <level>` works on `--resume`.
    #[default]
    Flag,
    /// `effortLevel` in the run settings works (`low`…`xhigh` only).
    Settings,
    /// Neither: show the `/effort <level>` line for Billy to type.
    Slash,
}

/// How a resumed step's effort is carried (DR-117).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EffortCarry {
    /// Pass `--effort`.
    pub flag: bool,
    /// Write `effortLevel` into the run settings.
    pub settings_level: Option<Effort>,
    /// Show this line (never typed by Foreman), with the warning that it saves Billy's default.
    pub slash_line: Option<String>,
}

/// DR-117: new sessions always pass `--effort`; resumed ones follow S-3's branch.
#[must_use]
pub fn effort_carry(args: &SessionArgs, effort: Effort, branch: EffortOnResume) -> EffortCarry {
    let resumed = !matches!(args, SessionArgs::New { .. });
    if !resumed {
        return EffortCarry {
            flag: true,
            ..EffortCarry::default()
        };
    }
    let settings_ok = matches!(
        effort,
        Effort::Low | Effort::Medium | Effort::High | Effort::Xhigh
    );
    match branch {
        EffortOnResume::Flag => EffortCarry {
            flag: true,
            ..EffortCarry::default()
        },
        EffortOnResume::Settings if settings_ok => EffortCarry {
            settings_level: Some(effort),
            ..EffortCarry::default()
        },
        EffortOnResume::Settings | EffortOnResume::Slash => EffortCarry {
            slash_line: Some(format!("/effort {}", effort.as_str())),
            ..EffortCarry::default()
        },
    }
}

/// DR-112, DR-118: the chain list after a launch of `step` with `plan` and effective `start`.
/// A new chain on a chain-start step supersedes the previous live chain with that start.
#[must_use]
pub fn apply_launch(chains: &[Chain], step: &Step, plan: &SessionPlan, start: Start) -> Vec<Chain> {
    let mut v = chains.to_vec();
    let start_key = step.chain.clone().unwrap_or_else(|| step.key.clone());
    let new_chain = |id: Uuid, forked_from: Option<String>| Chain {
        id: id.hyphenated().to_string(),
        surface: ChainSurface::Cli,
        cwd: step.cwd,
        start: start_key.clone(),
        steps: vec![step.key.clone()],
        status: ChainStatus::Open,
        forked_from,
        last_start: Some(start),
    };
    match plan {
        SessionPlan::New { id } | SessionPlan::FreshWithContext { id } => {
            for c in &mut v {
                if c.start == start_key && c.status != ChainStatus::Superseded {
                    c.status = ChainStatus::Superseded;
                }
            }
            v.push(new_chain(*id, None));
        }
        SessionPlan::Continue { chain } => {
            let id = chain.hyphenated().to_string();
            if let Some(c) = v.iter_mut().find(|c| c.id == id) {
                if !c.steps.contains(&step.key) {
                    c.steps.push(step.key.clone());
                }
                c.last_start = Some(start);
                if c.status == ChainStatus::HandedOff {
                    c.status = ChainStatus::Open;
                }
            }
        }
        SessionPlan::Fork { from, new_id } => {
            // The handed-off chain keeps its status (Desktop may still hold it); the fork is the
            // newest chain with this start, so later continuing steps join it.
            v.push(new_chain(*new_id, Some(from.hyphenated().to_string())));
        }
        SessionPlan::Broken { .. } | SessionPlan::NeedsStart { .. } => {}
    }
    v
}

/// Marks a chain `broken` (its transcript is gone).
#[must_use]
pub fn mark_broken(chains: &[Chain], chain: Uuid) -> Vec<Chain> {
    let id = chain.hyphenated().to_string();
    chains
        .iter()
        .cloned()
        .map(|mut c| {
            if c.id == id && c.status == ChainStatus::Open {
                c.status = ChainStatus::Broken;
            }
            c
        })
        .collect()
}

/// S-23's branch (AC-F024-03; pending Billy's run, `Shared` is the specified pass branch): does
/// Desktop continue the CLI session's own transcript?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DesktopShare {
    /// Desktop appends to the same transcript: the chain is `handed-off` and the next continuing
    /// step forks (AC-F010-15).
    #[default]
    Shared,
    /// Desktop copies the conversation: no fork; the CLI chain continues unaffected.
    Copied,
}

/// The chain list after `claude --desktop --resume <chain>` succeeded, by S-23's branch.
#[must_use]
pub fn after_handoff(chains: &[Chain], chain: Uuid, share: DesktopShare) -> Vec<Chain> {
    match share {
        DesktopShare::Shared => mark_handed_off(chains, chain),
        DesktopShare::Copied => chains.to_vec(),
    }
}

/// Billy confirmed Claude Desktop is closed (DR-110's exception): a handed-off chain is open
/// again, so the next continuing step resumes it instead of forking.
#[must_use]
pub fn desktop_closed(chains: &[Chain], chain: Uuid) -> Vec<Chain> {
    let id = chain.hyphenated().to_string();
    chains
        .iter()
        .cloned()
        .map(|mut c| {
            if c.id == id && c.status == ChainStatus::HandedOff {
                c.status = ChainStatus::Open;
            }
            c
        })
        .collect()
}

/// DR-118: `claude --desktop --resume <id>` marks the chain handed off.
#[must_use]
pub fn mark_handed_off(chains: &[Chain], chain: Uuid) -> Vec<Chain> {
    let id = chain.hyphenated().to_string();
    chains
        .iter()
        .cloned()
        .map(|mut c| {
            if c.id == id {
                c.status = ChainStatus::HandedOff;
            }
            c
        })
        .collect()
}
