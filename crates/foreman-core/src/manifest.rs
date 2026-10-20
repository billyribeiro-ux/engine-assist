//! The knowledge manifest (F-023, AC-F023-01, -02): which files belong in the claude.ai Project
//! for the chat stages. It lists every **signed** packet (and its attachments file) and every
//! declared output (`out=`) of a chat step on the applicable path, each with the step that
//! produces it. Whether Billy has put a file in the Project is his mark, kept by the app.

use std::collections::BTreeSet;

use crate::ids::{StepKey, Surface};
use crate::rules::ApplicablePath;
use crate::template::Template;

/// What a manifest file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `docs/handoff/stage-NN.md` (or its `-attachments.md`) of a signed stage.
    Packet(u8),
    /// A chat step's declared output.
    Output,
}

/// One manifest file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Repo-relative path, as declared.
    pub path: String,
    /// The first step on the path that declares it.
    pub producer: StepKey,
    /// Packet or output.
    pub source: Source,
}

/// The stage of `docs/handoff/stage-NN.md` or `docs/handoff/stage-NN-attachments.md`.
#[must_use]
pub fn packet_stage(path: &str) -> Option<u8> {
    let name = path.strip_prefix("docs/handoff/stage-")?;
    let n = name
        .strip_suffix("-attachments.md")
        .or_else(|| name.strip_suffix(".md"))?;
    (n.len() == 2 && n.bytes().all(|b| b.is_ascii_digit()))
        .then(|| n.parse().ok())
        .flatten()
}

/// What a declared output would contribute, regardless of signing.
fn source_of(path: &str, chat: bool) -> Option<Source> {
    match packet_stage(path) {
        Some(n) => Some(Source::Packet(n)),
        None if chat => Some(Source::Output),
        None => None,
    }
}

/// Every manifest file in template order (deduplicated by path): signed packets and chat
/// outputs of steps on the applicable path.
#[must_use]
pub fn entries(t: &Template, path: &ApplicablePath, signed: &BTreeSet<u8>) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for s in &t.steps {
        if !path.step(s.key.as_str()).is_some_and(crate::rules::Applicability::on_path) {
            continue;
        }
        let chat = s.surface == Some(Surface::Chat);
        for o in &s.out {
            let Some(source) = source_of(o, chat) else {
                continue;
            };
            if let Source::Packet(n) = source
                && !signed.contains(&n)
            {
                continue;
            }
            if out.iter().any(|e| e.path == *o) {
                continue;
            }
            out.push(Entry {
                path: o.clone(),
                producer: s.key.clone(),
                source,
            });
        }
    }
    out
}

/// The first step on the path that produces a manifest file (AC-F023-02's empty state).
#[must_use]
pub fn first_producer(t: &Template, path: &ApplicablePath) -> Option<StepKey> {
    t.steps
        .iter()
        .filter(|s| path.step(s.key.as_str()).is_some_and(crate::rules::Applicability::on_path))
        .find(|s| {
            let chat = s.surface == Some(Surface::Chat);
            s.out.iter().any(|o| source_of(o, chat).is_some())
        })
        .map(|s| s.key.clone())
}
