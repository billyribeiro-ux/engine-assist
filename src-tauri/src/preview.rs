//! Payloads held under a one-time `PreviewId` between a `*_preview` and its `*_apply` (IPC-R7).
//! Each is bound to what it read: the project revision and the files' stamps.

/// A held preview.
#[derive(Debug)]
pub enum Preview {
    /// `setup_preview` → `setup_apply`.
    Setup(Box<crate::ipc::commands::projects::SetupPlan>),
    /// `action_preview` → `action_apply`.
    Action(Box<crate::ipc::commands::steps::HeldAction>),
}
