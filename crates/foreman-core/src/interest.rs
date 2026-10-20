//! The watcher's exact interest set (ADR-013, DR-162, SR-31). A path is interesting only if its
//! repo-relative form, after NFC and compared exactly (case-sensitive), is one of these; secrets
//! and Foreman's temp files never are. Pure: `foreman-sys::watch` applies it to debounced hints,
//! then re-reads the file through `store` (hints are never truth).

use icu_normalizer::ComposingNormalizer;
use serde::{Deserialize, Serialize};

use crate::pathpolicy;
use crate::store::is_temp_name;

/// What an interesting path is.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum Interest {
    /// `docs/handoff/stage-NN.md`
    Packet {
        /// Stage.
        stage: u8,
    },
    /// `docs/handoff/stage-NN-attachments.md`
    Attachments {
        /// Stage.
        stage: u8,
    },
    /// `docs/lessons.md`
    Lessons,
    /// `.foreman/{project,overlay,blueprint.lock}.json`
    Decision {
        /// `project`, `overlay` or `blueprint.lock`.
        file: String,
    },
    /// An applicable step's declared `out` path (exact string).
    Out {
        /// The path.
        path: String,
    },
    /// `events/<runId>/<seq>-<event>.json` in app data (P2).
    SensorEvent {
        /// The run.
        run_id: String,
        /// The file name.
        name: String,
    },
}

fn two_digits(s: &str) -> Option<u8> {
    (s.len() == 2 && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

/// Matches a repo-relative path against the interest set. `outs` are the applicable steps'
/// declared `out` paths.
#[must_use]
pub fn repo_interest(rel: &str, outs: &[String]) -> Option<Interest> {
    let nfc = ComposingNormalizer::new_nfc().normalize(rel);
    let rel = nfc.as_ref();
    let last = rel.rsplit('/').next().unwrap_or(rel);
    if is_temp_name(last) || pathpolicy::is_secret(rel) || rel.contains("//") {
        return None;
    }
    if let Some(stem) = rel
        .strip_prefix("docs/handoff/")
        .and_then(|n| n.strip_prefix("stage-"))
    {
        if let Some(nn) = stem.strip_suffix("-attachments.md") {
            return two_digits(nn).map(|stage| Interest::Attachments { stage });
        }
        if let Some(nn) = stem.strip_suffix(".md") {
            return two_digits(nn).map(|stage| Interest::Packet { stage });
        }
    }
    if rel == "docs/lessons.md" {
        return Some(Interest::Lessons);
    }
    for f in ["project", "overlay", "blueprint.lock"] {
        if rel == format!(".foreman/{f}.json") {
            return Some(Interest::Decision { file: f.into() });
        }
    }
    outs.iter()
        .find(|o| o.as_str() == rel)
        .map(|o| Interest::Out { path: o.clone() })
}

/// Matches an app-data-relative path: `events/<uuid>/<20 digits>-<digits>-<Event>.json`.
#[must_use]
pub fn app_data_interest(rel: &str) -> Option<Interest> {
    let mut parts = rel.split('/');
    let (Some("events"), Some(run), Some(name), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let uuid_ok = run.len() == 36
        && run
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-');
    (uuid_ok && crate::session_state::event_file_seq(name).is_some()).then(|| {
        Interest::SensorEvent {
            run_id: run.to_owned(),
            name: name.to_owned(),
        }
    })
}

/// The fixed part of the interest set as paths to re-stat on focus, open, launch and sign
/// (ADR-013): packets for `stages`, lessons, the decision files and the `outs`.
#[must_use]
pub fn restat_paths(stages: &[u8], outs: &[String]) -> Vec<String> {
    let mut v: Vec<String> = stages
        .iter()
        .flat_map(|s| {
            [
                format!("docs/handoff/stage-{s:02}.md"),
                format!("docs/handoff/stage-{s:02}-attachments.md"),
            ]
        })
        .collect();
    v.push("docs/lessons.md".into());
    for f in ["project", "overlay", "blueprint.lock"] {
        v.push(format!(".foreman/{f}.json"));
    }
    v.extend(outs.iter().cloned());
    v
}
