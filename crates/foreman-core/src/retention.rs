//! Retention and removal rules (ADR-018 §5 and §7, SR-53, PR-04, PR-05, M4-02): which output tails
//! are old enough to prune, which templates no project uses any more, and which app-data paths
//! belong to a project or to the run history. The rules are pure; the walks and deletes go through
//! [`crate::store::app_dirs`] (no symlink is followed).

use std::collections::{BTreeMap, BTreeSet};

use crate::decisions;
use crate::pathpolicy::{self, DecisionPath, ReadClass};
use crate::schema::lock::LockFile;
use crate::schema::progress::DecisionFileKind;
use crate::store::{Root, StoreError, app_dirs};

/// Output tails are kept this many days (SR-53).
pub const TAIL_DAYS: i64 = 90;

/// A template (`templates/<sourceSha256>.md` with its `.p<parserVersion>.json` caches) goes this many
/// days after no project pinned it (files.md rows 20 and 21, ADR-018 §5).
pub const TEMPLATE_DAYS: i64 = 90;

/// Sensor events go this many days after their run ended (files.md row 18, SR-53). The sweep that
/// applies it is `app::sensor::prune_events`; a stage's events also go at its signature.
pub const EVENT_DAYS: i64 = 30;

/// The run folders and event folders that Clear run history deletes (PR-05).
pub const HISTORY_TREES: [&str; 2] = ["runs", "events"];

/// Whether an output tail is past retention: `run_started` (RFC 3339, the run's `wallStart`) is
/// more than [`TAIL_DAYS`] before `now`. A start time that can't be read is never expired, so a
/// damaged record keeps its tail rather than losing it.
#[must_use]
pub fn tail_expired(run_started: &str, now: jiff::Timestamp) -> bool {
    run_started
        .parse::<jiff::Timestamp>()
        .is_ok_and(|t| now.as_second().saturating_sub(t.as_second()) > TAIL_DAYS * 86_400)
}

/// `runs/<runId>/output.log`.
#[must_use]
pub fn tail_path(run_id: &str) -> String {
    format!("runs/{run_id}/output.log")
}

/// What removing a project deletes from app data (never anything in the repo, PR-05):
/// `projects/<id>/`, `approvals/<id>/` and `approvals/<id>.json`. The redacted config snapshots
/// live in `cache/fingerprints/<cwdHash>.json`; the app adds those from the project's folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectData {
    /// Folders.
    pub trees: Vec<String>,
    /// Files.
    pub files: Vec<String>,
}

/// The app-data paths of one project (`id` must be a UUID).
#[must_use]
pub fn project_data(project_id: &str) -> ProjectData {
    ProjectData {
        trees: vec![
            format!("projects/{project_id}"),
            format!("approvals/{project_id}"),
        ],
        files: vec![format!("approvals/{project_id}.json")],
    }
}

/// The `<sourceSha256>` a template file name starts with: 64 lowercase hex digits followed by `.md`
/// or `.p<digits>.json`. Anything else in `templates/` is not Foreman's and is left alone.
#[must_use]
pub fn template_sha(name: &str) -> Option<&str> {
    let (sha, rest) = name.split_at_checked(64)?;
    if !sha
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let cache = rest
        .strip_prefix(".p")
        .and_then(|r| r.strip_suffix(".json"))
        .is_some_and(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()));
    (rest == ".md" || cache).then_some(sha)
}

/// What one pass over `templates/` did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplatePass {
    /// Files of templates still pinned whose "last used" time was refreshed.
    pub refreshed: usize,
    /// Files deleted (a template's text and its caches count one each).
    pub removed: usize,
}

/// Seconds between refreshes of a pinned template's "last used" time: once a day is plenty for a
/// 90-day rule, and it keeps the startup scan from touching every file every start.
const REFRESH_SECS: u64 = 86_400;

/// One pass over `templates/` (ADR-018 §5): a template pinned by some project (`in_use`) has its
/// files' modification times refreshed to `now`, so the time on disk means "last used"; a template
/// nobody pins whose newest file is more than [`TEMPLATE_DAYS`] old is deleted with its caches.
/// The text can always come back from the blueprint repo (DR-156). A name that isn't a template
/// file, and a link or folder with a template's name, stays.
///
/// # Errors
/// [`StoreError`] when the folder can't be listed or a delete fails.
pub fn prune_templates(
    app: &Root,
    in_use: &BTreeSet<String>,
    now: jiff::Timestamp,
) -> Result<TemplatePass, StoreError> {
    let now_s = u64::try_from(now.as_second()).unwrap_or(0);
    let mut groups: BTreeMap<String, Vec<(String, u64)>> = BTreeMap::new();
    for name in app_dirs::list_app_data_dir(app, "templates")? {
        let Some(sha) = template_sha(&name).map(str::to_owned) else {
            continue;
        };
        // A link or folder with a template's name isn't Foreman's file: left where it is.
        match app_dirs::modified_secs(app, &format!("templates/{name}")) {
            Ok(Some(m)) => groups.entry(sha).or_default().push((name, m)),
            Ok(None) | Err(StoreError::NotRegular(_)) => {}
            Err(e) => return Err(e),
        }
    }
    let mut pass = TemplatePass::default();
    for (sha, files) in groups {
        let newest = files.iter().map(|(_, m)| *m).max().unwrap_or(0);
        if in_use.contains(&sha) {
            for (name, m) in &files {
                if now_s.saturating_sub(*m) >= REFRESH_SECS
                    && app_dirs::touch(app, &format!("templates/{name}"), now_s)?
                {
                    pass.refreshed += 1;
                }
            }
        } else if now_s.saturating_sub(newest) > u64::try_from(TEMPLATE_DAYS * 86_400).unwrap_or(0)
        {
            for (name, _) in &files {
                if app_dirs::remove_file(app, &format!("templates/{name}"))? {
                    pass.removed += 1;
                }
            }
        }
    }
    Ok(pass)
}

/// The template hashes a project pins: its approved lock copy in app data and the lock in the
/// repo (they differ while an outside edit of the lock waits for review). A project whose folder
/// is unavailable passes `None` and counts by its approved copy alone.
///
/// # Errors
/// [`StoreError`] when a lock exists but can't be read or parsed: the caller then prunes nothing,
/// because it can't tell what is in use.
pub fn pinned_templates(
    app: &Root,
    project: Option<&Root>,
    project_id: &str,
) -> Result<Vec<String>, StoreError> {
    let mut out = Vec::new();
    let copy = pathpolicy::check_app_data(&decisions::approved_path(
        project_id,
        DecisionFileKind::BlueprintLock,
    ))?;
    if let (Some(b), _) = app.read_opt(&copy, ReadClass::Decision)? {
        out.push(lock_sha(&copy.to_string(), &b)?);
    }
    if let Some(project) = project {
        let rel = pathpolicy::check_read(&DecisionPath::Lock.path(), ReadClass::Decision)?;
        if let (Some(b), _) = project.read_opt(&rel, ReadClass::Decision)? {
            out.push(lock_sha(&rel.to_string(), &b)?);
        }
    }
    out.dedup();
    Ok(out)
}

fn lock_sha(name: &str, bytes: &[u8]) -> Result<String, StoreError> {
    crate::json::parse::<LockFile>(bytes)
        .map(|l| l.source_sha256)
        .map_err(|e| StoreError::Io {
            name: name.to_owned(),
            message: e.to_string(),
            errno: 0,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> jiff::Timestamp {
        s.parse().unwrap()
    }

    #[test]
    fn expiry_boundary() {
        let now = at("2026-10-20T12:00:00Z");
        assert!(
            !tail_expired("2026-07-22T12:00:00Z", now),
            "exactly 90 days"
        );
        assert!(tail_expired("2026-07-22T11:59:59Z", now));
        assert!(!tail_expired("not a date", now));
    }

    #[test]
    fn template_names() {
        let sha = "ab".repeat(32);
        assert_eq!(template_sha(&format!("{sha}.md")), Some(sha.as_str()));
        assert_eq!(template_sha(&format!("{sha}.p3.json")), Some(sha.as_str()));
        for bad in [
            "notes.md".to_owned(),
            format!("{sha}.json"),
            format!("{sha}.p.json"),
            format!("{sha}.px.json"),
            format!("{sha}.md.bak"),
            format!("{}.md", "AB".repeat(32)),
            format!("{}.md", "ab".repeat(31)),
        ] {
            assert_eq!(template_sha(&bad), None, "{bad}");
        }
    }

    #[test]
    fn project_paths() {
        let d = project_data("0b1c2d3e-0000-4000-8000-000000000001");
        assert_eq!(d.trees[0], "projects/0b1c2d3e-0000-4000-8000-000000000001");
        assert_eq!(
            d.files[0],
            "approvals/0b1c2d3e-0000-4000-8000-000000000001.json"
        );
    }
}
