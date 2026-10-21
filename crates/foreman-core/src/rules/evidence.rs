//! Late-condition evidence (DR-063, F-023, AC-F023-06): the build-package assembler proposes `yes`
//! for a late condition that is still `undecided` when a **path** (never file contents) matches the
//! table below. With no evidence the condition stays `undecided`; nothing here ever proposes `no`,
//! and a proposal takes effect only when Billy confirms it (`condition_set`).

use std::collections::BTreeMap;

use crate::ids::{ConditionId, TriState};
use crate::schema::project::Conditions;

/// The four late conditions, in DR-027 order.
pub const LATE: [ConditionId; 4] = [
    ConditionId::HasDatabase,
    ConditionId::HasUploads,
    ConditionId::HasEmail,
    ConditionId::HasIntegrations,
];

/// Whether one repo-relative path is evidence for `c` (DR-063's table; lowercased).
#[must_use]
pub fn matches(c: ConditionId, path: &str) -> bool {
    let p = path.trim().trim_start_matches("./").to_lowercase();
    let comps: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    let last = comps.last().copied().unwrap_or("");
    // "Under" a folder means something inside it: the bare folder entry isn't evidence.
    let under = |prefix: &str| p.starts_with(prefix) && p.len() > prefix.len();
    match c {
        ConditionId::HasDatabase => {
            under("docs/migrations/")
                || (!p.ends_with('/')
                    && std::path::Path::new(last)
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("sql")))
                || last == "schema.prisma"
        }
        ConditionId::HasUploads => comps
            .iter()
            .any(|x| x.contains("upload") || x.contains("attachments-storage")),
        ConditionId::HasEmail => comps
            .iter()
            .any(|x| x.contains("email") || x.contains("mail")),
        ConditionId::HasIntegrations => {
            under("docs/integrations/")
                || comps
                    .iter()
                    .any(|x| x.contains("webhook") || x.contains("integration"))
        }
        _ => false,
    }
}

/// A proposal: the condition, the value proposed (always `yes`) and the first path that is its
/// evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// Always [`TriState::Yes`] (DR-063 never proposes `no`).
    pub value: TriState,
    /// The evidence path, as given.
    pub evidence: String,
}

/// DR-063 over `paths` (the Stage 4 packet's REPO FILES first, then the paths that exist in the
/// project): a proposal for each late condition that is `undecided` in `conditions` and has
/// evidence. A condition Billy already decided gets no proposal, and one without evidence is left
/// out (it stays `undecided`).
#[must_use]
pub fn late_condition_evidence<S: AsRef<str>>(
    paths: &[S],
    conditions: &Conditions,
) -> BTreeMap<ConditionId, Proposal> {
    let mut out = BTreeMap::new();
    for c in LATE {
        if conditions.get(&c).copied().unwrap_or(TriState::Undecided) != TriState::Undecided {
            continue;
        }
        if let Some(p) = paths.iter().map(AsRef::as_ref).find(|p| matches(c, p)) {
            out.insert(
                c,
                Proposal {
                    value: TriState::Yes,
                    evidence: p.to_owned(),
                },
            );
        }
    }
    out
}
