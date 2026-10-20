//! Declares Foreman's app commands so Tauri checks them against capabilities (SR-23, ADR-014).
#![allow(
    clippy::print_stdout,
    reason = "cargo build-script protocol uses stdout"
)]

mod commands_list {
    include!("src/ipc/commands_list.rs");
}

fn main() {
    println!("cargo:rerun-if-changed=src/ipc/commands_list.rs");
    println!("cargo:rerun-if-changed=capabilities");
    let attrs = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(commands_list::COMMANDS));
    if let Err(e) = tauri_build::try_build(attrs) {
        println!("cargo:warning=tauri-build failed: {e:#}");
        std::process::exit(1);
    }
}
