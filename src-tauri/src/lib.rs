//! foreman-app: the Tauri shell (ADR-001, ADR-003). IPC commands live in [`ipc`]; the window is
//! built in Rust in [`window`].
#![forbid(unsafe_code)]

pub mod ipc;
pub mod window;

use tauri::{Builder, Runtime};

/// The builder with every IPC command registered. `generate_handler!` must list exactly
/// `ipc::COMMANDS` (capabilities::every_command_declared_and_granted).
#[must_use]
pub fn builder<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    let builder =
        builder.invoke_handler(tauri::generate_handler![ipc::commands::app::app_bootstrap]);
    #[cfg(feature = "e2e")]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());
    builder
}

/// Runs the app.
///
/// # Errors
/// Any error from building or running the Tauri app.
pub fn run() -> tauri::Result<()> {
    builder(Builder::default())
        .setup(|app| {
            window::build_main(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
}
