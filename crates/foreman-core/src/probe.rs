//! Contract probe classification (F-019, ADR-017 §2 as amended 2026-10-20, SR-17). Pure: the
//! outputs come from `foreman-sys::claude::run_probes`.
//!
//! **Probe A** — `claude --init-only --settings <probe settings> --session-id <uuid> --model m
//! --effort e --permission-mode mode --name fm-probe` with a `SessionStart` marker hook. Pass =
//! exit 0, the marker exists, and stderr names no flag problem.
//!
//! **Probe B** (redesigned: an unknown `--resume` UUID exits 0 with no output, so exit codes alone
//! can't tell "parsed" from "ignored"). Two zero-token runs:
//! - B1 `--init-only --resume <r> --fork-session --session-id <n>` (Foreman's fork form) must
//!   exit 0 with no flag error;
//! - B2 `--init-only --resume <r> --session-id <n>` (SR-14's forbidden form) must be **refused**
//!   (exit ≠ 0 with a message naming `--session-id` and `--resume`/`--fork-session`). The refusal
//!   proves the parser read `--resume` and still enforces the combination Foreman relies on.
//!
//! Evidence (Claude Code 2.1.304, Linux, 2026-10-20): an unknown flag exits 1 with
//! `error: unknown option '--x'`; a bad `--permission-mode` exits 1 with `option
//! '--permission-mode <mode>' argument 'x' is invalid`; a bad `--effort` exits **0** with
//! `Warning: Unknown --effort value 'x' — ignoring it`; B2 exits 1 with `Error: --session-id can
//! only be used with --continue or --resume if --fork-session is also specified.`; B1 exits 0.

use serde::{Deserialize, Serialize};

/// A probe's result (files.md §4.1 `probeOutcome.result`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProbeResult {
    /// Everything parsed and (A) the hook fired.
    Pass,
    /// A named flag failed: only its dependent features are disabled.
    FlagFailure,
    /// Timeout, trust prompt, or anything else: keep the last verified state and warn.
    Inconclusive,
    /// Not run (the version didn't change).
    Skipped,
}

/// One outcome (files.md §4.1 `probeOutcome`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeOutcome {
    /// Result.
    pub result: ProbeResult,
    /// The flag that failed (`--effort`), for `FlagFailure`.
    pub flag: Option<String>,
}

/// What one probe run produced.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProbeRun {
    /// Exit code (`None`: killed or timed out).
    pub exit: Option<i32>,
    /// stderr text (lossy UTF-8, capped by the spawner).
    pub stderr: String,
    /// It hit the 20 s timeout.
    pub timed_out: bool,
}

/// The flags whose failure probe A can name, and the features that depend on each.
pub const FLAG_FEATURES: [(&str, &str); 7] = [
    ("--settings", "launch"),
    ("--session-id", "chains"),
    ("--model", "launch"),
    ("--effort", "effort"),
    ("--permission-mode", "launch"),
    ("--name", "launch"),
    ("--resume", "re-arm"),
];

/// The flag a stderr line blames, if it is a parse problem: `unknown option '--x'`,
/// `option '--x <v>' argument 'y' is invalid`, `Unknown --x value`, `unrecognized`.
#[must_use]
pub fn flag_error(stderr: &str) -> Option<String> {
    for line in stderr.lines() {
        let l = line.trim();
        let lower = l.to_ascii_lowercase();
        let parse_problem = lower.contains("unknown option")
            || lower.contains("unrecognized")
            || lower.contains("is invalid")
            || lower.contains("invalid value")
            || (lower.contains("unknown --") && lower.contains(" value"));
        if !parse_problem {
            continue;
        }
        let flag = l
            .split(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == '`')
            .find(|w| w.starts_with("--") && w.len() > 2)
            .map(|w| w.trim_end_matches([',', '.', ':']).to_owned());
        return Some(flag.unwrap_or_else(|| "unknown".into()));
    }
    None
}

/// Probe A: pass = exit 0 + marker + no flag problem on stderr.
#[must_use]
pub fn classify_a(run: &ProbeRun, marker: bool) -> ProbeOutcome {
    if run.timed_out {
        return ProbeOutcome {
            result: ProbeResult::Inconclusive,
            flag: None,
        };
    }
    if let Some(flag) = flag_error(&run.stderr) {
        return ProbeOutcome {
            result: ProbeResult::FlagFailure,
            flag: Some(flag),
        };
    }
    if run.exit == Some(0) && marker {
        ProbeOutcome {
            result: ProbeResult::Pass,
            flag: None,
        }
    } else {
        ProbeOutcome {
            result: ProbeResult::Inconclusive,
            flag: None,
        }
    }
}

fn mentions_combination(stderr: &str) -> bool {
    let l = stderr.to_ascii_lowercase();
    l.contains("--session-id") && (l.contains("--resume") || l.contains("--fork-session"))
}

/// Probe B from its two runs (see the module docs).
#[must_use]
pub fn classify_b(fork: &ProbeRun, control: &ProbeRun) -> ProbeOutcome {
    if fork.timed_out || control.timed_out {
        return ProbeOutcome {
            result: ProbeResult::Inconclusive,
            flag: None,
        };
    }
    if let Some(flag) = flag_error(&fork.stderr) {
        return ProbeOutcome {
            result: ProbeResult::FlagFailure,
            flag: Some(flag),
        };
    }
    if fork.exit != Some(0) {
        // The fork form itself was refused: the forked-session feature can't be used.
        return ProbeOutcome {
            result: ProbeResult::FlagFailure,
            flag: Some(if mentions_combination(&fork.stderr) {
                "--fork-session".into()
            } else {
                "--resume".into()
            }),
        };
    }
    let refused = control.exit.is_some_and(|c| c != 0) && mentions_combination(&control.stderr);
    if refused {
        ProbeOutcome {
            result: ProbeResult::Pass,
            flag: None,
        }
    } else {
        // The forbidden form was accepted (or refused for another reason): the parser no longer
        // proves it read `--resume`. Keep the last verified state.
        ProbeOutcome {
            result: ProbeResult::Inconclusive,
            flag: None,
        }
    }
}

/// The features a flag failure disables (only those, SR-17).
#[must_use]
pub fn disabled_features(outcomes: &[ProbeOutcome]) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = outcomes
        .iter()
        .filter(|o| o.result == ProbeResult::FlagFailure)
        .filter_map(|o| {
            let f = o.flag.as_deref()?;
            Some(
                FLAG_FEATURES
                    .iter()
                    .find(|(name, _)| *name == f)
                    .map_or("launch", |(_, feat)| *feat),
            )
        })
        .collect();
    if outcomes.iter().any(|o| {
        o.result == ProbeResult::FlagFailure && o.flag.as_deref() == Some("--fork-session")
    }) {
        v.push("desktop-handoff");
    }
    v.sort_unstable();
    v.dedup();
    v
}

/// SR-17: probes run when the observed version differs from the last verified one.
#[must_use]
pub fn due(found: &str, verified: Option<&str>) -> bool {
    verified != Some(found)
}

/// The stored contract after a run: an inconclusive outcome keeps the last verified outcome for
/// that probe (SR-17), and the version advances only when nothing was inconclusive.
#[must_use]
pub fn merge(
    last: Option<(&str, &ProbeOutcome, &ProbeOutcome)>,
    found: &str,
    a: ProbeOutcome,
    b: ProbeOutcome,
) -> (String, ProbeOutcome, ProbeOutcome) {
    let inconclusive =
        a.result == ProbeResult::Inconclusive || b.result == ProbeResult::Inconclusive;
    match last {
        Some((v, la, lb)) if inconclusive => (
            v.to_owned(),
            if a.result == ProbeResult::Inconclusive {
                la.clone()
            } else {
                a
            },
            if b.result == ProbeResult::Inconclusive {
                lb.clone()
            } else {
                b
            },
        ),
        _ => (found.to_owned(), a, b),
    }
}
