//! The `claude` CLI wrappers (ADR-017, ADR-021): `claude -v`, `claude auth status` (SR-08, fail
//! closed), and the zero-token contract probes (SR-17, P2). Every spawn goes through
//! [`crate::spawn::run`].

use std::path::Path;
use std::time::Duration;

use foreman_core::launch::AuthProblem;
use foreman_core::probe::{self, ProbeOutcome, ProbeRun};
use semver::Version;
use serde_json::Value;

use crate::spawn::{self, Cwd, Output, RunOpts, SpawnCtx, SpawnError, SpawnRequest};

/// `-v` and `auth status` timeout (ADR-007).
pub const HELPER_TIMEOUT: Duration = Duration::from_secs(10);
/// Probe timeout (SR-17).
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(20);
/// Output cap for these helpers.
pub const OUTPUT_CAP: usize = 64 * 1024;

/// The only methods that may launch (SR-08; CLAUDE.md "subscription login only").
pub const ALLOWED_METHODS: [&str; 2] = ["claude.ai", "oauth_token"];

/// What `claude -v` said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    /// The first line (≤ 200 characters).
    pub raw: String,
    /// The parsed version.
    pub version: Option<Version>,
}

fn opts(env: &[(String, String)], timeout: Duration, cwd: Cwd) -> RunOpts {
    RunOpts {
        env: env.to_vec(),
        timeout,
        output_cap: OUTPUT_CAP,
        cwd,
    }
}

/// `claude -v` in app data (every launch, ADR-017; no cache).
///
/// # Errors
/// [`SpawnError`] when it can't run.
pub fn version(ctx: &SpawnCtx, env: &[(String, String)]) -> Result<VersionInfo, SpawnError> {
    let out = spawn::run(
        &SpawnRequest::ClaudeVersion,
        &opts(env, HELPER_TIMEOUT, Cwd::AppData),
        ctx,
    )?;
    let text = String::from_utf8_lossy(&out.stdout);
    let raw: String = text
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect();
    Ok(VersionInfo {
        version: (out.code == Some(0))
            .then(|| foreman_core::compat::parse_version_output(&raw))
            .flatten(),
        raw,
    })
}

/// SR-08: reads only `authMethod`, the exit code and `forcedLoginMethod` (a forced login method
/// other than `claudeai` blocks too: S-7′ rehearsal, SR-08 as amended 2026-10-20).
///
/// # Errors
/// The [`AuthProblem`].
pub fn parse_auth(out: &Output) -> Result<String, AuthProblem> {
    if out.code != Some(0) {
        return Err(AuthProblem::Exit { code: out.code });
    }
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|_| AuthProblem::Unparsable)?;
    let Some(obj) = v.as_object() else {
        return Err(AuthProblem::Unparsable);
    };
    let method = obj
        .get("authMethod")
        .and_then(Value::as_str)
        .ok_or(AuthProblem::UnknownMethod)?;
    if let Some(forced) = obj.get("forcedLoginMethod").filter(|f| !f.is_null())
        && forced.as_str() != Some("claudeai")
    {
        return Err(AuthProblem::WrongMethod {
            method: format!(
                "forcedLoginMethod {}",
                forced.as_str().unwrap_or("(not a string)")
            ),
        });
    }
    if ALLOWED_METHODS.contains(&method) {
        Ok(method.to_owned())
    } else if method.is_empty() || method.len() > 64 {
        Err(AuthProblem::UnknownMethod)
    } else {
        Err(AuthProblem::WrongMethod {
            method: method.to_owned(),
        })
    }
}

/// The auth check's result for the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthCheck {
    /// The method read (when the output parsed).
    pub method: Option<String>,
    /// The exit code.
    pub exit: Option<i32>,
    /// Pass or the problem.
    pub result: Result<String, AuthProblem>,
}

/// `claude auth status` in the step cwd with the exact launch environment (SR-08). A timeout or
/// spawn failure is a typed problem, never a pass.
#[must_use]
pub fn auth_status(ctx: &SpawnCtx, cwd: &Path, env: &[(String, String)]) -> AuthCheck {
    let r = spawn::run(
        &SpawnRequest::ClaudeAuthStatusInLaunch,
        &opts(env, HELPER_TIMEOUT, Cwd::Dir(cwd.to_path_buf())),
        ctx,
    );
    match r {
        Ok(out) => {
            let method = serde_json::from_slice::<Value>(&out.stdout)
                .ok()
                .and_then(|v| {
                    v.get("authMethod")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .filter(|m| m.len() <= 64);
            AuthCheck {
                method,
                exit: out.code,
                result: parse_auth(&out),
            }
        }
        Err(SpawnError::Timeout(_)) => AuthCheck {
            method: None,
            exit: None,
            result: Err(AuthProblem::Timeout),
        },
        Err(_) => AuthCheck {
            method: None,
            exit: None,
            result: Err(AuthProblem::Exit { code: None }),
        },
    }
}

/// How long `claude --desktop` may take to hand over to Claude Desktop and exit.
pub const DESKTOP_TIMEOUT: Duration = Duration::from_secs(30);

/// `claude --desktop [--resume <id>]` (P2, F-024): in the step cwd with the exact launch
/// environment (the same allowlisted program and environment as a launch, AC-F024-01). `args`
/// come from `foreman_core::launch::argv::desktop_open` or `desktop`.
///
/// # Errors
/// [`SpawnError`] when it can't run (a non-zero exit is the caller's to report).
pub fn desktop(
    ctx: &SpawnCtx,
    cwd: &Path,
    env: &[(String, String)],
    args: Vec<String>,
) -> Result<Output, SpawnError> {
    spawn::run(
        &SpawnRequest::ClaudeDesktop(args),
        &opts(env, DESKTOP_TIMEOUT, Cwd::Dir(cwd.to_path_buf())),
        ctx,
    )
}

/// The two contract probes' outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeReport {
    /// Probe A.
    pub a: ProbeOutcome,
    /// Probe B.
    pub b: ProbeOutcome,
}

fn probe_run(ctx: &SpawnCtx, args: Vec<String>, env: &[(String, String)]) -> ProbeRun {
    match spawn::run(
        &SpawnRequest::ClaudeProbe(args),
        &opts(env, PROBE_TIMEOUT, Cwd::AppData),
        ctx,
    ) {
        Ok(o) => ProbeRun {
            exit: o.code,
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
            timed_out: false,
        },
        Err(SpawnError::Timeout(_)) => ProbeRun {
            exit: None,
            stderr: String::new(),
            timed_out: true,
        },
        Err(e) => ProbeRun {
            exit: None,
            stderr: e.to_string(),
            timed_out: false,
        },
    }
}

/// Runs probe A then B (ADR-017 §2 amended). `probe_a` and `probe_b` are the argv from
/// `foreman_core::launch::argv`; `marker` reports whether A's `SessionStart` hook wrote its marker.
/// The probe folder must already be reset (`crate::launch::reset_probe_dir`).
#[must_use]
pub fn run_probes(
    ctx: &SpawnCtx,
    env: &[(String, String)],
    probe_a: Vec<String>,
    probe_b: (Vec<String>, Vec<String>),
    marker: &dyn Fn() -> bool,
) -> ProbeReport {
    let a_run = probe_run(ctx, probe_a, env);
    let a = probe::classify_a(&a_run, marker());
    let fork = probe_run(ctx, probe_b.0, env);
    let control = probe_run(ctx, probe_b.1, env);
    ProbeReport {
        a,
        b: probe::classify_b(&fork, &control),
    }
}
