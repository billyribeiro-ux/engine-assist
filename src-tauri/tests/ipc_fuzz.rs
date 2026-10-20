//! `ipc::fuzz_<command>` (IPC-R2, ticket M2-01): for every command, arbitrary JSON and mutated
//! fixture requests sent over the real invoke path get a typed result or a typed `IpcError`,
//! never a panic, an `Internal` error or an untyped string.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "integration-test code (tests/): unwrap/expect are denied only outside tests (SR-62)"
)]

mod common;

use common::{TestApp, invoke, request_fixture, tauri_app};
use foreman_app::ipc::{IpcError, IpcErrorCode};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use serde_json::{Map, Value};

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i32>().prop_map(Value::from),
        "[a-zA-Z0-9 ./_:-]{0,40}".prop_map(Value::from),
        "\\PC{0,20}".prop_map(Value::from),
    ];
    leaf.prop_recursive(3, 24, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
            prop::collection::btree_map("[a-zA-Z]{1,12}", inner, 0..5)
                .prop_map(|m| Value::Object(m.into_iter().collect::<Map<_, _>>())),
        ]
    })
}

/// A valid fixture with one field dropped, replaced or added.
fn arb_mutation(cmd: &str) -> impl Strategy<Value = Value> + use<> {
    let base = request_fixture(cmd);
    let keys: Vec<String> = base
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    (0..3usize, any::<prop::sample::Index>(), arb_json()).prop_map(move |(op, idx, v)| {
        let mut m = base.as_object().cloned().unwrap_or_default();
        match (op, keys.is_empty()) {
            (0, false) => {
                m.remove(&keys[idx.index(keys.len())]);
            }
            (1, false) => {
                m.insert(keys[idx.index(keys.len())].clone(), v);
            }
            _ => {
                m.insert("fuzz".into(), v);
            }
        }
        Value::Object(m)
    })
}

fn fuzz(cmd: &str) {
    let t = TestApp::new();
    let (_app, webview) = tauri_app(t.app.clone());
    let mut runner = TestRunner::new(Config {
        cases: 24,
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(&(arb_json(), arb_mutation(cmd)), |(raw, mutated)| {
            for v in [raw, mutated] {
                if let Err(e) = invoke(&webview, cmd, v) {
                    let typed: Result<IpcError, _> = serde_json::from_value(e.clone());
                    prop_assert!(typed.is_ok(), "{}: untyped error {}", cmd, e);
                    prop_assert_ne!(
                        typed.unwrap().code,
                        IpcErrorCode::Internal,
                        "{}: {}",
                        cmd,
                        e
                    );
                }
            }
            Ok(())
        })
        .unwrap();
}

mod ipc {
    use super::fuzz;

    /// `action_apply`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_action_apply() {
        fuzz("action_apply");
    }

    /// `action_preview`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_action_preview() {
        fuzz("action_preview");
    }

    /// `add_item`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_add_item() {
        fuzz("add_item");
    }

    /// `add_step`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_add_step() {
        fuzz("add_step");
    }

    /// `app_bootstrap`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_app_bootstrap() {
        fuzz("app_bootstrap");
    }

    /// `assembler_paste_apply`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_assembler_paste_apply() {
        fuzz("assembler_paste_apply");
    }

    /// `assembler_paste_preview`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_assembler_paste_preview() {
        fuzz("assembler_paste_preview");
    }

    /// `assembler_view`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_assembler_view() {
        fuzz("assembler_view");
    }

    /// `auth_key_allowlist_add`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_auth_key_allowlist_add() {
        fuzz("auth_key_allowlist_add");
    }

    /// `blueprint_pick_repo`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_blueprint_pick_repo() {
        fuzz("blueprint_pick_repo");
    }

    /// `blueprint_repin`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_blueprint_repin() {
        fuzz("blueprint_repin");
    }

    /// `chain_resume_prepare`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_chain_resume_prepare() {
        fuzz("chain_resume_prepare");
    }

    /// `chat_copy_and_open`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_chat_copy_and_open() {
        fuzz("chat_copy_and_open");
    }

    /// `chat_link_set`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_chat_link_set() {
        fuzz("chat_link_set");
    }

    /// `clipboard_copy`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_clipboard_copy() {
        fuzz("clipboard_copy");
    }

    /// `condition_set`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_condition_set() {
        fuzz("condition_set");
    }

    /// `config_ack`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_config_ack() {
        fuzz("config_ack");
    }

    /// `desktop_handoff`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_desktop_handoff() {
        fuzz("desktop_handoff");
    }

    /// `desktop_open`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_desktop_open() {
        fuzz("desktop_open");
    }

    /// `do_next`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_do_next() {
        fuzz("do_next");
    }

    /// `doctor_capture_login_path`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_doctor_capture_login_path() {
        fuzz("doctor_capture_login_path");
    }

    /// `doctor_login_path_apply`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_doctor_login_path_apply() {
        fuzz("doctor_login_path_apply");
    }

    /// `doctor_run`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_doctor_run() {
        fuzz("doctor_run");
    }

    /// `file_open`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_file_open() {
        fuzz("file_open");
    }

    /// `file_reveal`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_file_reveal() {
        fuzz("file_reveal");
    }

    /// `gate_fix_start`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_gate_fix_start() {
        fuzz("gate_fix_start");
    }

    /// `gate_import_confirm`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_gate_import_confirm() {
        fuzz("gate_import_confirm");
    }

    /// `gate_import_results`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_gate_import_results() {
        fuzz("gate_import_results");
    }

    /// `gate_item_attest`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_gate_item_attest() {
        fuzz("gate_item_attest");
    }

    /// `gate_view`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_gate_view() {
        fuzz("gate_view");
    }

    /// `item_kind_confirm`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_item_kind_confirm() {
        fuzz("item_kind_confirm");
    }

    /// `launch_prepare`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_launch_prepare() {
        fuzz("launch_prepare");
    }

    /// `launch_start`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_launch_start() {
        fuzz("launch_start");
    }

    /// `lessons_status`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_lessons_status() {
        fuzz("lessons_status");
    }

    /// `manifest_mark`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_manifest_mark() {
        fuzz("manifest_mark");
    }

    /// `manifest_view`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_manifest_view() {
        fuzz("manifest_view");
    }

    /// `metrics_export`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_metrics_export() {
        fuzz("metrics_export");
    }

    /// `metrics_view`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_metrics_view() {
        fuzz("metrics_view");
    }

    /// `optional_part_set`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_optional_part_set() {
        fuzz("optional_part_set");
    }

    /// `packet_normalize_apply`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_packet_normalize_apply() {
        fuzz("packet_normalize_apply");
    }

    /// `packet_repair_prepare`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_packet_repair_prepare() {
        fuzz("packet_repair_prepare");
    }

    /// `packet_signature_confirm`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_packet_signature_confirm() {
        fuzz("packet_signature_confirm");
    }

    /// `packet_view`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_packet_view() {
        fuzz("packet_view");
    }

    /// `paste_back_apply`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_paste_back_apply() {
        fuzz("paste_back_apply");
    }

    /// `paste_back_preview`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_paste_back_preview() {
        fuzz("paste_back_preview");
    }

    /// `pending_change_accept`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_pending_change_accept() {
        fuzz("pending_change_accept");
    }

    /// `pending_change_reject`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_pending_change_reject() {
        fuzz("pending_change_reject");
    }

    /// `pending_changes_list`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_pending_changes_list() {
        fuzz("pending_changes_list");
    }

    /// `probe_folder_open`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_probe_folder_open() {
        fuzz("probe_folder_open");
    }

    /// `probe_run`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_probe_run() {
        fuzz("probe_run");
    }

    /// `progress_rebuild_apply`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_progress_rebuild_apply() {
        fuzz("progress_rebuild_apply");
    }

    /// `progress_rebuild_preview`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_progress_rebuild_preview() {
        fuzz("progress_rebuild_preview");
    }

    /// `project_env_approve`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_project_env_approve() {
        fuzz("project_env_approve");
    }

    /// `project_open`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_project_open() {
        fuzz("project_open");
    }

    /// `project_pin_set`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_project_pin_set() {
        fuzz("project_pin_set");
    }

    /// `project_remove`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_project_remove() {
        fuzz("project_remove");
    }

    /// `project_url_set`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_project_url_set() {
        fuzz("project_url_set");
    }

    /// `projects_list`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_projects_list() {
        fuzz("projects_list");
    }

    /// `pty_attach`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_pty_attach() {
        fuzz("pty_attach");
    }

    /// `pty_flow`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_pty_flow() {
        fuzz("pty_flow");
    }

    /// `pty_interrupt`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_pty_interrupt() {
        fuzz("pty_interrupt");
    }

    /// `pty_paste`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_pty_paste() {
        fuzz("pty_paste");
    }

    /// `pty_resize`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_pty_resize() {
        fuzz("pty_resize");
    }

    /// `pty_terminate`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_pty_terminate() {
        fuzz("pty_terminate");
    }

    /// `pty_write`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_pty_write() {
        fuzz("pty_write");
    }

    /// `rearm_continue`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_rearm_continue() {
        fuzz("rearm_continue");
    }

    /// `rearm_start`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_rearm_start() {
        fuzz("rearm_start");
    }

    /// `run_input_set`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_run_input_set() {
        fuzz("run_input_set");
    }

    /// `run_override_set`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_run_override_set() {
        fuzz("run_override_set");
    }

    /// `run_sheet_export`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_run_sheet_export() {
        fuzz("run_sheet_export");
    }

    /// `runs_clear`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_runs_clear() {
        fuzz("runs_clear");
    }

    /// `runs_search`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_runs_search() {
        fuzz("runs_search");
    }

    /// `settings_get`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_settings_get() {
        fuzz("settings_get");
    }

    /// `settings_pick_tool`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_settings_pick_tool() {
        fuzz("settings_pick_tool");
    }

    /// `settings_update`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_settings_update() {
        fuzz("settings_update");
    }

    /// `setup_apply`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_setup_apply() {
        fuzz("setup_apply");
    }

    /// `setup_pick_folder`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_setup_pick_folder() {
        fuzz("setup_pick_folder");
    }

    /// `setup_preview`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_setup_preview() {
        fuzz("setup_preview");
    }

    /// `sign_apply`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_sign_apply() {
        fuzz("sign_apply");
    }

    /// `sign_preview`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_sign_preview() {
        fuzz("sign_preview");
    }

    /// `sign_retry_commit`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_sign_retry_commit() {
        fuzz("sign_retry_commit");
    }

    /// `step_mark_done`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_step_mark_done() {
        fuzz("step_mark_done");
    }

    /// `step_skip`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_step_skip() {
        fuzz("step_skip");
    }

    /// `step_view`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_step_view() {
        fuzz("step_view");
    }

    /// `tamper_restore`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_tamper_restore() {
        fuzz("tamper_restore");
    }

    /// `template_health`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_template_health() {
        fuzz("template_health");
    }

    /// `terminal_link_open`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_terminal_link_open() {
        fuzz("terminal_link_open");
    }

    /// `terminal_open_here`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_terminal_open_here() {
        fuzz("terminal_open_here");
    }

    /// `today_get`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_today_get() {
        fuzz("today_get");
    }

    /// `variable_set`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_variable_set() {
        fuzz("variable_set");
    }

    /// `wait_start`: arbitrary and mutated requests get a typed answer.
    #[test]
    fn fuzz_wait_start() {
        fuzz("wait_start");
    }
}
