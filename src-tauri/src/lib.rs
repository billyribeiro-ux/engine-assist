//! foreman-app: the Tauri shell (ADR-001, ADR-003). IPC commands live in [`ipc`]; the window is
//! built in Rust in [`window`]; native dialogs, the clipboard and events go through [`os`].
#![forbid(unsafe_code)]

pub mod closeout;
pub mod convert;
pub mod diff;
pub mod inbox;
pub mod ipc;
pub mod launch;
pub mod logging;
pub mod notify;
pub mod os;
pub mod preview;
pub mod probe;
pub mod project;
pub mod quit;
pub mod render;
pub mod runs;
pub mod sensor;
pub mod sensor_ingest;
pub mod signing;
pub mod startup;
pub mod state;
pub mod text;
pub mod watch;
pub mod window;

use std::sync::Arc;

use tauri::{Builder, Manager as _, Runtime};

/// The builder with every IPC command and the Rust-side plugins registered. `generate_handler!`
/// must list exactly `ipc::COMMANDS` (capabilities::every_command_declared_and_granted). The
/// dialog, clipboard and notification plugins are used from Rust only; no capability grants the webview any of
/// their permissions (SR-22).
#[must_use]
pub fn builder<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    let builder = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            ipc::handlers::app_bootstrap,
            ipc::handlers::settings_get,
            ipc::handlers::settings_update,
            ipc::handlers::settings_pick_tool,
            ipc::handlers::today_get,
            ipc::handlers::do_next,
            ipc::handlers::blueprint_pick_repo,
            ipc::handlers::template_health,
            ipc::handlers::doctor_run,
            ipc::handlers::doctor_capture_login_path,
            ipc::handlers::doctor_login_path_apply,
            ipc::handlers::probe_folder_open,
            ipc::handlers::probe_run,
            ipc::handlers::projects_list,
            ipc::handlers::setup_pick_folder,
            ipc::handlers::setup_preview,
            ipc::handlers::setup_apply,
            ipc::handlers::project_open,
            ipc::handlers::project_url_set,
            ipc::handlers::condition_set,
            ipc::handlers::variable_set,
            ipc::handlers::blueprint_repin,
            ipc::handlers::project_remove,
            ipc::handlers::project_pin_set,
            ipc::handlers::step_view,
            ipc::handlers::run_input_set,
            ipc::handlers::optional_part_set,
            ipc::handlers::run_override_set,
            ipc::handlers::step_mark_done,
            ipc::handlers::step_skip,
            ipc::handlers::action_preview,
            ipc::handlers::action_apply,
            ipc::handlers::wait_start,
            ipc::handlers::clipboard_copy,
            ipc::handlers::chat_copy_and_open,
            ipc::handlers::chat_link_set,
            ipc::handlers::paste_back_preview,
            ipc::handlers::paste_back_apply,
            ipc::handlers::launch_prepare,
            ipc::handlers::chain_resume_prepare,
            ipc::handlers::launch_start,
            ipc::handlers::config_ack,
            ipc::handlers::auth_key_allowlist_add,
            ipc::handlers::project_env_approve,
            ipc::handlers::pty_attach,
            ipc::handlers::pty_write,
            ipc::handlers::pty_paste,
            ipc::handlers::pty_resize,
            ipc::handlers::pty_flow,
            ipc::handlers::pty_interrupt,
            ipc::handlers::pty_terminate,
            ipc::handlers::rearm_start,
            ipc::handlers::rearm_continue,
            ipc::handlers::terminal_link_open,
            ipc::handlers::file_open,
            ipc::handlers::file_reveal,
            ipc::handlers::terminal_open_here,
            ipc::handlers::pending_changes_list,
            ipc::handlers::pending_change_accept,
            ipc::handlers::pending_change_reject,
            ipc::handlers::item_kind_confirm,
            ipc::handlers::progress_rebuild_preview,
            ipc::handlers::progress_rebuild_apply,
            ipc::handlers::packet_view,
            ipc::handlers::packet_normalize_apply,
            ipc::handlers::packet_signature_confirm,
            ipc::handlers::packet_repair_prepare,
            ipc::handlers::metrics_export,
            ipc::handlers::metrics_view,
            ipc::handlers::runs_clear,
            ipc::handlers::runs_search,
            ipc::handlers::gate_view,
            ipc::handlers::gate_item_attest,
            ipc::handlers::gate_import_results,
            ipc::handlers::gate_import_confirm,
            ipc::handlers::gate_fix_start,
            ipc::handlers::sign_preview,
            ipc::handlers::sign_apply,
            ipc::handlers::sign_retry_commit,
            ipc::handlers::tamper_restore,
            ipc::handlers::lessons_status,
            ipc::handlers::manifest_view,
            ipc::handlers::manifest_mark,
            ipc::handlers::assembler_view,
            ipc::handlers::assembler_paste_preview,
            ipc::handlers::assembler_paste_apply,
            ipc::handlers::desktop_open,
            ipc::handlers::desktop_handoff,
            ipc::handlers::add_step,
            ipc::handlers::add_item,
            ipc::handlers::run_sheet_export,
        ]);
    #[cfg(feature = "e2e")]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());
    builder
}

/// Runs the app.
///
/// # Errors
/// Any error from building or running the Tauri app.
pub fn run() -> tauri::Result<()> {
    logging::install();
    builder(Builder::default())
        .setup(|app| {
            let paths = state::Paths {
                app_data: app.path().app_data_dir()?,
                home: app.path().home_dir()?,
            };
            if logging::attach(&paths.app_data).is_ok() {
                tracing::info!(target: "app", version = env!("CARGO_PKG_VERSION"), "started");
            }
            let os = Arc::new(os::TauriOs::new(app.handle().clone()));
            startup::harden(&paths.app_data)?;
            let state = state::App::new(os, paths);
            watch::start(state.weak());
            inbox::start(&state);
            // ADR-009: install the bundled sensor (if this build has one) off the main thread.
            let weak = state.weak();
            let _ = std::thread::Builder::new()
                .name("fm-sensor-install".into())
                .spawn(move || {
                    if let Some(app) = weak.upgrade() {
                        let _ = sensor::install(&app);
                    }
                });
            app.manage::<ipc::Shared>(state);
            window::build_main(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // AC-F009-09: closing the window with live sessions goes through the quit guard.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && !quit::confirmed()
                && let Some(s) = window.try_state::<ipc::Shared>()
                && !s.runs().live().is_empty()
            {
                api.prevent_close();
                ask_to_quit(window.app_handle().clone(), Arc::clone(s.inner()));
                return;
            }
            // ADR-013: a focus re-stats the watched projects and re-checks live runs' config, so a
            // missed file event can't hide a change past the next interaction.
            if let tauri::WindowEvent::Focused(true) = event
                && let Some(s) = window.try_state::<ipc::Shared>()
            {
                let state = Arc::clone(s.inner());
                let _ = std::thread::Builder::new()
                    .name("fm-focus-restat".into())
                    .spawn(move || {
                        watch::on_focus(&state);
                        inbox::schedule(&state);
                    });
            }
        })
        .build(tauri::generate_context!())?
        .run(|handle, event| {
            // AC-F009-09: ⌘Q with live sessions goes through the quit guard first.
            if let tauri::RunEvent::ExitRequested { api, .. } = event
                && !quit::confirmed()
                && let Some(s) = handle.try_state::<ipc::Shared>()
                && !s.runs().live().is_empty()
            {
                api.prevent_exit();
                ask_to_quit(handle.clone(), Arc::clone(s.inner()));
            }
        });
    Ok(())
}

/// Asks the quit guard off the main thread (native dialogs block) and exits on OK.
fn ask_to_quit<R: Runtime>(handle: tauri::AppHandle<R>, state: ipc::Shared) {
    if !quit::begin_asking() {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("fm-quit-guard".into())
        .spawn(move || {
            let ok = quit::guard(&state);
            quit::end_asking();
            if ok {
                quit::confirm();
                handle.exit(0);
            }
        });
    if spawned.is_err() {
        quit::end_asking();
    }
}
