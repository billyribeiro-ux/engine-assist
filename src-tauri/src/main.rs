//! Foreman's entry point.
#![forbid(unsafe_code)]
// Release builds on macOS/Windows get no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    match foreman_app::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "foreman: {e}");
            ExitCode::FAILURE
        }
    }
}
