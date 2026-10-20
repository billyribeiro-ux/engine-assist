// The single list of IPC commands (ADR-004, IPC-R8, SR-23). Included verbatim by `build.rs`
// (AppManifest::commands) and by `ipc::COMMANDS`; `generate_handler!` in `lib.rs` must register
// exactly these, and `capabilities/main-window.json` must grant exactly `allow-<command>` for each
// (capabilities::every_command_declared_and_granted). Keep sorted.
/// Every command the webview may invoke.
pub const COMMANDS: &[&str] = &["app_bootstrap"];
