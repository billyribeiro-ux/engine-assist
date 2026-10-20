//! The file watcher (ADR-013, DR-162, SR-18, SR-31): notify 8.2.0 + notify-debouncer-full 0.7.0
//! (250 ms), one per project root, as hints only. For each debounced path it
//! 1. `lstat`s the watch root and every ancestor up to the project root (a symlink stops the
//!    watcher for that root: notify's FSEvents backend would follow it);
//! 2. keeps only paths in the exact interest set (`foreman_core::interest`);
//! 3. re-reads the file through `store::Root::read` (no symlink at any component, regular file,
//!    one link) and compares its `ReadStamp` with the last one, reporting only real changes.
//!
//! It produces [`Change`]s for `foreman-app` to turn into invalidation events. It never acts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use foreman_core::interest::{self, Interest};
use foreman_core::pathpolicy::{ReadClass, RepoPath};
use foreman_core::store::{ReadStamp, Root, RootKind, StoreError};
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

/// The debounce timeout (ADR-013).
pub const DEBOUNCE: Duration = Duration::from_millis(250);

/// One real change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// An interesting file's content changed (or it appeared or disappeared).
    Changed {
        /// What it is.
        interest: Interest,
        /// Repo-relative path.
        path: String,
        /// The new stamp (`Absent` when deleted).
        stamp: ReadStamp,
    },
    /// The path is interesting but couldn't be read safely (a symlink, a hard link, a FIFO):
    /// reported, never followed.
    Refused {
        /// What it is.
        interest: Interest,
        /// Repo-relative path.
        path: String,
        /// Why.
        why: String,
    },
    /// The watch root (or an ancestor up to the project root) became a symlink: the watcher
    /// stopped for this root (Doctor warning).
    RootRefused {
        /// The root.
        root: PathBuf,
    },
}

/// Why a watcher couldn't start.
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    /// The root or an ancestor is a symlink (SR-31).
    #[error("{0}: the folder is a symlink or under one")]
    SymlinkedRoot(PathBuf),
    /// The store refused the root.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// notify failed (e.g. no FSEvents on the volume): "file watching unavailable".
    #[error("file watching unavailable: {0}")]
    Notify(String),
}

/// `lstat` the root and its ancestors up to (and including) `project_root`; none may be a
/// symlink. The root must be inside the project root.
///
/// # Errors
/// [`WatchError::SymlinkedRoot`].
pub fn check_root(root: &Path, project_root: &Path) -> Result<(), WatchError> {
    if !root.starts_with(project_root) {
        return Err(WatchError::SymlinkedRoot(root.to_path_buf()));
    }
    for a in root.ancestors() {
        let st = rustix::fs::lstat(a).map_err(|_| WatchError::SymlinkedRoot(a.to_path_buf()))?;
        let is_link =
            rustix::fs::FileType::from_raw_mode(st.st_mode) == rustix::fs::FileType::Symlink;
        if is_link {
            return Err(WatchError::SymlinkedRoot(a.to_path_buf()));
        }
        if a == project_root {
            break;
        }
    }
    Ok(())
}

/// Judges one hint: interest set, then a safe re-read and a stamp compare. Pure apart from the
/// read; used by the watcher thread and by the re-stat on focus/open/launch/sign.
pub fn judge(
    root: &Root,
    rel: &str,
    outs: &[String],
    known: &mut HashMap<String, ReadStamp>,
) -> Option<Change> {
    let interest = interest::repo_interest(rel, outs)?;
    let path = RepoPath::app_data(rel).ok()?;
    let r = root.read_opt(&path, ReadClass::Decision);
    match r {
        Ok((_, stamp)) => {
            let prev = known.insert(rel.to_owned(), stamp.clone());
            (prev.as_ref() != Some(&stamp)).then(|| Change::Changed {
                interest,
                path: rel.to_owned(),
                stamp,
            })
        }
        Err(e) => {
            known.remove(rel);
            Some(Change::Refused {
                interest,
                path: rel.to_owned(),
                why: e.to_string(),
            })
        }
    }
}

/// The re-stat of the interest set (ADR-013: on project open, window focus, before every launch
/// or sign), so a coalesced or missed event can't hide a change past the next interaction.
#[must_use]
pub fn restat(
    root: &Root,
    stages: &[u8],
    outs: &[String],
    known: &mut HashMap<String, ReadStamp>,
) -> Vec<Change> {
    interest::restat_paths(stages, outs)
        .iter()
        .filter_map(|p| judge(root, p, outs, known))
        .collect()
}

/// A running watcher for one project root.
pub struct ProjectWatcher {
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
    rx: Receiver<Change>,
    outs: Arc<Mutex<Vec<String>>>,
}

impl std::fmt::Debug for ProjectWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectWatcher").finish_non_exhaustive()
    }
}

impl ProjectWatcher {
    /// Starts watching `project_root` recursively. `outs` is the applicable steps' `out` paths
    /// (update with [`ProjectWatcher::set_outs`]).
    ///
    /// # Errors
    /// [`WatchError`].
    pub fn start(project_root: &Path, outs: Vec<String>) -> Result<Self, WatchError> {
        check_root(project_root, project_root)?;
        let root = Arc::new(Root::open(project_root, RootKind::Project)?);
        let root_path = project_root.to_path_buf();
        let outs = Arc::new(Mutex::new(outs));
        let known = Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx): (Sender<Change>, Receiver<Change>) = channel();
        let handler = {
            let outs = Arc::clone(&outs);
            let root = Arc::clone(&root);
            move |res: DebounceEventResult| {
                let Ok(events) = res else { return };
                if check_root(&root_path, &root_path).is_err() {
                    let _ = tx.send(Change::RootRefused {
                        root: root_path.clone(),
                    });
                    return;
                }
                let outs = outs.lock().unwrap_or_else(PoisonError::into_inner).clone();
                let mut known = known.lock().unwrap_or_else(PoisonError::into_inner);
                let mut paths: Vec<String> = events
                    .iter()
                    .flat_map(|e| e.event.paths.iter())
                    .filter_map(|p| p.strip_prefix(&root_path).ok())
                    .filter_map(|p| p.to_str().map(str::to_owned))
                    .collect();
                paths.sort();
                paths.dedup();
                for rel in paths {
                    if let Some(c) = judge(&root, &rel, &outs, &mut known) {
                        let _ = tx.send(c);
                    }
                }
            }
        };
        let mut debouncer = new_debouncer(DEBOUNCE, None, handler)
            .map_err(|e| WatchError::Notify(e.to_string()))?;
        debouncer
            .watch(project_root, RecursiveMode::Recursive)
            .map_err(|e| WatchError::Notify(e.to_string()))?;
        Ok(Self {
            _debouncer: debouncer,
            rx,
            outs,
        })
    }

    /// Replaces the `out` paths of the interest set.
    pub fn set_outs(&self, outs: Vec<String>) {
        *self.outs.lock().unwrap_or_else(PoisonError::into_inner) = outs;
    }

    /// The next change, waiting up to `timeout`.
    #[must_use]
    pub fn next(&self, timeout: Duration) -> Option<Change> {
        self.rx.recv_timeout(timeout).ok()
    }

    /// Every change available now.
    #[must_use]
    pub fn drain(&self) -> Vec<Change> {
        self.rx.try_iter().collect()
    }
}
