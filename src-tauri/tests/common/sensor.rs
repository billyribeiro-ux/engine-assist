//! The real `foreman-sensor` for app tests (M4-07…M4-10): built once, handed to the app as its
//! bundled sensor, so the app installs it into `<app-data>/bin/` exactly as a release does.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::disallowed_methods,
    reason = "shared test helpers; tests may spawn cargo and use std::fs (design doc §3.2)"
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};

/// The built foreman-sensor binary.
pub fn sensor_bin() -> PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let st = Command::new(cargo)
            .args(["build", "-q", "-p", "foreman-sensor"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .unwrap();
        assert!(st.success());
        let target = std::env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../target"));
        target.join("debug/foreman-sensor")
    })
    .clone()
}

/// The sensor's bytes, as a release build embeds them.
pub fn sensor_bytes() -> Arc<[u8]> {
    Arc::from(std::fs::read(sensor_bin()).unwrap())
}

/// Gives `app` the real sensor as its bundled one.
pub fn enable(app: &foreman_app::state::App) {
    app.sensor().set_bundle(Some(sensor_bytes()));
}

/// The installed sensor's path in app data.
pub fn installed(app_data: &Path) -> PathBuf {
    let sha = foreman_core::store::sha256_hex(&sensor_bytes());
    app_data.join(format!("bin/foreman-sensor-{}", &sha[..12]))
}

/// Writes one event file into `events/<run>/` as the sensor would (for derived-state cases the
/// fake TUI can't produce: tasks, subagents, permission prompts).
pub fn put_event(
    app_data: &Path,
    run: &str,
    seq: u64,
    token: &str,
    event: &str,
    payload: serde_json::Value,
) {
    let dir = app_data.join("events").join(run);
    let body = serde_json::json!({
        "event": event,
        "payload": payload,
        "receivedAt": "2026-10-20T12:00:00.000Z",
        "schemaVersion": 1,
        "token": token,
        "truncated": false,
    });
    let f = dir.join(format!("{seq:020}-{:010}-{event}.json", 1));
    std::fs::write(&f, serde_json::to_string_pretty(&body).unwrap()).unwrap();
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600)).unwrap();
}
