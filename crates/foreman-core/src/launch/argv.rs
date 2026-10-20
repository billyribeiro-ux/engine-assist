//! The argv builder (SR-03, SR-04, SR-14, SR-16, SR-28; ADR-007 §3; DR-111, DR-121, DR-122).
//!
//! Every argument comes from [`Flag`], a closed enum whose values are typed (UUIDs, model, effort
//! and mode enums, a validated session name, an absolute settings path). The prompt is the final
//! positional argument and passes [`crate::sanitize::check_prompt`] first. There is no way to
//! express `--bare`, `-p`/`--print`, `--dangerously-skip-permissions`,
//! `--allow-dangerously-skip-permissions` or `bypassPermissions` (the [`Mode`] enum has no such
//! value).

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ids::{Effort, Mode, Model, Start};
use crate::sanitize;

/// Every flag Foreman may pass, by its exact spelling (ADR-007 §3). `auth status` is a subcommand,
/// listed for completeness.
pub const KNOWN_FLAGS: [&str; 13] = [
    "--session-id",
    "--resume",
    "--fork-session",
    "--model",
    "--effort",
    "--permission-mode",
    "--name",
    "--settings",
    "--ax-screen-reader",
    "--init-only",
    "--desktop",
    "-v",
    "auth",
];

/// Flags and values Foreman never passes (SR-03, SR-16).
pub const FORBIDDEN: [&str; 6] = [
    "--bare",
    "-p",
    "--print",
    "--dangerously-skip-permissions",
    "--allow-dangerously-skip-permissions",
    "bypassPermissions",
];

/// A validated session name: `^fm-[a-z0-9-]{1,80}$` (SR-03, DR-121).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SessionName(String);

impl SessionName {
    /// Parses a name.
    ///
    /// # Errors
    /// When it doesn't match `^fm-[a-z0-9-]{1,80}$`.
    pub fn parse(s: &str) -> Result<Self, ArgvError> {
        let rest = s.strip_prefix("fm-").unwrap_or_default();
        if s.starts_with("fm-")
            && (1..=80).contains(&rest.len())
            && rest
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            Ok(Self(s.to_owned()))
        } else {
            Err(ArgvError::InvalidValue {
                flag: "--name",
                value: s.chars().take(100).collect(),
            })
        }
    }

    /// DR-121: `fm-<slug>-s<NN>-<step>`, the step key lowercased with `.` and `:` as `-`; the
    /// slug is reduced to `[a-z0-9-]` and truncated so the whole name fits 83 characters.
    #[must_use]
    pub fn for_step(project_slug: &str, stage: u8, step_key: &str) -> Self {
        let clean = |s: &str| -> String {
            let mut out = String::new();
            for c in s.chars().flat_map(char::to_lowercase) {
                let c = if c.is_ascii_lowercase() || c.is_ascii_digit() {
                    c
                } else {
                    '-'
                };
                if !(c == '-' && out.ends_with('-')) {
                    out.push(c);
                }
            }
            out.trim_matches('-').to_owned()
        };
        let step = clean(step_key);
        let tail = format!("-s{stage:02}-{step}");
        let room = 80usize.saturating_sub(tail.len());
        let mut slug = clean(project_slug);
        slug.truncate(room);
        let slug = slug.trim_end_matches('-');
        let slug = if slug.is_empty() { "project" } else { slug };
        let mut body = format!("{slug}{tail}");
        body.truncate(80);
        Self(format!("fm-{}", body.trim_end_matches('-')))
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SessionName {
    type Error = ArgvError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<SessionName> for String {
    fn from(n: SessionName) -> Self {
        n.0
    }
}

impl fmt::Display for SessionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One flag with its typed value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flag {
    /// `--session-id <uuid>`
    SessionId(Uuid),
    /// `--resume <uuid>`
    Resume(Uuid),
    /// `--fork-session`
    ForkSession,
    /// `--model <alias>`
    Model(Model),
    /// `--effort <level>` (never `adaptive`, which is chat only)
    Effort(Effort),
    /// `--permission-mode <mode>`
    PermissionMode(Mode),
    /// `--name <fm-…>`
    Name(SessionName),
    /// `--settings <absolute path>`
    Settings(PathBuf),
    /// `--ax-screen-reader`
    AxScreenReader,
    /// `--init-only`
    InitOnly,
    /// `--desktop`
    Desktop,
}

/// Why argv couldn't be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArgvError {
    /// Prompt hygiene (SR-04, SR-28).
    #[error(transparent)]
    Prompt(#[from] sanitize::Blocker),
    /// A value outside its validated set.
    #[error("{flag}: invalid value {value:?}")]
    InvalidValue {
        /// The flag.
        flag: &'static str,
        /// The value (truncated).
        value: String,
    },
    /// `--session-id` together with `--resume` outside the fork form (SR-14).
    #[error("--session-id with --resume is only allowed together with --fork-session")]
    SessionIdWithResume,
}

impl Flag {
    fn push(&self, out: &mut Vec<String>) -> Result<(), ArgvError> {
        match self {
            Self::SessionId(u) => push2(out, "--session-id", uuid_v4(*u, "--session-id")?),
            Self::Resume(u) => push2(out, "--resume", uuid_v4(*u, "--resume")?),
            Self::ForkSession => out.push("--fork-session".into()),
            Self::Model(m) => push2(out, "--model", m.as_str().to_owned()),
            Self::Effort(e) => {
                if *e == Effort::Adaptive {
                    return Err(ArgvError::InvalidValue {
                        flag: "--effort",
                        value: "adaptive".into(),
                    });
                }
                push2(out, "--effort", e.as_str().to_owned());
            }
            Self::PermissionMode(m) => push2(out, "--permission-mode", m.as_str().to_owned()),
            Self::Name(n) => push2(out, "--name", n.as_str().to_owned()),
            Self::Settings(p) => {
                let s = p.to_str().unwrap_or_default();
                if !p.is_absolute() || s.is_empty() || s.chars().any(char::is_control) {
                    return Err(ArgvError::InvalidValue {
                        flag: "--settings",
                        value: p.display().to_string(),
                    });
                }
                push2(out, "--settings", s.to_owned());
            }
            Self::AxScreenReader => out.push("--ax-screen-reader".into()),
            Self::InitOnly => out.push("--init-only".into()),
            Self::Desktop => out.push("--desktop".into()),
        }
        Ok(())
    }
}

fn push2(out: &mut Vec<String>, flag: &str, value: String) {
    out.push(flag.to_owned());
    out.push(value);
}

fn uuid_v4(u: Uuid, flag: &'static str) -> Result<String, ArgvError> {
    let s = u.hyphenated().to_string();
    if crate::ids::is_uuid_v4(&s) {
        Ok(s)
    } else {
        Err(ArgvError::InvalidValue { flag, value: s })
    }
}

/// How the session is addressed (DR-111).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionArgs {
    /// Chain start: `--session-id <id>`, never `--resume`.
    New {
        /// The new session's UUID (Foreman-owned).
        id: Uuid,
    },
    /// Continue: `--resume <id>`, never `--session-id`.
    Resume {
        /// The chain's UUID.
        id: Uuid,
    },
    /// Fork after a Desktop handoff: `--resume <from> --fork-session`, plus `--session-id <new>`
    /// only on S-8's pass branch (the single allowed combination, SR-14).
    Fork {
        /// The handed-off chain.
        from: Uuid,
        /// Foreman's ID for the fork (S-8 pass) or `None` (S-8 fail: discovered after launch).
        new_id: Option<Uuid>,
    },
}

/// The inputs of an interactive CLI launch (DR-122).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchInputs {
    /// Session addressing.
    pub session: SessionArgs,
    /// The effective start (mode required for CLI).
    pub start: Start,
    /// Whether `--effort` is passed (always for new sessions; on resume per S-3, DR-117).
    pub pass_effort: bool,
    /// `--name`.
    pub name: SessionName,
    /// The run settings file (absolute, in app data).
    pub settings: PathBuf,
    /// `--ax-screen-reader` (Settings → Screen reader mode).
    pub screen_reader: bool,
    /// The resolved prompt (`None` for a bare resume, `chain_resume_prepare`).
    pub prompt: Option<String>,
}

/// The arguments after the program, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Argv(pub Vec<String>);

impl Argv {
    /// The arguments.
    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    /// The argv for the run log: the prompt (the last element when present) replaced by its
    /// SHA-256 (SR-52).
    #[must_use]
    pub fn redacted(&self, has_prompt: bool) -> Vec<String> {
        let mut v = self.0.clone();
        if has_prompt && let Some(last) = v.last_mut() {
            *last = format!("<prompt sha256:{}>", sha256_hex(last.as_bytes()));
        }
        v
    }
}

fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    Sha256::digest(b)
        .iter()
        .fold(String::with_capacity(64), |mut s, x| {
            use std::fmt::Write as _;
            let _ = write!(s, "{x:02x}");
            s
        })
}

fn flags_to_argv(flags: &[Flag]) -> Result<Vec<String>, ArgvError> {
    let has_sid = flags.iter().any(|f| matches!(f, Flag::SessionId(_)));
    let has_resume = flags.iter().any(|f| matches!(f, Flag::Resume(_)));
    let has_fork = flags.iter().any(|f| matches!(f, Flag::ForkSession));
    if has_sid && has_resume && !has_fork {
        return Err(ArgvError::SessionIdWithResume);
    }
    let mut out = Vec::new();
    for f in flags {
        f.push(&mut out)?;
    }
    Ok(out)
}

/// The session flags for a plan (DR-111).
#[must_use]
pub fn session_flags(s: SessionArgs) -> Vec<Flag> {
    match s {
        SessionArgs::New { id } => vec![Flag::SessionId(id)],
        SessionArgs::Resume { id } => vec![Flag::Resume(id)],
        SessionArgs::Fork { from, new_id } => {
            let mut v = vec![Flag::Resume(from), Flag::ForkSession];
            if let Some(n) = new_id {
                v.push(Flag::SessionId(n));
            }
            v
        }
    }
}

/// Builds the interactive argv (DR-122):
/// `<session flags> --model m [--effort e] --permission-mode mode --name n [--ax-screen-reader]
/// --settings <file> [<prompt>]`.
///
/// # Errors
/// [`ArgvError`]: prompt hygiene, a CLI start without a mode, an invalid value.
pub fn build_argv(p: &LaunchInputs) -> Result<Argv, ArgvError> {
    if let Some(prompt) = &p.prompt {
        sanitize::check_prompt(prompt)?;
    }
    let mode = p.start.mode.ok_or(ArgvError::InvalidValue {
        flag: "--permission-mode",
        value: "none".into(),
    })?;
    let mut flags = session_flags(p.session);
    flags.push(Flag::Model(p.start.model));
    let pass_effort = p.pass_effort || matches!(p.session, SessionArgs::New { .. });
    if pass_effort {
        flags.push(Flag::Effort(p.start.effort));
    }
    flags.push(Flag::PermissionMode(mode));
    flags.push(Flag::Name(p.name.clone()));
    if p.screen_reader {
        flags.push(Flag::AxScreenReader);
    }
    flags.push(Flag::Settings(p.settings.clone()));
    let mut out = flags_to_argv(&flags)?;
    if let Some(prompt) = &p.prompt {
        out.push(prompt.clone());
    }
    Ok(Argv(out))
}

/// Contract probe A (ADR-017 §2, P2): zero tokens; every interactive flag Foreman passes.
///
/// # Errors
/// As [`build_argv`].
pub fn probe_a(settings: &Path, session: Uuid, start: Start) -> Result<Argv, ArgvError> {
    let mode = start.mode.unwrap_or(Mode::Plan);
    flags_to_argv(&[
        Flag::InitOnly,
        Flag::Settings(settings.to_path_buf()),
        Flag::SessionId(session),
        Flag::Model(start.model),
        Flag::Effort(start.effort),
        Flag::PermissionMode(mode),
        Flag::Name(SessionName("fm-probe".into())),
    ])
    .map(Argv)
}

/// Contract probe B (ADR-017 §2, amended 2026-10-20): the resume and fork forms Foreman uses.
/// `fork` is the accepted form `--resume <r> --fork-session --session-id <n>`; `control` is the
/// refused form `--resume <r> --session-id <n>`, whose refusal proves the parser read `--resume`.
///
/// # Errors
/// [`ArgvError::InvalidValue`] for a non-v4 UUID.
pub fn probe_b(resume: Uuid, new_id: Uuid) -> Result<(Argv, Vec<String>), ArgvError> {
    let fork = flags_to_argv(&[
        Flag::InitOnly,
        Flag::Resume(resume),
        Flag::ForkSession,
        Flag::SessionId(new_id),
    ])?;
    // The control is deliberately the SR-14-forbidden combination; it never reaches a session
    // (`--init-only`, and Claude Code refuses it), so it bypasses `flags_to_argv`'s guard.
    let control = vec![
        "--init-only".to_owned(),
        "--resume".to_owned(),
        uuid_v4(resume, "--resume")?,
        "--session-id".to_owned(),
        uuid_v4(new_id, "--session-id")?,
    ];
    Ok((Argv(fork), control))
}

/// Desktop handoff (P2, F-024): `--desktop --resume <id>`.
///
/// # Errors
/// [`ArgvError::InvalidValue`] for a non-v4 UUID.
pub fn desktop(id: Uuid) -> Result<Argv, ArgvError> {
    flags_to_argv(&[Flag::Desktop, Flag::Resume(id)]).map(Argv)
}

/// `true` when `arg` is one of the [`FORBIDDEN`] spellings (also as `--flag=value`).
#[must_use]
pub fn is_forbidden(arg: &str) -> bool {
    let head = arg.split('=').next().unwrap_or(arg);
    FORBIDDEN.contains(&head)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(
            SessionName::for_step("TradeLog", 2, "2.3").as_str(),
            "fm-tradelog-s02-2-3"
        );
        assert_eq!(
            SessionName::for_step("foreman", 5, "5:gate.A").as_str(),
            "fm-foreman-s05-5-gate-a"
        );
        assert_eq!(
            SessionName::for_step("foreman", 2, "2:F.2").as_str(),
            "fm-foreman-s02-2-f-2"
        );
        let long = SessionName::for_step(&"x".repeat(200), 17, "x-17-123");
        assert!(SessionName::parse(long.as_str()).is_ok(), "{long}");
        assert!(SessionName::parse("fm-").is_err());
        assert!(SessionName::parse("fm-A").is_err());
        assert!(is_forbidden("--print=x") && !is_forbidden("--resume"));
    }
}
