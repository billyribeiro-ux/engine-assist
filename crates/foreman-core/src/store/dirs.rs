//! Creating directories through the store (AC-F018-03: setup creates empty `docs/handoff/`,
//! `spec/` …). The same component walk as reads and writes: no symlink at any component, the deny
//! rules on every reported name, repo directories `0755`, app-data directories `0700`. Added in
//! Stage 5 for project setup (ticket M2-17); nothing else changes in the store.

use crate::pathpolicy::RepoPath;

use super::{Root, StoreError, walk};

/// Creates every component of `p` that doesn't exist yet (like `mkdir -p`, but refusing
/// symlinks and denied names). Existing directories are left alone.
///
/// # Errors
/// A policy refusal, a component that is a file or a symlink, or an OS error.
pub fn ensure_dir(root: &Root, p: &RepoPath) -> Result<(), StoreError> {
    let comps: Vec<&str> = p.components().collect();
    let exempt = Root::exempt_for(p);
    walk::walk(root.fd(), &comps, root.opts(true, exempt)).map(|_| ())
}
