//! The pure halves of the launch pipeline (F-008, ADR-007, ADR-021, DR-120–DR-124): the argv
//! builder with its fixed flag list, the child environment and `PATH`, the run settings file, the
//! preflight order with its blockers and confirmation reasons, and the progress messages.
//!
//! Nothing here touches the filesystem or spawns: `foreman-sys::launch` gathers the facts
//! (`claude -v`, `auth status`, fingerprint reads, directory stats) and calls these functions.

pub mod argv;
pub mod blocker;
pub mod env;
pub mod preflight;
pub mod run_settings;

pub use blocker::{
    AuthProblem, Blocker, ConfirmReason, LaunchProgress, NotRunnableReason, ReadOnlyInfo,
    ReadOnlyReason, ToolPathProblem,
};
