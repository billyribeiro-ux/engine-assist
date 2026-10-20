//! The per-run settings file (SR-09, ADR-021 §6, files.md §4.4): built here as canonical bytes;
//! `foreman-sys::launch` writes it fresh (`0600`, app data) and records its SHA-256.

use std::collections::BTreeMap;
use std::path::Path;

use crate::ids::Effort;
use crate::launch::Blocker;
use crate::schema::run_settings::{HookCommand, HookGroup, Permissions, RunSettingsFile};

/// The hook events the sensor registers (files.md §4.4 / §4.8).
pub const SENSOR_EVENTS: [&str; 11] = [
    "SessionStart",
    "Stop",
    "StopFailure",
    "Notification",
    "PermissionRequest",
    "SessionEnd",
    "SubagentStart",
    "SubagentStop",
    "TaskCreated",
    "TaskCompleted",
    "PostToolUse",
];

/// Whether the `.foreman` deny rules must also name case variants (S-1′ side result).
///
/// Pending Billy's Mac run of S-1′ (`docs/spikes/S-1prime.md`): APFS is case-insensitive, so
/// `.FOREMAN/x` is the same file as `.foreman/x`. Until the spike says the rule already denies the
/// variant, this stays [`CaseVariants::NotListed`] (the frozen four-rule contract); on the fail
/// branch it becomes [`CaseVariants::Listed`] and files.md §4.4's `maxItems` grows with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaseVariants {
    /// The four rules of SR-09 only.
    #[default]
    NotListed,
    /// Also `Read`/`Edit` for `.FOREMAN/**`.
    Listed,
}

/// The P2 sensor hooks (ADR-009): the hash-verified binary, the run and its token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensorHooks {
    /// `<app-data>/bin/foreman-sensor-<sha256-12>`.
    pub command: String,
    /// The run ID.
    pub run_id: String,
    /// 64 lowercase hex digits.
    pub token: String,
}

/// What goes into the file besides the fixed parts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunSettingsOpts {
    /// `effortLevel` (S-3 fail branch only, `low`…`xhigh`, DR-117).
    pub effort_level: Option<Effort>,
    /// P2 sensor hooks.
    pub sensor: Option<SensorHooks>,
    /// S-1′ case-variant branch.
    pub case_variants: CaseVariants,
}

/// Characters that make a permission-rule path ambiguous: gitignore-style glob metacharacters,
/// the rule's own parentheses, and the escape character. Escaping inside `Read(…)` rules is not
/// documented (code.claude.com permissions page, 2026-10-20), so a path containing one blocks
/// rather than being escaped (ADR-021 §6, `Blocker::RunSettingsPathUnsafe`).
pub const UNSAFE_IN_RULE: [char; 9] = ['*', '?', '[', ']', '{', '}', '(', ')', '\\'];

/// `//` + the absolute path without its leading `/` (files.md §4.4: "absolute from the
/// filesystem root"), checked for [`UNSAFE_IN_RULE`] and control characters.
///
/// # Errors
/// [`Blocker::RunSettingsPathUnsafe`].
pub fn rule_path(abs: &Path) -> Result<String, Blocker> {
    let s = abs.to_str().unwrap_or_default();
    let unsafe_path = || Blocker::RunSettingsPathUnsafe {
        path: abs.display().to_string(),
    };
    if !abs.is_absolute()
        || s.is_empty()
        || s.chars()
            .any(|c| UNSAFE_IN_RULE.contains(&c) || c.is_control())
    {
        return Err(unsafe_path());
    }
    let trimmed = s.trim_end_matches('/');
    Ok(format!("/{trimmed}"))
}

/// The settings file for one run (SR-09). `repo` and `app_data` are canonical absolute paths.
///
/// # Errors
/// [`Blocker::RunSettingsPathUnsafe`] when a path can't be written into a rule.
pub fn build(
    repo: &Path,
    app_data: &Path,
    opts: &RunSettingsOpts,
) -> Result<RunSettingsFile, Blocker> {
    let r = rule_path(repo)?;
    let a = rule_path(app_data)?;
    let mut deny = vec![
        format!("Read({r}/.foreman/**)"),
        format!("Edit({r}/.foreman/**)"),
        format!("Read({a}/**)"),
        format!("Edit({a}/**)"),
    ];
    if opts.case_variants == CaseVariants::Listed {
        deny.push(format!("Read({r}/.FOREMAN/**)"));
        deny.push(format!("Edit({r}/.FOREMAN/**)"));
    }
    let effort_level = opts
        .effort_level
        .filter(|e| {
            matches!(
                e,
                Effort::Low | Effort::Medium | Effort::High | Effort::Xhigh
            )
        })
        .map(|e| e.as_str().to_owned());
    let hooks = opts.sensor.as_ref().map(|s| {
        SENSOR_EVENTS
            .iter()
            .map(|ev| {
                (
                    (*ev).to_owned(),
                    vec![HookGroup {
                        matcher: (*ev == "PostToolUse").then(|| "Bash".to_owned()),
                        hooks: vec![HookCommand {
                            r#type: "command".into(),
                            command: s.command.clone(),
                            args: vec![
                                "--run".into(),
                                s.run_id.clone(),
                                "--token".into(),
                                s.token.clone(),
                                "--event".into(),
                                (*ev).to_owned(),
                                "--v".into(),
                                "1".into(),
                            ],
                        }],
                    }],
                )
            })
            .collect::<BTreeMap<_, _>>()
    });
    Ok(RunSettingsFile {
        disable_all_hooks: false,
        permissions: Permissions {
            disable_bypass_permissions_mode: "disable".into(),
            deny,
        },
        effort_level,
        hooks,
    })
}

/// Canonical bytes (FILE-R1) and their SHA-256.
#[must_use]
pub fn to_bytes(f: &RunSettingsFile) -> (Vec<u8>, String) {
    let text = crate::json::to_canonical(f).unwrap_or_default();
    let hash = crate::store::sha256_hex(text.as_bytes());
    (text.into_bytes(), hash)
}
