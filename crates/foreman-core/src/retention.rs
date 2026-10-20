//! Retention and removal rules (ADR-018 §5 and §7, SR-53, PR-04, PR-05, M4-02). Pure: which output
//! tails are old enough to prune, and which app-data paths belong to a project or to the run
//! history. The app walks and deletes them through [`crate::store::app_dirs`].

/// Output tails are kept this many days (SR-53).
pub const TAIL_DAYS: i64 = 90;

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
    fn project_paths() {
        let d = project_data("0b1c2d3e-0000-4000-8000-000000000001");
        assert_eq!(d.trees[0], "projects/0b1c2d3e-0000-4000-8000-000000000001");
        assert_eq!(
            d.files[0],
            "approvals/0b1c2d3e-0000-4000-8000-000000000001.json"
        );
    }
}
