//! Declares Foreman's app commands so Tauri checks them against capabilities (SR-23, ADR-014).
#![allow(
    clippy::print_stdout,
    reason = "cargo build-script protocol uses stdout"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "build script: copies the sensor binary named at build time into OUT_DIR (ADR-009)"
)]

mod commands_list {
    include!("src/ipc/commands_list.rs");
}

/// ADR-009: the release build embeds the `foreman-sensor` binary named by `FOREMAN_SENSOR_BIN`
/// (`scripts/build-release` builds it first). Without it the app has no sensor (P1 behaviors).
fn sensor() {
    println!("cargo:rerun-if-env-changed=FOREMAN_SENSOR_BIN");
    println!("cargo:rustc-check-cfg=cfg(foreman_sensor_bundled)");
    let Some(src) = std::env::var_os("FOREMAN_SENSOR_BIN") else {
        return;
    };
    let Some(out) = std::env::var_os("OUT_DIR") else {
        println!("cargo:warning=OUT_DIR is not set");
        std::process::exit(1);
    };
    let dst = std::path::Path::new(&out).join("foreman-sensor");
    println!(
        "cargo:rerun-if-changed={}",
        std::path::Path::new(&src).display()
    );
    if let Err(e) = std::fs::copy(&src, &dst) {
        println!("cargo:warning=FOREMAN_SENSOR_BIN: {e}");
        std::process::exit(1);
    }
    println!("cargo:rustc-cfg=foreman_sensor_bundled");
}

fn main() {
    sensor();
    println!("cargo:rerun-if-changed=src/ipc/commands_list.rs");
    println!("cargo:rerun-if-changed=capabilities");
    let attrs = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(commands_list::COMMANDS));
    if let Err(e) = tauri_build::try_build(attrs) {
        println!("cargo:warning=tauri-build failed: {e:#}");
        std::process::exit(1);
    }
}
